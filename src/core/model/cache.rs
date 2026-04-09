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

use std::{
    collections::HashMap,
    fmt::Display,
    sync::{atomic::AtomicBool, Arc},
    thread::sleep,
    time::Duration,
};

use tokio::sync::RwLock;

use pole_specification::v1::{
    config_discover_request::ConfigDiscoverRequestType,
    config_discover_response::ConfigDiscoverResponseType, discover_request::DiscoverRequestType,
    discover_response::DiscoverResponseType, CircuitBreakerRule, ConfigDiscoverRequest,
    ConfigDiscoverResponse, ConfigFileRelease, DiscoverFilter, DiscoverRequest, DiscoverResponse,
    FaultDetector, LaneGroup, RateLimit, Service,
};

use super::{
    config::{ConfigFile, ConfigGroup},
    naming::{Instance, ServiceInfo},
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
    ConfigGroups,
}

impl EventType {
    pub fn to_persist_file(&self) -> String {
        match self {
            EventType::Instance => "instance.data".to_string(),
            EventType::RouterRule => "router_rule.data".to_string(),
            EventType::CircuitBreakerRule => "circuit_breaker_rule.data".to_string(),
            EventType::RateLimitRule => "rate_limit_rule.data".to_string(),
            EventType::Service => "service.data".to_string(),
            EventType::FaultDetectRule => "fault_detect_rule.data".to_string(),
            EventType::ServiceContract => "service_contract.data".to_string(),
            EventType::LaneRule => "lane_rule.data".to_string(),
            EventType::Namespaces => "namespaces.data".to_string(),
            EventType::ConfigFile => "config_file.data".to_string(),
            EventType::ConfigGroup => "config_group.data".to_string(),
            _ => "unknown".to_string(),
        }
    }

    pub fn naming_spec_to_persist_file(t: DiscoverResponseType) -> String {
        match t {
            DiscoverResponseType::Instance => "instance.data".to_string(),
            DiscoverResponseType::CustomRouteRule => "router_rule.data".to_string(),
            DiscoverResponseType::CircuitBreaker => "circuit_breaker_rule.data".to_string(),
            DiscoverResponseType::RateLimit => "rate_limit_rule.data".to_string(),
            DiscoverResponseType::Services => "service.data".to_string(),
            DiscoverResponseType::FaultDetector => "fault_detect_rule.data".to_string(),
            DiscoverResponseType::Lane => "lane_rule.data".to_string(),
            DiscoverResponseType::Namespaces => "namespaces.data".to_string(),
            _ => "unknown".to_string(),
        }
    }

