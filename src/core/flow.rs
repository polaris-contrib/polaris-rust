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
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use dashmap::DashMap;
use pole_specification::v1::{
    limit_trigger, match_string, LimitTrigger, MatchArgument, MatchString, RateLimit,
};
use prost_types::Duration as ProtoDuration;
use tokio::{sync::RwLock, task::JoinHandle, time::sleep};

use crate::{
    core::{
        helper::helper,
        model::{
            self,
            cache::{CacheItemType, EventType, ResourceEventKey, ServerEvent},
            error::ErrorCode,
            ratelimit::{
                LimiterConfig, QuotaConfig, QuotaMode, RemoteUpdate, ResponseConfig, WindowConfig,
            },
            ArgumentType,
        },
        plugin::{
            cache::{Action, Filter, ResourceListener},
            ratelimit::ServiceRateLimiter,
            router::ServiceRouter,
        },
    },
    ratelimit::{
        connector::async_connector::{AsyncRateLimitConnector, SyncTarget},
        req::{QuotaRequest, QuotaResponse},
        sync::pole::PoleRemoteSyncTask,
    },
};

use super::{
    model::{
        circuitbreaker::{CheckResult, CircuitBreakerStatus, Resource, ResourceStat, Status},
        error::PoleError,
        naming::ServiceInstances,
        router::RouteInfo,
        ClientContext, ReportClientRequest,
    },
    plugin::{
        loadbalance::LoadBalancer, location::LocationSupplier, plugins::Extensions,
        router::RouteContext,
    },
};

pub struct ClientFlow
where
    Self: Send + Sync,
{
    client: Arc<ClientContext>,
    extensions: Arc<Extensions>,

    futures: Vec<JoinHandle<()>>,

    closed: Arc<AtomicBool>,
}

