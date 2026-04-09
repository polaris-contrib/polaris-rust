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
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use dashmap::DashMap;
use parking_lot::Mutex;
use pole_specification::v1::{
    match_string, trigger_condition, CircuitBreakerRule, ErrorCondition, FallbackConfig,
    TriggerCondition,
};

use crate::core::{
    helper::helper,
    model::{
        cache::{EventType, ResourceEventKey},
        circuitbreaker::{
            CircuitBreakerStatus, CustomResource, FallbackInfo, MethodResource, Resource,
            ResourceStat, RetStatus, ServiceResource, Status,
        },
        error::PoleError,
        instance_stat::{CircuitStatus, InstanceStatManager},
    },
    plugin::{
        cache::{Filter, ResourceCache},
        circuitbreaker::CircuitBreaker,
        plugins::Plugin,
    },
};

static PLUGIN_NAME: &str = "composite";

fn new_circuir_breaker() -> Box<dyn CircuitBreaker> {
    Box::new(CompositeCircuitBreaker::new())
}

struct RuleRuntimeState {
    status: Status,
    open_start_ms: u64,
    half_open_inflight: bool,
    half_open_success: u32,
    consecutive_error: u32,
    samples: VecDeque<(u64, bool)>,
}

impl Default for RuleRuntimeState {
    fn default() -> Self {
        Self {
            status: Status::Close,
            open_start_ms: 0,
            half_open_inflight: false,
            half_open_success: 0,
            consecutive_error: 0,
            samples: VecDeque::new(),
        }
    }
}

impl RuleRuntimeState {
    fn on_check(&mut self, rule: &CircuitBreakerRule, now_ms: u64) -> (bool, Status) {
        match self.status {
            Status::Close => (true, Status::Close),
            Status::Destroy => (false, Status::Destroy),
            Status::Open => {
                let sleep_window_secs = rule
                    .recover_condition
                    .as_ref()
                    .map(|cond| cond.sleep_window)
                    .filter(|v| *v > 0)
                    .unwrap_or(30) as u64;
                if now_ms.saturating_sub(self.open_start_ms) >= sleep_window_secs * 1000 {
                    self.status = Status::HalfOpen;
                    self.half_open_inflight = false;
                    self.half_open_success = 0;
                    return self.on_check(rule, now_ms);
                }
                (false, Status::Open)
            }
            Status::HalfOpen => {
                if self.half_open_inflight {
                    // 半开只放行一个探测请求；其余继续阻断
                    return (false, Status::Open);
                }
                self.half_open_inflight = true;
                (true, Status::HalfOpen)
            }
        }
    }

    fn record_stat(&mut self, rule: &CircuitBreakerRule, is_error: bool, now_ms: u64) {
        let max_interval_secs = max_trigger_interval_secs(rule).max(1) as u64;
        let keep_from = now_ms.saturating_sub(max_interval_secs * 1000);

        while let Some((ts, _)) = self.samples.front() {
            if *ts < keep_from {
                self.samples.pop_front();
            } else {
                break;
            }
        }
        self.samples.push_back((now_ms, is_error));

        if is_error {
            self.consecutive_error += 1;
        } else {
            self.consecutive_error = 0;
        }

        match self.status {
            Status::Open | Status::Destroy => {}
            Status::HalfOpen => {
                self.half_open_inflight = false;
                if is_error {
                    self.open(now_ms);
                    return;
                }

                self.half_open_success += 1;
                let need_success = rule
                    .recover_condition
                    .as_ref()
                    .map(|v| v.consecutive_success)
                    .filter(|v| *v > 0)
                    .unwrap_or(1);
                if self.half_open_success >= need_success {
                    self.close();
                }
            }
            Status::Close => {
                if is_triggered(rule, self, now_ms) {
                    self.open(now_ms);
                }
            }
        }
    }

    fn open(&mut self, now_ms: u64) {
        self.status = Status::Open;
        self.open_start_ms = now_ms;
        self.half_open_inflight = false;
        self.half_open_success = 0;
    }

    fn close(&mut self) {
        self.status = Status::Close;
        self.half_open_inflight = false;
        self.half_open_success = 0;
        self.consecutive_error = 0;
        self.samples.clear();
    }
}

pub struct CompositeCircuitBreaker {
    cache: Option<Arc<Box<dyn ResourceCache>>>,
    states: Arc<DashMap<String, Arc<Mutex<RuleRuntimeState>>>>,
    stat_manager: Option<Arc<InstanceStatManager>>,
}

