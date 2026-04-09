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

use pole_specification::v1::{CircuitBreakerRule, FaultDetector, LaneGroup, RateLimit, RouteRule};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;

use super::failover::DiskCacheFailover;
use crate::core::model::cache::{
    CacheItemType, CircuitBreakerRulesCacheItem, ConfigFileCacheItem, ConfigGroupCacheItem,
    EventType, FaultDetectRulesCacheItem, LaneRulesCacheItem, RatelimitRulesCacheItem,
    RegistryCacheValue, RemoteData, ResourceEventKey, RouterRulesCacheItem, ServerEvent,
    ServiceInstancesCacheItem, ServicesCacheItem,
};
use crate::core::model::config::{ConfigFile, ConfigGroup};
use crate::core::model::error::{ErrorCode, PoleError};
use crate::core::model::naming::{Instance, Services};
use crate::core::plugin::cache::{
    Action, Filter, InitResourceCacheOption, ResourceCache, ResourceCacheFailover, ResourceListener,
};
use crate::core::plugin::connector::{Connector, ResourceHandler};
use crate::core::plugin::plugins::Plugin;
use crate::{error, info};
use std::collections::HashMap;
use std::sync::Arc;

static MEMORY_CACHE_NAME: &str = "memory";

struct MemoryResourceHandler {
    failover: Option<Arc<dyn ResourceCacheFailover>>,
    // 资源类型变化监听
    listeners: Arc<RwLock<HashMap<EventType, Vec<Arc<dyn ResourceListener>>>>>,
    // services 服务列表缓存
    services: Arc<RwLock<HashMap<String, ServicesCacheItem>>>,
    // instances 服务实例缓存 key: namespace#service
    instances: Arc<RwLock<HashMap<String, ServiceInstancesCacheItem>>>,
    // router_rules 路由规则缓存 key: namespace#service
    router_rules: Arc<RwLock<HashMap<String, RouterRulesCacheItem>>>,
    // lane_rules 灰度规则缓存 key: namespace#service
    lane_rules: Arc<RwLock<HashMap<String, LaneRulesCacheItem>>>,
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

pub struct MemoryCache {
    opt: InitResourceCacheOption,
    // 资源连接器
    server_connector: Arc<Box<dyn Connector>>,
    handler: Arc<MemoryResourceHandler>,
    remote_sender: UnboundedSender<RemoteData>,
    //
    failover: Option<Arc<dyn ResourceCacheFailover>>,
}

impl MemoryCache {
    pub fn builder() -> (
        fn(InitResourceCacheOption) -> Box<dyn ResourceCache>,
        String,
    ) {
        (new_resource_cache, MEMORY_CACHE_NAME.to_string())
    }

    async fn run_remote_data_recive(
        handler: Arc<MemoryResourceHandler>,
        remote_reciver: &mut UnboundedReceiver<RemoteData>,
    ) {
        loop {
            tokio::select! {
                remote_data = remote_reciver.recv() => {
                    if let Some(remote_data) = remote_data {
                        MemoryCache::on_spec_event(handler.clone(), remote_data).await;
                    }
                }
            }
        }
    }

    fn submit_resource_watch(&self, event_type: EventType, resource_key: ResourceEventKey) {
        let search_namespace = resource_key.namespace.clone();
        info!(
            "[pole][resource_cache][memory] load remote resource: {:?}",
            resource_key
        );
        let server_connector = self.server_connector.clone();
        let remote_reciver = self.remote_sender.clone();
        // 提交数据异步同步任务
        self.opt.runtime.spawn(async move {
            let register_ret = server_connector
                .register_resource_handler(Box::new(MemoryResourceWatcher::new(
                    remote_reciver,
                    ResourceEventKey {
                        namespace: search_namespace,
                        event_type,
                        filter: resource_key.filter,
                    },
                )))
                .await;
            if register_ret.is_err() {
                error!(
                    "[pole][resource_cache][memory] register resource handler failed: {}, err: {}",
                    resource_key.namespace.clone(),
                    register_ret.err().unwrap()
                );
            }
        });
    }

