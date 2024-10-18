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
use tokio::sync::RwLock;

use crate::config::req::{
    CreateConfigFileRequest, GetConfigFileRequest, PublishConfigFileRequest,
    UpdateConfigFileRequest,
};
use crate::core::config::config::Configuration;
use crate::core::model::cache::{EventType, ResourceEventKey};
use crate::core::model::error::PolarisError;
use crate::core::model::naming::InstanceRequest;
use crate::core::plugin::location::new_location_provider;
use crate::core::plugin::plugins::Extensions;
use crate::discovery::req::{
    GetAllInstanceRequest, GetServiceRuleRequest, InstanceDeregisterRequest,
    InstanceHeartbeatRequest, InstanceRegisterRequest, InstanceRegisterResponse, InstancesResponse,
    ServiceRuleResponse,
};

use super::model::config::ConfigFile;
use super::model::naming::ServiceInstances;
use super::plugin::cache::{Filter, ResourceCache, ResourceListener};
use super::plugin::connector::Connector;
use super::plugin::loadbalance::LoadBalancer;
use super::plugin::location::{LocationProvider, LocationSupplier};

pub struct Engine
where
    Self: Send + Sync,
{
    runtime: Arc<Runtime>,
    local_cache: Arc<Box<dyn ResourceCache>>,
    server_connector: Arc<Box<dyn Connector>>,
    location_provider: Arc<LocationProvider>,
    load_balancer: Arc<RwLock<HashMap<String, Arc<Box<dyn LoadBalancer>>>>>,
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

        let client_id = crate::core::plugin::plugins::acquire_client_id(arc_conf.clone());

        // 初始化 extensions
        let extensions_ret = Extensions::build(client_id, arc_conf.clone(), runtime.clone());
        if extensions_ret.is_err() {
            return Err(extensions_ret.err().unwrap());
        }
        let mut extension = extensions_ret.unwrap();

        let ret = extension.load_config_file_filters(&arc_conf.config.config_filter);
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        // 初始化 server_connector
        let ret = extension.load_server_connector(&arc_conf.global.server_connectors);
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }
        let server_connector = ret.unwrap();

        // 初始化 local_cache
        let ret = extension.load_resource_cache(&arc_conf.global.local_cache);
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }
        let local_cache = ret.unwrap();

        // 初始化 location_provider
        let ret = new_location_provider(&arc_conf.global.location);
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }
        let location_provider = ret.unwrap();

        Ok(Self {
            runtime,
            local_cache: Arc::new(local_cache),
            server_connector: server_connector,
            location_provider: Arc::new(location_provider),
            load_balancer: Arc::new(RwLock::new(extension.load_loadbalancers())),
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
                ttl: req.ttl.clone(),
                instance: instance,
            })
            .await;

        return match rsp {
            Ok(ins_rsp) => Ok(InstanceRegisterResponse {
                instance_id: ins_rsp.instance.id.clone(),
                exist: ins_rsp.exist.clone(),
            }),
            Err(err) => Err(err),
        };
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

        return match rsp {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        };
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

        return match rsp {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        };
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
                    filter: filter,
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

        return match rsp {
            Ok(ret_rsp) => Ok(ret_rsp),
            Err(err) => Err(err),
        };
    }

    /// update_config_file 更新配置文件
    pub async fn update_config_file(
        &self,
        req: UpdateConfigFileRequest,
    ) -> Result<bool, PolarisError> {
        let config_file = req.to_config_request();

        let connector = self.server_connector.clone();
        let rsp = connector.update_config_file(config_file).await;

        return match rsp {
            Ok(ret_rsp) => Ok(ret_rsp),
            Err(err) => Err(err),
        };
    }

    /// publish_config_file 发布配置文件
    pub async fn publish_config_file(
        &self,
        req: PublishConfigFileRequest,
    ) -> Result<bool, PolarisError> {
        let config_file = req.to_config_request();

        let connector = self.server_connector.clone();
        let rsp = connector.release_config_file(config_file).await;

        return match rsp {
            Ok(ret_rsp) => Ok(ret_rsp),
            Err(err) => Err(err),
        };
    }

    pub async fn lookup_loadbalancer(&self, name: &str) -> Option<Arc<Box<dyn LoadBalancer>>> {
        let lb = self.load_balancer.read().await;
        lb.get(name).map(|lb| lb.clone())
    }

    /// register_resource_listener 注册资源监听器
    pub async fn register_resource_listener(&self, listener: Arc<dyn ResourceListener>) {
        self.local_cache.register_resource_listener(listener).await;
    }

    pub fn get_executor(&self) -> Arc<Runtime> {
        self.runtime.clone()
    }
}
