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

use std::env::VarError;

use pole_specification::v1::{match_string::ValueType, CustomRoute, CustomRouteRule};

use crate::core::{helper::helper, model::ArgumentType, plugin::router::RouteContext};

// route_traffic_match 匹配主、被调服务信息，以及匹配请求流量标签
pub fn route_traffic_match(
    ctx: &RouteContext,
    custom_route: &CustomRoute,
) -> Option<CustomRouteRule> {
    if !match_callee_caller(ctx, custom_route) {
        return None;
    }

    let traffic_provider = ctx.route_info.traffic_label_provider;
    let ext_provider = ctx.route_info.external_parameter_supplier;

    // 匹配流量标签
    for (_, ele) in custom_route.rules.iter().enumerate() {
        let mut matched = true;
        let traffic_labels = ele.arguments.as_ref().unwrap();

        for (_, source_match) in traffic_labels.arguments.iter().enumerate() {
            let mut match_key = source_match.key.as_str();
            let match_value = source_match.value.as_ref().unwrap();
            let mut actual_val = Option::<String>::default();

            match match_value.value_type() {
                ValueType::Text => {
                    let mut traffic_type = ArgumentType::Custom;
                    if match_key.contains(".") {
                        let parts: Vec<&str> = match_key.splitn(2, '.').collect();
                        match_key = parts[1];
                        let mut label_prefix = parts[0];
                        if parts[0].starts_with('$') {
                            label_prefix = &parts[0][1..];
                        }
                        traffic_type = ArgumentType::parse_from_str(label_prefix);
                    }
                    // 假设 traffic_provider 返回 Option<&str>，需转成 Option<String>
                    actual_val = traffic_provider(traffic_type, match_key);
                }
                // 匹配参数，那就直接从 traffic_labels 中获取
                ValueType::Parameter => continue,
                // 上下文环境变量，value 为环境变量名
                ValueType::Variable => match std::env::var(match_key) {
                    Ok(v) => actual_val = Some(v),
                    Err(err) => {
                        if err != VarError::NotPresent {
                            continue;
                        }
                        match ext_provider(match_key) {
                            Some(v) => actual_val = Some(v.to_string()),
                            None => continue,
                        }
                    }
                },
            }

            if helper::match_label_value(match_value, actual_val.as_deref()) {
                matched = false;
                break;
            }
        }

        if matched {
            return Some(ele.clone());
        }
    }
    return None;
}

/// match_callee_caller 匹配主被调服务信息
pub fn match_callee_caller(rctx: &RouteContext, rule: &CustomRoute) -> bool {
    let caller = &rctx.route_info.caller;
    let callee = &rctx.route_info.callee;

    let rule_caller = rule.caller.as_ref().unwrap();
    let rule_callee = rule.callee.as_ref().unwrap();

    let svc = &rule_caller.service;
    let ns = &rule_caller.namespace;
    if svc != &caller.name && !helper::is_match_all(&svc) {
        return false;
    }
    if ns != &caller.namespace && !helper::is_match_all(&ns) {
        return false;
    }

    let svc = &rule_callee.service;
    let ns = &rule_callee.namespace;
    if svc != &callee.name && !helper::is_match_all(&svc) {
        return false;
    }
    if ns != &callee.namespace && !helper::is_match_all(&ns) {
        return false;
    }
    true
}