    pub fn config_spec_to_persist_file(t: ConfigDiscoverResponseType) -> String {
        match t {
            ConfigDiscoverResponseType::ConfigFile => "config_file.data".to_string(),
            ConfigDiscoverResponseType::ConfigFileNames => "config_files.data".to_string(),
            ConfigDiscoverResponseType::ConfigFileGroups => "config_group.data".to_string(),
            _ => "unknown".to_string(),
        }
    }
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
            EventType::ConfigGroups => "ConfigGroups".to_string(),
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

impl Display for CacheItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheItemType::Instance(_) => write!(f, "Instance"),
            CacheItemType::RouterRule(_) => write!(f, "RouterRule"),
            CacheItemType::CircuitBreakerRule(_) => write!(f, "CircuitBreakerRule"),
            CacheItemType::RateLimitRule(_) => write!(f, "RateLimitRule"),
            CacheItemType::Service(_) => write!(f, "Service"),
            CacheItemType::FaultDetectRule(_) => write!(f, "FaultDetectRule"),
            CacheItemType::LaneRule(_) => write!(f, "LaneRule"),
            CacheItemType::ConfigFile(_) => write!(f, "ConfigFile"),
            CacheItemType::ConfigGroup(_) => write!(f, "ConfigGroup"),
            _ => write!(f, "Unknown"),
        }
    }
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

    pub fn to_config_group(&self) -> Option<ConfigGroupCacheItem> {
        match self {
            CacheItemType::ConfigGroup(item) => Some(item.clone()),
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
                r#type: DiscoverRequestType::CustomRouteRule.into(),
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
                file: Some(self.to_spec_config_file()),
                filter: None,
                revision,
            }),
            crate::core::model::cache::EventType::ConfigGroup => Some(ConfigDiscoverRequest {
                r#type: ConfigDiscoverRequestType::ConfigFileNames.into(),
                file: Some(self.to_spec_config_group()),
                filter: None,
                revision,
            }),
            _ => None,
        }
    }

    pub fn to_spec_service(&self, revision: String) -> Service {
        let svc = self.filter.get("service").unwrap().to_string();
        Service {
            namespace: self.namespace.clone(),
            name: svc,
            revision: revision,
            ..Service::default()
        }
    }

    pub fn to_spec_config_file(&self) -> pole_specification::v1::ConfigFile {
        let group = self.filter.get("group").unwrap().to_string();
        let file_name = self.filter.get("file").unwrap().to_string();
        pole_specification::v1::ConfigFile {
            namespace: self.namespace.clone(),
            group: group,
            name: file_name,
            ..pole_specification::v1::ConfigFile::default()
        }
    }

    pub fn to_spec_config_group(&self) -> pole_specification::v1::ConfigFile {
        let group = self.filter.get("group").unwrap().to_string();
        pole_specification::v1::ConfigFile {
            namespace: self.namespace.clone(),
            group: group,
            ..pole_specification::v1::ConfigFile::default()
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
    pub namespace: String,
    pub value: Arc<RwLock<Vec<ServiceInfo>>>,
    pub revision: String,
}

impl Default for ServicesCacheItem {
    fn default() -> Self {
        Self::new()
    }
}

impl ServicesCacheItem {
    pub fn new() -> Self {
        Self {
            namespace: String::new(),
            initialized: Arc::new(AtomicBool::new(false)),
            value: Arc::new(RwLock::new(Vec::new())),
            revision: String::new(),
        }
    }
}

impl Clone for ServicesCacheItem {
    fn clone(&self) -> Self {
        Self {
            namespace: self.namespace.clone(),
            initialized: self.initialized.clone(),
            value: self.value.clone(),
            revision: self.revision.clone(),
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
            id,
            namespace,
            name,
            metadata: self.svc_info.metadata.clone(),
            revision: revision,
        }
    }

    /// 以最少分配替换实例列表
    pub async fn replace_instances(&self, mut new_list: Vec<Instance>) {
        let mut dst = self.value.write().await;
        if dst.capacity() >= new_list.len() {
            dst.clear();
            dst.extend(new_list.into_iter());
        } else {
            std::mem::swap(&mut *dst, &mut new_list);
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
    pub value: Arc<RwLock<Vec<pole_specification::v1::RouteRule>>>,
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

    /// 以最少分配替换路由规则
    pub async fn replace_rules(&self, mut new_rules: Vec<pole_specification::v1::RouteRule>) {
        let mut dst = self.value.write().await;
        if dst.capacity() >= new_rules.len() {
            dst.clear();
            dst.extend(new_rules.into_iter());
        } else {
            std::mem::swap(&mut *dst, &mut new_rules);
        }
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
    pub value: Arc<RwLock<Vec<pole_specification::v1::LaneGroup>>>,
    pub revision: String,
}

impl Default for LaneRulesCacheItem {
    fn default() -> Self {
        Self::new()
    }
}

impl LaneRulesCacheItem {
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

    /// 以最少分配替换路由规则
    pub async fn replace_rules(&self, mut new_rules: Vec<pole_specification::v1::LaneGroup>) {
        let mut dst = self.value.write().await;
        if dst.capacity() >= new_rules.len() {
            dst.clear();
            dst.extend(new_rules.into_iter());
        } else {
            std::mem::swap(&mut *dst, &mut new_rules);
        }
    }
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
    pub value: Arc<RwLock<Vec<pole_specification::v1::RateLimit>>>,
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

    /// 批量替换限流规则，最少分配
    pub async fn replace_rules(&self, mut new_rules: Vec<pole_specification::v1::RateLimit>) {
        let mut dst = self.value.write().await;
        if dst.capacity() >= new_rules.len() {
            dst.clear();
            dst.extend(new_rules.into_iter());
        } else {
            std::mem::swap(&mut *dst, &mut new_rules);
        }
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
    pub value: Arc<RwLock<Vec<pole_specification::v1::CircuitBreakerRule>>>,
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

    /// 批量替换熔断规则，最少分配
    pub async fn replace_rules(
        &self,
        mut new_rules: Vec<pole_specification::v1::CircuitBreakerRule>,
    ) {
        let mut dst = self.value.write().await;
        if dst.capacity() >= new_rules.len() {
            dst.clear();
            dst.extend(new_rules.into_iter());
        } else {
            std::mem::swap(&mut *dst, &mut new_rules);
        }
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
    pub value: Arc<RwLock<FaultDetector>>,
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
            value: Arc::new(RwLock::new(FaultDetector::default())),
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

    /// 覆盖主动探测规则
    pub fn set_value(&mut self, val: FaultDetector) {
        self.value = Arc::new(RwLock::new(val));
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

    pub async fn to_config_group(&self) -> ConfigGroup {
        let cache_files = self.files.read().await;
        let mut files = Vec::<ConfigFile>::with_capacity(cache_files.len());
        for item in cache_files.iter() {
            files.push(item.clone());
        }

        ConfigGroup {
            namespace: self.namespace.clone(),
            group: self.group.clone(),
            files: files,
            revision: self.revision.clone(),
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

    /// 以最少分配替换文件列表
    pub async fn replace_files(&self, mut files: Vec<ConfigFile>) {
        let mut dst = self.files.write().await;
        if dst.capacity() >= files.len() {
            dst.clear();
            dst.extend(files.into_iter());
        } else {
            std::mem::swap(&mut *dst, &mut files);
        }
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
    pub value: ConfigFileRelease,
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
            value: ConfigFileRelease::default(),
            revision: String::new(),
        }
    }

    pub fn to_config_file(&self) -> ConfigFile {
        ConfigFile {
            id: self.value.id.clone(),
            namespace: self.value.namespace.clone(),
            group: self.value.group.clone(),
            name: self.value.file_name.clone(),
            version: self.value.version,
            content: self.value.content.clone(),
            labels: self.value.labels.clone(),
            encrypt_algo: "".to_string(),
            encrypt_key: "".to_string(),
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

    /// 覆盖单个配置文件的发布信息
    pub fn set_value(&mut self, val: ConfigFileRelease) {
        self.value = val;
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
