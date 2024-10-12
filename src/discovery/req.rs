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

use prost::Message;

use crate::core::model::cache::EventType;
use crate::core::model::error::{ErrorCode, PolarisError};
use crate::core::model::naming::{Instance, Location, ServiceInfo, ServiceInstancesChangeEvent};
use crate::core::model::router::{CalleeInfo, CallerInfo};
use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct InstanceRegisterRequest {
    pub flow_id: String,
    pub timeout: Duration,
    pub id: Option<String>,
    pub namespace: String,
    pub service: String,
    pub ip: String,
    pub port: u32,
    pub vpc_id: String,
    pub version: String,
    pub protocol: String,
    pub health: bool,
    pub isolated: bool,
    // weight is used to indicate the instance's weight.
    // If it is 0, the instance will not be used for load balancing.
    pub weight: u32,
    pub priority: u32,
    // metadata is used to store the instance's metadata. It can be used to store
    // the instance's custom information.
    pub metadata: HashMap<String, String>,
    // location record geolocation information of service instances, mainly for nearby routing
    pub location: Location,
    // ttl is used to indicate the instance's ttl. If it is 0, the instance will not expire.
    pub ttl: u32,
    // auto_heartbeat is used to indicate whether to enable automatic heartbeat
    // when registering instances. If it is true, the instance will be automatically
    // do heartbeat action by the SDK.
    // If it is false, the instance will not be automatically do heartbeat action by the SDK.
    pub auto_heartbeat: bool,
}

impl InstanceRegisterRequest {
    pub fn convert_instance(&self) -> Instance {
        Instance {
            id: self.id.as_ref().unwrap_or(&"".to_string()).clone(),
            namespace: self.namespace.clone(),
            service: self.service.clone(),
            ip: self.ip.clone(),
            port: self.port.clone(),
            vpc_id: self.vpc_id.clone(),
            version: self.version.clone(),
            protocol: self.protocol.clone(),
            health: self.health.clone(),
            isolated: self.isolated.clone(),
            weight: self.weight.clone(),
            priority: self.priority.clone(),
            metadata: self.metadata.clone(),
            location: self.location.clone(),
            revision: "".to_string(),
        }
    }

    pub fn to_heartbeat_request(&self) -> InstanceHeartbeatRequest {
        InstanceHeartbeatRequest {
            flow_id: "".to_string(),
            timeout: self.timeout.clone(),
            id: None,
            namespace: self.namespace.clone(),
            service: self.service.clone(),
            ip: self.ip.clone(),
            port: self.port,
            vpc_id: self.vpc_id.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct InstanceRegisterResponse {
    pub instance_id: String,
    pub exist: bool,
}

pub struct InstanceDeregisterRequest {
    pub flow_id: String,
    pub timeout: Duration,
    pub namespace: String,
    pub service: String,
    pub ip: String,
    pub port: u32,
    pub vpc_id: String,
}

impl InstanceDeregisterRequest {
    pub fn convert_instance(&self) -> Instance {
        Instance {
            id: "".to_string(),
            namespace: self.namespace.clone(),
            service: self.service.clone(),
            ip: self.ip.clone(),
            port: self.port.clone(),
            vpc_id: self.vpc_id.clone(),
            version: "".to_string(),
            protocol: "".to_string(),
            health: false,
            isolated: false,
            weight: 0,
            priority: 0,
            metadata: Default::default(),
            location: Default::default(),
            revision: "".to_string(),
        }
    }

    pub fn to_heartbeat_request(&self) -> InstanceHeartbeatRequest {
        InstanceHeartbeatRequest {
            flow_id: self.flow_id.clone(),
            timeout: self.timeout.clone(),
            id: Some("".to_string()),
            namespace: self.namespace.clone(),
            service: self.service.clone(),
            ip: self.ip.clone(),
            port: self.port,
            vpc_id: self.vpc_id.clone(),
        }
    }
}

#[derive(Clone, Debug)]
#[warn(unreachable_code)]
pub struct InstanceHeartbeatRequest {
    pub flow_id: String,
    pub timeout: Duration,
    pub id: Option<String>,
    pub namespace: String,
    pub service: String,
    pub ip: String,
    pub port: u32,
    pub vpc_id: String,
}

impl InstanceHeartbeatRequest {
    pub fn beat_key(&self) -> String {
        let namespace = self.namespace.clone();
        let service = self.service.clone();
        let ip = self.ip.clone();
        let port: u32 = self.port;
        let vpc_id = self.vpc_id.clone();
        return format!("{}_{}_{}_{}_{}", namespace, service, ip, port, vpc_id);
    }

    pub fn convert_instance(&self) -> Instance {
        let instance_id = self.id.clone();
        let ip = self.ip.clone();
        let namespace = self.namespace.clone();
        let service = self.service.clone();
        let port = self.port;
        let vpc_id = self.vpc_id.clone();
        Instance {
            id: instance_id.as_ref().unwrap_or(&"".to_string()).clone(),
            namespace: namespace,
            service: service,
            ip: ip,
            port: port,
            vpc_id: vpc_id,
            version: "".to_string(),
            protocol: "".to_string(),
            health: false,
            isolated: false,
            weight: 0,
            priority: 0,
            metadata: Default::default(),
            location: Default::default(),
            revision: "".to_string(),
        }
    }
}

pub struct ReportServiceContractRequest {}

// ConsumerAPI request and response definition

pub struct GetOneInstanceRequest {
    pub flow_id: String,
    pub timeout: Duration,
    pub service: String,
    pub namespace: String,
    pub caller_info: CallerInfo,
    pub callee_info: CalleeInfo,
}

impl GetOneInstanceRequest {
    pub fn check_valid(&self) -> Result<(), PolarisError> {
        if self.service.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::ApiInvalidArgument,
                "service is empty".to_string(),
            ));
        }

        if self.namespace.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::ApiInvalidArgument,
                "namespace is empty".to_string(),
            ));
        }
        Ok(())
    }
}