impl ClientFlow {
    pub fn new(client: Arc<ClientContext>, extensions: Arc<Extensions>) -> Self {
        ClientFlow {
            client,
            extensions: extensions,
            futures: vec![],
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn run_flow(&mut self) {
        let client = self.client.clone();
        let extensions = self.extensions.clone();
        let is_closed = self.closed.clone();

        let f: JoinHandle<()> = self.extensions.runtime.spawn(async move {
            loop {
                ClientFlow::report_client(client.clone(), extensions.clone()).await;
                sleep(Duration::from_secs(60)).await;
                if is_closed.load(Ordering::Relaxed) {
                    return;
                }
            }
        });

        self.futures.push(f);
    }

    /// report_client 上报客户端信息数据
    pub async fn report_client(client: Arc<ClientContext>, extensions: Arc<Extensions>) {
        let server_connector = extensions.get_server_connector();
        let loc_provider = extensions.get_location_provider();
        let loc = loc_provider.get_location();
        let req = ReportClientRequest {
            client_id: client.client_id.clone(),
            host: client.host.clone(),
            version: client.version.clone(),
            location: loc,
        };
        let ret = server_connector.report_client(req).await;
        if let Err(e) = ret {
            crate::error!("report client failed: {:?}", e);
        }
    }

    pub fn stop_flow(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
    }
}

/// CircuitBreakerFlow
pub struct CircuitBreakerFlow {
    extensions: Arc<Extensions>,
}

impl CircuitBreakerFlow {
    pub fn new(extensions: Arc<Extensions>) -> Self {
        CircuitBreakerFlow { extensions }
    }

    pub async fn check_resource(&self, resource: Resource) -> Result<CheckResult, PoleError> {
        let circuit_breaker_opt = self.extensions.circuit_breaker.clone();
        // 没有熔断插件，直接结束流程
        if circuit_breaker_opt.is_none() {
            return Ok(CheckResult::pass());
        }

        let circuit_breaker = circuit_breaker_opt.unwrap();
        let status = circuit_breaker.check_resource(resource).await?;
        Ok(CircuitBreakerFlow::convert_from_status(status))
    }

    pub async fn report_stat(&self, stat: ResourceStat) -> Result<(), PoleError> {
        let circuit_breaker_opt = self.extensions.circuit_breaker.clone();
        if circuit_breaker_opt.is_none() {
            return Ok(());
        }

        let circuit_breaker = circuit_breaker_opt.unwrap();
        circuit_breaker.report_stat(stat).await
    }

    fn convert_from_status(ret: CircuitBreakerStatus) -> CheckResult {
        let status = ret.status;
        CheckResult {
            pass: status != Status::Open,
            rule_name: ret.circuit_breaker,
            fallback_info: ret.fallback_info.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_status(status: Status) -> CircuitBreakerStatus {
        CircuitBreakerStatus {
            circuit_breaker: "test-breaker".to_string(),
            status,
            start_ms: 0,
            fallback_info: None,
            destroy: false,
        }
    }

    #[test]
    fn test_convert_from_status_open_should_block() {
        let ret = CircuitBreakerFlow::convert_from_status(build_status(Status::Open));
        assert!(!ret.pass);
    }

    #[test]
    fn test_convert_from_status_close_should_pass() {
        let ret = CircuitBreakerFlow::convert_from_status(build_status(Status::Close));
        assert!(ret.pass);
    }

    #[test]
    fn test_convert_from_status_half_open_should_pass() {
        let ret = CircuitBreakerFlow::convert_from_status(build_status(Status::HalfOpen));
        assert!(ret.pass);
    }
}

/// RouterFlow 路由流程
pub struct RouterFlow {
    load_balancer: Arc<RwLock<HashMap<String, Arc<Box<dyn LoadBalancer>>>>>,
    extensions: Arc<Extensions>,
}

impl RouterFlow {
    pub fn new(extensions: Arc<Extensions>) -> Self {
        RouterFlow {
            load_balancer: extensions.get_loadbalancers(),
            extensions,
        }
    }

    /// lookup_loadbalancer 查找负载均衡器
    pub async fn lookup_loadbalancer(&self, name: &str) -> Option<Arc<Box<dyn LoadBalancer>>> {
        let lb = self.load_balancer.read().await;
        lb.get(name).cloned()
    }

    pub async fn choose_instances(
        &self,
        route_info: RouteInfo,
        instances: ServiceInstances,
    ) -> Result<ServiceInstances, PoleError> {
        let router_container = self.extensions.get_router_container();

        let route_ctx = RouteContext {
            route_info,
            extensions: Some(self.extensions.clone()),
        };

        let mut routers = Vec::<Arc<Box<dyn ServiceRouter>>>::new();
        let chain = &route_ctx.route_info.chain;

        // 处理前置路由
        chain.before.iter().for_each(|name| {
            if let Some(router) = router_container.before_routers.get(name) {
                routers.push(router.clone());
            }
        });

        let mut tmp_instance = instances;
        for (_, ele) in routers.iter().enumerate() {
            let ret = ele.choose_instances(route_ctx.clone(), tmp_instance).await;
            if let Err(e) = ret {
                return Err(e);
            }
            tmp_instance = ret.unwrap().instances;
        }
        routers.clear();

        // 处理核心路由
        chain.core.iter().for_each(|name| {
            if let Some(router) = router_container.core_routers.get(name) {
                routers.push(router.clone());
            }
        });

        for (_, ele) in routers.iter().enumerate() {
            let ret = ele.choose_instances(route_ctx.clone(), tmp_instance).await;
            if let Err(e) = ret {
                return Err(e);
            }
            tmp_instance = ret.unwrap().instances;
        }
        routers.clear();

        // 处理后置路由
        chain.after.iter().for_each(|name| {
            if let Some(router) = router_container.after_routers.get(name) {
                routers.push(router.clone());
            }
        });

        for (_, ele) in routers.iter().enumerate() {
            let ret = ele.choose_instances(route_ctx.clone(), tmp_instance).await;
            if let Err(e) = ret {
                return Err(e);
            }
            tmp_instance = ret.unwrap().instances;
        }

        Ok(tmp_instance)
    }
}

/// 服务规则快照：缓存 namespace#service 对应的限流规则
#[derive(Clone)]
struct ServiceRuleSnapshot {
    rules: Arc<RwLock<Option<Arc<Vec<RateLimit>>>>>,
    revision: Arc<RwLock<String>>,
}

impl ServiceRuleSnapshot {
    fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(None)),
            revision: Arc::new(RwLock::new(String::new())),
        }
    }

    /// 更新规则快照
    async fn update(&self, rules: Vec<RateLimit>, revision: String) {
        let mut snapshot = self.rules.write().await;
        *snapshot = Some(Arc::new(rules));
        let mut rev = self.revision.write().await;
        *rev = revision;
    }

    /// 获取缓存的规则（如果没有则返回 None）
    async fn get(&self) -> Option<Arc<Vec<RateLimit>>> {
        self.rules.read().await.clone()
    }
}

/// RatelimitFlow 限流流程
pub struct RatelimitFlow {
    extensions: Arc<Extensions>,
    limiter_registry: Arc<LimiterRegistry>,
    // 规则快照缓存：namespace#service -> ServiceRuleSnapshot
    rule_snapshots: Arc<DashMap<String, ServiceRuleSnapshot>>,
    // 缓存监听器，用于监听规则变更
    cache_listener: Arc<RateFlowListener>,
    // 监听器初始化标志
    listener_initialized: Arc<AtomicBool>,
}

impl RatelimitFlow {
    pub fn new(extensions: Arc<Extensions>) -> Self {
        let limiter_registry = Arc::new(LimiterRegistry::new(extensions.clone()));
        let rule_snapshots = Arc::new(DashMap::new());
        let cache_listener = Arc::new(RateFlowListener {
            rule_snapshots: rule_snapshots.clone(),
            limiter_registry: limiter_registry.clone(),
        });

        Self {
            extensions,
            limiter_registry,
            rule_snapshots,
            cache_listener,
            listener_initialized: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 确保监听器已初始化
    async fn ensure_listener_initialized(&self) {
        if self
            .listener_initialized
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.init_listener().await;
        }
    }

    /// 初始化监听器：注册到资源缓存系统
    async fn init_listener(&self) {
        let listener: Arc<dyn ResourceListener> = self.cache_listener.clone();
        self.extensions
            .get_resource_cache()
            .register_resource_listener(listener)
            .await;
    }

    /// 检查配额，滑动窗口限流主入口
    pub async fn check_quota(&self, req: QuotaRequest) -> Result<QuotaResponse, PoleError> {
        // 首次调用时初始化监听器
        self.ensure_listener_initialized().await;

        let service_key = format!("{}#{}", req.namespace, req.service);

        // 步骤 1：尝试从快照缓存读取规则
        let rules = if let Some(snapshot) = self.rule_snapshots.get(&service_key) {
            if let Some(cached_rules) = snapshot.get().await {
                // 使用缓存的规则
                (*cached_rules).clone()
            } else {
                // 快照存在但还未初始化，从资源缓存加载并更新快照
                self.load_and_cache_rules(&service_key, &req).await?
            }
        } else {
            // 快照不存在，创建并初始化
            let snapshot = ServiceRuleSnapshot::new();
            self.rule_snapshots
                .insert(service_key.clone(), snapshot.clone());
            self.load_and_cache_rules(&service_key, &req).await?
        };

        let matched_rules = RuleMatcher::match_rules(&req, &rules)?;
        if matched_rules.is_empty() {
            return Ok(QuotaResponse {
                allowed: true,
                message: String::new(),
            });
        }

        for matched in matched_rules {
            let limiter_instance = self.limiter_registry.get_or_create(&matched).await?;
            let resp = limiter_instance.check_quota(&req).await?;
            if !resp.allowed {
                return Ok(resp);
            }
        }

        Ok(QuotaResponse {
            allowed: true,
            message: String::new(),
        })
    }

    /// 从资源缓存加载规则并更新快照
    async fn load_and_cache_rules(
        &self,
        service_key: &str,
        req: &QuotaRequest,
    ) -> Result<Vec<RateLimit>, PoleError> {
        let rules = self
            .extensions
            .get_resource_cache()
            .load_service_ratelimit(Filter {
                resource_key: ResourceEventKey {
                    namespace: req.namespace.clone(),
                    event_type: EventType::RateLimitRule,
                    filter: {
                        let mut f = HashMap::<String, String>::new();
                        f.insert("service".to_string(), req.service.clone());
                        f
                    },
                },
                internal_request: false,
                include_cache: true,
                timeout: Duration::from_secs(1),
            })
            .await;

        match rules {
            Ok(rules) => {
                // 更新快照
                if let Some(snapshot) = self.rule_snapshots.get(service_key) {
                    snapshot.update(rules.clone(), String::new()).await;
                }
                Ok(rules)
            }
            Err(err) => {
                crate::warn!("load ratelimit rules failed: {}", err);
                Ok(Vec::new())
            }
        }
    }

    /// 启动定时清理过期窗口任务（需在 tokio runtime 下调用）
    pub fn spawn_purge_task(self: Arc<Self>) {
        let registry = self.limiter_registry.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                registry.purge_expired();
            }
        });
    }

    /// 主动释放限流资源（包括分布式同步任务）
    pub fn shutdown(&self) {
        self.limiter_registry.shutdown();
    }
}

