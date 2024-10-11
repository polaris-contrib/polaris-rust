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

use std::time::Duration;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderConfig {
    pub rate_limit: RateLimitConfig,
    pub lossless: LosslessConfig,
    #[serde(with = "serde_duration_ext", default = "default_min_register_interval")]
    pub min_register_interval: Duration,
    pub heartbeat_worker_size: Option<u32>,
}

fn default_min_register_interval() -> Duration {
    Duration::from_secs(30)
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RateLimitConfig {
    pub enable: bool,
    pub service: String,
    pub namespace: String,
    pub addresses: Option<Vec<String>>,
    pub max_window_count: u32,
    pub fallback_on_exceed_window_count: String,
    #[serde(with = "serde_duration_ext")]
    pub remote_sync_timeout: Duration,
    #[serde(with = "serde_duration_ext")]
    pub max_queuing_time: Duration,
    pub report_metrics: bool
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LosslessConfig {
    pub enable: bool,
    pub host: String,
    pub port: u32,
    #[serde(with = "serde_duration_ext")]
    pub delay_register_interval: Duration,
    #[serde(with = "serde_duration_ext")]
    pub health_check_interval: Duration,
}
