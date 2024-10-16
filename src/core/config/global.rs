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

use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GlobalConfig {
    pub system: Option<SystemConfig>,
    pub api: APIConfig,
    pub server_connectors: ServerConnectorConfig,
    pub stat_reporter: StatReporterConfig,
    pub location: LocationConfig,
    pub local_cache: LocalCacheConfig,
}

impl GlobalConfig {
    pub fn update_server_connector_address(&mut self, addresses: Vec<String>) {
        self.server_connectors.update_addresses(addresses);
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemConfig {
    pub discover_cluster: Option<ClusterConfig>,
    pub config_cluster: Option<ClusterConfig>,
    pub health_check_cluster: Option<ClusterConfig>,
    pub variables: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct APIConfig {
    #[serde(with = "serde_duration_ext")]
    pub timeout: Duration,
    pub max_retry_times: u32,
    #[serde(with = "serde_duration_ext")]
    pub retry_interval: Duration,
    pub bind_if: Option<String>,
    pub bind_ip: Option<String>,
    #[serde(with = "serde_duration_ext")]
    pub report_interval: Duration,
}

pub static DISCOVER_SERVER_CONNECTOR: &str = "discover";
pub static CONFIG_SERVER_CONNECTOR: &str = "config";

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerConnectorConfig {
    pub addresses: Vec<String>,
    pub protocol: String,
    #[serde(with = "serde_duration_ext")]
    pub connect_timeout: Duration,
    #[serde(with = "serde_duration_ext")]
    pub server_switch_interval: Duration,
    #[serde(with = "serde_duration_ext")]
    pub message_timeout: Duration,
    #[serde(with = "serde_duration_ext")]
    pub connection_idle_timeout: Duration,
    #[serde(with = "serde_duration_ext")]
    pub reconnect_interval: Duration,
    pub metadata: Option<HashMap<String, String>>,
    pub ssl: Option<SSL>,
    pub token: Option<String>,
}

impl ServerConnectorConfig {
    pub fn get_protocol(&self) -> String {
        return self.protocol.clone();
    }

    pub fn update_addresses(&mut self, addresses: Vec<String>) {
        self.addresses.clear();
        self.addresses.extend(addresses);
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SSL {
    pub trusted_ca_file: String,
    pub cert_file: String,
    pub key_file: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatReporterConfig {
    pub enable: bool,
    pub chain: Option<Vec<StatReporterPluginConfig>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatReporterPluginConfig {
    pub name: String,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocationConfig {
    pub providers: Option<Vec<LocationProviderConfig>>,
}

fn default_location_providers() -> Vec<LocationProviderConfig> {
    let mut providers = Vec::new();
    providers.push(LocationProviderConfig {
        name: "local".to_string(),
        options: HashMap::new(),
    });
    return providers;
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocationProviderConfig {
    pub name: String,
    pub options: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClusterConfig {
    pub namespace: Option<String>,
    pub service: Option<String>,
    #[serde(with = "serde_duration_ext")]
    pub refresh_interval: Duration,
    pub routers: Vec<String>,
    pub lb_policy: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalCacheConfig {
    #[serde(default = "default_local_cache_name")]
    pub name: String,
    pub service_expire_enable: bool,
    #[serde(with = "serde_duration_ext")]
    pub service_expire_time: Duration,
    #[serde(with = "serde_duration_ext")]
    pub service_refresh_interval: Duration,
    #[serde(with = "serde_duration_ext")]
    pub service_list_refresh_interval: Duration,
    pub persist_enable: bool,
    pub persist_dir: String,
    pub persist_max_write_retry: u32,
    pub persist_max_read_retry: u32,
    #[serde(with = "serde_duration_ext")]
    pub persist_retry_interval: Duration,
}

fn default_local_cache_name() -> String {
    "memory".to_string()
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginConfig {
    pub name: String,
    pub options: Option<HashMap<String, String>>,
}