impl CompositeCircuitBreaker {
    pub fn new() -> Self {
        Self {
            cache: None,
            states: Arc::new(DashMap::new()),
            stat_manager: None,
        }
    }

    pub fn builder() -> (fn() -> Box<dyn CircuitBreaker>, String) {
        (new_circuir_breaker, PLUGIN_NAME.to_string())
    }

    fn get_state(&self, key: &str) -> Arc<Mutex<RuleRuntimeState>> {
        self.states
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(RuleRuntimeState::default())))
            .clone()
    }

    async fn load_rules(&self, resource: &Resource) -> Vec<CircuitBreakerRule> {
        let cache = match &self.cache {
            Some(cache) => cache,
            None => return Vec::new(),
        };

        let (namespace, service) = match extract_service(resource) {
            Some(ret) => ret,
            None => return Vec::new(),
        };

        let mut filter = HashMap::<String, String>::new();
        filter.insert("service".to_string(), service);

        match cache
            .load_service_circuitbreaker(Filter {
                resource_key: ResourceEventKey {
                    namespace,
                    event_type: EventType::CircuitBreakerRule,
                    filter,
                },
                internal_request: false,
                include_cache: true,
                timeout: Duration::from_millis(500),
            })
            .await
        {
            Ok(mut rules) => {
                rules.sort_by(|a, b| b.priority.cmp(&a.priority));
                rules
            }
            Err(err) => {
                crate::warn!("[circuitbreaker] load rules failed: {}", err);
                Vec::new()
            }
        }
    }

    fn update_instance_circuit_status(&self, resource: &Resource, status: Status) {
        let (namespace, service, instance) = match resource {
            Resource::InstanceResource(v) => (&v.namespace, &v.service, &v.instance),
            _ => return,
        };

        let manager = match &self.stat_manager {
            Some(v) => v,
            None => return,
        };

        let target = match status {
            Status::Open => CircuitStatus::Open,
            Status::HalfOpen => CircuitStatus::HalfOpen,
            Status::Close | Status::Destroy => CircuitStatus::Close,
        };
        manager.set_circuit_status(namespace, service, instance, target);
    }
}

impl Plugin for CompositeCircuitBreaker {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        PLUGIN_NAME.to_string()
    }
}

#[async_trait::async_trait]
impl CircuitBreaker for CompositeCircuitBreaker {
    fn set_resource_cache(&mut self, cache: Arc<Box<dyn ResourceCache>>) {
        self.cache = Some(cache);
    }

    fn set_instance_stat_manager(&mut self, manager: Arc<InstanceStatManager>) {
        self.stat_manager = Some(manager);
    }

    /// check_resource 检查资源
    async fn check_resource(&self, resource: Resource) -> Result<CircuitBreakerStatus, PoleError> {
        let now_ms = now_millis();
        let rules = self.load_rules(&resource).await;
        let resource_key = resource.resource_key();

        for rule in rules.iter() {
            if !rule_matches_resource(rule, &resource) {
                continue;
            }

            let state_key = format!("{}#{}", rule_identity(rule), resource_key);
            let state = self.get_state(&state_key);
            let (pass, status, open_start_ms) = {
                let mut guard = state.lock();
                let (allow, st) = guard.on_check(rule, now_ms);
                (allow, st, guard.open_start_ms)
            };

            if !pass {
                self.update_instance_circuit_status(&resource, status.clone());
                return Ok(CircuitBreakerStatus {
                    status,
                    start_ms: open_start_ms,
                    circuit_breaker: rule_identity(rule),
                    fallback_info: build_fallback_info(&rule.fallback_config),
                    destroy: false,
                });
            }
        }

        self.update_instance_circuit_status(&resource, Status::Close);
        Ok(CircuitBreakerStatus {
            status: Status::Close,
            start_ms: 0,
            circuit_breaker: String::new(),
            fallback_info: None,
            destroy: false,
        })
    }

