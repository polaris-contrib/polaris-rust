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

use crate::{
    core::model::error::PolarisError,
    router::req::{
        ProcessLoadBalanceRequest, ProcessLoadBalanceResponse, ProcessRouteRequest,
        ProcessRouteResponse,
    },
};

#[async_trait::async_trait]
pub trait RouterAPI
where
    Self: Send + Sync,
{
    // router 执行路由逻辑
    async fn router(&self, req: ProcessRouteRequest) -> Result<ProcessRouteResponse, PolarisError>;

    // load_balance 执行北极星负载均衡执行逻辑
    async fn load_balance(
        &self,
        req: ProcessLoadBalanceRequest,
    ) -> Result<ProcessLoadBalanceResponse, PolarisError>;
}
