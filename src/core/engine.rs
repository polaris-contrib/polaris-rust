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
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use tokio::runtime::{Builder, Runtime};

use crate::core::config::config::Configuration;
use crate::core::model::cache::{EventType, ResourceEventKey};
use crate::core::model::error::PolarisError;
use crate::core::model::naming::InstanceRequest;
use crate::core::plugin::location::new_location_provider;
use crate::core::plugin::plugins::{
    init_resource_cache, init_server_connector, Extensions, PluginContainer,
};
use crate::discovery::req::{
    GetAllInstanceRequest, GetServiceRuleRequest, InstanceDeregisterRequest,
    InstanceHeartbeatRequest, InstanceRegisterRequest, InstanceRegisterResponse, InstancesResponse,
    ServiceRuleResponse,
};

use super::plugin::cache::{Filter, ResourceCache};
use super::plugin::connector::Connector;
use super::plugin::location::{LocationProvider, LocationSupplier};

static SEQ: AtomicU64 = AtomicU64::new(1);

pub struct Engine
where
    Self: Send + Sync,
{
    runtime: Arc<Runtime>,
    local_cache: Arc<Box<dyn ResourceCache>>,
    server_connector: Arc<Box<dyn Connector>>,
    location_provider: Arc<LocationProvider>,
}

impl Engine {
    pub fn new(conf: Configuration) -> Result<Self, PolarisError> {
        let runtime = Arc::new(
            Builder::new_multi_thread()
                .enable_all()
                .thread_name("polaris-client-thread-pool")
                .worker_threads(4)
                .build()
                .unwrap(),
        );

        let arc_conf = Arc::new(conf);
        let client_id = crate::core::plugin::plugins::acquire_client_id(arc_conf.clone());

        let mut containers = PluginContainer::default();
        // 初始化所有的插件
        let start_time = std::time::Instant::now();
        containers.register_all_plugin();
        tracing::info!("register_all_plugin cost: {:?}", start_time.elapsed());

        // 初始化 extensions
        let extensions_ret = Extensions::build(client_id, arc_conf.clone(), runtime.clone());
        if extensions_ret.is_err() {
            return Err(extensions_ret.err().unwrap());
        }
        let extension = extensions_ret.unwrap();
        let arc_exts = Arc::new(extension.clone());

        // 初始化 server_connector
        let connector_opt = &arc_conf.global.server_connectors;
        let connector_ret = init_server_connector(connector_opt, &containers, arc_exts.clone());
        if connector_ret.is_err() {
            return Err(connector_ret.err().unwrap());
        }
        let arc_connector = Arc::new(connector_ret.unwrap());

        // 初始化 local_cache
        let mut copy_ext = extension.clone();
        copy_ext.server_connector = Some(arc_connector.clone());
        let cache_ret = init_resource_cache(
            &arc_conf.global.local_cache,
            &containers,
            Arc::new(copy_ext),
        );
        if cache_ret.is_err() {
            return Err(cache_ret.err().unwrap());
        }

        // 初始化 location_provider
        let location_provider_ret = new_location_provider(&arc_conf.global.location);
        if location_provider_ret.is_err() {
            return Err(location_provider_ret.err().unwrap());
        }

        Ok(Self {
            runtime,
            local_cache: Arc::new(cache_ret.unwrap()),
            server_connector: arc_connector,
            location_provider: Arc::new(location_provider_ret.unwrap()),
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

        let ret = self.local_cache.load_service_instances(Filter {
            resource_key: ResourceEventKey {
                namespace: req.namespace.clone(),
                event_type: EventType::Instance,
                filter: filter,
            },
            internal_request: false,
            include_cache: true,
            timeout: req.timeout,
        }).await;

        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        let svc_ins = ret.unwrap();

        if only_available {
            Ok(InstancesResponse {
                service_info: svc_ins.service,
                instances: svc_ins.available_instances,
            })
        } else {
            Ok(InstancesResponse {
                service_info: svc_ins.service,
                instances: svc_ins.instances,
            })
        }
    }

    /// get_service_rule 获取服务规则
    pub async fn get_service_rule(
        &self,
        req: GetServiceRuleRequest,
    ) -> Result<ServiceRuleResponse, PolarisError> {
        let local_cache = self.local_cache.clone();
        let mut filter = HashMap::<String, String>::new();
        filter.insert("service".to_string(), req.service.clone());
        let ret = local_cache.load_service_rule(Filter {
            resource_key: ResourceEventKey {
                namespace: req.namespace.clone(),
                event_type: req.rule_type.to_event_type(),
                filter,
            },
            internal_request: false,
            include_cache: true,
            timeout: req.timeout,
        }).await;

        if ret.is_err() {
            return Err(ret.err().unwrap());
        }

        Ok(ServiceRuleResponse {
            rules: ret.unwrap().rules,
        })
    }

    pub fn get_executor(&self) -> Arc<Runtime> {
        self.runtime.clone()
    }
}
