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
    model::error::{ErrorCode, PolarisError},
};

use super::{api::RouterAPI, req::ProcessRouteResponse};

pub struct DefaultRouterAPI {
    context: Arc<SDKContext>,
}

impl DefaultRouterAPI {
    pub fn new(context: Arc<SDKContext>) -> Self {
        Self { context }
    }
}

#[async_trait::async_trait]
impl RouterAPI for DefaultRouterAPI {
    async fn router(
        &self,
        req: super::req::ProcessRouteRequest,
    ) -> Result<super::req::ProcessRouteResponse, PolarisError> {
        // FIXME: 需要支持路由规则，当前直接原封不动进行返回
        Ok(ProcessRouteResponse {
            service_instances: req.service_instances,
        })
    }

    async fn load_balance(
        &self,
        req: super::req::ProcessLoadBalanceRequest,
    ) -> Result<super::req::ProcessLoadBalanceResponse, PolarisError> {
        let mut criteria = req.criteria.clone();
        let mut lb_policy = criteria.policy.clone();

        if lb_policy.is_empty() {
            lb_policy = self
                .context
                .conf
                .consumer
                .load_balancer
                .default_policy
                .clone();
        }

        let lb = self
            .context
            .get_engine()
            .lookup_loadbalancer(&lb_policy)
            .await;

        if lb.is_none() {
            tracing::error!(
                "[polaris][router_api] load balancer {} not found",
                lb_policy
            );
            return Err(PolarisError::new(
                ErrorCode::PluginError,
                format!("load balancer {} not found", lb_policy,),
            ));
        }

        let lb = lb.unwrap();
        let result = lb.choose_instance(req.criteria, req.service_instances);

        match result {
            Ok(instance) => Ok(super::req::ProcessLoadBalanceResponse { instance }),
            Err(e) => Err(e),
        }
    }
}
