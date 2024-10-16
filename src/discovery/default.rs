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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::core::context::SDKContext;
use crate::core::model::error::PolarisError;
use crate::core::model::naming::{ServiceInstances, ServiceInstancesChangeEvent};
use crate::core::plugin::cache::ResourceListener;
use crate::discovery::api::{ConsumerAPI, LosslessAPI, ProviderAPI};
use crate::discovery::req::{
    BaseInstance, GetAllInstanceRequest, GetHealthInstanceRequest, GetOneInstanceRequest,
    GetServiceRuleRequest, InstanceDeregisterRequest, InstanceHeartbeatRequest,
    InstanceRegisterRequest, InstanceRegisterResponse, InstancesResponse, LosslessActionProvider,
    ReportServiceContractRequest, ServiceCallResult, ServiceRuleResponse, WatchInstanceRequest,
};
use crate::router::api::{LoadBalanceAPI, RouterAPI};
use crate::router::default::{DefaultLoadBalancerAPI, DefaultRouterAPI};
use crate::router::req::{ProcessLoadBalanceRequest, ProcessRouteRequest};

use super::req::WatchInstanceResponse;

struct InstanceWatcher {
    req: WatchInstanceRequest,
}

struct InstanceResourceListener {
    // watchers: namespace#service -> InstanceWatcher
    watchers: Arc<RwLock<HashMap<String, Vec<InstanceWatcher>>>>,
}

#[async_trait::async_trait]
impl ResourceListener for InstanceResourceListener {
    async fn on_event(
        &self,
        _action: crate::core::plugin::cache::Action,
        val: crate::core::model::cache::ServerEvent,
    ) {
        let event_key = val.event_key;
        let mut watch_key = event_key.namespace.clone();
        let service = event_key.filter.get("service");
        watch_key.push_str("#");
        watch_key.push_str(service.unwrap().as_str());

        let watchers = self.watchers.read().await;
        if let Some(watchers) = watchers.get(&watch_key) {
            let ins_cache_opt = val.value.to_service_instances();
            match ins_cache_opt {
                Some(ins_cache_val) => {
                    for watcher in watchers {
                        (watcher.req.call_back)(ServiceInstancesChangeEvent {
                            service: ins_cache_val.get_service_info(),
                            instances: ins_cache_val.list_instances().await,
                        })
                    }
                }
                None => {
                    // do nothing
                }
            }
        }
    }

    fn watch_key(&self) -> crate::core::model::cache::EventType {
        crate::core::model::cache::EventType::Instance
    }
}

/// DefaultConsumerAPI
pub struct DefaultConsumerAPI {
    context: Arc<SDKContext>,
    router_api: Box<DefaultRouterAPI>,
    loadbalance_api: Box<DefaultLoadBalancerAPI>,
    // watchers: namespace#service -> InstanceWatcher
    watchers: Arc<InstanceResourceListener>,
    // register_resource_watcher: 是否已经注册资源监听器
    register_resource_watcher: AtomicBool,
}