impl Drop for RatelimitFlow {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// 缓存事件监听器：监听限流规则变更
struct RateFlowListener {
    rule_snapshots: Arc<DashMap<String, ServiceRuleSnapshot>>,
    limiter_registry: Arc<LimiterRegistry>,
}

#[async_trait::async_trait]
impl ResourceListener for RateFlowListener {
    async fn on_event(&self, action: Action, val: ServerEvent) {
        let namespace = val.event_key.namespace.clone();
        let service = val.event_key.filter.get("service").map(|s| s.clone());

        match action {
            Action::Add | Action::Update => {
                // 从事件中提取新的规则列表
                if let Some(service) = service {
                    if let Some(rules) = Self::extract_rules_from_event(&val).await {
                        let service_key = format!("{}#{}", namespace, service);

                        // 更新快照
                        if let Some(snapshot) = self.rule_snapshots.get(&service_key) {
                            snapshot.update(rules.clone(), String::new()).await;
                        } else {
                            let snapshot = ServiceRuleSnapshot::new();
                            snapshot.update(rules.clone(), String::new()).await;
                            self.rule_snapshots.insert(service_key, snapshot);
                        }

                        // 通知 LimiterRegistry 更新规则（清理相关的 Limiter）
                        self.limiter_registry.on_rules_updated(&namespace, &service);
                    }
                }
            }
            Action::Delete => {
                // 删除对应的快照
                if let Some(service) = service {
                    let service_key = format!("{}#{}", namespace, service);
                    self.rule_snapshots.remove(&service_key);
                    self.limiter_registry.on_rules_deleted(&namespace, &service);
                }
            }
        }
    }

