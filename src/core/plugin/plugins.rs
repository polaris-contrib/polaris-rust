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

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use tonic::transport::Server;
use crate::core::config::config::Configuration;
use crate::core::config::global::{CONFIG_SERVER_CONNECTOR, DISCOVER_SERVER_CONNECTOR};
use crate::core::model::error::{ErrorCode, PolarisError};
use crate::core::model::naming::InstanceRequest;
use crate::core::plugin::cache::ResourceCache;
use crate::core::plugin::connector::{Connector, NoopConnector};
use crate::core::plugin::lossless::LosslessPolicy;
use crate::core::plugin::plugins::PluginType::{PluginCache, PluginConnector};
use crate::core::plugin::router::ServiceRouter;
use crate::plugins::cache::memory::memory::MemoryCache;
use crate::plugins::connector::grpc::connector::GrpcConnector;

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum PluginType {
    PluginCache,
    PluginRouter,
    PluginLocation,
    PluginLoadBalance,
    PluginCircuitBreaker,
    PluginConnector,
    PluginRateLimit
}

impl Display for PluginType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait Plugin {
    fn init(&self, options: HashMap<String, String>, extensions: Arc<Extensions>);

    fn destroy(&self);

    fn name(&self) -> String;
}

pub struct Extensions {
    active_discover_connector: Arc<dyn Connector>,
    active_config_connector: Arc<dyn Connector>,
    pub containers: PluginContainer,
    pub resource_cache: Arc<()>,
    pub http_servers: HashMap<String, Server>,
    pub lossless_policies: Vec<Arc<dyn LosslessPolicy>>,
}

impl Default for Extensions {
    fn default() -> Self {
        let mut extension = Extensions{
            active_discover_connector: Arc::new(NoopConnector::default()),
            active_config_connector: Arc::new(NoopConnector::default()),
            containers: PluginContainer::default(),
            resource_cache: Arc::new(()),
            http_servers: Default::default(),
            lossless_policies: vec![],
        };
        extension
    }
}

impl Extensions {

    pub(crate) fn init(&mut self, conf: &Configuration) -> Result<bool, PolarisError> {
        self.containers.register_all_plugin();
        let connector_opt = &conf.global.server_connectors;
        if connector_opt.is_empty() {
            return Err(PolarisError::new(ErrorCode::InvalidConfig, "".to_string()))
        }
        let discover_connector_opt = connector_opt.get(DISCOVER_SERVER_CONNECTOR);
        discover_connector_opt.map(|mut connector| {
            let protocol = connector.get_protocol();
            self.active_discover_connector = self.containers.get_connector(&protocol);
        });

        let config_connector_opt = connector_opt.get(CONFIG_SERVER_CONNECTOR);
        config_connector_opt.map(|mut connector| {
            let protocol = connector.get_protocol();
            self.active_config_connector = self.containers.get_connector(&protocol);
        });

        Ok(true)
    }

    pub(crate) fn get_active_connector(&mut self, s: &str) -> Arc<dyn Connector> {
        if s.eq(DISCOVER_SERVER_CONNECTOR) {
            return Arc::clone(&self.active_discover_connector);
        }
        return Arc::clone(&self.active_config_connector);
    }
}

struct PluginContainer {
    connectors: HashMap<String, Arc<dyn Connector>>,
    routers: HashMap<String, Arc<dyn ServiceRouter>>,
    caches: HashMap<String, Arc<dyn ResourceCache>>,
}

impl Default for PluginContainer {
    fn default() -> Self {
        let mut c = Self {
            connectors: Default::default(),
            routers: Default::default(),
            caches: Default::default(),
        };

        return c
    }
}

impl PluginContainer {
    fn register_all_plugin(&mut self) {
        self.register_resource_cache();
        self.register_connector();
    }

    fn register_connector(&mut self) {
        let vec = vec![
            Arc::new(GrpcConnector::default()),
        ];
        for c in vec {
            self.connectors.insert(c.name(), c);
        }
    }

    fn register_resource_cache(&mut self) {
        let vec = vec![
            Arc::new(MemoryCache::default()),
        ];
        for c in vec {
            self.caches.insert(c.name(), c);
        }
    }

    fn get_connector(&self, name: &str) -> Arc<dyn Connector> {
        self.connectors.get(name).map_or(Arc::new(NoopConnector::default()), |c| c.clone())
    }
}