    async fn on_spec_event(handler: Arc<MemoryResourceHandler>, event: RemoteData) {
        info!("[pole][resource_cache][memory] on spec event: {:?}", event);
        let mut notify_event = ServerEvent {
            event_key: event.event_key.clone(),
            value: CacheItemType::Unknown,
        };

        let event_key = event.event_key.clone();
        let event_type = event_key.event_type;
        let filter = event_key.filter;

        let copy_event = event.clone();

        match event_type {
            EventType::Service => {}
            EventType::Instance => {
                let remote_val = event.discover_value.unwrap();
                let svc = remote_val.service.unwrap();

                let key = format!("{}#{}", &svc.namespace, &svc.name);
                let mut safe_map = handler.instances.write().await;
                let cache_val_opt = safe_map.get_mut(key.as_str());
                if cache_val_opt.is_none() {
                    error!(
                        "[pole][resource_cache][memory] service_instance cache not found: namespace={} service={}",
                        svc.namespace, svc.name
                    );
                    return;
                }
                let cache_val = cache_val_opt.unwrap();

                // 批量转换后一次性替换，减少分配与锁持有
                let mut new_list = Vec::with_capacity(remote_val.instances.len());
                for inst in remote_val.instances.into_iter() {
                    new_list.push(Instance::convert_from_spec(inst));
                }
                cache_val.replace_instances(new_list).await;

                cache_val.revision = svc.revision;
                cache_val.finish_initialize();
                notify_event.value = CacheItemType::Instance(cache_val.clone());
            }
            EventType::RouterRule => {
                let remote_val = event.discover_value.unwrap();
                let svc = remote_val.service.unwrap();

                let key = format!("{}#{}", &svc.namespace, &svc.name);
                let mut safe_map = handler.router_rules.write().await;
                let cache_val_opt = safe_map.get_mut(key.as_str());
                if cache_val_opt.is_none() {
                    error!(
                        "[pole][resource_cache][memory] router_rule cache not found: namespace={} service={}",
                        svc.namespace, svc.name
                    );
                    return;
                }
                let cache_val = cache_val_opt.unwrap();

                cache_val.replace_rules(remote_val.custom_route_rules).await;

                cache_val.revision = svc.revision;
                cache_val.finish_initialize();
                notify_event.value = CacheItemType::RouterRule(cache_val.clone());
            }
            EventType::CircuitBreakerRule => {
                let remote_val = event.discover_value.unwrap();
                let svc = remote_val.service.unwrap();

                let key = format!("{}#{}", &svc.namespace, &svc.name);
                let mut safe_map = handler.circuitbreaker_rules.write().await;
                let cache_val_opt = safe_map.get_mut(key.as_str());
                if cache_val_opt.is_none() {
                    error!(
                        "[pole][resource_cache][memory] circuit_breaker cache not found: namespace={} service={}",
                        svc.namespace, svc.name
                    );
                    return;
                }
                let cache_val = cache_val_opt.unwrap();

                // 响应里为单条规则
                cache_val.replace_rules(remote_val.circuit_breaker).await;

                cache_val.revision = svc.revision;
                cache_val.finish_initialize();
                notify_event.value = CacheItemType::CircuitBreakerRule(cache_val.clone());
            }
            EventType::RateLimitRule => {
                let remote_val = event.discover_value.unwrap();
                let svc = remote_val.service.unwrap();

                let key = format!("{}#{}", &svc.namespace, &svc.name);
                let mut safe_map = handler.ratelimit_rules.write().await;
                let cache_val_opt = safe_map.get_mut(key.as_str());
                if cache_val_opt.is_none() {
                    error!(
                        "[pole][resource_cache][memory] ratelimit cache not found: namespace={} service={}",
                        svc.namespace, svc.name
                    );
                    return;
                }
                let cache_val = cache_val_opt.unwrap();

                // 响应里为单条规则
                cache_val.replace_rules(remote_val.rate_limit).await;

                cache_val.revision = svc.revision;
                cache_val.finish_initialize();
                notify_event.value = CacheItemType::RateLimitRule(cache_val.clone());
            }
            EventType::FaultDetectRule => {
                let remote_val = event.discover_value.unwrap();
                let svc = remote_val.service.unwrap();

                let key = format!("{}#{}", &svc.namespace, &svc.name);
                let mut safe_map = handler.faultdetect_rules.write().await;
                let cache_val_opt = safe_map.get_mut(key.as_str());
                if cache_val_opt.is_none() {
                    error!(
                        "[pole][resource_cache][memory] fault_detect cache not found: namespace={} service={}",
                        svc.namespace, svc.name
                    );
                    return;
                }
                let cache_val = cache_val_opt.unwrap();

                cache_val.set_value(remote_val.fault_detector.unwrap_or_default());

                cache_val.revision = svc.revision;
                cache_val.finish_initialize();
                notify_event.value = CacheItemType::FaultDetectRule(cache_val.clone());
            }
            EventType::ConfigFile => {
                let group = filter.get("group").unwrap();
                let file = filter.get("file").unwrap();
                let search_key = format!("{}#{}#{}", &event_key.namespace, group, file);

                let mut safe_map = handler.config_files.write().await;
                let cache_val_opt = safe_map.get_mut(search_key.as_str());
                if cache_val_opt.is_none() {
                    error!(
                        "[pole][resource_cache][memory] config_file cache not found: namespace={} group={} file={}",
                        event_key.namespace, group, file
                    );
                    return;
                }
                let cache_val = cache_val_opt.unwrap();

                let remote_file = event.config_value.unwrap().file.unwrap_or_default();
                cache_val.set_value(remote_file.clone());

                cache_val.revision = remote_file.version.to_string();
                cache_val.finish_initialize();
                notify_event.value = CacheItemType::ConfigFile(cache_val.clone());
            }
            EventType::ConfigGroup => {
                let remote_val = event.config_value.unwrap();
                let group = filter.get("group").unwrap();
                let search_key = format!("{}#{}", &event_key.namespace, group);

                let mut safe_map = handler.config_groups.write().await;
                let cache_val_opt = safe_map.get_mut(search_key.as_str());
                if cache_val_opt.is_none() {
                    error!(
                        "[pole][resource_cache][memory] config_group cache not found: namespace={} group={}",
                        event_key.namespace, group
                    );
                    return;
                }
                let cache_val = cache_val_opt.unwrap();

                let mut files = Vec::with_capacity(remote_val.file_names.len());
                for ele in remote_val.file_names {
                    files.push(ConfigFile::convert_from_spec(ele));
                }
                cache_val.replace_files(files).await;

                cache_val.revision = remote_val.revision;
                cache_val.finish_initialize();
                notify_event.value = CacheItemType::ConfigGroup(cache_val.clone());
            }
            _ => {}
        }

        // 容灾调用（保持原逻辑占位）
        if copy_event.discover_value.is_some() {
            let _ = handler;
        }
        if copy_event.config_value.is_some() {
            let _ = handler;
        }

        // 通知所有的 listener：仅克隆当前事件类型的监听列表
        let expect_watcher = {
            let listeners = handler.listeners.read().await;
            listeners.get(&event_type).cloned()
        };
        if let Some(expect_watcher) = expect_watcher {
            for (_index, listener) in expect_watcher.iter().enumerate() {
                listener
                    .on_event(Action::Update, notify_event.clone())
                    .await;
            }
        }
    }
}

fn new_resource_cache(opt: InitResourceCacheOption) -> Box<dyn ResourceCache> {
    let (sx, mut rx) = mpsc::unbounded_channel::<RemoteData>();
    let server_connector = opt.server_connector.clone();
    let failover = Arc::new(DiskCacheFailover::new(opt.conf.clone()));

    let mc = MemoryCache {
        opt,
        server_connector,
        handler: Arc::new(MemoryResourceHandler {
            failover: Some(failover.clone()),
            listeners: Arc::new(RwLock::new(HashMap::new())),
            services: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
            router_rules: Arc::new(RwLock::new(HashMap::new())),
            lane_rules: Arc::new(RwLock::new(HashMap::new())),
            ratelimit_rules: Arc::new(RwLock::new(HashMap::new())),
            circuitbreaker_rules: Arc::new(RwLock::new(HashMap::new())),
            faultdetect_rules: Arc::new(RwLock::new(HashMap::new())),
            config_groups: Arc::new(RwLock::new(HashMap::new())),
            config_files: Arc::new(RwLock::new(HashMap::new())),
        }),
        remote_sender: sx,
        // 默认使用磁盘容灾
        failover: Some(failover),
    };

    let handler = mc.handler.clone();
    mc.opt.runtime.spawn(async move {
        MemoryCache::run_remote_data_recive(handler, &mut rx).await;
    });

    Box::new(mc) as Box<dyn ResourceCache + 'static>
}

impl Plugin for MemoryCache {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        MEMORY_CACHE_NAME.to_string()
    }
}

