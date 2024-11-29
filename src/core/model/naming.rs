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

use polaris_specification::v1::HeartbeatHealthCheck;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
        ServiceKey { namespace, name }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub id: String,
    pub namespace: String,
    pub name: String,
    pub metadata: HashMap<String, String>,
    pub revision: String,
}

#[derive(Default, Clone, Debug)]
pub struct ServiceInstances {
    pub service: ServiceInfo,
    pub instances: Vec<Instance>,
    pub total_weight: u64,
}

impl ServiceInstances {
    pub fn get_cache_key(&self) -> String {
        format!(
            "Instance-namespace={}-service={}",
            self.service.namespace, self.service.name
        )
    }

    pub fn new(svc_info: ServiceInfo, all_ins: Vec<Instance>) -> Self {
        let mut total_weight: u64 = 0;
        for (_, val) in all_ins.iter().enumerate() {
            total_weight += val.weight as u64;
        }

        Self {
            service: svc_info,
            instances: all_ins,
            total_weight,
        }
    }

    pub fn get_total_weight(&self) -> u64 {
        self.total_weight
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

    pub fn is_available(&self) -> bool {
        if self.weight == 0 {
            return false;
        }
        if !self.health {
            return false;
        }
        if self.isolated {
            return false;
        }
        true
    }

    pub fn convert_from_spec(data: polaris_specification::v1::Instance) -> Instance {
        let mut metadata = HashMap::<String, String>::new();
        for ele in data.metadata {
            metadata.insert(ele.0, ele.1);
        }
        let location = data.location.unwrap_or_default();

        Self {
            id: data.id.unwrap_or_default(),
            namespace: data.namespace.unwrap_or_default(),
            service: data.service.unwrap_or_default(),
            ip: data.host.unwrap_or_default(),
            port: data.port.unwrap_or_default(),
            vpc_id: data.vpc_id.unwrap_or_default(),
            version: data.version.unwrap_or_default(),
            protocol: data.protocol.unwrap_or_default(),
            health: data.healthy.unwrap_or(false),
            isolated: data.isolate.unwrap_or(false),
            weight: data.weight.unwrap_or(100),
            priority: data.priority.unwrap_or_default(),
            metadata,
            location: Location {
                region: location.region.unwrap_or_default(),
                zone: location.zone.unwrap_or_default(),
                campus: location.campus.unwrap_or_default(),
            },
            revision: data.revision.unwrap_or_default(),
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub region: String,
    pub zone: String,
    pub campus: String,
}

impl Location {
    pub fn clone(&self) -> Location {
        Self {
            region: self.region.clone(),
            zone: self.zone.clone(),
            campus: self.campus.clone(),
        }
    }

    pub fn convert_spec(&self) -> polaris_specification::v1::Location {
        polaris_specification::v1::Location {
            region: Some(self.region.clone()),
            zone: Some(self.zone.clone()),
            campus: Some(self.campus.clone()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.region.is_empty() && self.zone.is_empty() && self.campus.is_empty()
    }
}

pub struct ServiceRule {
    pub rules: Vec<Box<dyn Message>>,
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

#[derive(Clone, Debug)]
pub struct InstanceRequest {
    pub flow_id: String,
    pub ttl: u32,
    pub instance: Instance,
}

impl InstanceRequest {
    pub fn convert_beat_spec(&self) -> polaris_specification::v1::Instance {
        polaris_specification::v1::Instance {
            id: None,
            namespace: Some(self.instance.namespace.clone()),
            service: Some(self.instance.service.clone()),
            host: Some(self.instance.ip.clone()),
            port: Some(self.instance.port),
            vpc_id: Some(self.instance.vpc_id.clone()),
            protocol: None,
            version: None,
            priority: None,
            weight: None,
            enable_health_check: None,
            health_check: None,
            healthy: None,
            isolate: None,
            location: None,
            metadata: HashMap::new(),
            logic_set: None,
            ctime: None,
            mtime: None,
            revision: None,
            service_token: None,
        }
    }

    pub fn convert_spec(&self) -> polaris_specification::v1::Instance {
        let ttl = self.ttl;
        let mut enable_health_check = Some(false);
        let mut health_check = None;
        if ttl != 0 {
            enable_health_check = Some(true);
            health_check = Some(polaris_specification::v1::HealthCheck {
                r#type: i32::from(
                    polaris_specification::v1::health_check::HealthCheckType::Heartbeat,
                ),
                heartbeat: Some(HeartbeatHealthCheck { ttl: Some(ttl) }),
            });
        }

        let mut spec_ins = polaris_specification::v1::Instance {
            id: None,
            service: Some(self.instance.service.to_string()),
            namespace: Some(self.instance.namespace.to_string()),
            vpc_id: Some(self.instance.vpc_id.to_string()),
            host: Some(self.instance.ip.to_string()),
            port: Some(self.instance.port),
            protocol: Some(self.instance.protocol.to_string()),
            version: Some(self.instance.version.to_string()),
            priority: Some(self.instance.priority),
            weight: Some(self.instance.weight),
            enable_health_check,
            health_check,
            healthy: Some(self.instance.health),
            isolate: Some(self.instance.isolated),
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
            spec_ins.health_check = Some(polaris_specification::v1::HealthCheck {
                r#type: i32::from(
                    polaris_specification::v1::health_check::HealthCheckType::Heartbeat,
                ),
                heartbeat: Some(HeartbeatHealthCheck {
                    ttl: Some(self.ttl),
                }),
            });
        }
        spec_ins
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
            instance: Instance::default(),
        };
        ret.instance.id = id;
        ret
    }

    pub fn exist_resource() -> Self {
        Self {
            exist: true,
            instance: Instance::default(),
        }
    }
}

pub struct ServiceInstancesChangeEvent {
    pub service: ServiceInfo,
    pub instances: Vec<Instance>,
}

#[derive(Clone, Debug)]
pub struct ServiceContractRequest {
    pub flow_id: String,
    pub contract: ServiceContract,
}

#[derive(Clone, Debug)]
pub struct ServiceContract {
    pub name: String,
    // 所属命名空间
    pub namespace: String,
    // 所属服务名称
    pub service: String,
    // 契约版本
    pub version: String,
    // 协议，http/grpc/dubbo/thrift
    pub protocol: String,
    // 额外描述
    pub content: String,
    // 接口描述信息
    pub interfaces: Vec<ServiceInterfaceDescripitor>,
    // 标签
    pub metadata: HashMap<String, String>,
}

impl ServiceContract {
    pub fn parse_from_spec(spec: polaris_specification::v1::ServiceContract) -> Self {
        let mut interfaces = Vec::<ServiceInterfaceDescripitor>::new();
        for ele in spec.interfaces {
            interfaces.push(ServiceInterfaceDescripitor {
                name: ele.r#type.clone(),
                namespace: ele.namespace.clone(),
                service: ele.service.clone(),
                version: ele.version.clone(),
                protocol: ele.protocol.clone(),
                path: ele.path.clone(),
                method: ele.method.clone(),
                content: ele.content.clone(),
            });
        }
        Self {
            name: spec.name.clone(),
            namespace: spec.namespace.clone(),
            service: spec.service.clone(),
            content: spec.content.clone(),
            version: spec.version.clone(),
            protocol: spec.protocol.clone(),
            interfaces,
            metadata: HashMap::new(),
        }
    }

    pub fn convert_spec(&self) -> polaris_specification::v1::ServiceContract {
        let mut spec = polaris_specification::v1::ServiceContract {
            id: "".to_string(),
            name: self.name.clone(),
            namespace: self.namespace.clone(),
            service: self.service.clone(),
            content: self.content.clone(),
            version: self.version.clone(),
            protocol: self.protocol.clone(),
            interfaces: Vec::new(),
            status: "".to_string(),
            revision: "".to_string(),
            r#type: self.name.clone(),
            ctime: "".to_string(),
            mtime: "".to_string(),
            metadata: HashMap::new(),
        };
        for ele in self.interfaces.iter() {
            spec.interfaces
                .push(polaris_specification::v1::InterfaceDescriptor {
                    id: "".to_string(),
                    name: self.name.clone(),
                    namespace: ele.namespace.clone(),
                    service: ele.service.clone(),
                    version: ele.version.clone(),
                    content: ele.content.clone(),
                    path: ele.path.clone(),
                    method: ele.method.clone(),
                    protocol: ele.protocol.clone(),
                    source: polaris_specification::v1::interface_descriptor::Source::Client.into(),
                    revision: "".to_string(),
                    r#type: ele.name.clone(),
                    ctime: "".to_string(),
                    mtime: "".to_string(),
                });
        }
        spec
    }
}

/// ServiceInterfaceDescripitor 服务接口信息描述
#[derive(Clone, Debug)]
pub struct ServiceInterfaceDescripitor {
    // 接口类型
    pub name: String,
    // 所属命名空间
    pub namespace: String,
    // 所属服务名称
    pub service: String,
    // 契约版本
    pub version: String,
    // 协议，http/grpc/dubbo/thrift
    pub protocol: String,
    // 接口名称，http path/dubbo interface/grpc service
    pub path: String,
    // 方法名称，对应 http method/ dubbo interface func/grpc service func
    pub method: String,
    // 接口描述信息
    pub content: String,
}
