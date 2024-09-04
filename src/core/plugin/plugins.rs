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
use crate::core::config::global::{
    ServerConnectorConfig, CONFIG_SERVER_CONNECTOR, DISCOVER_SERVER_CONNECTOR,
};
use crate::core::model::error::{ErrorCode, PolarisError};
use crate::core::plugin::cache::ResourceCache;
use crate::core::plugin::connector::{Connector, NoopConnector};
use crate::core::plugin::lossless::LosslessPolicy;
use crate::core::plugin::router::ServiceRouter;
use crate::plugins::cache::memory::memory::MemoryCache;
use crate::plugins::connector::grpc::connector::GrpcConnector;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::net::{IpAddr, ToSocketAddrs};
use std::ptr::eq;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::{env, fmt};
use tokio::runtime::{Builder, Runtime};
use tonic::transport::Server;

use super::cache::NoopResourceCache;

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

pub trait Plugin {
    fn init(&mut self, extensions: Extensions);

    fn destroy(&self);

    fn name(&self) -> String;
}

pub struct Extensions {
    pub runtime: Arc<Runtime>,

    pub client_id: String,

    pub conf: Arc<Configuration>,
    pub resource_cache: Arc<Box<dyn ResourceCache>>,
    pub http_servers: HashMap<String, Server>,
    pub lossless_policies: Vec<Arc<dyn LosslessPolicy>>,

    active_discover_connector: Arc<Box<dyn Connector>>,
    active_config_connector: Arc<Box<dyn Connector>>,
}

impl Extensions {
    pub fn build(conf: Configuration) -> Result<Self, PolarisError> {
        let mut extension = Extensions {
            runtime: Arc::new(
                Builder::new_multi_thread()
                    .enable_all()
                    .thread_name("polaris-client-thread-pool")
                    .worker_threads(1)
                    .build()
                    .unwrap(),
            ),
            client_id: acquire_client_id(&conf),
            conf: Arc::new(conf),
            active_discover_connector: Arc::new(Box::new(NoopConnector::default())),
            active_config_connector: Arc::new(Box::new(NoopConnector::default())),
            resource_cache: Arc::new(Box::new(NoopResourceCache::default())),
            http_servers: Default::default(),
            lossless_policies: vec![],
        };
        let mut containers = PluginContainer::default();
        containers.register_all_plugin();

        // 初始化所有的 ServerConnector 组件
        let ret = extension.inir_server_connector(&mut containers);
        if ret.is_err() {
            return Err(ret.unwrap_err());
        }
        Ok(extension)
    }

    pub fn get_client_id(&self) -> String {
        self.client_id.clone()
    }

    fn inir_server_connector(
        &mut self,
        containers: &mut PluginContainer,
    ) -> Result<bool, PolarisError> {
        let connector_opt = &self.conf.global.server_connectors;
        if connector_opt.is_empty() {
            return Err(PolarisError::new(ErrorCode::InvalidConfig, "".to_string()));
        }
        let connector_vec = vec![DISCOVER_SERVER_CONNECTOR, CONFIG_SERVER_CONNECTOR];
        for ele in connector_vec {
            let connector_opt = connector_opt.get(ele);
            if connector_opt.is_none() {
                return Err(PolarisError::new(
                    ErrorCode::InvalidConfig,
                    format!("{} connector not found", ele),
                ));
            }
            let protocol = connector_opt.unwrap().get_protocol();
            let supplier = containers.get_connector(&protocol);
            let mut active_connector = supplier(
                ele.to_string(),
                connector_opt.unwrap(),
                Arc::new(self.clone()),
            );
            active_connector.init(self.clone());
            if eq(ele, DISCOVER_SERVER_CONNECTOR) {
                self.active_discover_connector = Arc::new(active_connector);
            } else {
                self.active_config_connector = Arc::new(active_connector);
            }
        }

        Ok(true)
    }

    pub(crate) fn get_active_connector(&mut self, s: &str) -> Arc<Box<dyn Connector>> {
        if s.eq(DISCOVER_SERVER_CONNECTOR) {
            return self.active_discover_connector.clone();
        }
        return self.active_config_connector.clone();
    }

    fn clone(&self) -> Extensions {
        return Extensions {
            runtime: self.runtime.clone(),
            client_id: self.client_id.clone(),
            conf: self.conf.clone(),
            active_discover_connector: self.active_discover_connector.clone(),
            active_config_connector: self.active_config_connector.clone(),
            resource_cache: self.resource_cache.clone(),
            http_servers: self.http_servers.clone(),
            lossless_policies: self.lossless_policies.clone(),
        };
    }
}

struct PluginContainer {
    connectors:
        HashMap<String, fn(String, &ServerConnectorConfig, Arc<Extensions>) -> Box<dyn Connector>>,
    routers: HashMap<String, fn(&Configuration) -> Box<dyn ServiceRouter>>,
    caches: HashMap<String, fn(&Configuration) -> Box<dyn ResourceCache>>,
}

impl Default for PluginContainer {
    fn default() -> Self {
        let c = Self {
            connectors: Default::default(),
            routers: Default::default(),
            caches: Default::default(),
        };

        return c;
    }
}

impl PluginContainer {
    fn register_all_plugin(&mut self) {
        self.register_resource_cache();
        self.register_connector();
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

    fn get_connector(
        &self,
        name: &str,
    ) -> fn(String, &ServerConnectorConfig, Arc<Extensions>) -> Box<dyn Connector> {
        *self.connectors.get(name).unwrap()
    }
}

fn acquire_client_id(conf: &Configuration) -> String {
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
    let host = conf
        .global
        .server_connectors
        .get(DISCOVER_SERVER_CONNECTOR)
        .unwrap()
        .addresses
        .get(0);
    let addrs = (host.unwrap().as_str(), 80).to_socket_addrs();
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
        Err(err) => {
            return uuid::Uuid::new_v4().to_string();
        }
    }
}
