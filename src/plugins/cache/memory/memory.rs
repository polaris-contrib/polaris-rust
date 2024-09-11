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

use tokio::sync::RwLock;
use tokio::task::yield_now;

use crate::core::config::global::LocalCacheConfig;
use crate::core::model::cache::{RegistryCacheValue, ResourceEventKey};
use crate::core::model::config::{ConfigFile, ConfigGroup};
use crate::core::model::error::{ErrorCode, PolarisError};
use crate::core::model::naming::{Instance, ServiceInfo, ServiceRule, Services};
use crate::core::model::pb::lib::{CircuitBreaker, FaultDetector, LaneGroup, RateLimit, Routing};
use crate::core::plugin::cache::{Filter, ResourceCache, ResourceListener};
use crate::core::plugin::connector::{Connector, ResourceHandler};
use crate::core::plugin::plugins::{Extensions, Plugin};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub struct MemoryCache {
    extensions: Arc<Extensions>,
    // 资源连接器
    server_connector: Arc<Box<dyn Connector>>,
    // services 服务列表缓存
    services: Arc<RwLock<HashMap<String, ServicesCacheItem>>>,
    // instances 服务实例缓存 key: namespace#service
    instances: Arc<RwLock<HashMap<String, ServiceInstancesCacheItem>>>,
    // router_rules 路由规则缓存 key: namespace#service
    router_rules: Arc<RwLock<HashMap<String, RouterRulesCacheItem>>>,
    // ratelimit_rules 限流规则缓存 key: namespace#service
    ratelimit_rules: Arc<RwLock<HashMap<String, RatelimitRulesCacheItem>>>,
    // circuitbreaker_rules 熔断规则缓存 key: namespace#service
    circuitbreaker_rules: Arc<RwLock<HashMap<String, CircuitBreakerRulesCacheItem>>>,
    // faultdetect_rules 主动探测规则缓存 key: namespace#service
    faultdetect_rules: Arc<RwLock<HashMap<String, FaultDetectRulesCacheItem>>>,
    // config_groups 配置分组缓存 key: namespace#group_name
    config_groups: Arc<RwLock<HashMap<String, ConfigGroupCacheItem>>>,
    // config_files 配置文件缓存 key: namespace#group_name#file_name
    config_files: Arc<RwLock<HashMap<String, ConfigFileCacheItem>>>,
}

impl MemoryCache {
    pub fn builder() -> (
        fn(String, &LocalCacheConfig, Arc<Extensions>) -> Box<dyn ResourceCache>,
        String,
    ) {
        return (new_resource_cache, "memory".to_string());
    }
}

fn new_resource_cache(
    label: String,
    conf: &LocalCacheConfig,
    extensions: Arc<Extensions>,
) -> Box<dyn ResourceCache> {
    let extensions_clone = extensions.clone();
    let server_connector = extensions.server_connector.as_ref().unwrap();

    let mc = MemoryCache {
        extensions: extensions_clone,
        server_connector: server_connector.to_owned(),
        services: Arc::new(RwLock::new(HashMap::new())),
        instances: Arc::new(RwLock::new(HashMap::new())),
        router_rules: Arc::new(RwLock::new(HashMap::new())),
        ratelimit_rules: Arc::new(RwLock::new(HashMap::new())),
        circuitbreaker_rules: Arc::new(RwLock::new(HashMap::new())),
        faultdetect_rules: Arc::new(RwLock::new(HashMap::new())),
        config_groups: Arc::new(RwLock::new(HashMap::new())),
        config_files: Arc::new(RwLock::new(HashMap::new())),
    };

    Box::new(mc) as Box<dyn ResourceCache + 'static>
}

impl Plugin for MemoryCache {
    fn init(&mut self) {
        todo!()
    }

    fn destroy(&self) {
        todo!()
    }

    fn name(&self) -> String {
        "memory".to_string()
    }
}

#[async_trait::async_trait]
impl ResourceCache for MemoryCache {
    /// get_services 获取服务列表
    async fn get_services(&self, filter: Filter) -> Result<&[ServiceInfo], PolarisError> {
        let server_connector = self.server_connector.clone();
        let search_namespace = filter.resource_key.namespace.clone();
        let mut safe_map = self.services.write().await;
        let cache_val = safe_map.entry(search_namespace.clone()).or_insert_with(|| {
                // 触发 loadRemote 拉取
                tracing::info!("[polaris][resource_cache][memory] load remote services resource: {}", search_namespace);
                // 提交数据异步同步任务
                self.extensions.runtime.spawn(async move {
                    let register_ret = server_connector
                        .register_resource_handler(Arc::new(MemoryResourceHandler {}))
                        .await;
                if register_ret.is_err() {
                    tracing::error!(
                        "[polaris][resource_cache][memory] register resource handler failed: {}, err: {}",
                        search_namespace,
                        register_ret.err().unwrap()
                    );
                }
            });
                ServicesCacheItem::new()
            });
        // 等待资源
        cache_val.wait_initialize(filter.timeout).await;
        // 如果还是没有初始化
        if !cache_val.is_initialized() {
            return Err(PolarisError::new(
                ErrorCode::InternalError,
                "load remote resource timeout".to_string(),
            ));
        }

        Ok(cache_val.value.read().await.as_slice())
    }

