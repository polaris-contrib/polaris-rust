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

use std::collections::HashMap;

use super::{
    naming::{ServiceInstances, ServiceKey},
    ArgumentType, TrafficArgument,
};

pub static DEFAULT_ROUTER_ISOLATED: &str = "isolatedRouter";

pub static DEFAULT_ROUTER_RECOVER: &str = "recoverRouter";

pub static DEFAULT_ROUTER_METADATA: &str = "metadataRouter";

pub static DEFAULT_ROUTER_RULE: &str = "ruleBasedRouter";

pub static DEFAULT_ROUTER_NEARBY: &str = "nearbyBasedRouter";

pub static DEFAULT_ROUTER_SET: &str = "setRouter";

pub static DEFAULT_ROUTER_CANARY: &str = "canaryRouter";

pub static DEFAULT_ROUTER_LANE: &str = "laneRouter";

pub static DEFAULT_ROUTER_NAMESPACE: &str = "namespaceRouter";

#[derive(Clone, Debug)]
pub enum MetadataFailoverType {
    MetadataFailoverNone,
    MetadataFailoverAll,
    MetadataFailoverNoKey,
}

pub enum RouteState {
    Next,
    Retry,
}

pub struct RouteResult {
    pub state: RouteState,
    pub instances: ServiceInstances,
}

fn default_traffic_label_provider(_: ArgumentType, _: &str) -> Option<String> {
    None
}

fn default_external_parameter_supplier(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(v) => Some(v),
        Err(_) => None,
    }
}

#[derive(Clone, Debug)]
pub struct RouteInfo {
    // 主调服务数据信息
    pub caller: ServiceKey,
    // 被调服务数据信息
    pub callee: ServiceKey,
    // 路由链
    pub chain: RouterChain,
    // 用于元数据路由
    pub metadata: HashMap<String, String>,
    pub metadata_failover: MetadataFailoverType,
    // traffic_label_provider 流量标签提供者
    pub traffic_label_provider: fn(ArgumentType, &str) -> Option<String>,
    // 北极星内部治理规则执行时，会识别规则中的参数来源类别，如果发现规则中的参数来源指定为外部数据源时，会调用本接口进行获取
    pub external_parameter_supplier: fn(&str) -> Option<String>,
}

impl Default for RouteInfo {
    fn default() -> Self {
        Self {
            caller: Default::default(),
            callee: Default::default(),
            chain: Default::default(),
            metadata: HashMap::<String, String>::new(),
            metadata_failover: MetadataFailoverType::MetadataFailoverNone,
            external_parameter_supplier: default_external_parameter_supplier,
            traffic_label_provider: default_traffic_label_provider,
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct RouterChain {
    pub before: Vec<String>,
    pub core: Vec<String>,
    pub after: Vec<String>,
}

impl RouterChain {
    pub fn exist_route(&self, n: &str) -> bool {
        self.before.iter().any(|x| x == n)
            || self.core.iter().any(|x| x == n)
            || self.after.iter().any(|x| x == n)
    }
}