#[async_trait::async_trait]
impl ResourceCache for MemoryCache {
    fn set_failover_provider(&mut self, failover: Arc<dyn ResourceCacheFailover>) {
        self.failover = Some(failover);
    }

    async fn load_service_custom_route(&self, filter: Filter) -> Result<Vec<RouteRule>, PoleError> {
        let search_namespace = filter.resource_key.namespace.clone();
        let search_service = filter.resource_key.filter.get("service").unwrap();
        let search_key = format!("{}#{}", search_namespace.clone(), search_service);

        // 等待资源
        {
            let resource_key = filter.resource_key.clone();
            let mut safe_map = self.handler.router_rules.write().await;
            let _ = safe_map.entry(search_key.clone()).or_insert_with(|| {
                self.submit_resource_watch(EventType::RouterRule, resource_key);
                RouterRulesCacheItem::new()
            });
        }

        // 这里进行无锁等待资源的加载完成
        let waiter = {
            let safe_map = self.handler.router_rules.read().await;
            let cache_val = safe_map.get(&search_key).unwrap();

            cache_val.wait_initialize(filter.timeout).await
        };
        waiter();

        let safe_map = self.handler.router_rules.read().await;
        let cache_val = safe_map.get(&search_key).unwrap();
        // 如果还是没有初始化
        if !cache_val.is_initialized() {
            return Err(PoleError::new(
                ErrorCode::InternalError,
                "load remote resource timeout".to_string(),
            ));
        }

        let cache_rules = cache_val.value.read().await;
        Ok(cache_rules.clone())
    }

