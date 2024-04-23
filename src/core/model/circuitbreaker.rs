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

use std::iter::Map;

enum Status {
    Close,
    HalfOpen,
    Open,
    Destroy,
}

pub struct CircuitBreakerStatus {
    pub circuit_breaker: String,
    pub status: Status,
    pub start_ms: u64,
    pub fallback_info: FallbackInfo,
    pub destroy: bool,
}

enum RetStatus {
    RetUnknown,
    RetSuccess,
    RetFail,
    RetTimeout,
    RetReject,
    RetFlowControl,
}

pub struct ResourceStat {
    pub resource: *const dyn Resource,
    pub ret_code: String,
    pub delay: u32,
    pub status: RetStatus,
}

pub trait Resource {

}

pub struct ServiceResource {

}

impl ServiceResource {

}

pub struct MethodResource {

}

impl MethodResource {

}

pub struct InstanceResource {

}

impl InstanceResource {

}

pub struct CheckResult {
    pub pass: bool,
    pub rule_name: String,
}

pub struct FallbackInfo {
    pub code: String,
    pub headers: Map<String, String>,
    pub body: String,
}