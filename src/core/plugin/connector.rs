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

use std::sync::Arc;

use crate::core::model::error::PolarisError;
use crate::core::model::naming::{InstanceRequest, InstanceResponse};
use crate::core::plugin::plugins::{Extensions, Plugin};

pub trait ResourceHandler: Send + Sync {}

#[async_trait::async_trait]
pub trait Connector: Plugin + Send + Sync {
    /// register_resource_handler 注册资源处理器
    async fn register_resource_handler(
        &self,
        handler: Arc<dyn ResourceHandler>,
    ) -> Result<bool, PolarisError>;

    /// deregister_resource_handler
    async fn deregister_resource_handler(&self) -> Result<bool, PolarisError>;

    /// register_instance: 实例注册回调函数
    async fn register_instance(
        &self,
        req: InstanceRequest,
    ) -> Result<InstanceResponse, PolarisError>;

    /// deregister_instance
    async fn deregister_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError>;

    /// heartbeat_instance
    async fn heartbeat_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError>;

    /// report_client
    async fn report_client(&self) -> Result<bool, PolarisError>;

    /// report_service_contract
    async fn report_service_contract(&self) -> Result<bool, PolarisError>;

    /// get_service_contract
    async fn get_service_contract(&self) -> Result<String, PolarisError>;

    /// create_config_file
    async fn create_config_file(&self) -> Result<bool, PolarisError>;

    /// update_config_file
    async fn update_config_file(&self) -> Result<bool, PolarisError>;

    /// release_config_file
    async fn release_config_file(&self) -> Result<bool, PolarisError>;

    /// upsert_publish_config_file
    async fn upsert_publish_config_file(&self) -> Result<bool, PolarisError>;
}

pub struct NoopConnector {}

impl Default for NoopConnector {
    fn default() -> Self {
        Self {}
    }
}

impl Plugin for NoopConnector {
    fn init(&mut self, extensions: Arc<Extensions>) {
        todo!()
    }

    fn destroy(&self) {
        todo!()
    }

    fn name(&self) -> String {
        todo!()
    }
}

#[async_trait::async_trait]
impl Connector for NoopConnector {
    async fn register_resource_handler(
        &self,
        handler: Arc<dyn ResourceHandler>,
    ) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn deregister_resource_handler(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn register_instance(
        &self,
        req: InstanceRequest,
    ) -> Result<InstanceResponse, PolarisError> {
        todo!()
    }

    async fn deregister_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn heartbeat_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn report_client(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn report_service_contract(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn get_service_contract(&self) -> Result<String, PolarisError> {
        todo!()
    }

    async fn create_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn update_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn release_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn upsert_publish_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }
}
