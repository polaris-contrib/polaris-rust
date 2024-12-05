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
    context::SDKContext,
    flow::CircuitBreakerFlow,
    model::{
        circuitbreaker::{CheckResult, Resource, ResourceStat},
        error::PolarisError,
    },
};

use super::{
    api::{CircuitBreakerAPI, InvokeHandler},
    req::RequestContext,
};

/// DefaultCircuitBreakerAPI .
pub struct DefaultCircuitBreakerAPI {
    context: Arc<SDKContext>,
    // manage_sdk: 是否管理 sdk_context 的生命周期
    manage_sdk: bool,
    // flow: 熔断器流程
    flow: Arc<CircuitBreakerFlow>,
}

impl DefaultCircuitBreakerAPI {
    pub fn new_raw(context: SDKContext) -> Self {
        let ctx = Arc::new(context);
        let extensions = ctx.get_engine().get_extensions();
        Self {
            context: ctx,
            manage_sdk: true,
            flow: Arc::new(CircuitBreakerFlow::new(extensions)),
        }
    }

    pub fn new(context: Arc<SDKContext>) -> Self {
        let extensions = context.get_engine().get_extensions();
        Self {
            context,
            manage_sdk: false,
            flow: Arc::new(CircuitBreakerFlow::new(extensions)),
        }
    }
}

impl Drop for DefaultCircuitBreakerAPI {
    fn drop(&mut self) {
        if !self.manage_sdk {
            return;
        }
        let ctx = self.context.to_owned();
        let ret = Arc::try_unwrap(ctx);

        match ret {
            Ok(ctx) => {
                drop(ctx);
            }
            Err(_) => {
                // do nothing
            }
        }
    }
}

#[async_trait::async_trait]
impl CircuitBreakerAPI for DefaultCircuitBreakerAPI {
    async fn check_resource(&self, resource: Resource) -> Result<CheckResult, PolarisError> {
        self.flow.check_resource(resource).await
    }

    async fn report_stat(&self, stat: ResourceStat) -> Result<(), PolarisError> {
        self.flow.report_stat(stat).await
    }

    async fn make_invoke_handler(
        &self,
        req: RequestContext,
    ) -> Result<Arc<InvokeHandler>, PolarisError> {
        // Implement the method logic here
        Ok(Arc::new(InvokeHandler::new(req, self.flow.clone())))
    }
}
