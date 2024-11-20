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

use naming::Location;
use pb::lib::client::ClientType;
use pb::lib::{ConfigDiscoverRequest, ConfigDiscoverResponse, DiscoverRequest, DiscoverResponse};

pub mod cache;
pub mod circuitbreaker;
pub mod cluster;
pub mod config;
pub mod error;
pub mod loadbalance;
pub mod naming;
pub mod pb;
pub mod ratelimit;
pub mod router;
pub mod stat;

use std::collections::HashMap;
use std::hash::Hash;

use super::config::global::ClientConfig;

static RUST_CLIENT_VERSION: &str = "v0.0.1";
static RUST_CLIENT_TYPE: &str = "polaris-rust";

#[derive(Clone)]
pub enum DiscoverRequestInfo {
    Unknown,
    Naming(DiscoverRequest),
    Configuration(ConfigDiscoverRequest),
}

impl DiscoverRequestInfo {
    pub fn to_config_request(&self) -> pb::lib::ConfigDiscoverRequest {
        match self {
            DiscoverRequestInfo::Configuration(req) => req.clone(),
            _ => {
                panic!("DiscoverRequestInfo is not ConfigDiscoverRequest");
            }
        }
    }
}

#[derive(Clone)]
pub enum DiscoverResponseInfo {
    Unknown,
    Naming(DiscoverResponse),
    Configuration(ConfigDiscoverResponse),
}

impl DiscoverResponseInfo {
    pub fn to_config_response(&self) -> pb::lib::ConfigDiscoverResponse {
        match self {
            DiscoverResponseInfo::Configuration(resp) => resp.clone(),
            _ => {
                panic!("DiscoverResponseInfo is not ConfigDiscoverResponse");
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum ArgumentType {
    Custom,
    Method,
    Header,
    Query,
    CallerService,
    CallerIP,
}

#[derive(Debug, Clone)]
pub struct TrafficArgument {
    arg_type: ArgumentType,
    key: String,
    value: String,
}

impl TrafficArgument {
    pub fn new(arg_type: ArgumentType, key: String, value: String) -> Self {
        Self {
            arg_type,
            key,
            value,
        }
    }

    pub fn get_type(&self) -> &ArgumentType {
        &self.arg_type
    }

    pub fn get_key(&self) -> &String {
        &self.key
    }

    pub fn get_value(&self) -> &String {
        &self.value
    }

    pub fn build_custom(key: String, value: String) -> Self {
        TrafficArgument::new(ArgumentType::Custom, key, value)
    }

    pub fn build_method(method: String) -> Self {
        TrafficArgument::new(ArgumentType::Method, String::new(), method)
    }

    pub fn build_header(header_key: String, header_value: String) -> Self {
        TrafficArgument::new(ArgumentType::Header, header_key, header_value)
    }

    pub fn build_query(query_key: String, query_value: String) -> Self {
        TrafficArgument::new(ArgumentType::Query, query_key, query_value)
    }

    pub fn build_caller_service(namespace: String, service: String) -> Self {
        TrafficArgument::new(ArgumentType::CallerService, namespace, service)
    }

    pub fn build_caller_ip(caller_ip: String) -> Self {
        TrafficArgument::new(ArgumentType::CallerIP, String::new(), caller_ip)
    }

    pub fn from_label(label_key: String, label_value: String) -> Self {
        if label_key == "method" {
            TrafficArgument::build_method(label_value)
        } else if label_key == "caller_ip" {
            TrafficArgument::build_caller_ip(label_value)
        } else if label_key.starts_with("header") {
            TrafficArgument::build_header(label_key["header".len()..].to_string(), label_value)
        } else if label_key.starts_with("query") {
            TrafficArgument::build_query(label_key["query".len()..].to_string(), label_value)
        } else if label_key.starts_with("caller_service") {
            TrafficArgument::build_caller_service(
                label_key["caller_service".len()..].to_string(),
                label_value,
            )
        } else {
            TrafficArgument::build_custom(label_key, label_value)
        }
    }

    pub fn to_label(&self, labels: &mut HashMap<String, String>) {
        match self.arg_type {
            ArgumentType::Method => {
                labels.insert("method".to_string(), self.value.clone());
            }
            ArgumentType::CallerIP => {
                labels.insert("caller_ip".to_string(), self.value.clone());
            }
            ArgumentType::Header => {
                labels.insert(format!("header{}", self.key), self.value.clone());
            }
            ArgumentType::Query => {
                labels.insert(format!("query{}", self.key), self.value.clone());
            }
            ArgumentType::CallerService => {
                labels.insert(format!("caller_service{}", self.key), self.value.clone());
            }
            ArgumentType::Custom => {
                labels.insert(self.key.clone(), self.value.clone());
            }
        }
    }
}

pub struct ReportClientRequest {
    pub client_id: String,
    pub host: String,
    pub version: String,
    pub location: Location,
}

impl ReportClientRequest {
    pub fn convert_spec(&self) -> crate::core::model::pb::lib::Client {
        crate::core::model::pb::lib::Client {
            id: Some(self.client_id.clone()),
            host: Some(self.host.clone()),
            version: Some(self.version.clone()),
            location: Some(self.location.convert_spec()),
            r#type: ClientType::Sdk.into(),
            stat: vec![],
            ctime: None,
            mtime: None,
        }
    }
}

pub struct ClientContext {
    pub client_id: String,
    pub pid: u32,
    pub pod: String,
    pub host: String,
    pub version: String,
    pub labels: HashMap<String, String>,
}

impl ClientContext {
    pub fn new(client_id: String, ip: String, cfg: &ClientConfig) -> ClientContext {
        let mut labels = HashMap::<String, String>::new();
        labels.clone_from(&cfg.labels);

        labels.insert("CLIENT_IP".to_string(), ip.to_string());
        labels.insert("CLIENT_ID".to_string(), client_id.clone());
        labels.insert(
            "CLIENT_VERSION".to_string(),
            RUST_CLIENT_VERSION.to_string(),
        );
        labels.insert("CLIENT_LANGUAGE".to_string(), RUST_CLIENT_TYPE.to_string());

        Self {
            client_id: client_id,
            pid: std::process::id(),
            pod: get_pod_name(),
            host: std::env::var("HOSTNAME").unwrap_or_else(|_| "".to_string()),
            version: RUST_CLIENT_VERSION.to_string(),
            labels: labels,
        }
    }
}

pub fn get_pod_name() -> String {
    // 各种容器平台的获取容器名字的环境变量.
    let container_name_envs = vec![
        // taf/sumeru容器环境变量
        "CONTAINER_NAME",
        // 123容器的环境变量
        "SUMERU_POD_NAME",
        // STKE(CSIG)  微信TKE   TKE-x(TEG)
        "POD_NAME",
        // tkestack(CDG)
        "MY_POD_NAME",
    ];

    for k in container_name_envs {
        if let Ok(pod_name) = std::env::var(k) {
            return pod_name;
        }
    }
    return "".to_string();
}