    async fn load_service_lane_route(&self, filter: Filter) -> Result<Vec<LaneGroup>, PoleError> {
        let search_namespace = filter.resource_key.namespace.clone();
        let search_service = filter.resource_key.filter.get("service").unwrap();
        let search_key = format!("{}#{}", search_namespace.clone(), search_service);

        // 等待资源
        {
            let resource_key = filter.resource_key.clone();
            let mut safe_map = self.handler.lane_rules.write().await;
            let _ = safe_map.entry(search_key.clone()).or_insert_with(|| {
                self.submit_resource_watch(EventType::LaneRule, resource_key);
                LaneRulesCacheItem::new()
            });
        }

        // 这里进行无锁等待资源的加载完成
        let waiter = {
            let safe_map = self.handler.lane_rules.read().await;
            let cache_val = safe_map.get(&search_key).unwrap();

            cache_val.wait_initialize(filter.timeout).await
        };
        waiter();

        let safe_map = self.handler.lane_rules.read().await;
        let cache_val = safe_map.get(&search_key).unwrap();
        // 如果还是没有初始化
        if !cache_val.is_initialized() {
            return Err(PoleError::new(
                ErrorCode::InternalError,
                "load remote resource timeout".to_string(),
            ));
        }

        let cache_rules = cache_val.value.read().await;
        Ok(cache_rules.clone())
    }

