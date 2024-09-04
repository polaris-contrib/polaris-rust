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

use crate::core::model::error::PolarisError;
use crate::core::model::naming::{InstanceRequest, InstanceResponse};
use crate::core::plugin::plugins::{Extensions, Plugin};
use std::collections::HashMap;

pub trait Connector: Plugin {
    /// register_resource_handler 注册资源处理器
    fn register_resource_handler(&self) -> Result<bool, PolarisError>;

    /// deregister_resource_handler
    fn deregister_resource_handler(&self) -> Result<bool, PolarisError>;

    /// register_instance: 实例注册回调函数
    fn register_instance(&self, req: InstanceRequest) -> Result<InstanceResponse, PolarisError>;

    /// deregister_instance
    fn deregister_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError>;

    /// heartbeat_instance
    fn heartbeat_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError>;

    /// report_client
    fn report_client(&self) -> Result<bool, PolarisError>;

    /// report_service_contract
    fn report_service_contract(&self) -> Result<bool, PolarisError>;

    /// get_service_contract
    fn get_service_contract(&self) -> Result<String, PolarisError>;

    /// create_config_file
    fn create_config_file(&self) -> Result<bool, PolarisError>;

    /// update_config_file
    fn update_config_file(&self) -> Result<bool, PolarisError>;

    /// release_config_file
    fn release_config_file(&self) -> Result<bool, PolarisError>;

    /// upsert_publish_config_file
    fn upsert_publish_config_file(&self) -> Result<bool, PolarisError>;
}

pub struct NoopConnector {}

impl Default for NoopConnector {
    fn default() -> Self {
        Self {}
    }
}

impl Plugin for NoopConnector {
    fn init(&mut self, extensions: Extensions) {
        todo!()
    }

    fn destroy(&self) {
        todo!()
    }

    fn name(&self) -> String {
        todo!()
    }
}

impl Connector for NoopConnector {
    fn register_resource_handler(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn deregister_resource_handler(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn register_instance(&self, req: InstanceRequest) -> Result<InstanceResponse, PolarisError> {
        todo!()
    }

    fn deregister_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError> {
        todo!()
    }

    fn heartbeat_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError> {
        todo!()
    }

    fn report_client(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn report_service_contract(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn get_service_contract(&self) -> Result<String, PolarisError> {
        todo!()
    }

    fn create_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn update_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn release_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn upsert_publish_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }
}
