// Tencent is pleased to support the open source community by making Polaris available.
//
// Copyright (C) 2019 THL A29 Limited, a Tencent company. All rights reserved.
//
// Licensed under the BSD 3-Clause License (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://opensource.org/licenses/BSD-3-Clause
//
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use crate::core::config::config::Configuration;
use crate::core::config::config_file::ConfigFilter;
use crate::core::config::consumer::{ServiceRouterConfig, ServiceRouterPluginConfig};
use crate::core::config::global::{LocalCacheConfig, LocationConfig, ServerConnectorConfig};
use crate::core::model::error::{ErrorCode, PolarisError};
use crate::core::model::ClientContext;
use crate::core::plugin::cache::ResourceCache;
use crate::core::plugin::connector::Connector;
use crate::core::plugin::router::ServiceRouter;
use crate::plugins::cache::memory::failover;
use crate::plugins::cache::memory::memory::MemoryCache;
use crate::plugins::circuitbreaker::composite::circuitbreaker::CompositeCircuitBreaker;
use crate::plugins::connector::grpc::connector::GrpcConnector;
use crate::plugins::filter::configcrypto::crypto::ConfigFileCryptoFilter;
use crate::plugins::loadbalance::random::random::WeightRandomLoadbalancer;
use crate::plugins::loadbalance::ringhash::ringhash::ConsistentHashLoadBalancer;
use crate::plugins::loadbalance::roundrobin::roundrobin::WeightedRoundRobinBalancer;
use crate::plugins::location::local::local::LocalLocationSupplier;
use crate::plugins::location::remotehttp::remotehttp::RemoteHttpLocationSupplier;
use crate::plugins::ratelimit::concurrency::concurrency::ConcurrencyLimiter;
use crate::plugins::router::health::health::HealthRouter;
use crate::plugins::router::lane::lane::LaneRouter;
use crate::plugins::router::metadata::metadata::MetadataRouter;
use crate::plugins::router::nearby::nearby::NearbyRouter;
use crate::plugins::router::rule::rule::RuleRouter;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};
use std::{env, fmt};
use tokio::runtime::Runtime;

use super::cache::{InitResourceCacheOption, ResourceCacheFailover};
use super::circuitbreaker::CircuitBreaker;
use super::connector::InitConnectorOption;
use super::filter::DiscoverFilter;
use super::loadbalance::LoadBalancer;
use super::location::{LocationProvider, LocationSupplier, LocationType};
use super::ratelimit::ServiceRateLimiter;
use super::router::RouterContainer;

static SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum PluginType {
    PluginCache,
    PluginRouter,
    PluginLocation,
    PluginLoadBalance,
    PluginCircuitBreaker,
    PluginConnector,
    PluginRateLimit,
}

impl Display for PluginType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait Plugin
where
    Self: Send + Sync,
{
    fn init(&mut self);

    fn destroy(&self);

    fn name(&self) -> String;
}

#[derive(Clone)]
pub struct Extensions
where
    Self: Send + Sync,
{
    // conf: SDK 配置信息
    pub conf: Arc<Configuration>,
    // runtime: 内部线程池
    pub runtime: Arc<Runtime>,
    // client_ctx: 客户端数据上下文
    pub client_ctx: Arc<ClientContext>,
    // config_filters: 配置文件过滤器
    pub config_filters: Option<Arc<Vec<Box<dyn DiscoverFilter>>>>,
    // server_connector: 服务端连接器
    server_connector: Option<Arc<Box<dyn Connector>>>,
    // resource_cache 资源缓存
    resource_cache: Option<Arc<Box<dyn ResourceCache>>>,
    // locatin_provider: 位置信息提供器
    locatin_provider: Option<Arc<LocationProvider>>,
    // circuit_breaker: 熔断器
    pub circuit_breaker: Option<Arc<Box<dyn CircuitBreaker>>>,
    // service_routers 服务路由插件
    pub service_routers: Option<Arc<RouterContainer>>,
    // load_balancers 负载均衡器
    pub load_balancers: Arc<tokio::sync::RwLock<HashMap<String, Arc<Box<dyn LoadBalancer>>>>>,
}

impl Extensions {
    pub fn build(
        client_ctx: Arc<ClientContext>,
        conf: Arc<Configuration>,
        runetime: Arc<Runtime>,
    ) -> Result<Self, PolarisError> {
        let mut extension = Self {
            client_ctx: client_ctx,
            runtime: runetime,
            conf: conf.clone(),
            config_filters: None,
            server_connector: None,
            locatin_provider: None,
            circuit_breaker: None,
            resource_cache: None,
            service_routers: None,
            load_balancers: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        };

        let ret = extension.load_all_plugins(conf.clone());
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        Ok(extension)
    }

