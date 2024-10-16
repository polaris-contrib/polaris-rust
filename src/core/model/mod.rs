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

#[derive(Clone)]
pub enum DiscoverRequestInfo {
    Unknown,
    Naming(DiscoverRequest),
    Configuration(ConfigDiscoverRequest),
}

impl DiscoverRequestInfo {
    pub fn to_config_request(&self) -> pb::lib::ConfigDiscoverRequest {
        match self {
            DiscoverRequestInfo::Configuration(req) => req.clone().into(),
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
            DiscoverResponseInfo::Configuration(resp) => resp.clone().into(),
            _ => {
                panic!("DiscoverResponseInfo is not ConfigDiscoverResponse");
            }
        }
    }
}
