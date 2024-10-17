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
use crate::core::config::global::{LocalCacheConfig, ServerConnectorConfig};
use crate::core::model::error::{ErrorCode, PolarisError};
use crate::core::plugin::cache::ResourceCache;
use crate::core::plugin::connector::Connector;
use crate::core::plugin::router::ServiceRouter;
use crate::plugins::cache::memory::memory::MemoryCache;
use crate::plugins::connector::grpc::connector::GrpcConnector;
use crate::plugins::filter::configcrypto::crypto::ConfigFileCryptoFilter;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::{env, fmt};
use tokio::runtime::Runtime;

use super::cache::InitResourceCacheOption;
use super::connector::InitConnectorOption;
use super::filter::DiscoverFilter;

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
    pub plugin_container: Arc<PluginContainer>,
    pub runtime: Arc<Runtime>,
    pub client_id: String,
    pub conf: Arc<Configuration>,
    // config_filters
    pub config_filters: Option<Arc<Vec<Box<dyn DiscoverFilter>>>>,
    // server_connector
    pub server_connector: Option<Arc<Box<dyn Connector>>>,
}

impl Extensions {
    pub fn build(
        client_id: String,
        conf: Arc<Configuration>,
        runetime: Arc<Runtime>,
    ) -> Result<Self, PolarisError> {
        let mut containers = PluginContainer::default();
        // 初始化所有的插件
        let start_time = std::time::Instant::now();
        containers.register_all_plugin();
        tracing::info!("register_all_plugin cost: {:?}", start_time.elapsed());

        let arc_container = Arc::new(containers);

        Ok(Self {
            plugin_container: arc_container,
            runtime: runetime,
            client_id: client_id,
            conf: conf,
            config_filters: None,
            server_connector: None,
        })
    }

    pub fn load_server_connector(
        &mut self,
        connector_opt: &ServerConnectorConfig,
    ) -> Result<Arc<Box<dyn Connector>>, PolarisError> {
        if connector_opt.addresses.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::InvalidConfig,
                "server_connector addresses is empty".to_string(),
            ));
        }

        let protocol = connector_opt.get_protocol();
        let supplier = self.plugin_container.get_connector_supplier(&protocol);
        let mut active_connector = supplier(InitConnectorOption {
            runtime: self.runtime.clone(),
            conf: self.conf.clone(),
            client_id: self.client_id.clone(),
            config_filters: self.config_filters.clone().unwrap().clone(),
        });
        active_connector.init();

        let active_connector = Arc::new(active_connector);

        self.server_connector = Some(active_connector.clone());
        Ok(active_connector)
    }

    pub fn load_resource_cache(
        &mut self,
        cache_opt: &LocalCacheConfig,
    ) -> Result<Box<dyn ResourceCache>, PolarisError> {
        let cache_name = cache_opt.name.clone();
        if cache_name.is_empty() {
            return Err(PolarisError::new(ErrorCode::InvalidConfig, "".to_string()));
        }

        let supplier = self.plugin_container.get_cache_supplier(&cache_name);
        let mut active_cache = supplier(InitResourceCacheOption {
            runtime: self.runtime.clone(),
            conf: self.conf.clone(),
            server_connector: self.server_connector.clone().unwrap().clone(),
        });
        active_cache.init();

        Ok(active_cache)
    }

    pub fn load_config_file_filters(
        &mut self,
        filter_conf: &ConfigFilter,
    ) -> Result<(), PolarisError> {
        let mut filters = Vec::<Box<dyn DiscoverFilter>>::new();
        if filter_conf.enable {
            for (_i, name) in filter_conf.chain.iter().enumerate() {
                if name == "crypto" {
                    let plugin_opt = filter_conf.plugin.get(name).unwrap();
                    let supplier = self.plugin_container.get_discover_filter_supplier(name);
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
}

pub struct PluginContainer {
    connectors: HashMap<String, fn(InitConnectorOption) -> Box<dyn Connector>>,
    routers: HashMap<String, fn(&Configuration) -> Box<dyn ServiceRouter>>,
    caches: HashMap<String, fn(InitResourceCacheOption) -> Box<dyn ResourceCache>>,
    discover_filters:
        HashMap<String, fn(serde_yaml::Value) -> Result<Box<dyn DiscoverFilter>, PolarisError>>,
}

impl Default for PluginContainer {
    fn default() -> Self {
        let c = Self {
            connectors: Default::default(),
            routers: Default::default(),
            caches: Default::default(),
            discover_filters: Default::default(),
        };

        return c;
    }
}

impl PluginContainer {
    pub fn register_all_plugin(&mut self) {
        self.register_resource_cache();
        self.register_connector();
        self.register_discover_filter();
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

    fn register_discover_filter(&mut self) {
        let vec = vec![ConfigFileCryptoFilter::builder];
        for c in vec {
            let (supplier, name) = c();
            self.discover_filters.insert(name, supplier);
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
}

pub fn acquire_client_id(conf: Arc<Configuration>) -> String {
    // 读取本地域名 HOSTNAME，如果存在，则客户端 ID 标识为 {HOSTNAME}_{进程 PID}_{单进程全局自增数字}
    // 不满足1的情况下，读取本地 IP，如果存在，则客户端 ID 标识为 {LOCAL_IP}_{进程 PID}_{单进程全局自增数字}
    // 不满足上述情况，使用UUID作为客户端ID。
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if env::var("HOSTNAME").is_ok() {
        return format!(
            "{}_{}_{}",
            env::var("HOSTNAME").unwrap(),
            std::process::id(),
            seq
        );
    }
    // 和北极星服务端做一个 TCP connect 连接获取 本地 IP 地址
    let host = conf.global.server_connectors.addresses.get(0);

    let mut origin_endpoint = host.unwrap().as_str().trim_start_matches("discover://");
    origin_endpoint = origin_endpoint.trim_start_matches("config://");
    let addrs = (origin_endpoint).to_socket_addrs();
    match addrs {
        Ok(mut addr_iter) => {
            if let Some(addr) = addr_iter.next() {
                if let IpAddr::V4(ipv4) = addr.ip() {
                    return format!("{}_{}_{}", ipv4, std::process::id(), seq);
                } else if let IpAddr::V6(ipv6) = addr.ip() {
                    return format!("{}_{}_{}", ipv6, std::process::id(), seq);
                } else {
                    return uuid::Uuid::new_v4().to_string();
                }
            } else {
                return uuid::Uuid::new_v4().to_string();
            }
        }
        Err(_err) => {
            return uuid::Uuid::new_v4().to_string();
        }
    }
}
