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

use std::{
    collections::HashMap,
    fmt::{self, Display},
    time::Duration,
};

use super::{
    error::{ErrorCode, PoleError},
    naming::{Instance, ServiceKey},
};

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
    CustomResource(CustomResource),
}

impl Resource {
    /// 统一资源键，用于熔断状态/统计聚合
    ///
    /// - service: `service_resource:namespace:service`
    /// - interface: `interface_resource:namespace:service:proto:interface_name`
    /// - instance: `instance_resource:namespace:service:ip:port`
    /// - custom: `{type}_resource:{segments...}`
    pub fn resource_key(&self) -> String {
        match self {
            Resource::ServiceResource(ServiceResource { callee, .. }) => {
                format!("service_resource:{}:{}", callee.namespace, callee.name)
            }
            Resource::MethodResource(MethodResource {
                callee,
                protocol,
                method,
                ..
            }) => format!(
                "interface_resource:{}:{}:{}:{}",
                callee.namespace, callee.name, protocol, method
            ),
            Resource::InstanceResource(InstanceResource {
                namespace,
                service,
                instance,
            }) => format!(
                "instance_resource:{}:{}:{}:{}",
                namespace, service, instance.ip, instance.port
            ),
            Resource::CustomResource(custom) => {
                let mut parts = Vec::<String>::new();
                if !custom.namespace.is_empty() {
                    parts.push(custom.namespace.clone());
                }
                if !custom.service.is_empty() {
                    parts.push(custom.service.clone());
                }
                parts.extend(custom.identifiers.clone());
                if parts.is_empty() {
                    format!("{}_resource", custom.resource_type)
                } else {
                    format!("{}_resource:{}", custom.resource_type, parts.join(":"))
                }
            }
        }
    }
}

/// ServiceResource 服务资源
pub struct ServiceResource {
    pub callee: ServiceKey,
    pub caller: Option<ServiceKey>,
}

impl ServiceResource {
    pub fn new(callee: ServiceKey) -> Self {
        ServiceResource {
            caller: None,
            callee,
        }
    }

    pub fn new_waith_caller(caller: ServiceKey, callee: ServiceKey) -> Self {
        ServiceResource {
            caller: Some(caller),
            callee,
        }
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
        MethodResource {
            caller: None,
            callee,
            protocol,
            method,
            path,
        }
    }

    pub fn new_waith_caller(
        caller: ServiceKey,
        callee: ServiceKey,
        protocol: String,
        method: String,
        path: String,
    ) -> Self {
        MethodResource {
            caller: Some(caller),
            callee,
            protocol,
            method,
            path,
        }
    }
}

pub struct InstanceResource {
    pub namespace: String,
    pub service: String,
    pub instance: Instance,
}

impl InstanceResource {}
impl InstanceResource {
    pub fn new(namespace: String, service: String, instance: Instance) -> Self {
        Self {
            namespace,
            service,
            instance,
        }
    }
}

pub struct CustomResource {
    pub resource_type: String,
    pub namespace: String,
    pub service: String,
    pub identifiers: Vec<String>,
    pub caller: Option<ServiceKey>,
    pub labels: HashMap<String, String>,
}

impl CustomResource {
    pub fn new(
        resource_type: String,
        namespace: String,
        service: String,
        identifiers: Vec<String>,
    ) -> Self {
        Self {
            resource_type,
            namespace,
            service,
            identifiers,
            caller: None,
            labels: HashMap::new(),
        }
    }

    pub fn new_sql(namespace: String, datasource: String, sql_id: String) -> Self {
        Self {
            resource_type: "sql".to_string(),
            namespace,
            service: datasource,
            identifiers: vec![sql_id],
            caller: None,
            labels: HashMap::new(),
        }
    }
}

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
    pub headers: HashMap<String, String>,
    pub body: String,
}

pub struct CallAbortedError
where
    Self: Display + Send + Sync,
{
    err: PoleError,
    pub rule_name: String,
    pub fallback_info: Option<FallbackInfo>,
}

impl CallAbortedError {
    pub fn new(rule_name: String, fallback_info: Option<FallbackInfo>) -> Self {
        CallAbortedError {
            err: PoleError::new(
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

#[cfg(test)]
mod tests {
    use super::{CustomResource, InstanceResource, MethodResource, Resource, ServiceResource};
    use crate::core::model::naming::{Instance, ServiceKey};

    #[test]
    fn test_resource_key_should_be_unified() {
        let service_key = Resource::ServiceResource(ServiceResource::new(ServiceKey::new(
            "default".to_string(),
            "svc-a".to_string(),
        )));
        assert_eq!(service_key.resource_key(), "service_resource:default:svc-a");

        let method_key = Resource::MethodResource(MethodResource::new(
            ServiceKey::new("default".to_string(), "svc-a".to_string()),
            "grpc".to_string(),
            "SayHello".to_string(),
            "/pkg.Greeter/SayHello".to_string(),
        ));
        assert_eq!(
            method_key.resource_key(),
            "interface_resource:default:svc-a:grpc:SayHello"
        );

        let instance_key = Resource::InstanceResource(InstanceResource::new(
            "default".to_string(),
            "svc-a".to_string(),
            Instance {
                ip: "127.0.0.1".to_string(),
                port: 8080,
                ..Instance::default()
            },
        ));
        assert_eq!(
            instance_key.resource_key(),
            "instance_resource:default:svc-a:127.0.0.1:8080"
        );

        let sql_key = Resource::CustomResource(CustomResource::new_sql(
            "prod".to_string(),
            "order-db".to_string(),
            "sql_template_001".to_string(),
        ));
        assert_eq!(
            sql_key.resource_key(),
            "sql_resource:prod:order-db:sql_template_001"
        );
    }
}
