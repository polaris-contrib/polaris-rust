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

use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
    thread::sleep,
    time::{Duration},
};

use tokio::sync::RwLock;

use super::{
    config::ConfigFile,
    naming::{Instance, ServiceInfo},
    pb::lib::{
        config_discover_request::ConfigDiscoverRequestType, discover_request::DiscoverRequestType,
        CircuitBreaker, ClientConfigFileInfo, ConfigDiscoverRequest, ConfigDiscoverResponse,
        DiscoverFilter, DiscoverRequest, DiscoverResponse, FaultDetector, LaneGroup, RateLimit,
        Routing, Service,
    },
};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum EventType {
    Unknown,
    Instance,
    RouterRule,
    CircuitBreakerRule,
    RateLimitRule,
    Service,
    FaultDetectRule,
    ServiceContract,
    LaneRule,
    Namespaces,
    ConfigFile,
    ConfigGroup,
}

impl Default for EventType {
    fn default() -> Self {
        Self::Unknown
    }
}

impl ToString for EventType {
    fn to_string(&self) -> String {
        match self {
            EventType::Unknown => "unknown".to_string(),
            EventType::Instance => "Instance".to_string(),
            EventType::RouterRule => "RouterRule".to_string(),
            EventType::CircuitBreakerRule => "CircuitBreakerRule".to_string(),
            EventType::RateLimitRule => "RateLimitRule".to_string(),
            EventType::Service => "Service".to_string(),
            EventType::FaultDetectRule => "FaultDetectRule".to_string(),
            EventType::ServiceContract => "ServiceContract".to_string(),
            EventType::LaneRule => "LaneRule".to_string(),
            EventType::Namespaces => "Namespaces".to_string(),
            EventType::ConfigFile => "ConfigFile".to_string(),
            EventType::ConfigGroup => "ConfigGroup".to_string(),
        }
    }
}

#[derive(Clone)]
pub enum CacheItemType {
    Unknown,
    Instance(ServiceInstancesCacheItem),
    RouterRule(RouterRulesCacheItem),
    CircuitBreakerRule(CircuitBreakerRulesCacheItem),
    RateLimitRule(RatelimitRulesCacheItem),
    Service(ServicesCacheItem),
    FaultDetectRule(FaultDetectRulesCacheItem),
    LaneRule(LaneRulesCacheItem),
    ConfigFile(ConfigFileCacheItem),
    ConfigGroup(ConfigGroupCacheItem),
}

impl CacheItemType {
    pub fn to_service_instances(&self) -> Option<ServiceInstancesCacheItem> {
        match self {
            CacheItemType::Instance(item) => Some(item.clone()),
            _ => None,
        }
    }

