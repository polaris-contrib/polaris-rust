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

use std::fmt;
use std::fmt::Display;
use std::time::Duration;

pub static BUILDIN_SERVER_NAMESPACE: &str = "Polaris";
pub static BUILDIN_SERVER_SERVICE: &str = "polaris.builtin";

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ClusterType {
    #[default]
    BuildIn,
    Discover,
    Config,
    HealthCheck,
}

impl Display for ClusterType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct ServerServiceInfo {
    pub namespace: Option<String>,
    pub service: Option<String>,
    pub refresh_interval: Duration,
    pub routers: Vec<String>,
    pub lb_policy: String,
}