    fn load_all_plugins(&mut self, conf: Arc<Configuration>) -> Result<(), PolarisError> {
        let ret = self.load_config_file_filters(&conf.config.config_filter);
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        // 初始化 server_connector
        let ret = self.load_server_connector(&conf.global.server_connectors);
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        // 初始化 local_cache
        let ret = self.load_resource_cache(&conf.consumer.local_cache);
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        // 初始化 location_provider
        let ret = self.load_location_providers(&conf.global.location);
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        // 初始化 service_routers
        let ret = self.load_service_routers(&conf.consumer.service_router);
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        // 初始化 loadbalancers
        let ret = self.load_loadbalancers();
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        Ok(())
    }

    pub fn get_server_connector(&self) -> Arc<Box<dyn Connector>> {
        self.server_connector.clone().unwrap()
    }

    pub fn get_resource_cache(&self) -> Arc<Box<dyn ResourceCache>> {
        self.resource_cache.clone().unwrap()
    }

    pub fn get_loadbalancers(
        &self,
    ) -> Arc<tokio::sync::RwLock<HashMap<String, Arc<Box<dyn LoadBalancer>>>>> {
        self.load_balancers.clone()
    }

    pub fn get_location_provider(&self) -> Arc<LocationProvider> {
        self.locatin_provider.clone().unwrap()
    }

    pub fn get_router_container(&self) -> Arc<RouterContainer> {
        self.service_routers.clone().unwrap()
    }

    fn load_server_connector(
        &mut self,
        connector_opt: &ServerConnectorConfig,
    ) -> Result<(), PolarisError> {
        if connector_opt.addresses.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::InvalidConfig,
                "server_connector addresses is empty".to_string(),
            ));
        }

        let protocol = connector_opt.get_protocol();
        let supplier = CLIENT_PLUGIN_CONTAINER
            .read()
            .unwrap()
            .get_connector_supplier(&protocol);
        let mut active_connector = supplier(InitConnectorOption {
            client_ctx: self.client_ctx.clone(),
            runtime: self.runtime.clone(),
            conf: self.conf.clone(),
            config_filters: self.config_filters.clone().unwrap().clone(),
        });
        active_connector.init();

        let active_connector = Arc::new(active_connector);

        self.server_connector = Some(active_connector);
        Ok(())
    }

    fn load_resource_cache(&mut self, cache_opt: &LocalCacheConfig) -> Result<(), PolarisError> {
        let cache_name = cache_opt.name.clone();
        if cache_name.is_empty() {
            return Err(PolarisError::new(ErrorCode::InvalidConfig, "".to_string()));
        }

        let supplier = CLIENT_PLUGIN_CONTAINER
            .read()
            .unwrap()
            .get_cache_supplier(&cache_name);
        let mut active_cache = supplier(InitResourceCacheOption {
            runtime: self.runtime.clone(),
            conf: cache_opt.clone(),
            server_connector: self.server_connector.clone().unwrap().clone(),
        });

        active_cache.init();

        if let Some(failover) = CLIENT_PLUGIN_CONTAINER
            .read()
            .unwrap()
            .custom_cache_failover
            .clone()
        {
            active_cache.set_failover_provider(failover);
        }

        self.resource_cache = Some(Arc::new(active_cache));
        Ok(())
    }

    fn load_config_file_filters(&mut self, filter_conf: &ConfigFilter) -> Result<(), PolarisError> {
        let mut filters = Vec::<Box<dyn DiscoverFilter>>::new();
        if filter_conf.enable {
            for (_i, name) in filter_conf.chain.iter().enumerate() {
                if name == "crypto" {
                    let plugin_opt = filter_conf.plugin.get(name).unwrap();
                    let supplier = CLIENT_PLUGIN_CONTAINER
                        .read()
                        .unwrap()
                        .get_discover_filter_supplier(name);
                    let filter = supplier(plugin_opt.clone());
                    if filter.is_err() {
                        return Err(filter.err().unwrap());
                    }
                    filters.push(filter.unwrap());
                }
            }
        }
        self.config_filters = Some(Arc::new(filters));
        Ok(())
    }

    fn load_service_routers(
        &mut self,
        route_conf: &ServiceRouterConfig,
    ) -> Result<(), PolarisError> {
        let mut container = RouterContainer::new();
        for (name, supplier) in CLIENT_PLUGIN_CONTAINER
            .read()
            .unwrap()
            .service_routers
            .iter()
        {
            for (_i, router_conf) in route_conf.before_chain.iter().enumerate() {
                if router_conf.name == name.to_string() {
                    let router = supplier(router_conf);
                    container
                        .before_routers
                        .insert(name.clone(), Arc::new(router));
                }
            }
            for (_i, router_conf) in route_conf.core_chain.iter().enumerate() {
                if router_conf.name == name.to_string() {
                    let router = supplier(router_conf);
                    container
                        .core_routers
                        .insert(name.clone(), Arc::new(router));
                }
            }

            for (_i, router_conf) in route_conf.after_chain.iter().enumerate() {
                if router_conf.name == name.to_string() {
                    let router = supplier(router_conf);
                    container
                        .after_routers
                        .insert(name.clone(), Arc::new(router));
                }
            }
        }

        self.service_routers = Some(Arc::new(container));
        Ok(())
    }

    fn load_loadbalancers(&mut self) -> Result<(), PolarisError> {
        let mut loadbalancers = HashMap::<String, Arc<Box<dyn LoadBalancer>>>::new();
        for (name, supplier) in CLIENT_PLUGIN_CONTAINER
            .read()
            .unwrap()
            .load_balancers
            .iter()
        {
            let lb = supplier();
            loadbalancers.insert(name.clone(), Arc::new(lb));
        }
        self.load_balancers = Arc::new(tokio::sync::RwLock::new(loadbalancers));
        Ok(())
    }

    fn load_location_providers(&mut self, opt: &LocationConfig) -> Result<(), PolarisError> {
        let mut chain = Vec::<Box<dyn LocationSupplier>>::new();
        let providers = opt.clone().providers;
        if providers.is_none() {
            return Err(PolarisError::new(
                ErrorCode::ApiInvalidArgument,
                "".to_string(),
            ));
        }

        providers.unwrap().iter().for_each(|provider| {
            let name: String = provider.name.clone();
            match LocationType::parse(name.as_str()) {
                LocationType::Local => {
                    chain.push(Box::new(LocalLocationSupplier::new(provider.clone())));
                }
                LocationType::Http => {
                    chain.push(Box::new(RemoteHttpLocationSupplier::new(provider.clone())));
                }
                LocationType::Service => {}
            }
        });

        let ret = Arc::new(LocationProvider { chain: chain });
        self.locatin_provider = Some(ret.clone());
        return Ok(());
    }
}

