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
    flow::RouterFlow,
    model::error::{ErrorCode, PolarisError},
    plugin::router::RouteContext,
};
use crate::debug;
use super::{api::RouterAPI, req::ProcessRouteResponse};

pub struct DefaultRouterAPI {
    manage_sdk: bool,
    context: Arc<SDKContext>,
    flow: Arc<RouterFlow>,
}

impl DefaultRouterAPI {
    pub fn new_raw(context: SDKContext) -> Self {
        let ctx = Arc::new(context);
        let extensions = ctx.get_engine().get_extensions();
        Self {
            manage_sdk: true,
            context: ctx,
            flow: Arc::new(RouterFlow::new(extensions)),
        }
    }

    pub fn new(context: Arc<SDKContext>) -> Self {
        let extensions = context.get_engine().get_extensions();
        Self {
            manage_sdk: false,
            context: context,
            flow: Arc::new(RouterFlow::new(extensions)),
        }
    }
}

impl Drop for DefaultRouterAPI {
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
impl RouterAPI for DefaultRouterAPI {
    async fn router(
        &self,
        req: super::req::ProcessRouteRequest,
    ) -> Result<super::req::ProcessRouteResponse, PolarisError> {
        debug!("[polaris][router_api] route request {:?}", req);

        let ret = self
            .flow
            .choose_instances(req.route_info.clone(), req.service_instances)
            .await;

        match ret {
            Ok(result) => Ok(ProcessRouteResponse {
                service_instances: result,
            }),
            Err(e) => Err(e),
        }
    }

    async fn load_balance(
        &self,
        req: super::req::ProcessLoadBalanceRequest,
    ) -> Result<super::req::ProcessLoadBalanceResponse, PolarisError> {
        debug!("[polaris][router_api] load_balance request {:?}", req);

        let criteria = req.criteria.clone();
        let mut lb_policy = criteria.policy.clone();

        if lb_policy.is_empty() {
            lb_policy.clone_from(&self.context.conf.consumer.load_balancer.default_policy);
        }

        let lb = self.flow.lookup_loadbalancer(&lb_policy).await;

        if lb.is_none() {
            crate::error!(
                "[polaris][router_api] load balancer {} not found",
                lb_policy
            );
            return Err(PolarisError::new(
                ErrorCode::PluginError,
                format!("load balancer {} not found", lb_policy),
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