    async fn load_service_ratelimit(&self, filter: Filter) -> Result<Vec<RateLimit>, PoleError> {
        let search_namespace = filter.resource_key.namespace.clone();
        let search_service = filter.resource_key.filter.get("service").unwrap();
        let search_key = format!("{}#{}", search_namespace.clone(), search_service);
        // 等待资源
        {
            let resource_key = filter.resource_key.clone();
            let mut safe_map = self.handler.ratelimit_rules.write().await;
            let _ = safe_map.entry(search_key.clone()).or_insert_with(|| {
                self.submit_resource_watch(EventType::RateLimitRule, resource_key);
                RatelimitRulesCacheItem::new()
            });
        }

        // 这里进行无锁等待资源的加载完成
        let waiter = {
            let safe_map = self.handler.ratelimit_rules.read().await;
            let cache_val = safe_map.get(&search_key).unwrap();

            cache_val.wait_initialize(filter.timeout).await
        };
        waiter();

        let safe_map = self.handler.ratelimit_rules.read().await;
        let cache_val = safe_map.get(&search_key).unwrap();
        // 如果还是没有初始化
        if !cache_val.is_initialized() {
            return Err(PoleError::new(
                ErrorCode::InternalError,
                "load remote resource timeout".to_string(),
            ));
        }

        let cache_rules = cache_val.value.read().await;
        Ok(cache_rules.clone())
    }

    async fn load_service_circuitbreaker(
        &self,
        filter: Filter,
    ) -> Result<Vec<CircuitBreakerRule>, PoleError> {
        let search_namespace = filter.resource_key.namespace.clone();
        let search_service = filter.resource_key.filter.get("service").unwrap();
        let search_key = format!("{}#{}", search_namespace.clone(), search_service);
        // 等待资源
        {
            let resource_key = filter.resource_key.clone();
            let mut safe_map = self.handler.circuitbreaker_rules.write().await;
            let _ = safe_map.entry(search_key.clone()).or_insert_with(|| {
                self.submit_resource_watch(EventType::CircuitBreakerRule, resource_key);
                CircuitBreakerRulesCacheItem::new()
            });
        }

        // 这里进行无锁等待资源的加载完成
        let waiter = {
            let safe_map = self.handler.circuitbreaker_rules.read().await;
            let cache_val = safe_map.get(&search_key).unwrap();

            cache_val.wait_initialize(filter.timeout).await
        };
        waiter();

        let safe_map = self.handler.circuitbreaker_rules.read().await;
        let cache_val = safe_map.get(&search_key).unwrap();
        // 如果还是没有初始化
        if !cache_val.is_initialized() {
            return Err(PoleError::new(
                ErrorCode::InternalError,
                "load remote resource timeout".to_string(),
            ));
        }

        let cache_rules = cache_val.value.read().await;
        Ok(cache_rules.clone())
    }

