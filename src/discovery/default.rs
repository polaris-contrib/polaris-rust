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
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;
use tokio::time::sleep;

use crate::core::context::SDKContext;
use crate::core::model::error::PolarisError;
use crate::discovery::api::{ConsumerAPI, LosslessAPI, ProviderAPI};
use crate::discovery::req::{
    BaseInstance, GetAllInstanceRequest, GetHealthInstanceRequest, GetOneInstanceRequest,
    GetServiceRuleRequest, InstanceDeregisterRequest, InstanceHeartbeatRequest,
    InstanceRegisterRequest, InstanceRegisterResponse, InstancesResponse, LosslessActionProvider,
    ReportServiceContractRequest, ServiceCallResult, ServiceRuleResponse, UnWatchInstanceRequest,
    UnWatchInstanceResponse, WatchInstanceRequest, WatchInstanceResponse,
};

pub struct DefaultConsumerAPI {
    context: SDKContext,
}

impl DefaultConsumerAPI {
    pub fn new(context: SDKContext) -> Self {
        Self { context }
    }
}

impl ConsumerAPI for DefaultConsumerAPI {
    fn get_one_instance(&self, req: GetOneInstanceRequest) -> InstancesResponse {
        todo!()
    }

    fn get_health_instance(&self, req: GetHealthInstanceRequest) -> InstancesResponse {
        todo!()
    }

    fn get_all_instance(&self, req: GetAllInstanceRequest) -> InstancesResponse {
        todo!()
    }

    fn watch_instance(&self, req: WatchInstanceRequest) -> WatchInstanceResponse {
        todo!()
    }

    fn un_watch_instance(&self, req: UnWatchInstanceRequest) -> UnWatchInstanceResponse {
        todo!()
    }

    fn get_service_rule(&self, req: GetServiceRuleRequest) -> ServiceRuleResponse {
        todo!()
    }

    fn report_service_call(&self, req: ServiceCallResult) {
        todo!()
    }
}

pub struct DefaultProviderAPI {
    manage_sdk: bool,
    context: SDKContext,
    beat_tasks: Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
}

impl DefaultProviderAPI {
    pub fn new(context: SDKContext, manage_sdk: bool) -> Self {
        Self {
            context,
            manage_sdk: manage_sdk,
            beat_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ProviderAPI for DefaultProviderAPI {
    async fn register(
        &self,
        req: InstanceRegisterRequest,
    ) -> Result<InstanceRegisterResponse, PolarisError> {
        let auto_heartbeat = req.auto_heartbeat;
        let ttl = req.ttl;
        let beat_req = req.to_heartbeat_request();
        let rsp = self.context.get_engine().sync_register_instance(req).await;
        tracing::info!("[polaris][discovery][provider] register instance result: {rsp:?}");
        let duration = Duration::from_secs(u64::from(ttl));
        let engine = self.context.get_engine();
        if rsp.is_ok() && auto_heartbeat {
            let task_key = beat_req.beat_key();
            tracing::info!(
                "[polaris][discovery][provider] start to auto heartbeat task={} duration={}",
                task_key,
                duration.as_secs(),
            );
            // 开启了心跳自动上报功能，这里需要维护一个自动心跳上报的任务
            let handler = self.context.get_engine().get_executor().spawn(async move {
                loop {
                    tracing::info!(
                        "[polaris][discovery][provider] start to auto_beat instance: {beat_req:?}"
                    );
                    let beat_ret = engine.sync_instance_heartbeat(beat_req.clone()).await;
                    if let Err(e) = beat_ret {
                        tracing::error!(
                            "[polaris][discovery][provider] auto_beat instance to server fail: {e}"
                        );
                        break;
                    }
                    sleep(duration).await;
                }
            });
            self.beat_tasks.write().unwrap().insert(task_key, handler);
        }
        return rsp;
    }

    async fn deregister(&self, req: InstanceDeregisterRequest) -> Result<(), PolarisError> {
        let engine = self.context.get_engine();
        engine.sync_deregister_instance(req).await
    }

    async fn heartbeat(&self, req: InstanceHeartbeatRequest) -> Result<(), PolarisError> {
        let engine = self.context.get_engine();
        engine.sync_instance_heartbeat(req).await
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
