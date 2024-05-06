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

use crate::core::context::SDKContext;
use crate::core::model::error::PolarisError;
use crate::discovery::default::{DefaultConsumerAPI, DefaultLosslessAPI, DefaultProviderAPI};
use crate::discovery::req::*;

pub(crate) fn new_provider_api() -> Result<Box<dyn ProviderAPI>, PolarisError> {
    let context_ret = SDKContext::default();
    if context_ret.is_err() {
        return Err(context_ret.err().unwrap());
    }

    Ok(Box::new(DefaultProviderAPI::new(context_ret.unwrap())))
}

pub(crate) fn new_provider_api_by_context(context: SDKContext) -> Result<Box<dyn ProviderAPI>, PolarisError> {
    Ok(Box::new(DefaultProviderAPI::new(context)))
}

pub(crate) trait ProviderAPI {
    fn register(&mut self, req: InstanceRegisterRequest) -> Result<InstanceRegisterResponse, PolarisError>;

    fn deregister(&mut self, req: InstanceDeregisterRequest) -> Result<(), PolarisError>;

    fn heartbeat(&mut self, req: InstanceHeartbeatRequest) -> Result<(), PolarisError>;

    fn report_service_contract(&mut self, req: ReportServiceContractRequest) -> Result<(), PolarisError>;
}

pub(crate) fn new_consumer_api() -> Result<Box<dyn ConsumerAPI>, PolarisError> {
    let context_ret = SDKContext::default();
    if context_ret.is_err() {
        return Err(context_ret.err().unwrap());
    }

    Ok(Box::new(DefaultConsumerAPI::new(context_ret.unwrap())))
}

pub(crate) fn new_consumer_api_by_context(context: SDKContext) -> Result<Box<dyn ConsumerAPI>, PolarisError> {
    Ok(Box::new(DefaultConsumerAPI::new(context)))
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

pub(crate) fn new_lossless_api() -> Result<Box<dyn LosslessAPI>, PolarisError> {
    let context_ret = SDKContext::default();
    if context_ret.is_err() {
        return Err(context_ret.err().unwrap());
    }

    Ok(Box::new(DefaultLosslessAPI::new(context_ret.unwrap())))
}

pub(crate) fn new_lossless_api_by_context(context: SDKContext) -> Result<Box<dyn LosslessAPI>, PolarisError> {
    Ok(Box::new(DefaultLosslessAPI::new(context)))
}

pub(crate) trait LosslessAPI {

    fn set_action_provider(&self, ins: Box<dyn BaseInstance>, action: Box<dyn LosslessActionProvider>);

    fn lossless_register(&self, ins: Box<dyn BaseInstance>);

    fn lossless_deregister(&self, ins: Box<dyn BaseInstance>);

}

mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

