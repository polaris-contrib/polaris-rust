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

use tokio::runtime::{Builder, Runtime};

use super::flow::{CircuitBreakerFlow, ClientFlow, RouterFlow};
use super::model::config::{ConfigFile, ConfigGroup};
use super::model::naming::{ServiceContractRequest, ServiceInstances};
use super::model::ClientContext;
use super::plugin::cache::{Filter, ResourceCache, ResourceListener};
use super::plugin::connector::Connector;
use super::plugin::location::{LocationProvider, LocationSupplier};
use crate::config::req::{
    CreateConfigFileRequest, GetConfigFileRequest, GetConfigGroupRequest, PublishConfigFileRequest,
    UpdateConfigFileRequest, UpsertAndPublishConfigFileRequest,
};
use crate::core::config::config::Configuration;
use crate::core::model::cache::{EventType, ResourceEventKey};
use crate::core::model::error::PolarisError;
use crate::core::model::naming::InstanceRequest;
use crate::core::plugin::plugins::Extensions;
use crate::discovery::req::{
    GetAllInstanceRequest, GetServiceRuleRequest, InstanceDeregisterRequest,
    InstanceHeartbeatRequest, InstanceRegisterRequest, InstanceRegisterResponse, InstancesResponse,
    ReportServiceContractRequest, ServiceRuleResponse,
};

pub struct Engine
where
    Self: Send + Sync,
{
    extensions: Arc<Extensions>,
    runtime: Arc<Runtime>,
    local_cache: Arc<Box<dyn ResourceCache>>,
    server_connector: Arc<Box<dyn Connector>>,
    location_provider: Arc<LocationProvider>,
    client_ctx: Arc<ClientContext>,
    client_flow: ClientFlow,
}

impl Engine {
    pub fn new(arc_conf: Arc<Configuration>) -> Result<Self, PolarisError> {
        let runtime = Arc::new(
            Builder::new_multi_thread()
                .enable_all()
                .thread_name("polaris-client-thread-pool")
                .worker_threads(4)
                .build()
                .unwrap(),
        );
        let client_ctx = crate::core::plugin::plugins::acquire_client_context(arc_conf.clone());
        let client_ctx = Arc::new(client_ctx);

        // 初始化 extensions
        let extensions_ret =
            Extensions::build(client_ctx.clone(), arc_conf.clone(), runtime.clone());
        if extensions_ret.is_err() {
            return Err(extensions_ret.err().unwrap());
        }

        let extension = Arc::new(extensions_ret.unwrap());
        let server_connector = extension.get_server_connector();
        let local_cache = extension.get_resource_cache();
        let location_provider = extension.get_location_provider();

        let mut client_flow = ClientFlow::new(client_ctx.clone(), extension.clone());
        client_flow.run_flow();

        Ok(Self {
            extensions: extension.clone(),
            runtime,
            local_cache,
            server_connector,
            location_provider: location_provider,
            client_ctx: client_ctx,
            client_flow,
        })
    }

    /// register_instance 同步注册实例
    pub async fn register_instance(
        &self,
        req: InstanceRegisterRequest,
    ) -> Result<InstanceRegisterResponse, PolarisError> {
        let mut instance = req.convert_instance();

        if instance.location.is_empty() {
            instance.location = self.location_provider.get_location();
        }

        let connector = self.server_connector.clone();
        let rsp = connector
            .register_instance(InstanceRequest {
                flow_id: {
                    let mut flow_id = req.flow_id.clone();
                    if flow_id.is_empty() {
                        flow_id = uuid::Uuid::new_v4().to_string();
                    }
                    flow_id
                },
                ttl: req.ttl,
                instance,
            })
            .await;

        match rsp {
            Ok(ins_rsp) => Ok(InstanceRegisterResponse {
                instance_id: ins_rsp.instance.id.clone(),
                exist: ins_rsp.exist,
            }),
            Err(err) => Err(err),
        }
    }

