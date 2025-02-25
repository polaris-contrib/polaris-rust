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

use std::{any::Any, collections::HashMap, sync::Arc, time::Duration};

use polaris_specification::v1::{Destination, Route, Routing};

use crate::core::{
    config::consumer::ServiceRouterPluginConfig,
    model::{
        cache::{EventType, ResourceEventKey},
        error::{ErrorCode, PolarisError},
        naming::{Instance, ServiceInstances},
        router::{RouteResult, RouteState, DEFAULT_ROUTER_RULE},
    },
    plugin::{
        cache::Filter,
        plugins::{Extensions, Plugin},
        router::{RouteContext, ServiceRouter},
    },
};
use crate::warn;
use super::helper::route_traffic_match;

#[derive(Debug, PartialEq, Eq)]
pub enum Direction {
    Callee,
    Caller,
}

#[derive(Debug, PartialEq, Eq)]
enum RouteFailoverPolicy {
    All,
    None,
}

#[derive(Debug, PartialEq, Eq)]
enum RuleStatus {
    // 无路由策略
    NoRule,
    // 被调服务路由策略匹配成功
    DestRuleSucc,
    // 被调服务路由策略匹配失败
    DestRuleFail,
    // 主调服务路由策略匹配成功
    SourceRuleSucc,
    // 主调服务路由策略匹配失败
    SourceRuleFail,
}

pub fn new_service_router(_conf: &ServiceRouterPluginConfig) -> Box<dyn ServiceRouter> {
    let mut policy = RouteFailoverPolicy::All;
    if let Some(opt) = _conf.options.clone() {
        let val = opt.get("failover");
        if let Some(val) = val {
            if val == "none" {
                policy = RouteFailoverPolicy::None;
            }
        }
    }
    Box::new(RuleRouter {
        failover_policy: policy,
    })
}

pub struct RuleRouter {
    failover_policy: RouteFailoverPolicy,
}

impl RuleRouter {
    pub fn builder() -> (
        fn(&ServiceRouterPluginConfig) -> Box<dyn ServiceRouter>,
        String,
    ) {
        (new_service_router, DEFAULT_ROUTER_RULE.to_string())
    }
}

impl Plugin for RuleRouter {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        DEFAULT_ROUTER_RULE.to_string()
    }
}

impl RuleRouter {
    async fn fetch_rule(
        &self,
        extensions: Arc<Extensions>,
        rctx: &RouteContext,
        dir: Direction,
    ) -> Result<Vec<Route>, PolarisError> {
        let local_cache = extensions.get_resource_cache();

        let mut ns = &rctx.route_info.caller.namespace;
        let mut svc = &rctx.route_info.caller.name;
        if dir == Direction::Callee {
            ns = &rctx.route_info.callee.namespace;
            svc = &rctx.route_info.callee.name;
        }

        let mut filter = HashMap::<String, String>::new();
        filter.insert("service".to_string(), svc.to_string());
        let ret = local_cache
            .load_service_rule(Filter {
                resource_key: ResourceEventKey {
                    namespace: ns.to_string(),
                    event_type: EventType::RouterRule,
                    filter,
                },
                internal_request: false,
                include_cache: true,
                timeout: Duration::from_secs(1),
            })
            .await;

        if ret.is_err() {
            return Err(ret.err().unwrap());
        }
        let ret = ret.unwrap();

        let mut rules = Vec::<Box<Routing>>::with_capacity(ret.rules.len());
        for ele in ret.rules {
            let type_id = ele.type_id();
            match ele.downcast::<Routing>() {
                Ok(rule) => rules.push(rule),
                Err(_) => {
                    return Err(PolarisError::new(
                        ErrorCode::InvalidRule,
                        format!("rule type error, expect Routing, but got {:?}", type_id),
                    ));
                }
            }
        }
        // rules 只会有一个的，所以这里指拿第一个即可
        let rule = rules.remove(0);
        if dir == Direction::Callee {
            return Ok(rule.inbounds);
        }
        return Ok(rule.outbounds);
    }

    fn filter_instances(
        &self,
        rctx: &RouteContext,
        instances: &ServiceInstances,
        rules: Vec<Route>,
    ) -> Result<Vec<Instance>, PolarisError> {
        for ele in rules {
            if !route_traffic_match(rctx, &ele) {
                continue;
            }
            // 匹配实例分组

            let destination = filter_available_destinations(ele.destinations);
            for (_, dest) in destination.iter().enumerate() {
                let ret = match_callee_group(dest, instances);
                if ret.is_empty() {
                    continue;
                }
                // 返回目标实例分组结果
                return Ok(ret);
            }
            // 没有符合的实例分组，需要看下兜底逻辑
        }
        // 返回空实例列表
        Ok(vec![])
    }
}