    async fn load_service_faultdetect(&self, filter: Filter) -> Result<FaultDetector, PoleError> {
        let search_namespace = filter.resource_key.namespace.clone();
        let search_service = filter.resource_key.filter.get("service").unwrap();
        let search_key = format!("{}#{}", search_namespace.clone(), search_service);
        // 等待资源
        {
            let resource_key = filter.resource_key.clone();
            let mut safe_map = self.handler.faultdetect_rules.write().await;
            let _ = safe_map.entry(search_key.clone()).or_insert_with(|| {
                self.submit_resource_watch(EventType::FaultDetectRule, resource_key);
                FaultDetectRulesCacheItem::new()
            });
        }

        // 这里进行无锁等待资源的加载完成
        let waiter = {
            let safe_map = self.handler.faultdetect_rules.read().await;
            let cache_val = safe_map.get(&search_key).unwrap();

            cache_val.wait_initialize(filter.timeout).await
        };
        waiter();

        let safe_map = self.handler.faultdetect_rules.read().await;
        let cache_val = safe_map.get(&search_key).unwrap();
        // 如果还是没有初始化
        if !cache_val.is_initialized() {
            return Err(PoleError::new(
                ErrorCode::InternalError,
                "load remote resource timeout".to_string(),
            ));
        }

        let cache_rules = cache_val.value.read().await;
        Ok(cache_rules.clone())
    }

    async fn load_services(&self, filter: Filter) -> Result<Services, PoleError> {
        let search_key = filter.resource_key.namespace.clone();
        {
            let resource_key = filter.resource_key.clone();
            let mut safe_map = self.handler.services.write().await;
            let _ = safe_map.entry(search_key.clone()).or_insert_with(|| {
                self.submit_resource_watch(EventType::Service, resource_key);
                ServicesCacheItem::new()
            });
        }

        // 这里进行无锁等待资源的加载完成
        let waiter = {
            let safe_map = self.handler.services.read().await;
            let cache_val = safe_map.get(&search_key).unwrap();

            cache_val.wait_initialize(filter.timeout).await
        };
        waiter();

        let safe_map = self.handler.services.read().await;
        let cache_val = safe_map.get(&search_key).unwrap();
        // 如果还是没有初始化
        if !cache_val.is_initialized() {
            return Err(PoleError::new(
                ErrorCode::InternalError,
                "load remote resource timeout".to_string(),
            ));
        }

        let mut services = vec![];
        services.clone_from_slice(cache_val.value.read().await.as_slice());
        Ok(Services {
            service_list: services,
            initialized: cache_val.is_initialized(),
            revision: cache_val.revision(),
        })
    }

    async fn load_service_instances(
        &self,
        filter: Filter,
    ) -> Result<ServiceInstancesCacheItem, PoleError> {
        let search_namespace = filter.resource_key.namespace.clone();
        let search_service = filter.resource_key.filter.get("service").unwrap();
        let search_key = format!("{}#{}", search_namespace.clone(), search_service);
        {
            let resource_key = filter.resource_key.clone();
            let mut safe_map = self.handler.instances.write().await;
            let _ = safe_map.entry(search_key.clone()).or_insert_with(|| {
                self.submit_resource_watch(EventType::Instance, resource_key);
                ServiceInstancesCacheItem::new()
            });
        }

        // 这里进行无锁等待资源的加载完成
        let waiter = {
            let safe_map = self.handler.instances.read().await;
            let cache_val = safe_map.get(&search_key).unwrap();

            cache_val.wait_initialize(filter.timeout).await
        };
        waiter();

        let safe_map = self.handler.instances.read().await;
        let cache_val = safe_map.get(&search_key).unwrap();
        // 等待资源，这里直接 clone 出来一个对象做数据拷贝，避免 read 锁长期持有
        // 如果还是没有初始化
        if !cache_val.is_initialized() {
            return Err(PoleError::new(
                ErrorCode::InternalError,
                "load remote resource timeout".to_string(),
            ));
        }

        Ok(cache_val.clone())
    }