    fn watch_key(&self) -> EventType {
        EventType::RateLimitRule
    }
}

impl RateFlowListener {
    /// 从 ServerEvent 中提取规则列表
    async fn extract_rules_from_event(val: &ServerEvent) -> Option<Vec<RateLimit>> {
        if let CacheItemType::RateLimitRule(cache_item) = &val.value {
            let rules = cache_item.value.read().await;
            Some(rules.clone())
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct LimiterKey {
    namespace: String,
    service: String,
    rule_id: String,
    trigger_id: String,
    label_key: String,
}

#[derive(Clone, Debug)]
struct MatchedRule {
    key: LimiterKey,
    config: LimiterConfig,
}

struct RuleMatcher;

impl RuleMatcher {
    fn match_rules(req: &QuotaRequest, rules: &[RateLimit]) -> Result<Vec<MatchedRule>, PoleError> {
        let mut matched = Vec::new();
        let label_seacher = req.traffic_label_provider;

        for rule in rules {
            if rule.disable {
                continue;
            }

            for (trigger_index, trigger) in rule.rules.iter().enumerate() {
                if trigger.disable || trigger.amounts.is_empty() {
                    continue;
                }

                if let Some(method) = &trigger.method {
                    if !helper::match_label_value(method, Some(&req.method)) {
                        continue;
                    }
                }

                let mut arg_match = true;
                let mut req_labels = HashMap::<ArgumentType, HashMap<String, String>>::new();
                for arg in &trigger.arguments {
                    let at = arg.r#type;
                    let labels = req_labels
                        .entry(ArgumentType::parse_from_spec(at))
                        .or_insert_with(HashMap::new);

                    let traffic_val = label_seacher(ArgumentType::parse_from_spec(at), &arg.key);
                    if traffic_val.is_none() {
                        arg_match = false;
                        break;
                    }

                    labels.insert(arg.key.clone(), traffic_val.as_ref().unwrap().clone());
                    if let Some(arg_value) = &arg.value {
                        if !helper::match_label_value(arg_value, traffic_val.as_deref()) {
                            arg_match = false;
                            break;
                        }
                    }
                }

                if !arg_match {
                    continue;
                }

                let (label_key, _regex_spread) =
                    Self::format_label_key(trigger, &trigger.method, &req_labels);
                let config = Self::parse_limiter_config(trigger)?;
                let limiter_key = LimiterKey {
                    namespace: rule.namespace.clone(),
                    service: rule.service.clone(),
                    rule_id: rule.id.clone(),
                    trigger_id: trigger_index.to_string(),
                    label_key,
                };

                matched.push(MatchedRule {
                    key: limiter_key,
                    config,
                });
            }
        }

        Ok(matched)
    }

    fn format_label_key(
        trigger: &LimitTrigger,
        req_method: &Option<MatchString>,
        req_labels: &HashMap<ArgumentType, HashMap<String, String>>,
    ) -> (String, bool) {
        let method_matcher = req_method;
        let regex_combine = trigger.regex_combine;
        let mut method_value = String::new();
        let mut regex_spread = false;

        if let Some(method_matcher) = method_matcher {
            if !helper::is_match_all(&method_matcher.value) {
                if regex_combine
                    && method_matcher.r#type != match_string::MatchStringType::Exact as i32
                {
                    method_value = method_matcher.clone().value;
                } else {
                    method_value = method_matcher.clone().value;
                    if method_matcher.r#type != match_string::MatchStringType::Exact as i32 {
                        regex_spread = true;
                    }
                }
            }
        }

        let mut tmp_list = Vec::<String>::new();
        for argument_matcher in &trigger.arguments {
            let value_matcher = &argument_matcher.value;
            let label_value = match value_matcher {
                Some(v) => {
                    if regex_combine && v.r#type != match_string::MatchStringType::Exact as i32 {
                        v.value.clone()
                    } else {
                        let arg_type = ArgumentType::parse_from_spec(argument_matcher.r#type);
                        let string_map = match req_labels.get(&arg_type) {
                            Some(map) => map,
                            None => continue,
                        };
                        if v.r#type != match_string::MatchStringType::Exact as i32 {
                            regex_spread = true;
                        }
                        match string_map.get(argument_matcher.key.as_str()) {
                            Some(val) => val.clone(),
                            None => continue,
                        }
                    }
                }
                None => continue,
            };
            let label_entry = get_label_entry(argument_matcher, &label_value);
            tmp_list.push(label_entry);
        }
        tmp_list.sort();
        let sep = model::DEFAULT_MAP_KV_TUPLE_SEPARATOR;
        (
            format!("{}{}{}", method_value, sep, tmp_list.join(sep)),
            regex_spread,
        )
    }

    fn parse_limiter_config(trigger: &LimitTrigger) -> Result<LimiterConfig, PoleError> {
        let amount = trigger.amounts.first().ok_or_else(|| {
            PoleError::new(
                ErrorCode::InvalidConfig,
                "empty trigger amounts".to_string(),
            )
        })?;

        let period = amount
            .valid_duration
            .as_ref()
            .map(proto_duration_to_std)
            .unwrap_or_else(|| Duration::from_secs(1));

        let response = trigger
            .custom_response
            .as_ref()
            .map(|custom| ResponseConfig {
                message: custom.body.clone(),
                status: custom.code.parse::<i32>().ok(),
            })
            .unwrap_or_else(|| ResponseConfig {
                message: "".to_string(),
                status: None,
            });

        let quota = QuotaConfig {
            amount: amount.max_amount as i64,
            period,
        };

        let window = WindowConfig {
            window_size: period,
            expire_duration: Duration::from_secs(60),
        };

        let limiter_type = resolve_limiter_type(trigger);
        let mode = resolve_quota_mode(trigger);
        let config_hash = hash_limiter_config(&limiter_type, &quota, &window, &response, &mode);

        Ok(LimiterConfig {
            window,
            quota,
            response,
            config_hash,
            limiter_type,
            mode,
        })
    }
}

struct LimiterRegistry {
    instances: DashMap<LimiterKey, Arc<LimiterInstance>>,
    extensions: Arc<Extensions>,
    expire_threshold: Duration,
    connector: Arc<AsyncRateLimitConnector>,
}

impl LimiterRegistry {
    fn new(extensions: Arc<Extensions>) -> Self {
        Self {
            instances: DashMap::new(),
            extensions,
            expire_threshold: Duration::from_secs(120),
            connector: Arc::new(AsyncRateLimitConnector::new()),
        }
    }