    /// deregister_instance 同步注销实例
    pub async fn deregister_instance(
        &self,
        req: InstanceDeregisterRequest,
    ) -> Result<(), PolarisError> {
        let connector = self.server_connector.clone();
        let rsp = connector
            .deregister_instance(InstanceRequest {
                flow_id: {
                    let mut flow_id = req.flow_id.clone();
                    if flow_id.is_empty() {
                        flow_id = uuid::Uuid::new_v4().to_string();
                    }
                    flow_id
                },
                ttl: 0,
                instance: req.convert_instance(),
            })
            .await;

        match rsp {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }

    /// instance_heartbeat 同步实例心跳
    pub async fn instance_heartbeat(
        &self,
        req: InstanceHeartbeatRequest,
    ) -> Result<(), PolarisError> {
        let connector = self.server_connector.clone();
        let rsp = connector
            .heartbeat_instance(InstanceRequest {
                flow_id: {
                    let mut flow_id = req.flow_id.clone();
                    if flow_id.is_empty() {
                        flow_id = uuid::Uuid::new_v4().to_string();
                    }
                    flow_id
                },
                ttl: 0,
                instance: req.convert_instance(),
            })
            .await;

        match rsp {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }

    /// get_service_instances 获取服务实例
    pub async fn get_service_instances(
        &self,
        req: GetAllInstanceRequest,
        only_available: bool,
    ) -> Result<InstancesResponse, PolarisError> {
        let mut filter = HashMap::<String, String>::new();
        filter.insert("service".to_string(), req.service.clone());

        let ret = self
            .local_cache
            .load_service_instances(Filter {
                resource_key: ResourceEventKey {
                    namespace: req.namespace.clone(),
                    event_type: EventType::Instance,
                    filter,
                },
                internal_request: false,
                include_cache: true,
                timeout: req.timeout,
            })
            .await;

        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        let svc_ins = ret.unwrap();

        Ok(InstancesResponse {
            instances: ServiceInstances::new(
                svc_ins.get_service_info(),
                svc_ins.list_instances(only_available).await,
            ),
        })
    }

    /// report_service_contract 上报服务契约数据
    pub async fn report_service_contract(
        &self,
        req: ReportServiceContractRequest,
    ) -> Result<bool, PolarisError> {
        let connector = self.server_connector.clone();
        return connector
            .report_service_contract(ServiceContractRequest {
                flow_id: {
                    let mut flow_id = req.flow_id.clone();
                    if flow_id.is_empty() {
                        flow_id = uuid::Uuid::new_v4().to_string();
                    }
                    flow_id
                },
                contract: req.contract,
            })
            .await;
    }

    /// get_service_rule 获取服务规则
    pub async fn get_service_rule(
        &self,
        req: GetServiceRuleRequest,
    ) -> Result<ServiceRuleResponse, PolarisError> {
        let local_cache = self.local_cache.clone();
        let mut filter = HashMap::<String, String>::new();
        filter.insert("service".to_string(), req.service.clone());
        let ret = local_cache
            .load_service_rule(Filter {
                resource_key: ResourceEventKey {
                    namespace: req.namespace.clone(),
                    event_type: req.rule_type.to_event_type(),
                    filter,
                },
                internal_request: false,
                include_cache: true,
                timeout: req.timeout,
            })
            .await;

        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        Ok(ServiceRuleResponse {
            rules: ret.unwrap().rules,
        })
    }

    /// get_config_file 获取配置文件
    pub async fn get_config_file(
        &self,
        req: GetConfigFileRequest,
    ) -> Result<ConfigFile, PolarisError> {
        let local_cache = self.local_cache.clone();
        let mut filter = HashMap::<String, String>::new();
        filter.insert("group".to_string(), req.group.clone());
        filter.insert("file".to_string(), req.file.clone());
        let ret = local_cache
            .load_config_file(Filter {
                resource_key: ResourceEventKey {
                    namespace: req.namespace.clone(),
                    event_type: EventType::ConfigFile,
                    filter,
                },
                internal_request: false,
                include_cache: true,
                timeout: req.timeout,
            })
            .await;

        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        Ok(ret.unwrap())
    }

    /// create_config_file 创建配置文件
    pub async fn create_config_file(
        &self,
        req: CreateConfigFileRequest,
    ) -> Result<bool, PolarisError> {
        let config_file = req.to_config_request();

        let connector = self.server_connector.clone();
        let rsp = connector.create_config_file(config_file).await;

        match rsp {
            Ok(ret_rsp) => Ok(ret_rsp),
            Err(err) => Err(err),
        }
    }

    /// update_config_file 更新配置文件
    pub async fn update_config_file(
        &self,
        req: UpdateConfigFileRequest,
    ) -> Result<bool, PolarisError> {
        let config_file = req.to_config_request();

        let connector = self.server_connector.clone();
        let rsp = connector.update_config_file(config_file).await;

        match rsp {
            Ok(ret_rsp) => Ok(ret_rsp),
            Err(err) => Err(err),
        }
    }

    /// publish_config_file 发布配置文件
    pub async fn publish_config_file(
        &self,
        req: PublishConfigFileRequest,
    ) -> Result<bool, PolarisError> {
        let config_file = req.to_config_request();

        let connector = self.server_connector.clone();
        let rsp = connector.release_config_file(config_file).await;

        match rsp {
            Ok(ret_rsp) => Ok(ret_rsp),
            Err(err) => Err(err),
        }
    }

    /// upsert_publish_config_file 更新或发布配置文件
    pub async fn upsert_publish_config_file(
        &self,
        req: UpsertAndPublishConfigFileRequest,
    ) -> Result<bool, PolarisError> {
        let config_file = req.to_config_request();

        let connector = self.server_connector.clone();
        let rsp = connector.upsert_publish_config_file(config_file).await;

        match rsp {
            Ok(ret_rsp) => Ok(ret_rsp),
            Err(err) => Err(err),
        }
    }

    pub async fn get_config_group_files(
        &self,
        req: GetConfigGroupRequest,
    ) -> Result<ConfigGroup, PolarisError> {
        let local_cache = self.local_cache.clone();
        let mut filter = HashMap::<String, String>::new();
        filter.insert("group".to_string(), req.group.clone());
        let ret = local_cache
            .load_config_group_files(Filter {
                resource_key: ResourceEventKey {
                    namespace: req.namespace.clone(),
                    event_type: EventType::ConfigGroup,
                    filter,
                },
                internal_request: false,
                include_cache: true,
                timeout: req.timeout,
            })
            .await;

        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        Ok(ret.unwrap())
    }

    /// register_resource_listener 注册资源监听器
    pub async fn register_resource_listener(&self, listener: Arc<dyn ResourceListener>) {
        self.local_cache.register_resource_listener(listener).await;
    }

    pub fn get_executor(&self) -> Arc<Runtime> {
        self.runtime.clone()
    }

    pub fn get_extensions(&self) -> Arc<Extensions> {
        self.extensions.clone()
    }
}
