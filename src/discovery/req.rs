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

use crate::core::model::naming::{Instance};
use crate::core::model::router::{CalleeInfo, CallerInfo};

pub struct InstanceRegisterRequest {
    pub flow_id: String,
    pub timeout_ms: u32,
    pub instance: Instance,
}

pub struct InstanceRegisterResponse {
    pub instance_id: String,
    pub exist: bool,
}

pub struct InstanceDeregisterRequest {
    pub flow_id: String,
    pub timeout_ms: u32,
    pub instance: Instance,
}

pub struct InstanceHeartbeatRequest {
    pub flow_id: String,
    pub timeout_ms: u32,
    pub instance: Instance,
}

pub struct ReportServiceContractRequest {

}

// ConsumerAPI request and response definition

pub struct GetOneInstanceRequest {
    pub flow_id: String,
    pub timeout_ms: u32,
    pub service: String,
    pub namespace: String,
    pub caller_info: CallerInfo,
    pub callee_info: CalleeInfo,
}

pub struct GetHealthInstanceRequest {
    pub flow_id: String,
    pub timeout_ms: u32,
    pub service: String,
    pub namespace: String,
}

pub struct GetAllInstanceRequest {
    pub flow_id: String,
    pub timeout_ms: u32,
    pub service: String,
    pub namespace: String,
}

pub struct InstancesResponse {
    pub instances: Vec<Instance>,
}

pub struct WatchInstanceRequest {
    pub namespace: String,
    pub service: String,
}

pub struct WatchInstanceResponse {

}

pub struct UnWatchInstanceRequest {
    pub namespace: String,
    pub service: String,
}

pub struct UnWatchInstanceResponse {

}

pub struct ServiceCallResult {

}

pub struct GetServiceRuleRequest {

}

pub struct ServiceRuleResponse {

}

// LossLessAPI request and response definition

pub struct InstanceProperties {

}

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
