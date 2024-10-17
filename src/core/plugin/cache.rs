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
use crate::core::model::cache::{EventType, ResourceEventKey, ServerEvent};
use crate::core::model::config::{ConfigFile, ConfigGroup};
use crate::core::model::error::PolarisError;
use crate::core::model::naming::{ServiceInstances, ServiceRule, Services};
use crate::core::plugin::plugins::Plugin;
use std::sync::Arc;
use std::time::Duration;

use super::connector::Connector;

#[derive(Clone, Default)]
pub struct Filter {
    pub resource_key: ResourceEventKey,
    pub internal_request: bool,
    pub include_cache: bool,
    pub timeout: Duration,
}

impl Filter {
    pub fn get_event_type(&self) -> EventType {
        self.resource_key.event_type.clone()
    }
}

pub enum Action {
    Add,
    Update,
    Delete,
}

#[async_trait::async_trait]
pub trait ResourceListener: Send + Sync {
    // 处理事件
    async fn on_event(&self, action: Action, val: ServerEvent);
    // 获取监听的key
    fn watch_key(&self) -> EventType;
}

pub struct InitResourceCacheOption {
    pub conf: Arc<Configuration>,
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub server_connector: Arc<Box<dyn Connector>>,
}

/// 资源缓存
#[async_trait::async_trait]
pub trait ResourceCache: Plugin {
    // 加载服务规则
    async fn load_service_rule(&self, filter: Filter) -> Result<ServiceRule, PolarisError>;
    // 加载服务
    async fn load_services(&self, filter: Filter) -> Result<Services, PolarisError>;
    // 加载服务实例
    async fn load_service_instances(
        &self,
        filter: Filter,
    ) -> Result<ServiceInstances, PolarisError>;
    // 加载配置文件
    async fn load_config_file(&self, filter: Filter) -> Result<ConfigFile, PolarisError>;
    // 加载配置文件组
    async fn load_config_group_files(&self, filter: Filter) -> Result<ConfigGroup, PolarisError>;
    // 注册资源监听器
    async fn register_resource_listener(&self, listener: Arc<dyn ResourceListener>);
}

pub struct NoopResourceCache {}

impl Default for NoopResourceCache {
    fn default() -> Self {
        Self {}
    }
}

impl Plugin for NoopResourceCache {
    fn init(&mut self) {
        todo!()
    }

    fn destroy(&self) {
        todo!()
    }

    fn name(&self) -> String {
        todo!()
    }
}

#[async_trait::async_trait]
impl ResourceCache for NoopResourceCache {
    async fn load_service_rule(&self, filter: Filter) -> Result<ServiceRule, PolarisError> {
        todo!()
    }

    async fn load_services(&self, filter: Filter) -> Result<Services, PolarisError> {
        todo!()
    }

    async fn load_service_instances(
        &self,
        filter: Filter,
    ) -> Result<ServiceInstances, PolarisError> {
        todo!()
    }

    async fn load_config_file(&self, filter: Filter) -> Result<ConfigFile, PolarisError> {
        todo!()
    }

    async fn load_config_group_files(&self, filter: Filter) -> Result<ConfigGroup, PolarisError> {
        todo!()
    }

    async fn register_resource_listener(&self, listener: Arc<dyn ResourceListener>) {
        todo!()
    }
}