static CLIENT_PLUGIN_CONTAINER: Lazy<Arc<RwLock<PluginContainer>>> = Lazy::new(|| {
    let mut container = PluginContainer::default();
    container.register_all_plugin();
    Arc::new(RwLock::new(container))
});

#[derive(Default)]
pub struct PluginContainer {
    /// ------- SDK 内部运行时插件 -------
    // connectors: 连接器
    connectors: HashMap<String, fn(InitConnectorOption) -> Box<dyn Connector>>,
    // caches: 缓存
    caches: HashMap<String, fn(InitResourceCacheOption) -> Box<dyn ResourceCache>>,
    // discover_filters: 发现过滤器
    discover_filters:
        HashMap<String, fn(serde_yaml::Value) -> Result<Box<dyn DiscoverFilter>, PolarisError>>,
    /// ------- 治理规则相关插件 -------
    // service_routers: 路由器
    service_routers: HashMap<String, fn(&ServiceRouterPluginConfig) -> Box<dyn ServiceRouter>>,
    // load_balancers: 负载均衡器
    load_balancers: HashMap<String, fn() -> Box<dyn LoadBalancer>>,
    // circuit_breakers: 熔断器
    circuit_breakers: HashMap<String, fn() -> Box<dyn CircuitBreaker>>,
    // ratelimiter: 限流器
    ratelimiter: HashMap<String, fn() -> Box<dyn ServiceRateLimiter>>,
    // custom_cache_failover 用户自定义缓存容灾实现
    custom_cache_failover: Option<Arc<dyn ResourceCacheFailover>>,
}

impl PluginContainer {
    pub fn register_all_plugin(&mut self) {
        self.register_resource_cache();
        self.register_connector();
        self.register_discover_filter();
        self.register_service_routers();
        self.register_load_balancer();
        self.register_circuit_breaker();
        self.register_service_ratelimiter();
    }

    fn register_connector(&mut self) {
        let vec = vec![GrpcConnector::builder];
        for c in vec {
            let (supplier, name) = c();
            self.connectors.insert(name, supplier);
        }
    }

    fn register_resource_cache(&mut self) {
        let vec = vec![MemoryCache::builder];
        for c in vec {
            let (supplier, name) = c();
            self.caches.insert(name, supplier);
        }
    }

    fn register_service_routers(&mut self) {
        let vec = vec![
            HealthRouter::builder,
            LaneRouter::builder,
            MetadataRouter::builder,
            NearbyRouter::builder,
            RuleRouter::builder,
        ];
        for c in vec {
            let (supplier, name) = c();
            self.service_routers.insert(name, supplier);
        }
    }

    fn register_discover_filter(&mut self) {
        let vec = vec![ConfigFileCryptoFilter::builder];
        for c in vec {
            let (supplier, name) = c();
            self.discover_filters.insert(name, supplier);
        }
    }

