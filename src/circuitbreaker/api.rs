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

use std::{sync::Arc, time::Duration};

use crate::core::{
    flow::CircuitBreakerFlow,
    model::{
        circuitbreaker::{CallAbortedError, CheckResult, MethodResource, Resource, ResourceStat, RetStatus, ServiceResource},
        error::PolarisError,
    },
};

use super::req::{RequestContext, ResponseContext};

/// CircuitBreakerAPI .
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

/// InvokeHandler .
pub struct InvokeHandler {
    // req_ctx: 请求上下文
    req_ctx: RequestContext,
    // flow: 熔断器流程
    flow: Arc<CircuitBreakerFlow>,
}

impl InvokeHandler {
    pub fn new(req_ctx: RequestContext, flow: Arc<CircuitBreakerFlow>) -> Self {
        InvokeHandler { req_ctx, flow }
    }

    /// acquire_permission 检查当前请求是否可放通
    async fn acquire_permission(&self) -> Result<(), CallAbortedError> {
        let svc_res = ServiceResource::new_waith_caller(
            self.req_ctx.caller_service.clone(), 
        self.req_ctx.callee_service.clone());

        match self.flow.check_resource(Resource::ServiceResource(svc_res)).await {
            Ok(ret) => {
                if ret.pass {
                    Ok(())
                } else {
                    Err(CallAbortedError::new(ret.rule_name, ret.fallback_info))
                }
            },
            Err(e) => {
                // 内部异常，不触发垄断，但是需要记录
                crate::error!("[circuitbreaker][invoke] check resource failed: {:?}", e);
                Ok(())
            },
        }
    }

    async fn on_success(&self, rsp: ResponseContext) -> Result<(), PolarisError> {
        let cost = rsp.duration.clone();
        let mut code = -1 as i32;
        let mut status = RetStatus::RetSuccess;

        if let Some(r) = &self.req_ctx.result_to_code {
            code = r.on_success(rsp.result.unwrap());
        }
        if let Some(e) = rsp.error {
            let ret = e.downcast::<CallAbortedError>();
            if ret.is_ok() {
                status = RetStatus::RetReject;
            }
        }
        self.common_report(cost, code, status).await
    }

    async fn on_error(&self, rsp: ResponseContext) -> Result<(), PolarisError> {
        let cost = rsp.duration.clone();
        let mut code = 0 as i32;
        let mut status = RetStatus::RetUnknown;

        if let Some(r) = &self.req_ctx.result_to_code {
            code = r.on_success(rsp.result.unwrap());
        }
        self.common_report(cost, code, status).await
    }

    async fn common_report(&self, cost: Duration, code: i32, status: RetStatus) -> Result<(), PolarisError> {
        let stat = ResourceStat {
            resource: Resource::ServiceResource(ServiceResource::new_waith_caller(
                self.req_ctx.caller_service.clone(), 
            self.req_ctx.callee_service.clone())),
            ret_code: code.to_string(),
            delay: cost,
            status: status.clone(),
        };

        let ret = self.flow.report_stat(stat).await;
        if ret.is_err() {
            crate::error!("[circuitbreaker][invoke] report stat failed");
            return ret;
        }

        if self.req_ctx.path.is_empty() {
            return Ok(());
        }

        // 补充一个接口级别的数据上报
        let stat = ResourceStat {
            resource: Resource::MethodResource(MethodResource::new_waith_caller(
                self.req_ctx.caller_service.clone(),
                self.req_ctx.callee_service.clone(),
                self.req_ctx.protocol.clone(),
                self.req_ctx.method.clone(),
                self.req_ctx.path.clone(),
            )),
            ret_code: code.to_string(),
            delay: cost,
            status,
        };
        self.flow.report_stat(stat).await
    }
}
