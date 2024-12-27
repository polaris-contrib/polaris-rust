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

use std::env::VarError;

use polaris_specification::v1::{match_string::ValueType, MatchString, Route};

use crate::core::{model::ArgumentType, plugin::router::RouteContext};

static WILDCARD: &str = "*";

// route_traffic_match 匹配主、被调服务信息，以及匹配请求流量标签
pub fn route_traffic_match(ctx: &RouteContext, rule: &Route) -> bool {
    if !match_callee_caller(ctx, rule) {
        return false;
    }

    let traffic_provider = ctx.route_info.traffic_label_provider;
    let ext_provider = ctx.route_info.external_parameter_supplier;

    // 匹配流量标签
    for (_, ele) in rule.sources.iter().enumerate() {
        let mut matched = true;
        for (_, (key, rule_value)) in ele.metadata.iter().enumerate() {
            let mut match_key = key.as_str();
            let mut actual_val = String::new();

            match rule_value.value_type() {
                ValueType::Text => {
                    let mut traffic_type = ArgumentType::Custom;
                    if key.contains(".") {
                        let parts: Vec<&str> = match_key.splitn(2, '.').collect();
                        match_key = parts[1];
                        let mut label_prefix = parts[0];
                        if parts[0].starts_with('$') {
                            label_prefix = &parts[0][1..];
                        }
                        traffic_type = ArgumentType::parse_from_str(label_prefix);
                    }
                    actual_val = traffic_provider(traffic_type, match_key).unwrap_or(String::new());
                }
                // 匹配参数，那就直接从 traffic_labels 中获取
                ValueType::Parameter => continue,
                // 上下文环境变量，value 为环境变量名
                ValueType::Variable => match std::env::var(match_key) {
                    Ok(v) => actual_val = v,
                    Err(err) => {
                        if err != VarError::NotPresent {
                            return false;
                        }
                        match ext_provider(match_key) {
                            Some(v) => actual_val = v,
                            None => return false,
                        }
                    }
                },
            }

            if match_label_value(rule_value, actual_val) {
                matched = false;
                break;
            }
        }

        if matched {
            return true;
        }
    }
    false
}

/// match_label_value 匹配标签值
pub fn match_label_value(rule_value: &MatchString, actual_val: String) -> bool {
    let match_value = rule_value.value.clone().unwrap_or("".to_string());
    if is_match_all(&match_value) {
        return true;
    }

    match rule_value.r#type() {
        polaris_specification::v1::match_string::MatchStringType::Exact => {
            return match_value == actual_val;
        }
        polaris_specification::v1::match_string::MatchStringType::NotEquals => {
            return match_value != actual_val;
        }
        polaris_specification::v1::match_string::MatchStringType::Regex => {
            return regex::Regex::new(&match_value)
                .unwrap()
                .is_match(&actual_val);
        }
        polaris_specification::v1::match_string::MatchStringType::In => {
            return match_value.split(',').any(|x| x == actual_val);
        }
        polaris_specification::v1::match_string::MatchStringType::NotIn => {
            return !match_value.split(',').any(|x| x == actual_val);
        }
        polaris_specification::v1::match_string::MatchStringType::Range => {
            let parts: Vec<&str> = match_value.split(',').collect();
            if parts.len() != 2 {
                return false;
            }
            let min = parts[0].parse::<i64>().unwrap();
            let max = parts[1].parse::<i64>().unwrap();
            let actual_val = actual_val.parse::<i64>().unwrap();
            return actual_val >= min && actual_val <= max;
        }
    }
}

/// match_callee_caller 匹配主被调服务信息
pub fn match_callee_caller(rctx: &RouteContext, rule: &Route) -> bool {
    let caller = &rctx.route_info.caller;
    let callee = &rctx.route_info.callee;

    for (_, ele) in rule.sources.iter().enumerate() {
        let svc = ele.service.clone().unwrap();
        let ns = ele.namespace.clone().unwrap();
        if svc != caller.name && !is_match_all(&svc) {
            return false;
        }
        if ns != caller.namespace && !is_match_all(&ns) {
            return false;
        }
    }

    for (_, ele) in rule.destinations.iter().enumerate() {
        let svc = ele.service.clone().unwrap();
        let ns = ele.namespace.clone().unwrap();
        if svc != callee.name && !is_match_all(&svc) {
            return false;
        }
        if ns != callee.namespace && !is_match_all(&ns) {
            return false;
        }
    }

    true
}

pub fn is_match_all(s: &str) -> bool {
    s == WILDCARD
}
