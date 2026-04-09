// Tencent is pleased to support the open source community by making Pole available.
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

use crate::core::config::global::LocalCacheConfig;
use crate::core::model::cache::{
    CircuitBreakerRulesCacheItem, EventType, FaultDetectRulesCacheItem, LaneRulesCacheItem,
    RatelimitRulesCacheItem, ResourceEventKey, RouterRulesCacheItem, ServerEvent,
    ServiceInstancesCacheItem,
};
use crate::core::model::config::{ConfigFile, ConfigGroup};
use crate::core::model::error::PoleError;
use crate::core::model::naming::{ServiceRule, Services};
use crate::core::plugin::plugins::Plugin;
use pole_specification::v1::{
    CircuitBreakerRule, ConfigDiscoverResponse, DiscoverResponse, FaultDetectRule, FaultDetector,
    LaneGroup, RateLimit, RouteRule,
};
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
        self.resource_key.event_type
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
    pub conf: LocalCacheConfig,
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub server_connector: Arc<Box<dyn Connector>>,
}

/// 资源缓存
#[async_trait::async_trait]
pub trait ResourceCache: Plugin {
    fn set_failover_provider(&mut self, failover: Arc<dyn ResourceCacheFailover>);
    // 加载服务自定义路由规则
    async fn load_service_custom_route(&self, filter: Filter) -> Result<Vec<RouteRule>, PoleError>;
    // 加载全链路灰度路由规则
    async fn load_service_lane_route(&self, filter: Filter) -> Result<Vec<LaneGroup>, PoleError>;
    // 加载服务限流规则
    async fn load_service_ratelimit(&self, filter: Filter) -> Result<Vec<RateLimit>, PoleError>;
    // 加载服务熔断规则
    async fn load_service_circuitbreaker(
        &self,
        filter: Filter,
    ) -> Result<Vec<CircuitBreakerRule>, PoleError>;
    // 加载服务故障检测规则
    async fn load_service_faultdetect(&self, filter: Filter) -> Result<FaultDetector, PoleError>;
    // 加载服务
    async fn load_services(&self, filter: Filter) -> Result<Services, PoleError>;
    // 加载服务实例
    async fn load_service_instances(
        &self,
        filter: Filter,
    ) -> Result<ServiceInstancesCacheItem, PoleError>;
    // 加载配置文件
    async fn load_config_file(&self, filter: Filter) -> Result<ConfigFile, PoleError>;
    // 加载配置文件组
    async fn load_config_group_files(&self, filter: Filter) -> Result<ConfigGroup, PoleError>;
    // 注册资源监听器
    async fn register_resource_listener(&self, listener: Arc<dyn ResourceListener>);
}

#[async_trait::async_trait]
pub trait ResourceCacheFailover: Send + Sync {
    // failover_naming_load 兜底加载
    async fn failover_naming_load(&self, filter: Filter) -> Result<DiscoverResponse, PoleError>;
    // save_naming_failover 保存容灾数据
    async fn save_naming_failover(&self, value: DiscoverResponse) -> Result<(), PoleError>;

    // failover_config_load 兜底加载
    async fn failover_config_load(
        &self,
        filter: Filter,
    ) -> Result<ConfigDiscoverResponse, PoleError>;
    // save_config_failover 保存容灾数据
    async fn save_config_failover(&self, value: ConfigDiscoverResponse) -> Result<(), PoleError>;
}

#[derive(Default)]
pub struct NoopResourceCache {}

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
    fn set_failover_provider(&mut self, failover: Arc<dyn ResourceCacheFailover>) {
        todo!()
    }

    async fn load_service_custom_route(&self, filter: Filter) -> Result<Vec<RouteRule>, PoleError> {
        todo!()
    }

    async fn load_service_lane_route(&self, filter: Filter) -> Result<Vec<LaneGroup>, PoleError> {
        todo!()
    }

    async fn load_service_ratelimit(&self, filter: Filter) -> Result<Vec<RateLimit>, PoleError> {
        todo!()
    }

    async fn load_service_circuitbreaker(
        &self,
        filter: Filter,
    ) -> Result<Vec<CircuitBreakerRule>, PoleError> {
        todo!()
    }

    async fn load_service_faultdetect(&self, filter: Filter) -> Result<FaultDetector, PoleError> {
        todo!()
    }

    async fn load_services(&self, filter: Filter) -> Result<Services, PoleError> {
        todo!()
    }

    async fn load_service_instances(
        &self,
        filter: Filter,
    ) -> Result<ServiceInstancesCacheItem, PoleError> {
        todo!()
    }

    async fn load_config_file(&self, filter: Filter) -> Result<ConfigFile, PoleError> {
        todo!()
    }

    async fn load_config_group_files(&self, filter: Filter) -> Result<ConfigGroup, PoleError> {
        todo!()
    }

    async fn register_resource_listener(&self, listener: Arc<dyn ResourceListener>) {
        todo!()
    }
}
