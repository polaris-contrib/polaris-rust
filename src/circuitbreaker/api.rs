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

use crate::core::{
    flow::CircuitBreakerFlow,
    model::{
        circuitbreaker::{CallAbortedError, CheckResult, Resource, ResourceStat},
        error::PolarisError,
    },
};

use super::req::{RequestContext, ResponseContext};

#[async_trait::async_trait]
pub trait CircuitBreakerAPI
where
    Self: Send + Sync,
{
    /// check_resource .
    async fn check_resource(&self, resource: Resource) -> Result<CheckResult, PolarisError>;
    /// report_stat .
    async fn report_stat(&self, stat: ResourceStat) -> Result<(), PolarisError>;
    /// make_invoke_handler .
    async fn make_invoke_handler(
        &self,
        req: RequestContext,
    ) -> Result<Arc<InvokeHandler>, PolarisError>;
}

pub struct InvokeHandler {
    req_ctx: RequestContext,
    flow: Arc<CircuitBreakerFlow>,
}

impl InvokeHandler {
    pub fn new(req_ctx: RequestContext, flow: Arc<CircuitBreakerFlow>) -> Self {
        InvokeHandler { req_ctx, flow }
    }

    /// acquire_permission 检查当前请求是否可放通
    async fn acquire_permission(&self) -> Result<(), CallAbortedError> {
        Ok(())
    }

    async fn on_success(&self, rsp: ResponseContext) -> Result<(), PolarisError> {
        Ok(())
    }

    async fn on_error(&self, rsp: ResponseContext) -> Result<(), PolarisError> {
        Ok(())
    }
}
