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
use std::sync::Arc;
use crate::core::model::error::{ErrorCode, PolarisError};
use crate::core::model::naming::{InstanceRequest, InstanceResponse};
use crate::core::plugin::plugins::{Extensions, Plugin};

pub trait Connector: Plugin {

    // register_resource_handler 注册资源处理器
    fn register_resource_handler(&self) -> Result<bool, PolarisError>;

    // deregister_resource_handler
    fn deregister_resource_handler(&self) -> Result<bool, PolarisError>;

    fn register_instance(&mut self, req: InstanceRequest) -> Result<InstanceResponse, PolarisError>;

    fn deregister_instance(&mut self, req: InstanceRequest) -> Result<bool, PolarisError>;

    fn heartbeat_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError>;

    fn report_client(&self) -> Result<bool, PolarisError>;

    fn report_service_contract(&self) -> Result<bool, PolarisError>;

    fn get_service_contract(&self) -> Result<String, PolarisError>;

    fn create_config_file(&self) -> Result<bool, PolarisError>;

    fn update_config_file(&self) -> Result<bool, PolarisError>;

    fn release_config_file(&self) -> Result<bool, PolarisError>;

    fn upsert_publish_config_file(&self) -> Result<bool, PolarisError>;
}

pub struct NoopConnector {

}

impl Default for NoopConnector {
    fn default() -> Self {
        Self{}
    }
}

impl Plugin for NoopConnector {
    fn init(&self, options: HashMap<String, String>, extensions: Arc<Extensions>) {
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

    fn register_instance(&mut self, req: InstanceRequest) -> Result<InstanceResponse, PolarisError> {
        todo!()
    }

    fn deregister_instance(&mut self, req: InstanceRequest) -> Result<bool, PolarisError> {
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