    async fn load_config_file(&self, filter: Filter) -> Result<ConfigFile, PoleError> {
        let search_namespace = filter.resource_key.namespace.clone();
        let search_group = filter.resource_key.filter.get("group").unwrap();
        let search_file = filter.resource_key.filter.get("file").unwrap();
        let search_key = format!(
            "{}#{}#{}",
            search_namespace.clone(),
            search_group,
            search_file
        );
        {
            let resource_key = filter.resource_key.clone();
            let mut safe_map = self.handler.config_files.write().await;
            let _ = safe_map.entry(search_key.clone()).or_insert_with(|| {
                self.submit_resource_watch(EventType::ConfigFile, resource_key);
                ConfigFileCacheItem::new()
            });
        }

        // 这里进行无锁等待资源的加载完成
        let waiter = {
            let safe_map = self.handler.config_files.read().await;
            let cache_val = safe_map.get(&search_key).unwrap();

            cache_val.wait_initialize(filter.timeout).await
        };
        waiter();
        let safe_map = self.handler.config_files.read().await;
        let cache_val = safe_map.get(&search_key).unwrap();
        // 如果还是没有初始化
        if !cache_val.is_initialized() {
            return Err(PoleError::new(
                ErrorCode::InternalError,
                "load remote resource timeout".to_string(),
            ));
        }

        let spec_conf = cache_val.value.clone();

        Ok(ConfigFile {
            id: spec_conf.id.clone(),
            namespace: spec_conf.namespace.clone(),
            group: spec_conf.group.clone(),
            name: spec_conf.name.clone(),
            version: spec_conf.version,
            content: spec_conf.content.clone(),
            labels: spec_conf.labels.clone(),
            encrypt_algo: "".to_string(),
            encrypt_key: "".to_string(),
        })
    }

    async fn load_config_group_files(&self, filter: Filter) -> Result<ConfigGroup, PoleError> {
        let search_namespace = filter.resource_key.namespace.clone();
        let search_group = filter.resource_key.filter.get("group").unwrap();
        let search_key = format!("{}#{}", search_namespace.clone(), search_group);
        {
            let resource_key = filter.resource_key.clone();
            let mut safe_map = self.handler.config_groups.write().await;
            let _ = safe_map.entry(search_key.clone()).or_insert_with(|| {
                self.submit_resource_watch(EventType::ConfigGroup, resource_key);
                ConfigGroupCacheItem::new()
            });
        }

        // 这里进行无锁等待资源的加载完成
        let waiter = {
            let safe_map = self.handler.config_groups.read().await;
            let cache_val = safe_map.get(&search_key).unwrap();

            cache_val.wait_initialize(filter.timeout).await
        };
        waiter();

        let safe_map = self.handler.config_groups.read().await;
        let cache_val = safe_map.get(&search_key).unwrap();
        // 如果还是没有初始化
        if !cache_val.is_initialized() {
            return Err(PoleError::new(
                ErrorCode::InternalError,
                "load remote resource timeout".to_string(),
            ));
        }
        let ret = ConfigGroup {
            namespace: cache_val.namespace.clone(),
            group: cache_val.group.clone(),
            files: cache_val.files.read().await.clone(),
            revision: cache_val.revision.clone(),
        };

        Ok(ret)
    }

    async fn register_resource_listener(&self, listener: Arc<dyn ResourceListener>) {
        let watch_key = listener.watch_key();
        let mut safe_map = self.handler.listeners.write().await;
        let listeners = safe_map.entry(watch_key).or_insert_with(Vec::new);

        listeners.push(listener);
    }
}

/// MemoryResourceWatcher 用于监听远程资源变化
struct MemoryResourceWatcher {
    event_key: ResourceEventKey,
    processor: UnboundedSender<RemoteData>,
}

impl MemoryResourceWatcher {
    fn new(f: UnboundedSender<RemoteData>, event_key: ResourceEventKey) -> Self {
        Self {
            event_key,
            processor: f,
        }
    }
}

impl ResourceHandler for MemoryResourceWatcher {
    fn handle_event(&self, event: crate::core::model::cache::RemoteData) {
        match self.processor.send(event) {
            Ok(_) => {}
            Err(err) => {
                error!(
                    "[pole][resource_cache][memory] send event to processor failed: {}",
                    err
                );
            }
        }
    }

    fn interest_resource(&self) -> ResourceEventKey {
        self.event_key.clone()
    }
}
