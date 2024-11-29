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

use super::{naming::ServiceInstances, TrafficArgument};

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

#[derive(Clone, Debug)]
pub struct RouteInfo {
    // 主调服务数据信息
    pub namespace: String,
    pub service: String,
    // 路由链
    pub chain: RouterChain,
    // 用于元数据路由
    pub metadata: HashMap<String, String>,
    pub metadata_failover: MetadataFailoverType,
    // 用于路由规则
    pub route_labels: HashMap<String, Vec<TrafficArgument>>,
    // 北极星内部治理规则执行时，会识别规则中的参数来源类别，如果发现规则中的参数来源指定为外部数据源时，会调用本接口进行获取
    pub external_parameter_supplier: Option<fn(str) -> str>,
}

impl Default for RouteInfo {
    fn default() -> Self {
        Self {
            namespace: Default::default(),
            service: Default::default(),
            chain: Default::default(),
            metadata: HashMap::<String, String>::new(),
            metadata_failover: MetadataFailoverType::MetadataFailoverNone,
            route_labels: Default::default(),
            external_parameter_supplier: Default::default(),
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct RouterChain {
    pub before: Vec<String>,
    pub core: Vec<String>,
    pub after: Vec<String>,
}
