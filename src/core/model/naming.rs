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

use std::collections::HashMap;
use prost::Message;
use crate::core::model::pb::lib::HeartbeatHealthCheck;

#[derive(Default)]
pub struct Services {
    pub service_list: Vec<ServiceInfo>,
    pub revision: String,
    pub initialized: bool,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct ServiceKey {
    pub namespace: String,
    pub name: String,
}

impl ServiceKey {
    pub fn new(namespace: String, name: String) -> Self {
        ServiceKey{ namespace, name }
    }
}

#[derive(Default, Debug)]
pub struct ServiceInfo {
    pub id: String,
    pub namespace: String,
    pub name: String,
    pub metadata: HashMap<String, String>,
    pub revision: String,
}

#[derive(Default)]
pub struct ServiceInstances {
    pub service: ServiceInfo,
    pub instances: Vec<Instance>,
}

#[derive(Default, Debug)]
pub struct Instance {
    pub id: String,
    pub namespace: String,
    pub service: String,
    pub ip: String,
    pub port: u32,
    pub vpc_id: String,
    pub version: String,
    pub protocol: String,
    pub health: bool,
    pub isolated: bool,
    pub weight: u32,
    pub priority: u32,
    pub metadata: HashMap<String, String>,
    pub location: Location,
    pub revision: String,
}

impl Instance {
    pub fn new() -> Instance {
        Default::default()
    }
}

#[derive(Default, Debug)]
pub struct Location {
    pub region: String,
    pub zone: String,
    pub campus: String,
}

impl Location {

    pub fn clone(&self) -> Location {
        Self{
            region: self.region.clone(),
            zone: self.zone.clone(),
            campus: self.campus.clone(),
        }
    }

    pub fn convert_spec(&self) -> crate::core::model::pb::lib::Location {
        crate::core::model::pb::lib::Location{
            region: Some(self.region.clone()),
            zone: Some(self.zone.clone()),
            campus: Some(self.campus.clone()),
        }
    }
}

pub struct ServiceRule {
    pub rule: Box<dyn Message>,
    pub revision: String,
    pub initialized: bool,
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct Endpoint {
    host: String,
    port: u32,
    protocol: String,
}

impl Endpoint {
    pub fn format_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

// Connector request and response 请求对象

pub struct InstanceRequest {
    pub flow_id: String,
    pub ttl: u32,
    pub instance: Instance,
}

impl InstanceRequest {

    pub fn convert_spec(&self) -> crate::core::model::pb::lib::Instance {
        let mut spec_ins = crate::core::model::pb::lib::Instance{
            id: None,
            service: Some(self.instance.service.to_string()),
            namespace: Some(self.instance.namespace.to_string()),
            vpc_id: Some(self.instance.vpc_id.to_string()),
            host: Some(self.instance.ip.to_string()),
            port: Some(self.instance.port.clone()),
            protocol: Some(self.instance.protocol.to_string()),
            version: Some(self.instance.version.to_string()),
            priority: Some(self.instance.priority.clone()),
            weight: Some(self.instance.weight.clone()),
            enable_health_check: None,
            health_check: None,
            healthy: Some(self.instance.health.clone()),
            isolate: Some(self.instance.isolated.clone()),
            location: Some(self.instance.location.convert_spec()),
            metadata: self.instance.metadata.clone(),
            logic_set: None,
            ctime: None,
            mtime: None,
            revision: None,
            service_token: None,
        };
        if self.ttl != 0 {
            spec_ins.enable_health_check = Some(true);
            spec_ins.health_check = Some(crate::core::model::pb::lib::HealthCheck {
                r#type: i32::from(crate::core::model::pb::lib::health_check::HealthCheckType::Heartbeat),
                heartbeat: Some(HeartbeatHealthCheck {
                    ttl: Some(self.ttl.clone()),
                }),
            });
        }
        return spec_ins;
    }
}

#[derive(Default)]
pub struct InstanceResponse {
    pub exist: bool,
    pub instance: Instance,
}

impl InstanceResponse {

    pub fn success(id: String) -> Self {
        let mut ret = Self {
            exist: false,
            instance: Instance::default()
        };
        ret.instance.id = id;
        return ret
    }

    pub fn exist_resource() -> Self {
        Self {
            exist: true,
            instance: Instance::default(),
        }
    }
}