    pub fn to_config_file(&self) -> Option<ConfigFileCacheItem> {
        match self {
            CacheItemType::ConfigFile(item) => Some(item.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RemoteData {
    pub event_key: ResourceEventKey,
    pub discover_value: Option<DiscoverResponse>,
    pub config_value: Option<ConfigDiscoverResponse>,
}

pub struct ServerEvent {
    pub event_key: ResourceEventKey,
    pub value: CacheItemType,
}

impl Clone for ServerEvent {
    fn clone(&self) -> Self {
        Self {
            event_key: self.event_key.clone(),
            value: self.value.clone(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResourceEventKey {
    pub namespace: String,
    pub event_type: EventType,
    pub filter: HashMap<String, String>,
}

impl ResourceEventKey {
    pub fn to_discover_request(&self, revision: String) -> Option<DiscoverRequest> {
        match self.event_type {
            crate::core::model::cache::EventType::Instance => Some(DiscoverRequest {
                r#type: DiscoverRequestType::Instance.into(),
                service: Some(self.to_spec_service(revision)),
                filter: Some(DiscoverFilter::default()),
            }),
            crate::core::model::cache::EventType::RouterRule => Some(DiscoverRequest {
                r#type: DiscoverRequestType::Routing.into(),
                service: Some(self.to_spec_service(revision)),
                filter: Some(DiscoverFilter::default()),
            }),
            crate::core::model::cache::EventType::CircuitBreakerRule => Some(DiscoverRequest {
                r#type: DiscoverRequestType::CircuitBreaker.into(),
                service: Some(self.to_spec_service(revision)),
                filter: Some(DiscoverFilter::default()),
            }),
            crate::core::model::cache::EventType::RateLimitRule => Some(DiscoverRequest {
                r#type: DiscoverRequestType::RateLimit.into(),
                service: Some(self.to_spec_service(revision)),
                filter: Some(DiscoverFilter::default()),
            }),
            crate::core::model::cache::EventType::Service => Some(DiscoverRequest {
                r#type: DiscoverRequestType::Services.into(),
                service: Some(self.to_spec_service(revision)),
                filter: Some(DiscoverFilter::default()),
            }),
            crate::core::model::cache::EventType::FaultDetectRule => Some(DiscoverRequest {
                r#type: DiscoverRequestType::FaultDetector.into(),
                service: Some(self.to_spec_service(revision)),
                filter: Some(DiscoverFilter::default()),
            }),
            crate::core::model::cache::EventType::LaneRule => Some(DiscoverRequest {
                r#type: DiscoverRequestType::Lane.into(),
                service: Some(self.to_spec_service(revision)),
                filter: Some(DiscoverFilter::default()),
            }),
            _ => None,
        }
    }

    pub fn to_config_request(&self, revision: String) -> Option<ConfigDiscoverRequest> {
        match self.event_type {
            crate::core::model::cache::EventType::ConfigFile => Some(ConfigDiscoverRequest {
                r#type: ConfigDiscoverRequestType::ConfigFile.into(),
                config_file: Some(self.to_spec_config_file()),
                revision,
            }),
            crate::core::model::cache::EventType::ConfigGroup => Some(ConfigDiscoverRequest {
                r#type: ConfigDiscoverRequestType::ConfigFileNames.into(),
                config_file: Some(self.to_spec_config_group()),
                revision,
            }),
            _ => None,
        }
    }

    pub fn to_spec_service(&self, revision: String) -> Service {
        let svc = self.filter.get("service").unwrap().to_string();
        Service {
            namespace: Some(self.namespace.clone()),
            name: Some(svc),
            revision: Some(revision),
            ..Service::default()
        }
    }

    pub fn to_spec_config_file(&self) -> ClientConfigFileInfo {
        let group = self.filter.get("group").unwrap().to_string();
        let file_name = self.filter.get("file").unwrap().to_string();
        ClientConfigFileInfo {
            namespace: Some(self.namespace.clone()),
            group: Some(group),
            name: Some(file_name),
            ..ClientConfigFileInfo::default()
        }
    }

    pub fn to_spec_config_group(&self) -> ClientConfigFileInfo {
        let group = self.filter.get("group").unwrap().to_string();
        ClientConfigFileInfo {
            namespace: Some(self.namespace.clone()),
            group: Some(group),
            ..ClientConfigFileInfo::default()
        }
    }
}

impl ToString for ResourceEventKey {
    fn to_string(&self) -> String {
        let mut key = String::new();
        key.push_str(&self.event_type.to_string());
        key.push('#');
        key.push_str(self.namespace.clone().as_str());
        key.push('#');
        match self.event_type {
            EventType::ConfigFile => {
                let service = self.filter.get("group");
                key.push_str(service.unwrap().as_str());
                key.push('#');
                let service = self.filter.get("file");
                key.push_str(service.unwrap().as_str());
            }
            EventType::ConfigGroup => {
                let service = self.filter.get("group");
                key.push_str(service.unwrap().as_str());
            }
            _ => {
                let service = self.filter.get("service");
                key.push_str(service.unwrap().as_str());
            }
        }
        key
    }
}

fn build_waiter(initialized: Arc<AtomicBool>, timeout: Duration) -> Box<dyn Fn() + Send> {
    Box::new(move || {
        let start = std::time::Instant::now();
        while !initialized.load(std::sync::atomic::Ordering::Acquire) {
            if start.elapsed() > timeout {
                break;
            }
            sleep(Duration::from_millis(100));
        }
    })
}

#[async_trait::async_trait]
pub trait RegistryCacheValue {
    fn is_loaded_from_file(&self) -> bool;

    fn event_type(&self) -> EventType;

    async fn wait_initialize(&self, timeout: Duration) -> Box<dyn Fn() + Send>;

    fn is_initialized(&self) -> bool;

    fn revision(&self) -> String;
}

// ServicesCacheItem 服务列表
pub struct ServicesCacheItem {
    initialized: Arc<AtomicBool>,
    pub value: Arc<RwLock<Vec<ServiceInfo>>>,
}

impl Default for ServicesCacheItem {
    fn default() -> Self {
        Self::new()
    }
}

impl ServicesCacheItem {
    pub fn new() -> Self {
        Self {
            initialized: Arc::new(AtomicBool::new(false)),
            value: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Clone for ServicesCacheItem {
    fn clone(&self) -> Self {
        Self {
            initialized: self.initialized.clone(),
            value: self.value.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RegistryCacheValue for ServicesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::Service
    }

    async fn wait_initialize(&self, timeout: Duration) -> Box<dyn Fn() + Send> {
        build_waiter(self.initialized.clone(), timeout)
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn revision(&self) -> String {
        todo!()
    }
}

// ServiceInstancesCacheItem 服务实例
pub struct ServiceInstancesCacheItem {
    initialized: Arc<AtomicBool>,
    pub svc_info: Service,
    pub value: Arc<RwLock<Vec<Instance>>>,
    pub available_instances: Arc<RwLock<Vec<Instance>>>,
    pub total_weight: u64,
    pub revision: String,
}

impl Default for ServiceInstancesCacheItem {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceInstancesCacheItem {
    pub fn finish_initialize(&self) {
        let _ = self.initialized.compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        );
    }

    pub fn new() -> Self {
        Self {
            initialized: Arc::new(AtomicBool::new(false)),
            svc_info: Service::default(),
            value: Arc::new(RwLock::new(Vec::new())),
            available_instances: Arc::new(RwLock::new(Vec::new())),
            total_weight: 0,
            revision: String::new(),
        }
    }

    pub async fn list_instances(&self, only_available: bool) -> Vec<Instance> {
        if only_available {
            return self.available_instances.read().await.clone();
        }
        self.value.read().await.clone()
    }

    pub fn get_service_info(&self) -> ServiceInfo {
        let svc_info = &self.svc_info;
        let id = svc_info.id.clone();
        let namespace = svc_info.namespace.clone();
        let name = svc_info.name.clone();
        let revision = svc_info.revision.clone();

        ServiceInfo {
            id: id.unwrap_or_default().clone(),
            namespace: namespace.unwrap_or_default().to_string(),
            name: name.unwrap_or_default().to_string(),
            metadata: self.svc_info.metadata.clone(),
            revision: revision.unwrap_or_default().clone(),
        }
    }
}

impl Clone for ServiceInstancesCacheItem {
    fn clone(&self) -> Self {
        Self {
            initialized: self.initialized.clone(),
            svc_info: self.svc_info.clone(),
            value: self.value.clone(),
            available_instances: self.available_instances.clone(),
            total_weight: self.total_weight,
            revision: self.revision.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RegistryCacheValue for ServiceInstancesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::Instance
    }

    async fn wait_initialize(&self, timeout: Duration) -> Box<dyn Fn() + Send> {
        build_waiter(self.initialized.clone(), timeout)
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(std::sync::atomic::Ordering::Acquire)
    }

    fn revision(&self) -> String {
        self.revision.clone()
    }
}

// RouterRulesCacheItem 路由规则
pub struct RouterRulesCacheItem {
    initialized: Arc<AtomicBool>,
    pub revision: String,
    pub value: Arc<RwLock<Vec<Routing>>>,
}

impl Default for RouterRulesCacheItem {
    fn default() -> Self {
        Self::new()
    }
}

impl RouterRulesCacheItem {
    pub fn new() -> Self {
        Self {
            initialized: Arc::new(AtomicBool::new(false)),
            value: Arc::new(RwLock::new(Vec::new())),
            revision: String::new(),
        }
    }

    pub fn finish_initialize(&self) {
        let _ = self.initialized.compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        );
    }
}

impl Clone for RouterRulesCacheItem {
    fn clone(&self) -> Self {
        Self {
            initialized: self.initialized.clone(),
            value: self.value.clone(),
            revision: self.revision.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RegistryCacheValue for RouterRulesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::RouterRule
    }

    async fn wait_initialize(&self, timeout: Duration) -> Box<dyn Fn() + Send> {
        build_waiter(self.initialized.clone(), timeout)
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(std::sync::atomic::Ordering::Acquire)
    }

    fn revision(&self) -> String {
        self.revision.clone()
    }
}

// LaneRulesCacheItem 泳道规则
pub struct LaneRulesCacheItem {
    initialized: Arc<AtomicBool>,
    value: Vec<LaneGroup>,
    revision: String,
}

impl Clone for LaneRulesCacheItem {
    fn clone(&self) -> Self {
        Self {
            initialized: Arc::new(AtomicBool::new(false)),
            value: self.value.clone(),
            revision: self.revision.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RegistryCacheValue for LaneRulesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::LaneRule
    }

    async fn wait_initialize(&self, timeout: Duration) -> Box<dyn Fn() + Send> {
        build_waiter(self.initialized.clone(), timeout)
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(std::sync::atomic::Ordering::Acquire)
    }

    fn revision(&self) -> String {
        self.revision.clone()
    }
}

// RatelimitRulesCacheItem 限流规则
pub struct RatelimitRulesCacheItem {
    initialized: Arc<AtomicBool>,
    pub value: RateLimit,
    pub revision: String,
}

impl Default for RatelimitRulesCacheItem {
    fn default() -> Self {
        Self::new()
    }
}

impl RatelimitRulesCacheItem {
    pub fn new() -> Self {
        Self {
            initialized: Arc::new(AtomicBool::new(false)),
            value: RateLimit::default(),
            revision: String::new(),
        }
    }

    pub fn finish_initialize(&self) {
        let _ = self.initialized.compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        );
    }
}

impl Clone for RatelimitRulesCacheItem {
    fn clone(&self) -> Self {
        Self {
            initialized: self.initialized.clone(),
            value: self.value.clone(),
            revision: self.revision.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RegistryCacheValue for RatelimitRulesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::RateLimitRule
    }

    async fn wait_initialize(&self, timeout: Duration) -> Box<dyn Fn() + Send> {
        build_waiter(self.initialized.clone(), timeout)
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(std::sync::atomic::Ordering::Acquire)
    }

    fn revision(&self) -> String {
        self.revision.clone()
    }
}

// CircuitBreakerRulesCacheItem 熔断规则
pub struct CircuitBreakerRulesCacheItem {
    initialized: Arc<AtomicBool>,
    pub value: CircuitBreaker,
    pub revision: String,
}

impl Default for CircuitBreakerRulesCacheItem {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreakerRulesCacheItem {
    pub fn new() -> Self {
        Self {
            initialized: Arc::new(AtomicBool::new(false)),
            value: CircuitBreaker::default(),
            revision: String::new(),
        }
    }

    pub fn finish_initialize(&self) {
        let _ = self.initialized.compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        );
    }
}

impl Clone for CircuitBreakerRulesCacheItem {
    fn clone(&self) -> Self {
        Self {
            initialized: self.initialized.clone(),
            value: self.value.clone(),
            revision: self.revision.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RegistryCacheValue for CircuitBreakerRulesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::CircuitBreakerRule
    }

    async fn wait_initialize(&self, timeout: Duration) -> Box<dyn Fn() + Send> {
        build_waiter(self.initialized.clone(), timeout)
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(std::sync::atomic::Ordering::Acquire)
    }

    fn revision(&self) -> String {
        self.revision.clone()
    }
}

// FaultDetectRulesCacheItem 主动探测规则
pub struct FaultDetectRulesCacheItem {
    initialized: Arc<AtomicBool>,
    pub value: FaultDetector,
    pub revision: String,
}

impl Default for FaultDetectRulesCacheItem {
    fn default() -> Self {
        Self::new()
    }
}

impl FaultDetectRulesCacheItem {
    pub fn new() -> Self {
        Self {
            initialized: Arc::new(AtomicBool::new(false)),
            value: FaultDetector::default(),
            revision: String::new(),
        }
    }

    pub fn finish_initialize(&self) {
        let _ = self.initialized.compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        );
    }
}

impl Clone for FaultDetectRulesCacheItem {
    fn clone(&self) -> Self {
        Self {
            initialized: self.initialized.clone(),
            value: self.value.clone(),
            revision: self.revision.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RegistryCacheValue for FaultDetectRulesCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::FaultDetectRule
    }

    async fn wait_initialize(&self, timeout: Duration) -> Box<dyn Fn() + Send> {
        build_waiter(self.initialized.clone(), timeout)
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(std::sync::atomic::Ordering::Acquire)
    }

    fn revision(&self) -> String {
        self.revision.clone()
    }
}

// ConfigGroupCacheItem 当个配置分组下已发布的文件列表信息
pub struct ConfigGroupCacheItem {
    initialized: Arc<AtomicBool>,
    pub namespace: String,
    pub group: String,
    pub files: Arc<RwLock<Vec<ConfigFile>>>,
    pub revision: String,
}

impl Default for ConfigGroupCacheItem {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigGroupCacheItem {
    pub fn new() -> Self {
        Self {
            initialized: Arc::new(AtomicBool::new(false)),
            namespace: String::new(),
            group: String::new(),
            files: Arc::new(RwLock::new(Vec::new())),
            revision: String::new(),
        }
    }

    pub fn finish_initialize(&self) {
        let _ = self.initialized.compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        );
    }
}

impl Clone for ConfigGroupCacheItem {
    fn clone(&self) -> Self {
        Self {
            initialized: self.initialized.clone(),
            namespace: self.namespace.clone(),
            group: self.group.clone(),
            files: self.files.clone(),
            revision: self.revision.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RegistryCacheValue for ConfigGroupCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::ConfigGroup
    }

    async fn wait_initialize(&self, timeout: Duration) -> Box<dyn Fn() + Send> {
        build_waiter(self.initialized.clone(), timeout)
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(std::sync::atomic::Ordering::Acquire)
    }

    fn revision(&self) -> String {
        self.revision.clone()
    }
}

// ConfigFileCacheItem 单个配置文件的最新发布信息
pub struct ConfigFileCacheItem {
    initialized: Arc<AtomicBool>,
    pub value: ClientConfigFileInfo,
    pub revision: String,
}

impl Default for ConfigFileCacheItem {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigFileCacheItem {
    pub fn new() -> Self {
        Self {
            initialized: Arc::new(AtomicBool::new(false)),
            value: ClientConfigFileInfo::default(),
            revision: String::new(),
        }
    }

    pub fn to_config_file(&self) -> ConfigFile {
        let mut labels = HashMap::<String, String>::new();
        for label in self.value.tags.iter() {
            labels.insert(label.key.clone().unwrap(), label.value.clone().unwrap());
        }

        ConfigFile {
            namespace: self.value.namespace.clone().unwrap_or_default(),
            group: self.value.group.clone().unwrap_or_default(),
            name: self.value.file_name.clone().unwrap_or_default(),
            version: self.value.version.unwrap_or_default(),
            content: self.value.content.clone().unwrap_or_default(),
            labels,
            encrypt_algo: String::new(),
            encrypt_key: String::new(),
        }
    }

    pub fn finish_initialize(&self) {
        let _ = self.initialized.compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        );
    }
}

impl Clone for ConfigFileCacheItem {
    fn clone(&self) -> Self {
        Self {
            initialized: self.initialized.clone(),
            value: self.value.clone(),
            revision: self.revision.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RegistryCacheValue for ConfigFileCacheItem {
    fn is_loaded_from_file(&self) -> bool {
        todo!()
    }

    fn event_type(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::ConfigFile
    }

    async fn wait_initialize(&self, timeout: Duration) -> Box<dyn Fn() + Send> {
        build_waiter(self.initialized.clone(), timeout)
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(std::sync::atomic::Ordering::Acquire)
    }

    fn revision(&self) -> String {
        self.revision.clone()
    }
}
