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

use crate::discovery::req::*;

pub trait ProviderAPI {
    fn register(&self, req: InstanceRegisterRequest) -> InstanceRegisterResponse;

    fn deregister(&self, req: InstanceDeregisterRequest);

    fn heartbeat(&self, req: InstanceHeartbeatRequest);

    fn report_service_contract(&self, req: ReportServiceContractRequest);
}

pub trait ConsumerAPI {
    fn get_one_instance(&self, req: GetOneInstanceRequest) -> InstancesResponse;

    fn get_health_instance(&self, req: GetHealthInstanceRequest) -> InstancesResponse;

    fn get_all_instance(&self, req: GetAllInstanceRequest) -> InstancesResponse;

    fn watch_instance(&self, req: WatchInstanceRequest) -> WatchInstanceResponse;

    fn un_watch_instance(&self, req: UnWatchInstanceRequest) -> UnWatchInstanceResponse;

    fn get_service_rule(&self, req: GetServiceRuleRequest) -> ServiceRuleResponse;

    fn report_service_call(&self, req: ServiceCallResult);

}

pub trait LosslessAPI {

    fn set_action_provider(&self, ins: dyn BaseInstance, action: dyn LosslessActionProvider);

    fn lossless_register(&self, ins: dyn BaseInstance);

    fn lossless_deregister(&self, ins: dyn BaseInstance);

}