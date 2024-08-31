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
    context: SDKContext,
}

impl DefaultProviderAPI {
    pub fn new(context: SDKContext) -> Self {
        Self { context }
    }
}

impl ProviderAPI for DefaultProviderAPI {
    fn register(
        &mut self,
        req: InstanceRegisterRequest,
    ) -> Result<InstanceRegisterResponse, PolarisError> {
        self.context.get_engine().sync_register_instance(req)
    }

    fn deregister(&mut self, req: InstanceDeregisterRequest) -> Result<(), PolarisError> {
        todo!()
    }

    fn heartbeat(&mut self, req: InstanceHeartbeatRequest) -> Result<(), PolarisError> {
        todo!()
    }

    fn report_service_contract(
        &mut self,
        req: ReportServiceContractRequest,
    ) -> Result<(), PolarisError> {
        todo!()
    }
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
