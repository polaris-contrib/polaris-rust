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

use naming::Location;
use pole_specification::v1::client::ClientType;
use pole_specification::v1::{
    ConfigDiscoverRequest, ConfigDiscoverResponse, DiscoverRequest, DiscoverResponse,
};

pub mod cache;
pub mod circuitbreaker;
pub mod cluster;
pub mod config;
pub mod error;
pub mod instance_stat;
pub mod loadbalance;
pub mod naming;
pub mod ratelimit;
pub mod router;
pub mod stat;

use std::collections::HashMap;
use std::hash::Hash;

use super::config::global::ClientConfig;

static RUST_CLIENT_VERSION: &str = "v0.0.1";
static RUST_CLIENT_TYPE: &str = "pole-client-rust";

pub static DEFAULT_MAP_KV_TUPLE_SEPARATOR: &str = "|";

#[derive(Clone)]
pub enum DiscoverRequestInfo {
    Unknown,
    Naming(DiscoverRequest),
    Configuration(ConfigDiscoverRequest),
}

impl DiscoverRequestInfo {
    pub fn to_config_request(&self) -> ConfigDiscoverRequest {
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
    pub fn to_config_response(&self) -> pole_specification::v1::ConfigDiscoverResponse {
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
    Path,
    Header,
    Cookie,
    Query,
    CallerService,
    CallerIP,
}

impl ArgumentType {
    pub fn parse_from_str(s: &str) -> Self {
        match s {
            "custom" => ArgumentType::Custom,
            "method" => ArgumentType::Method,
            "path" => ArgumentType::Path,
            "header" => ArgumentType::Header,
            "cookie" => ArgumentType::Cookie,
            "query" => ArgumentType::Query,
            "caller_service" => ArgumentType::CallerService,
            "caller_ip" => ArgumentType::CallerIP,
            _ => ArgumentType::Custom,
        }
    }

    pub fn parse_from_spec(i: i32) -> Self {
        match pole_specification::v1::match_argument::Type::from_i32(i) {
            Some(pole_specification::v1::match_argument::Type::Custom) => ArgumentType::Custom,
            Some(pole_specification::v1::match_argument::Type::Method) => ArgumentType::Method,
            Some(pole_specification::v1::match_argument::Type::Header) => ArgumentType::Header,
            Some(pole_specification::v1::match_argument::Type::Query) => ArgumentType::Query,
            Some(pole_specification::v1::match_argument::Type::CallerService) => {
                ArgumentType::CallerService
            }
            Some(pole_specification::v1::match_argument::Type::CallerIp) => ArgumentType::CallerIP,
            _ => ArgumentType::Custom,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            ArgumentType::Custom => "custom".to_string(),
            ArgumentType::Method => "method".to_string(),
            ArgumentType::Path => "path".to_string(),
            ArgumentType::Header => "header".to_string(),
            ArgumentType::Cookie => "cookie".to_string(),
            ArgumentType::Query => "query".to_string(),
            ArgumentType::CallerService => "caller_service".to_string(),
            ArgumentType::CallerIP => "caller_ip".to_string(),
        }
    }
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

    pub fn build_path(path: String) -> Self {
        TrafficArgument::new(ArgumentType::Path, String::new(), path)
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

    pub fn build_cookie(cookie_key: String, cookie_value: String) -> Self {
        TrafficArgument::new(ArgumentType::Cookie, cookie_key, cookie_value)
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
        } else if label_key == "path" {
            TrafficArgument::build_path(label_value)
        } else if label_key.starts_with("cookie") {
            TrafficArgument::build_cookie(label_key["cookie".len()..].to_string(), label_value)
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
                labels.insert(format!("header.{}", self.key), self.value.clone());
            }
            ArgumentType::Query => {
                labels.insert(format!("query.{}", self.key), self.value.clone());
            }
            ArgumentType::CallerService => {
                labels.insert(format!("caller_service{}", self.key), self.value.clone());
            }
            ArgumentType::Custom => {
                labels.insert(self.key.clone(), self.value.clone());
            }
            ArgumentType::Path => {
                labels.insert("path".to_string(), self.value.clone());
            }
            ArgumentType::Cookie => {
                labels.insert(format!("cookie.{}", self.key), self.value.clone());
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
    pub fn convert_spec(&self) -> pole_specification::v1::Client {
        pole_specification::v1::Client {
            id: self.client_id.clone(),
            host: self.host.clone(),
            version: self.version.clone(),
            location: Some(self.location.convert_spec()),
            r#type: ClientType::Sdk.into(),
            stat: vec![],
            ctime: "".to_string(),
            mtime: "".to_string(),
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