    async fn get_or_create(
        &self,
        matched: &MatchedRule,
    ) -> Result<Arc<LimiterInstance>, PoleError> {
        use dashmap::mapref::entry::Entry;

        let instance = match self.instances.entry(matched.key.clone()) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let limiter = self.create_limiter(&matched.config.limiter_type)?;
                let sync_target = SyncTarget {
                    namespace: matched.key.namespace.clone(),
                    service: matched.key.service.clone(),
                    labels: matched.key.label_key.clone(),
                };
                let instance = Arc::new(LimiterInstance::new(
                    limiter,
                    matched.config.clone(),
                    self.connector.clone(),
                    sync_target,
                ));
                entry.insert(instance.clone());
                instance
            }
        };

        instance.apply_config(matched.config.clone()).await?;
        Ok(instance)
    }

    fn create_limiter(
        &self,
        limiter_type: &str,
    ) -> Result<Arc<Box<dyn ServiceRateLimiter>>, PoleError> {
        let supplier = self
            .extensions
            .get_ratelimiter_supplier(limiter_type)
            .or_else(|| self.extensions.get_ratelimiter_supplier("unirate"))
            .ok_or_else(|| {
                PoleError::new(
                    ErrorCode::InvalidConfig,
                    format!("unknown limiter type: {}", limiter_type),
                )
            })?;

        let mut limiter = supplier();
        limiter.init();
        Ok(Arc::new(limiter))
    }

