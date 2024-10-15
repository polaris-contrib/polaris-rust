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

use crate::core::model::cache::{RemoteData, ResourceEventKey, ServerEvent};
use crate::core::model::config::ConfigFileRequest;
use crate::core::model::error::PolarisError;
use crate::core::model::naming::{InstanceRequest, InstanceResponse};
use crate::core::plugin::plugins::{Extensions, Plugin};

pub trait ResourceHandler: Send + Sync {
    // handle_event 处理资源事件
    fn handle_event(&self, event: RemoteData);
    /// interest_resource 获取感兴趣的资源
    fn interest_resource(&self) -> ResourceEventKey;
}

#[async_trait::async_trait]
pub trait Connector: Plugin {
    /// register_resource_handler 注册资源处理器
    async fn register_resource_handler(
        &self,
        handler: Box<dyn ResourceHandler>,
    ) -> Result<bool, PolarisError>;

    /// register_instance: 实例注册回调函数
    async fn register_instance(
        &self,
        req: InstanceRequest,
    ) -> Result<InstanceResponse, PolarisError>;

    /// deregister_instance 实例注销回调函数
    async fn deregister_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError>;

    /// heartbeat_instance 实例心跳回调函数
    async fn heartbeat_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError>;

    /// report_client 上报客户端信息
    async fn report_client(&self) -> Result<bool, PolarisError>;

    /// report_service_contract 上报服务契约
    async fn report_service_contract(&self) -> Result<bool, PolarisError>;

    /// get_service_contract 获取服务契约
    async fn get_service_contract(&self) -> Result<String, PolarisError>;

    /// create_config_file 创建配置文件
    async fn create_config_file(&self, req: ConfigFileRequest) -> Result<bool, PolarisError>;

    /// update_config_file 更新配置文件
    async fn update_config_file(&self, req: ConfigFileRequest) -> Result<bool, PolarisError>;

    /// release_config_file 删除配置文件
    async fn delete_config_file(&self, req: ConfigFileRequest) -> Result<bool, PolarisError>;

    /// release_config_file 删除配置文件
    async fn release_config_file(&self, req: ConfigFileRequest) -> Result<bool, PolarisError>;

    /// upsert_publish_config_file 更新发布配置文件
    async fn upsert_publish_config_file(&self) -> Result<bool, PolarisError>;
}

pub struct NoopConnector {}

impl Default for NoopConnector {
    fn default() -> Self {
        Self {}
    }
}

impl Plugin for NoopConnector {
    fn init(&mut self) {
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
        handler: Box<dyn ResourceHandler>,
    ) -> Result<bool, PolarisError> {
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

    async fn create_config_file(&self, req: ConfigFileRequest) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn update_config_file(&self, req: ConfigFileRequest) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn delete_config_file(&self, req: ConfigFileRequest) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn release_config_file(&self, req: ConfigFileRequest) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn upsert_publish_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }
}