    fn register_load_balancer(&mut self) {
        let vec = vec![
            WeightRandomLoadbalancer::builder,
            ConsistentHashLoadBalancer::builder,
            WeightedRoundRobinBalancer::builder,
        ];
        for c in vec {
            let (supplier, name) = c();
            self.load_balancers.insert(name, supplier);
        }
    }

    fn register_circuit_breaker(&mut self) {
        let vec = vec![CompositeCircuitBreaker::builder];
        for c in vec {
            let (supplier, name) = c();
            self.circuit_breakers.insert(name, supplier);
        }
    }

    fn register_service_ratelimiter(&mut self) {
        let vec = vec![ConcurrencyLimiter::builder];
        for c in vec {
            let (supplier, name) = c();
            self.ratelimiter.insert(name, supplier);
        }
    }

    fn get_connector_supplier(
        &self,
        name: &str,
    ) -> fn(opt: InitConnectorOption) -> Box<dyn Connector> {
        *self.connectors.get(name).unwrap()
    }

    fn get_cache_supplier(
        &self,
        name: &str,
    ) -> fn(opt: InitResourceCacheOption) -> Box<dyn ResourceCache> {
        *self.caches.get(name).unwrap()
    }

    fn get_discover_filter_supplier(
        &self,
        name: &str,
    ) -> fn(serde_yaml::Value) -> Result<Box<dyn DiscoverFilter>, PolarisError> {
        *self.discover_filters.get(name).unwrap()
    }

    fn get_ratelimiter_supplier(&self, name: &str) -> fn() -> Box<dyn ServiceRateLimiter> {
        *self.ratelimiter.get(name).unwrap()
    }

    /// register_custom_service_router 注册自定义的服务路由
    pub fn register_custom_service_router(
        &mut self,
        name: String,
        supplier: fn(&ServiceRouterPluginConfig) -> Box<dyn ServiceRouter>,
    ) {
        self.service_routers.insert(name, supplier);
    }

    /// register_custom_load_balancer 注册自定义的负载均衡器
    pub fn register_custom_load_balancer(
        &mut self,
        name: String,
        supplier: fn() -> Box<dyn LoadBalancer>,
    ) {
        self.load_balancers.insert(name, supplier);
    }

    /// register_custom_cache_failover 注册自定义的缓存容灾
    pub fn register_custom_cache_failover(&mut self, failover: Arc<dyn ResourceCacheFailover>) {
        self.custom_cache_failover = Some(failover);
    }
}

pub fn acquire_client_context(conf: Arc<Configuration>) -> ClientContext {
    let mut client_id = conf.global.client.id.clone();
    let self_ip = acquire_client_self_ip(conf.clone());

    if conf.global.client.id.is_empty() {
        // 读取本地域名 HOSTNAME，如果存在，则客户端 ID 标识为 {HOSTNAME}_{进程 PID}_{单进程全局自增数字}
        // 不满足1的情况下，读取本地 IP，如果存在，则客户端 ID 标识为 {LOCAL_IP}_{进程 PID}_{单进程全局自增数字}
        // 不满足上述情况，使用UUID作为客户端ID。
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if env::var("HOSTNAME").is_ok() {
            client_id = format!(
                "{}_{}_{}",
                env::var("HOSTNAME").unwrap(),
                std::process::id(),
                seq
            );
        } else {
            // 和北极星服务端做一个 TCP connect 连接获取 本地 IP 地址
            if self_ip == "127.0.0.1".to_string() {
                client_id = uuid::Uuid::new_v4().to_string();
            } else {
                client_id = format!("{}_{}_{}", self_ip, std::process::id(), seq);
            }
        }
    }
    ClientContext::new(client_id, self_ip, &conf.global.client)
}

pub fn acquire_client_self_ip(conf: Arc<Configuration>) -> String {
    // 和北极星服务端做一个 TCP connect 连接获取 本地 IP 地址
    let host = conf.global.server_connectors.addresses.first();

    let mut origin_endpoint = host.unwrap().as_str().trim_start_matches("discover://");
    origin_endpoint = origin_endpoint.trim_start_matches("config://");
    let addrs = (origin_endpoint).to_socket_addrs();
    match addrs {
        Ok(mut addr_iter) => {
            if let Some(addr) = addr_iter.next() {
                if let IpAddr::V4(ipv4) = addr.ip() {
                    return format!("{}", ipv4);
                } else if let IpAddr::V6(ipv6) = addr.ip() {
                    return format!("{}", ipv6);
                }
            }
            tracing::error!("acquire_client_self_ip not ipv4 or ipv6, impossible run here");
            "127.0.0.1".to_string()
        }
        Err(_err) => {
            tracing::error!("acquire_client_self_ip error: {:?}", _err);
            "127.0.0.1".to_string()
        }
    }
}