pub struct GetHealthInstanceRequest {
    pub flow_id: String,
    pub timeout: Duration,
    pub service: String,
    pub namespace: String,
}

impl GetHealthInstanceRequest {
    pub fn check_valid(&self) -> Result<(), PolarisError> {
        if self.service.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::ApiInvalidArgument,
                "service is empty".to_string(),
            ));
        }

        if self.namespace.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::ApiInvalidArgument,
                "namespace is empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct GetAllInstanceRequest {
    pub flow_id: String,
    pub timeout: Duration,
    pub service: String,
    pub namespace: String,
}

impl GetAllInstanceRequest {
    pub fn check_valid(&self) -> Result<(), PolarisError> {
        if self.service.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::ApiInvalidArgument,
                "service is empty".to_string(),
            ));
        }

        if self.namespace.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::ApiInvalidArgument,
                "namespace is empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct InstancesResponse {
    pub service_info: ServiceInfo,
    pub instances: Vec<Instance>,
}

pub struct WatchInstanceRequest {
    pub namespace: String,
    pub service: String,
    pub call_back: Box<dyn Fn(ServiceInstancesChangeEvent) + Send + Sync>,
}

impl WatchInstanceRequest {
    pub fn check_valid(&self) -> Result<(), PolarisError> {
        if self.service.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::ApiInvalidArgument,
                "service is empty".to_string(),
            ));
        }

        if self.namespace.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::ApiInvalidArgument,
                "namespace is empty".to_string(),
            ));
        }
        Ok(())
    }

    pub fn get_key(&self) -> String {
        let namespace = self.namespace.clone();
        let service = self.service.clone();
        return format!("{}#{}", namespace, service);
    }
}

pub struct ServiceCallResult {}

pub enum ServiceRuleType {
    Router,
    CircuitBreaker,
    RateLimit,
    FaultDetector,
    Lane,
}

impl ServiceRuleType {
    pub fn to_event_type(&self) -> EventType {
        match self {
            ServiceRuleType::Router => EventType::RouterRule,
            ServiceRuleType::CircuitBreaker => EventType::CircuitBreakerRule,
            ServiceRuleType::RateLimit => EventType::RateLimitRule,
            ServiceRuleType::FaultDetector => EventType::FaultDetectRule,
            ServiceRuleType::Lane => EventType::LaneRule,
        }
    }
}

pub struct GetServiceRuleRequest {
    pub namespace: String,
    pub service: String,
    pub rule_type: ServiceRuleType,
    pub timeout: Duration,
}

pub struct ServiceRuleResponse {
    pub rules: Vec<Box<dyn Message>>,
}

// LossLessAPI request and response definition

pub struct InstanceProperties {}

pub trait BaseInstance {
    fn get_namespace(&self) -> String;

    fn get_service(&self) -> String;

    fn get_ip(&self) -> String;

    fn get_port(&self) -> u32;
}

pub trait LosslessActionProvider {
    fn get_name(&self) -> String;

    fn do_register(&self, prop: InstanceProperties);

    fn do_deregister(&self);

    fn is_enable_healthcheck(&self) -> bool;

    fn do_healthcheck(&self) -> bool;
}