    fn purge_expired(&self) {
        let now = now_epoch_secs();
        let threshold = self.expire_threshold.as_secs();
        self.instances.retain(|_, v| !v.is_expired(now, threshold));
    }

    fn shutdown(&self) {
        self.instances.clear();
    }

    /// 规则更新时清理相关的 Limiter：删除该服务相关的所有限流实例
    fn on_rules_updated(&self, namespace: &str, service: &str) {
        self.instances.retain(|key, _| {
            // 保留不属于该服务的限流实例
            !(key.namespace == namespace && key.service == service)
        });
    }

    /// 规则删除时清理相关的 Limiter
    fn on_rules_deleted(&self, namespace: &str, service: &str) {
        self.instances.retain(|key, _| {
            // 保留不属于该服务的限流实例
            !(key.namespace == namespace && key.service == service)
        });
    }
}

struct LimiterInstance {
    limiter: Arc<Box<dyn ServiceRateLimiter>>,
    config: RwLock<LimiterConfig>,
    last_access: AtomicU64,
    remote_sync_task: Arc<PoleRemoteSyncTask>,
}

impl LimiterInstance {
    fn new(
        limiter: Arc<Box<dyn ServiceRateLimiter>>,
        config: LimiterConfig,
        connector: Arc<AsyncRateLimitConnector>,
        target: SyncTarget,
    ) -> Self {
        let remote_sync_task =
            Arc::new(PoleRemoteSyncTask::new(connector, limiter.clone(), target));
        Self {
            limiter,
            config: RwLock::new(config),
            last_access: AtomicU64::new(now_epoch_secs()),
            remote_sync_task,
        }
    }

    async fn check_quota(&self, _req: &QuotaRequest) -> Result<QuotaResponse, PoleError> {
        self.touch();
        let config = self.config.read().await;
        let mode = config.mode;
        drop(config);

        match mode {
            QuotaMode::Global => self.allocate_global().await,
            QuotaMode::Local => self.allocate_local().await,
        }
    }

    async fn allocate_global(&self) -> Result<QuotaResponse, PoleError> {
        if let Err(err) = self.remote_sync_task.do_remote_init(false).await {
            crate::warn!(
                "global ratelimit init failed, fallback to local allocation: {}",
                err
            );
            return self.allocate_local().await;
        }

        if let Err(err) = self.remote_sync_task.do_remote_acquire().await {
            crate::warn!(
                "global ratelimit acquire sync failed, trying re-init once before fallback: {}",
                err
            );
            if let Err(reinit_err) = self.remote_sync_task.do_remote_init(true).await {
                crate::warn!(
                    "global ratelimit re-init failed after acquire error, fallback to local \
                     allocation: {}",
                    reinit_err
                );
            } else if let Err(retry_err) = self.remote_sync_task.do_remote_acquire().await {
                crate::warn!(
                    "global ratelimit acquire retry failed, fallback to local allocation: {}",
                    retry_err
                );
            }
        }

        self.allocate_local().await
    }