    /// report_stat 上报统计信息
    async fn report_stat(&self, stat: ResourceStat) -> Result<(), PoleError> {
        let now_ms = now_millis();
        let rules = self.load_rules(&stat.resource).await;
        let resource_key = stat.resource.resource_key();

        for rule in rules.iter() {
            if !rule_matches_resource(rule, &stat.resource) {
                continue;
            }

            let is_error = is_error_by_rule(rule, &stat);
            let state_key = format!("{}#{}", rule_identity(rule), resource_key);
            let state = self.get_state(&state_key);
            state.lock().record_stat(rule, is_error, now_ms);
        }

        Ok(())
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn extract_service(resource: &Resource) -> Option<(String, String)> {
    match resource {
        Resource::ServiceResource(ServiceResource { callee, .. })
        | Resource::MethodResource(MethodResource { callee, .. }) => {
            Some((callee.namespace.clone(), callee.name.clone()))
        }
        Resource::InstanceResource(v) => Some((v.namespace.clone(), v.service.clone())),
        Resource::CustomResource(v) => {
            if v.namespace.is_empty() || v.service.is_empty() {
                None
            } else {
                Some((v.namespace.clone(), v.service.clone()))
            }
        }
    }
}

fn rule_identity(rule: &CircuitBreakerRule) -> String {
    if !rule.name.is_empty() {
        return rule.name.clone();
    }
    if !rule.id.is_empty() {
        return rule.id.clone();
    }
    "unnamed-circuit-breaker".to_string()
}

fn rule_matches_resource(rule: &CircuitBreakerRule, resource: &Resource) -> bool {
    if !rule.enable {
        return false;
    }

    let level = pole_specification::v1::Level::try_from(rule.level).unwrap_or_default();

    match resource {
        Resource::ServiceResource(res) => {
            if !matches!(
                level,
                pole_specification::v1::Level::Service | pole_specification::v1::Level::Unknown
            ) {
                return false;
            }
            match_caller_callee(rule, &res.caller, &res.callee.namespace, &res.callee.name)
        }
        Resource::MethodResource(res) => {
            if !matches!(
                level,
                pole_specification::v1::Level::Service
                    | pole_specification::v1::Level::Method
                    | pole_specification::v1::Level::Unknown
            ) {
                return false;
            }

            if !match_caller_callee(rule, &res.caller, &res.callee.namespace, &res.callee.name) {
                return false;
            }

            if level == pole_specification::v1::Level::Method {
                return rule
                    .block_configs
                    .iter()
                    .any(|cfg| match_api(cfg.api.as_ref(), &res.protocol, &res.method, &res.path));
            }
            true
        }
        Resource::InstanceResource(res) => {
            matches!(level, pole_specification::v1::Level::Instance)
                && match_caller_callee(rule, &None, &res.namespace, &res.service)
        }
        Resource::CustomResource(res) => match_custom_resource(rule, res),
    }
}

fn match_custom_resource(rule: &CircuitBreakerRule, res: &CustomResource) -> bool {
    let caller = &res.caller;

    match res.resource_type.as_str() {
        "service" => match_caller_callee(rule, caller, &res.namespace, &res.service),
        "interface" | "method" => match_caller_callee(rule, caller, &res.namespace, &res.service),
        "instance" => {
            let level = pole_specification::v1::Level::try_from(rule.level).unwrap_or_default();
            matches!(level, pole_specification::v1::Level::Instance)
                && match_caller_callee(rule, caller, &res.namespace, &res.service)
        }
        // SQL 等非服务资源，当前仅做目的资源维度匹配，不强依赖 caller
        _ => match_caller_callee(rule, caller, &res.namespace, &res.service),
    }
}

fn match_caller_callee(
    rule: &CircuitBreakerRule,
    caller: &Option<crate::core::model::naming::ServiceKey>,
    callee_ns: &str,
    callee_name: &str,
) -> bool {
    if let Some(matcher) = &rule.rule_matcher {
        if let Some(dest) = &matcher.destination {
            if !field_match(&dest.namespace, callee_ns) || !field_match(&dest.service, callee_name)
            {
                return false;
            }
        }

        if let Some(source) = &matcher.source {
            if let Some(caller) = caller {
                if !field_match(&source.namespace, &caller.namespace)
                    || !field_match(&source.service, &caller.name)
                {
                    return false;
                }
            } else {
                // 规则指定了 caller，实际请求没有 caller 信息，则视为不匹配
                if !(source.namespace.is_empty() || helper::is_match_all(&source.namespace))
                    || !(source.service.is_empty() || helper::is_match_all(&source.service))
                {
                    return false;
                }
            }
        }
    }

    true
}

fn field_match(expected: &str, actual: &str) -> bool {
    expected.is_empty() || helper::is_match_all(expected) || expected == actual
}

fn match_api(
    api: Option<&pole_specification::v1::Api>,
    protocol: &str,
    method: &str,
    path: &str,
) -> bool {
    let api = match api {
        Some(api) => api,
        None => return true,
    };

    let protocol_ok = field_match(&api.protocol, protocol);
    let method_ok = field_match(&api.method, method);
    let path_ok = match &api.path {
        Some(matcher) => match_string_value(matcher, path),
        None => true,
    };

    protocol_ok && method_ok && path_ok
}

fn build_fallback_info(config: &Option<FallbackConfig>) -> Option<FallbackInfo> {
    let cfg = config.as_ref()?;
    if !cfg.enable {
        return None;
    }

    let response = cfg.response.as_ref()?;
    let mut headers = HashMap::new();
    for header in response.headers.iter() {
        headers.insert(header.key.clone(), header.value.clone());
    }

    Some(FallbackInfo {
        code: response.code.to_string(),
        headers,
        body: response.body.clone(),
    })
}

fn is_error_by_rule(rule: &CircuitBreakerRule, stat: &ResourceStat) -> bool {
    let default_error = !matches!(stat.status, RetStatus::RetSuccess);

    let mut conditions = Vec::<ErrorCondition>::new();
    if !rule.block_configs.is_empty() {
        for block in rule.block_configs.iter() {
            conditions.extend(block.error_conditions.iter().cloned());
        }
    }
    conditions.extend(rule.error_conditions.iter().cloned());

    if conditions.is_empty() {
        return default_error;
    }

    conditions
        .iter()
        .any(|cond| match_error_condition(cond, stat))
}

fn match_error_condition(cond: &ErrorCondition, stat: &ResourceStat) -> bool {
    let matcher = match &cond.condition {
        Some(v) => v,
        None => return false,
    };

    let actual_val =
        match pole_specification::v1::error_condition::InputType::try_from(cond.input_type)
            .unwrap_or_default()
        {
            pole_specification::v1::error_condition::InputType::RetCode => stat.ret_code.clone(),
            pole_specification::v1::error_condition::InputType::Delay => {
                stat.delay.as_millis().to_string()
            }
            pole_specification::v1::error_condition::InputType::Unknown => return false,
        };

    match_string_value(matcher, &actual_val)
}

fn match_string_value(matcher: &pole_specification::v1::MatchString, actual: &str) -> bool {
    let rule_value = matcher.value.as_str();
    if helper::is_match_all(rule_value) {
        return true;
    }

    match match_string::MatchStringType::try_from(matcher.r#type).unwrap_or_default() {
        match_string::MatchStringType::Exact => rule_value == actual,
        match_string::MatchStringType::NotEquals => rule_value != actual,
        match_string::MatchStringType::Regex => regex::Regex::new(rule_value)
            .map(|re| re.is_match(actual))
            .unwrap_or(false),
        match_string::MatchStringType::In => rule_value.split(',').any(|x| x == actual),
        match_string::MatchStringType::NotIn => !rule_value.split(',').any(|x| x == actual),
        match_string::MatchStringType::Range => {
            let parts: Vec<&str> = rule_value.split(',').collect();
            if parts.len() != 2 {
                return false;
            }
            let min = parts[0].trim().parse::<i64>();
            let max = parts[1].trim().parse::<i64>();
            let cur = actual.parse::<i64>();
            if let (Ok(min), Ok(max), Ok(cur)) = (min, max, cur) {
                cur >= min && cur <= max
            } else {
                false
            }
        }
    }
}

fn max_trigger_interval_secs(rule: &CircuitBreakerRule) -> u32 {
    let mut max_val = 60_u32;

    for block in rule.block_configs.iter() {
        for trigger in block.trigger_conditions.iter() {
            max_val = max_val.max(if trigger.interval == 0 {
                60
            } else {
                trigger.interval
            });
        }
    }

    for trigger in rule.trigger_condition.iter() {
        max_val = max_val.max(if trigger.interval == 0 {
            60
        } else {
            trigger.interval
        });
    }

    max_val
}

fn is_triggered(rule: &CircuitBreakerRule, state: &RuleRuntimeState, now_ms: u64) -> bool {
    let mut triggers = Vec::<TriggerCondition>::new();
    for block in rule.block_configs.iter() {
        triggers.extend(block.trigger_conditions.iter().cloned());
    }
    triggers.extend(rule.trigger_condition.iter().cloned());

    if triggers.is_empty() {
        triggers.push(TriggerCondition {
            trigger_type: trigger_condition::TriggerType::ErrorRate as i32,
            error_count: 0,
            error_percent: 50,
            interval: 60,
            minimum_request: 20,
        });
        triggers.push(TriggerCondition {
            trigger_type: trigger_condition::TriggerType::ConsecutiveError as i32,
            error_count: 5,
            error_percent: 0,
            interval: 60,
            minimum_request: 0,
        });
    }

    for trigger in triggers.iter() {
        match trigger_condition::TriggerType::try_from(trigger.trigger_type).unwrap_or_default() {
            trigger_condition::TriggerType::ErrorRate => {
                let interval_secs = if trigger.interval == 0 {
                    60
                } else {
                    trigger.interval
                } as u64;
                let since_ms = now_ms.saturating_sub(interval_secs * 1000);

                let mut total = 0_u32;
                let mut errors = 0_u32;
                for (ts, is_error) in state.samples.iter().rev() {
                    if *ts < since_ms {
                        break;
                    }
                    total += 1;
                    if *is_error {
                        errors += 1;
                    }
                }

                let min_req = if trigger.minimum_request == 0 {
                    20
                } else {
                    trigger.minimum_request
                };
                let threshold = if trigger.error_percent == 0 {
                    50
                } else {
                    trigger.error_percent
                };
                if total >= min_req && total > 0 {
                    let ratio = errors * 100 / total;
                    if ratio >= threshold {
                        return true;
                    }
                }
            }
            trigger_condition::TriggerType::ConsecutiveError => {
                let threshold = if trigger.error_count == 0 {
                    5
                } else {
                    trigger.error_count
                };
                if state.consecutive_error >= threshold {
                    return true;
                }
            }
            trigger_condition::TriggerType::Unknown => {}
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use pole_specification::v1::{
        trigger_condition, BlockConfig, CircuitBreakerRule, ErrorCondition, MatchString,
        TriggerCondition,
    };

    use crate::core::model::circuitbreaker::{Resource, ResourceStat, RetStatus, ServiceResource};
    use crate::core::model::naming::ServiceKey;

    use super::{is_error_by_rule, is_triggered, match_string_value, RuleRuntimeState};

    fn build_rule() -> CircuitBreakerRule {
        CircuitBreakerRule {
            enable: true,
            name: "rule-a".to_string(),
            block_configs: vec![BlockConfig {
                trigger_conditions: vec![TriggerCondition {
                    trigger_type: trigger_condition::TriggerType::ConsecutiveError as i32,
                    error_count: 2,
                    error_percent: 0,
                    interval: 60,
                    minimum_request: 0,
                }],
                ..BlockConfig::default()
            }],
            ..CircuitBreakerRule::default()
        }
    }

    #[test]
    fn test_match_string_range_should_work() {
        let matcher = MatchString {
            r#type: pole_specification::v1::match_string::MatchStringType::Range as i32,
            value: "100,200".to_string(),
            value_type: 0,
        };

        assert!(match_string_value(&matcher, "150"));
        assert!(!match_string_value(&matcher, "50"));
    }

    #[test]
    fn test_state_should_trip_by_consecutive_error() {
        let rule = build_rule();
        let mut state = RuleRuntimeState::default();
        state.record_stat(&rule, true, 1_000);
        assert!(!is_triggered(&rule, &state, 1_000));
        state.record_stat(&rule, true, 2_000);
        assert!(is_triggered(&rule, &state, 2_000));
    }

    #[test]
    fn test_error_condition_should_match_ret_code() {
        let cond = ErrorCondition {
            input_type: pole_specification::v1::error_condition::InputType::RetCode as i32,
            condition: Some(MatchString {
                r#type: pole_specification::v1::match_string::MatchStringType::Exact as i32,
                value: "500".to_string(),
                value_type: 0,
            }),
        };
        let mut rule = CircuitBreakerRule::default();
        rule.error_conditions = vec![cond];

        let stat = ResourceStat {
            resource: Resource::ServiceResource(ServiceResource::new(ServiceKey::new(
                "default".to_string(),
                "svc-a".to_string(),
            ))),
            ret_code: "500".to_string(),
            delay: Duration::from_millis(1),
            status: RetStatus::RetFail,
        };

        assert!(is_error_by_rule(&rule, &stat));
    }
}