    async fn load_service_rule(&self, filter: Filter) -> Result<ServiceRule, PolarisError> {
        todo!()
    }

    async fn load_services(&self, filter: Filter) -> Result<Services, PolarisError> {
        todo!()
    }

    async fn load_service_instances(&self, filter: Filter) -> Result<Services, PolarisError> {
        todo!()
    }

    async fn load_config_file(&self, filter: Filter) -> Result<ConfigFile, PolarisError> {
        todo!()
    }

    async fn load_config_groups(&self, filter: Filter) -> Result<ConfigGroup, PolarisError> {
        todo!()
    }

    async fn load_config_group_files(&self, filter: Filter) -> Result<ConfigGroup, PolarisError> {
        todo!()
    }

    async fn register_resource_listener(&self, listener: Arc<dyn ResourceListener>) {
        todo!()
    }

    async fn watch_resource(&self, key: ResourceEventKey) {
        todo!()
    }

    async fn un_watch_resource(&self, key: ResourceEventKey) {
        todo!()
    }
}

struct MemoryResourceHandler {}

impl ResourceHandler for MemoryResourceHandler {
    fn handle_event(&self, event: crate::core::model::cache::ServerEvent) {
        todo!()
    }

    fn interest_resource(&self) -> ResourceEventKey {
        todo!()
    }
}

// 缓存数据结构

// ServicesCacheItem 服务列表
struct ServicesCacheItem {
    initialized: AtomicBool,
    value: Arc<RwLock<Vec<ServiceInfo>>>,
}

impl ServicesCacheItem {
    fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            value: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn wait_initialize(&self, timeout: Duration) {
        let start = std::time::Instant::now();
        while !self.initialized.load(Ordering::SeqCst) {
            yield_now().await;
            if start.elapsed() > timeout {
                break;
            }
        }
    }
}

impl RegistryCacheValue for ServicesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::Service
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    fn revision(&self) -> String {
        todo!()
    }
}

// ServiceInstancesCacheItem 服务实例
struct ServiceInstancesCacheItem {
    value: Vec<Instance>,
}

impl RegistryCacheValue for ServiceInstancesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::Instance
    }

    fn is_initialized(&self) -> bool {
        todo!()
    }

    fn revision(&self) -> String {
        todo!()
    }
}

// RouterRulesCacheItem 路由规则
struct RouterRulesCacheItem {
    value: Vec<Routing>,
}

impl RegistryCacheValue for RouterRulesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::RouterRule
    }

    fn is_initialized(&self) -> bool {
        todo!()
    }

    fn revision(&self) -> String {
        todo!()
    }
}

// LaneRulesCacheItem 泳道规则
struct LaneRulesCacheItem {
    value: Vec<LaneGroup>,
}

impl RegistryCacheValue for LaneRulesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::LaneRule
    }

    fn is_initialized(&self) -> bool {
        todo!()
    }

    fn revision(&self) -> String {
        todo!()
    }
}

// RatelimitRulesCacheItem 限流规则
struct RatelimitRulesCacheItem {
    value: Vec<RateLimit>,
}

impl RegistryCacheValue for RatelimitRulesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::RateLimitRule
    }

    fn is_initialized(&self) -> bool {
        todo!()
    }

    fn revision(&self) -> String {
        todo!()
    }
}

// CircuitBreakerRulesCacheItem 熔断规则
struct CircuitBreakerRulesCacheItem {
    value: CircuitBreaker,
}

impl RegistryCacheValue for CircuitBreakerRulesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::CircuitBreakerRule
    }

    fn is_initialized(&self) -> bool {
        todo!()
    }

    fn revision(&self) -> String {
        todo!()
    }
}

// FaultDetectRulesCacheItem 主动探测规则
struct FaultDetectRulesCacheItem {
    value: FaultDetector,
}

impl RegistryCacheValue for FaultDetectRulesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::FaultDetectRule
    }

    fn is_initialized(&self) -> bool {
        todo!()
    }

    fn revision(&self) -> String {
        todo!()
    }
}

// ConfigGroupCacheItem 当个配置分组下已发布的文件列表信息
struct ConfigGroupCacheItem {
    value: ConfigGroup,
}

impl RegistryCacheValue for ConfigGroupCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::ConfigGroup
    }

    fn is_initialized(&self) -> bool {
        todo!()
    }

    fn revision(&self) -> String {
        todo!()
    }
}

// ConfigFileCacheItem 单个配置文件的最新发布信息
struct ConfigFileCacheItem {
    value: ConfigFile,
}

impl RegistryCacheValue for ConfigFileCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::ConfigFile
    }

    fn is_initialized(&self) -> bool {
        todo!()
    }

    fn revision(&self) -> String {
        todo!()
    }
}
