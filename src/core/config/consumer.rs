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

use serde::Deserialize;


#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConsumerConfig {
    pub service_router: ServiceRouterConfig,
    pub circuit_breaker: CircuitBreakerConfig,
    pub load_balancer: LoadBalancerConfig,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceRouterConfig {
    pub before_chain: Vec<ServiceRouterPluginConfig>,
    pub core_chain: Vec<ServiceRouterPluginConfig>,
    pub after_chain: Vec<ServiceRouterPluginConfig>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceRouterPluginConfig {
    pub name: String,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoadBalancerConfig {
    pub default_policy: String,
    pub plugins: Option<Vec<String>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CircuitBreakerConfig {
    pub enable: bool,
    pub enable_remote_pull: bool,
}