impl DefaultConsumerAPI {
    pub fn new(context: Arc<SDKContext>) -> Self {
        Self {
            context: context.clone(),
            router_api: Box::new(DefaultRouterAPI::default()),
            loadbalance_api: Box::new(DefaultLoadBalancerAPI::default()),
            watchers: Arc::new(InstanceResourceListener {
                watchers: Arc::new(RwLock::new(HashMap::new())),
            }),
            register_resource_watcher: AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl ConsumerAPI for DefaultConsumerAPI {
    async fn get_one_instance(
        &self,
        req: GetOneInstanceRequest,
    ) -> Result<InstancesResponse, PolarisError> {
        let check_ret = req.check_valid();
        if let Err(e) = check_ret {
            return Err(e);
        }

        let engine = self.context.get_engine();
        let rsp = engine
            .get_service_instances(
                GetAllInstanceRequest {
                    flow_id: req.flow_id.clone(),
                    timeout: req.timeout.clone(),
                    service: req.service.clone(),
                    namespace: req.namespace.clone(),
                },
                true,
            )
            .await;

        match rsp {
            Ok(mut rsp) => {
                let instances = rsp.instances;

                let criteria = req.caller_info.clone().criteria;

                // 执行路由逻辑
                let route_ret = self
                    .router_api
                    .router(ProcessRouteRequest {
                        service_instances: ServiceInstances {
                            service: rsp.service_info.clone(),
                            available_instances: instances,
                            instances: vec![],
                        },
                        caller_info: req.caller_info,
                        callee_info: req.callee_info,
                    })
                    .await;

                // 执行负载均衡逻辑
                let balance_ret = self
                    .loadbalance_api
                    .load_balance(ProcessLoadBalanceRequest {
                        service_instances: route_ret.service_instances,
                        criteria: criteria,
                    })
                    .await;
                rsp.instances = vec![balance_ret.instance];
                Ok(rsp)
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    async fn get_health_instance(
        &self,
        req: GetHealthInstanceRequest,
    ) -> Result<InstancesResponse, PolarisError> {
        let check_ret = req.check_valid();
        if let Err(e) = check_ret {
            return Err(e);
        }

        let engine = self.context.get_engine();
        let rsp: Result<InstancesResponse, PolarisError> = engine
            .get_service_instances(
                GetAllInstanceRequest {
                    flow_id: req.flow_id,
                    timeout: req.timeout,
                    service: req.service,
                    namespace: req.namespace,
                },
                true,
            )
            .await;

        match rsp {
            Ok(rsp) => Ok(rsp),
            Err(e) => Err(e),
        }
    }

    async fn get_all_instance(
        &self,
        req: GetAllInstanceRequest,
    ) -> Result<InstancesResponse, PolarisError> {
        let check_ret = req.check_valid();
        if let Err(e) = check_ret {
            return Err(e);
        }

        let engine = self.context.get_engine();
        let rsp = engine.get_service_instances(req, false).await;

        match rsp {
            Ok(rsp) => Ok(rsp),
            Err(e) => Err(e),
        }
    }

    async fn watch_instance(&self, req: WatchInstanceRequest) -> Result<WatchInstanceResponse, PolarisError> {
        if self
            .register_resource_watcher
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::SeqCst)
            .is_ok()
        {
            // 延迟注册资源监听器
            self.context
                .get_engine()
                .register_resource_listener(self.watchers.clone())
                .await;
        }

        let mut watchers = self.watchers.watchers.write().await;

        let watch_key = req.get_key();
        let items = watchers
            .entry(watch_key.clone())
            .or_insert_with(|| Vec::new());

        items.push(InstanceWatcher { req });
        Ok(WatchInstanceResponse{})
    }

    async fn get_service_rule(
        &self,
        req: GetServiceRuleRequest,
    ) -> Result<ServiceRuleResponse, PolarisError> {
        let engine = self.context.get_engine();
        engine.get_service_rule(req).await
    }

    async fn report_service_call(&self, req: ServiceCallResult) {
        todo!()
    }
}

/// DefaultProviderAPI
pub struct DefaultProviderAPI
where
    Self: Send + Sync,
{
    manage_sdk: bool,
    context: Arc<SDKContext>,
    beat_tasks: Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
}

impl DefaultProviderAPI {
    pub fn new(context: Arc<SDKContext>, manage_sdk: bool) -> Self {
        Self {
            context,
            manage_sdk: manage_sdk,
            beat_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl ProviderAPI for DefaultProviderAPI {
    async fn register(
        &self,
        req: InstanceRegisterRequest,
    ) -> Result<InstanceRegisterResponse, PolarisError> {
        let auto_heartbeat = req.auto_heartbeat;
        let ttl = req.ttl;
        let beat_req = req.to_heartbeat_request();
        tracing::info!("[polaris][discovery][provider] register instance request: {req:?}");
        let rsp = self.context.get_engine().register_instance(req).await;
        let engine = self.context.get_engine();
        if rsp.is_ok() && auto_heartbeat {
            let task_key = beat_req.beat_key();
            tracing::info!(
                "[polaris][discovery][heartbeat] add one auto_beat task={} duration={}s",
                task_key,
                ttl,
            );
            let beat_engine = engine.clone();
            // 开启了心跳自动上报功能，这里需要维护一个自动心跳上报的任务
            let handler = engine.get_executor().spawn(async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(u64::from(ttl))).await;
                    tracing::debug!(
                        "[polaris][discovery][heartbeat] start to auto_beat instance: {beat_req:?}"
                    );
                    let beat_ret = beat_engine.instance_heartbeat(beat_req.clone()).await;
                    if let Err(e) = beat_ret {
                        tracing::error!(
                        "[polaris][discovery][heartbeat] auto_beat instance to server fail: {e}"
                    );
                    }
                }
            });
            self.beat_tasks.write().await.insert(task_key, handler);
        }
        return rsp;
    }

    async fn deregister(&self, req: InstanceDeregisterRequest) -> Result<(), PolarisError> {
        let beat_req = req.to_heartbeat_request();
        let task_key = beat_req.beat_key();
        let wait_remove_task = self.beat_tasks.write().await.remove(&task_key);
        if let Some(task) = wait_remove_task {
            tracing::info!(
                "[polaris][discovery][heartbeat] remove one auto_beat task={}",
                task_key,
            );
            task.abort();
        }

        let engine = self.context.get_engine();
        engine.deregister_instance(req).await
    }

    async fn heartbeat(&self, req: InstanceHeartbeatRequest) -> Result<(), PolarisError> {
        let engine = self.context.get_engine();
        engine.instance_heartbeat(req).await
    }

    async fn report_service_contract(
        &self,
        req: ReportServiceContractRequest,
    ) -> Result<(), PolarisError> {
        todo!()
    }

    async fn close(&mut self) {}
}

pub struct DefaultLosslessAPI {
    context: SDKContext,
}

impl DefaultLosslessAPI {
    pub fn new(context: SDKContext) -> Self {
        Self { context }
    }
}

impl LosslessAPI for DefaultLosslessAPI {
    fn set_action_provider(
        &self,
        ins: Arc<dyn BaseInstance>,
        action: Arc<dyn LosslessActionProvider>,
    ) {
        todo!()
    }

    fn lossless_register(&self, ins: Arc<dyn BaseInstance>) {
        todo!()
    }

    fn lossless_deregister(&self, ins: Arc<dyn BaseInstance>) {
        todo!()
    }
}
