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
use crate::discovery::default::{DefaultConsumerAPI, DefaultLosslessAPI, DefaultProviderAPI};
use crate::discovery::req::*;

/// new_provider_api
pub fn new_provider_api() -> Result<impl ProviderAPI, PolarisError> {
    let start_time = std::time::Instant::now();
    let context_ret = SDKContext::default();
    tracing::info!("create sdk context cost: {:?}", start_time.elapsed());
    if context_ret.is_err() {
        return Err(context_ret.err().unwrap());
    }

    Ok(DefaultProviderAPI::new(context_ret.unwrap(), true))
}

pub fn new_provider_api_by_context(context: SDKContext) -> Result<impl ProviderAPI, PolarisError> {
    Ok(DefaultProviderAPI::new(context, false))
}

/// ProviderAPI 负责服务提供者的生命周期管理
pub trait ProviderAPI
where
    Self: Send + Sync,
{
    /// register 实例注册
    async fn register(
        &self,
        req: InstanceRegisterRequest,
    ) -> Result<InstanceRegisterResponse, PolarisError>;

    /// deregister 实例反注册
    async fn deregister(&self, req: InstanceDeregisterRequest) -> Result<(), PolarisError>;

    /// heartbeat 实例心跳上报
    async fn heartbeat(&self, req: InstanceHeartbeatRequest) -> Result<(), PolarisError>;

    /// report_service_contract 上报服务接口定义信息
    async fn report_service_contract(
        &self,
        req: ReportServiceContractRequest,
    ) -> Result<(), PolarisError>;

    /// close 关闭 ProviderAPI 实例
    async fn close(&mut self);
}

/// new_consumer_api
pub(crate) fn new_consumer_api() -> Result<impl ConsumerAPI, PolarisError> {
    let context_ret = SDKContext::default();
    if context_ret.is_err() {
        return Err(context_ret.err().unwrap());
    }

    Ok(DefaultConsumerAPI::new(context_ret.unwrap()))
}

pub(crate) fn new_consumer_api_by_context(
    context: SDKContext,
) -> Result<impl ConsumerAPI, PolarisError> {
    Ok(DefaultConsumerAPI::new(context))
}

pub(crate) trait ConsumerAPI {
    fn get_one_instance(&self, req: GetOneInstanceRequest) -> InstancesResponse;

    fn get_health_instance(&self, req: GetHealthInstanceRequest) -> InstancesResponse;

    fn get_all_instance(&self, req: GetAllInstanceRequest) -> InstancesResponse;

    fn watch_instance(&self, req: WatchInstanceRequest) -> WatchInstanceResponse;

    fn un_watch_instance(&self, req: UnWatchInstanceRequest) -> UnWatchInstanceResponse;

    fn get_service_rule(&self, req: GetServiceRuleRequest) -> ServiceRuleResponse;

    fn report_service_call(&self, req: ServiceCallResult);
}

pub(crate) fn new_lossless_api() -> Result<impl LosslessAPI, PolarisError> {
    let context_ret = SDKContext::default();
    if context_ret.is_err() {
        return Err(context_ret.err().unwrap());
    }

    Ok(DefaultLosslessAPI::new(context_ret.unwrap()))
}

pub(crate) fn new_lossless_api_by_context(
    context: SDKContext,
) -> Result<Arc<dyn LosslessAPI>, PolarisError> {
    Ok(Arc::new(DefaultLosslessAPI::new(context)))
}

pub(crate) trait LosslessAPI {
    fn set_action_provider(
        &self,
        ins: Arc<dyn BaseInstance>,
        action: Arc<dyn LosslessActionProvider>,
    );

    fn lossless_register(&self, ins: Arc<dyn BaseInstance>);

    fn lossless_deregister(&self, ins: Arc<dyn BaseInstance>);
}

mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