#[async_trait::async_trait]
impl ServiceRouter for RuleRouter {
    /// choose_instances 实例路由
    async fn choose_instances(
        &self,
        route_ctx: RouteContext,
        instances: ServiceInstances,
    ) -> Result<RouteResult, PolarisError> {
        let extensions = route_ctx.extensions.clone().unwrap();

        // 匹配顺序 -> 先按照被调方路由规则匹配，然后再按照主调方规则进行匹配
        let mut filtered_ins = Option::<Vec<Instance>>::None;

        let mut status = RuleStatus::NoRule;
        let callee_rules = self
            .fetch_rule(extensions.clone(), &route_ctx, Direction::Callee)
            .await?;
        if !callee_rules.is_empty() {
            status = RuleStatus::DestRuleSucc;
            let ret = self.filter_instances(&route_ctx, &instances, callee_rules)?;
            if ret.is_empty() {
                status = RuleStatus::DestRuleFail;
            } else {
                filtered_ins = Some(ret);
            }
        }

        // 如果被调服务路由规则匹配失败，则判断主调方的路由规则
        if status != RuleStatus::DestRuleSucc {
            let caller_rules = self
                .fetch_rule(extensions, &route_ctx, Direction::Caller)
                .await?;
            if !caller_rules.is_empty() {
                status = RuleStatus::SourceRuleSucc;
                let ret = self.filter_instances(&route_ctx, &instances, caller_rules)?;
                if ret.is_empty() {
                    status = RuleStatus::SourceRuleFail;
                } else {
                    filtered_ins = Some(ret);
                }
            }
        }

        match status {
            RuleStatus::NoRule => Ok(RouteResult {
                instances,
                state: RouteState::Next,
            }),
            RuleStatus::DestRuleSucc | RuleStatus::SourceRuleSucc => {
                let mut total_weight = 0 as u64;
                let filtered_ins = filtered_ins.unwrap();
                let ins = Vec::<Instance>::with_capacity(filtered_ins.capacity());
                for ele in filtered_ins {
                    total_weight += ele.weight as u64;
                }
                Ok(RouteResult {
                    instances: ServiceInstances {
                        service: instances.service.clone(),
                        instances: ins,
                        total_weight: total_weight,
                    },
                    state: RouteState::Next,
                })
            }
            _ => {
                warn!(
                    "[router][rule] route rule not match, rule status: {:?}, not matched callee:{:?} caller:{:?}",
                    status,
                    route_ctx.route_info.caller,
                    route_ctx.route_info.callee,
                );
                match self.failover_policy {
                    RouteFailoverPolicy::All => Ok(RouteResult {
                        instances,
                        state: RouteState::Next,
                    }),
                    RouteFailoverPolicy::None => Ok(RouteResult {
                        instances: ServiceInstances {
                            service: instances.service.clone(),
                            instances: vec![],
                            total_weight: 0,
                        },
                        state: RouteState::Next,
                    }),
                }
            }
        }
    }

    /// enable 是否启用
    async fn enable(&self, route_ctx: RouteContext, instances: ServiceInstances) -> bool {
        let route_info = &route_ctx.route_info;
        let chain = &route_ctx.route_info.chain;
        let has_router = chain.exist_route(DEFAULT_ROUTER_RULE);
        if !has_router {
            return false;
        }

        let caller_ret = self
            .fetch_rule(
                route_ctx.extensions.clone().unwrap(),
                &route_ctx,
                Direction::Caller,
            )
            .await;
        if caller_ret.is_err() {
            return false;
        }
        let caller_empty = caller_ret.unwrap().is_empty();

        let callee_ret = self
            .fetch_rule(
                route_ctx.extensions.clone().unwrap(),
                &route_ctx,
                Direction::Callee,
            )
            .await;
        if callee_ret.is_err() {
            return false;
        }
        let callee_empty = callee_ret.unwrap().is_empty();

        // 其中一个有规则即可
        caller_empty || callee_empty
    }
}

fn match_callee_group(dest: &Destination, instances: &ServiceInstances) -> Vec<Instance> {
    todo!()
}

fn filter_available_destinations(dests: Vec<Destination>) -> Vec<Destination> {
    let mut ret = Vec::<Destination>::with_capacity(dests.capacity());

    for ele in dests {
        if ele.isolate.unwrap_or(false) {
            ret.push(ele);
        }
    }

    // 优先级按照 0 -> 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9 以此类推
    ret.sort_by(|a, b| {
        let a_weight = a.weight.unwrap_or(0);
        let b_weight = b.weight.unwrap_or(0);
        a_weight.cmp(&b_weight)
    });

    ret
}
