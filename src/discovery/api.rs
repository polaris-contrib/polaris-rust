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

    Ok(DefaultProviderAPI::new_raw(context_ret.unwrap()))
}

pub fn new_provider_api_by_context(
    context: Arc<SDKContext>,
) -> Result<impl ProviderAPI, PolarisError> {
    Ok(DefaultProviderAPI::new(context))
}

/// ProviderAPI 负责服务提供者的生命周期管理
#[async_trait::async_trait]
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
pub fn new_consumer_api() -> Result<impl ConsumerAPI, PolarisError> {
    let context_ret = SDKContext::default();
    if context_ret.is_err() {
        return Err(context_ret.err().unwrap());
    }

    Ok(DefaultConsumerAPI::new_raw(context_ret.unwrap()))
}

pub fn new_consumer_api_by_context(
    context: Arc<SDKContext>,
) -> Result<impl ConsumerAPI, PolarisError> {
    Ok(DefaultConsumerAPI::new(context))
}

/// ConsumerAPI 负责服务消费方完成获取被调服务的 IP 地址完成远程调用
#[async_trait::async_trait]
pub trait ConsumerAPI
where
    Self: Send + Sync,
{
    /// get_one_instance 拉取一个实例
    async fn get_one_instance(
        &self,
        req: GetOneInstanceRequest,
    ) -> Result<InstanceResponse, PolarisError>;

    /// get_health_instance 拉取健康实例
    async fn get_health_instance(
        &self,
        req: GetHealthInstanceRequest,
    ) -> Result<InstancesResponse, PolarisError>;

    /// get_all_instance 拉取所有实例
    async fn get_all_instance(
        &self,
        req: GetAllInstanceRequest,
    ) -> Result<InstancesResponse, PolarisError>;

    /// watch_instance 监听实例变化
    async fn watch_instance(
        &self,
        req: WatchInstanceRequest,
    ) -> Result<WatchInstanceResponse, PolarisError>;

    /// get_service_rule 获取服务规则
    async fn get_service_rule(
        &self,
        req: GetServiceRuleRequest,
    ) -> Result<ServiceRuleResponse, PolarisError>;

    /// report_service_call 上报服务调用结果
    async fn report_service_call(&self, req: ServiceCallResult);
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
