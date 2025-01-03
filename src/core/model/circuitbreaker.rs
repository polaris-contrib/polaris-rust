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

use std::{
    fmt::{self, format, Display},
    iter::Map,
    str, time::Duration,
};

use super::{error::{ErrorCode, PolarisError}, naming::ServiceKey};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Status {
    Close,
    HalfOpen,
    Open,
    Destroy,
}

/// CircuitBreakerStatus 资源熔断状态及数据
pub struct CircuitBreakerStatus {
    // 标识被哪个熔断器熔断
    pub circuit_breaker: String,
    // 熔断器状态
    pub status: Status,
    // 开始被熔断的时间
    pub start_ms: u64,
    // 熔断降级信息
    pub fallback_info: Option<FallbackInfo>,
    // 是否被销毁
    pub destroy: bool,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RetStatus {
    RetUnknown,
    RetSuccess,
    RetFail,
    RetTimeout,
    RetReject,
    RetFlowControl,
}

pub struct ResourceStat {
    pub resource: Resource,
    pub ret_code: String,
    pub delay: Duration,
    pub status: RetStatus,
}

pub enum Resource {
    ServiceResource(ServiceResource),
    MethodResource(MethodResource),
    InstanceResource(InstanceResource),
}

/// ServiceResource 服务资源
pub struct ServiceResource {
    pub callee: ServiceKey,
    pub caller: Option<ServiceKey>,
}

impl ServiceResource {

    pub fn new(callee: ServiceKey) -> Self {
        ServiceResource { caller: None, callee }
    }

    pub fn new_waith_caller(caller: ServiceKey, callee: ServiceKey) -> Self {
        ServiceResource { caller: Some(caller), callee }
    }

}

/// MethodResource 方法资源
pub struct MethodResource {
    pub callee: ServiceKey,
    pub caller: Option<ServiceKey>,
    pub protocol: String,
    pub method: String,
    pub path: String,
}

impl MethodResource {

    pub fn new(callee: ServiceKey, protocol: String, method: String, path: String) -> Self {
        MethodResource { caller: None, callee, protocol, method, path }
    }

    pub fn new_waith_caller(caller: ServiceKey, callee: ServiceKey, protocol: String, method: String, path: String) -> Self {
        MethodResource { caller: Some(caller), callee, protocol, method, path }
    }

}

pub struct InstanceResource {}

impl InstanceResource {}

pub struct CheckResult {
    pub pass: bool,
    pub rule_name: String,
    pub fallback_info: Option<FallbackInfo>,
}

impl CheckResult {
    pub fn pass() -> CheckResult {
        Self {
            pass: true,
            rule_name: "".to_string(),
            fallback_info: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FallbackInfo {
    pub code: String,
    pub headers: Map<String, String>,
    pub body: String,
}

pub struct CallAbortedError
where
    Self: Display + Send + Sync,
{
    err: PolarisError,
    pub rule_name: String,
    pub fallback_info: Option<FallbackInfo>,
}

impl CallAbortedError {
    pub fn new(rule_name: String, fallback_info: Option<FallbackInfo>) -> Self {
        CallAbortedError {
            err: PolarisError::new(
                ErrorCode::CircuitBreakError,
                format!("rule {}, fallbackInfo {:?}", rule_name, fallback_info),
            ),
            rule_name,
            fallback_info,
        }
    }
}

impl Display for CallAbortedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.err.fmt(f)
    }
}
