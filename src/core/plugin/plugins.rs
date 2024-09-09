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
use crate::core::engine::Engine;
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
use tokio::runtime::Runtime;
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

pub trait Plugin
where
    Self: Send + Sync,
{
    fn init(&mut self, extensions: Arc<Extensions>);

    fn destroy(&self);

    fn name(&self) -> String;
}

pub struct Extensions
where
    Self: Send + Sync,
{
    pub runtime: Arc<Runtime>,
    pub client_id: String,
    pub conf: Arc<Configuration>,
}

impl Extensions {
    pub fn build(
        client_id: String,
        conf: Arc<Configuration>,
        runetime: Arc<Runtime>,
    ) -> Result<Self, PolarisError> {
        Ok(Self {
            runtime: runetime,
            client_id: client_id,
            conf: conf,
        })
    }

    pub fn get_client_id(&self) -> String {
        self.client_id.clone()
    }
}

pub struct ConnectorResult {
    pub discover: Option<Box<dyn Connector>>,
    pub config: Option<Box<dyn Connector>>,
}

pub fn init_server_connector(
    connector_opt: &HashMap<String, ServerConnectorConfig>,
    containers: &PluginContainer,
    extensions: Arc<Extensions>
) -> Result<ConnectorResult, PolarisError> {
    if connector_opt.is_empty() {
        return Err(PolarisError::new(ErrorCode::InvalidConfig, "".to_string()));
    }

    let connector_vec = vec![DISCOVER_SERVER_CONNECTOR, CONFIG_SERVER_CONNECTOR];
    let mut connectors = ConnectorResult {
        discover: None,
        config: None,
    };
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
        let mut active_connector =
            supplier(ele.to_string(), connector_opt.unwrap(), extensions.clone());
        active_connector.init(extensions.clone());
        if eq(ele, DISCOVER_SERVER_CONNECTOR) {
            connectors.discover = Some(active_connector);
        } else {
            connectors.config = Some(active_connector);
        }
    }

    Ok(connectors)
}

pub struct PluginContainer {
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
    pub fn register_all_plugin(&mut self) {
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