    async fn allocate_local(&self) -> Result<QuotaResponse, PoleError> {
        let ret = self.limiter.allocate_quota().await;
        match ret {
            Ok(allowed) => {
                let config = self.config.read().await;
                let message = if allowed {
                    "".to_string()
                } else {
                    config.response.message.clone()
                };
                Ok(QuotaResponse { allowed, message })
            }
            Err(err) => Err(PoleError::new(
                ErrorCode::InternalError,
                format!("allocate quota failed: {}", err),
            )),
        }
    }

    async fn apply_config(&self, new_config: LimiterConfig) -> Result<(), PoleError> {
        let mut config = self.config.write().await;
        if config.config_hash == new_config.config_hash {
            return Ok(());
        }
        *config = new_config.clone();
        self.limiter
            .on_remote_update(RemoteUpdate::ConfigUpdate(&new_config))
            .await
    }

    fn is_expired(&self, now: u64, expire_threshold: u64) -> bool {
        let last_access = self.last_access.load(Ordering::Relaxed);
        now.saturating_sub(last_access) > expire_threshold
    }

    fn touch(&self) {
        self.last_access.store(now_epoch_secs(), Ordering::Relaxed);
    }
}

impl Drop for LimiterInstance {
    fn drop(&mut self) {
        self.remote_sync_task.shutdown();
    }
}

fn resolve_limiter_type(trigger: &LimitTrigger) -> String {
    if !trigger.action.is_empty() {
        return trigger.action.clone();
    }

    match limit_trigger::Resource::try_from(trigger.resource).ok() {
        Some(limit_trigger::Resource::Concurrency) => "concurrency".to_string(),
        Some(limit_trigger::Resource::Qps) => "unirate".to_string(),
        _ => "reject".to_string(),
    }
}

fn resolve_quota_mode(trigger: &LimitTrigger) -> QuotaMode {
    // amount_mode=0 (GlobalTotal) is proto default. To avoid accidental global
    // enablement in old configs, only ShareEqually (1) is treated as Global.
    match limit_trigger::AmountMode::try_from(trigger.amount_mode).ok() {
        Some(limit_trigger::AmountMode::ShareEqually) => QuotaMode::Global,
        _ => QuotaMode::Local,
    }
}

fn hash_limiter_config(
    limiter_type: &str,
    quota: &QuotaConfig,
    window: &WindowConfig,
    response: &ResponseConfig,
    mode: &QuotaMode,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    limiter_type.hash(&mut hasher);
    quota.amount.hash(&mut hasher);
    quota.period.as_secs().hash(&mut hasher);
    window.window_size.as_secs().hash(&mut hasher);
    window.expire_duration.as_secs().hash(&mut hasher);
    response.message.hash(&mut hasher);
    response.status.hash(&mut hasher);
    mode.hash(&mut hasher);
    hasher.finish()
}

fn proto_duration_to_std(d: &ProtoDuration) -> Duration {
    let secs = if d.seconds < 0 { 0 } else { d.seconds as u64 };
    let nanos = if d.nanos < 0 { 0 } else { d.nanos as u32 };
    Duration::new(secs, nanos)
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

fn get_label_entry(match_argument: &MatchArgument, label_value: &str) -> String {
    let arg_type = ArgumentType::parse_from_spec(match_argument.r#type);
    match arg_type {
        ArgumentType::Custom
        | ArgumentType::Header
        | ArgumentType::Query
        | ArgumentType::CallerService => {
            format!(
                "{}{}{}{}{}",
                arg_type.to_string(),
                model::DEFAULT_MAP_KV_TUPLE_SEPARATOR,
                match_argument.key,
                model::DEFAULT_MAP_KV_TUPLE_SEPARATOR,
                label_value
            )
        }
        ArgumentType::Method | ArgumentType::CallerIP => {
            format!(
                "{}{}{}",
                arg_type.to_string(),
                model::DEFAULT_MAP_KV_TUPLE_SEPARATOR,
                label_value
            )
        }
        _ => String::new(),
    }
}
