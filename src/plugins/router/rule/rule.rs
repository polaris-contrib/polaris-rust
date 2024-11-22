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

use crate::core::{
    config::consumer::ServiceRouterPluginConfig,
    model::{
        error::PolarisError,
        naming::ServiceInstances,
        router::{RouteInfo, RouteResult, RouteState, DEFAULT_ROUTER_RULE},
    },
    plugin::{
        plugins::Plugin,
        router::{RouteContext, ServiceRouter},
    },
};

pub fn new_service_router(_conf: &ServiceRouterPluginConfig) -> Box<dyn ServiceRouter> {
    Box::new(RuleRouter {})
}

pub struct RuleRouter {}

impl RuleRouter {
    pub fn builder() -> (
        fn(&ServiceRouterPluginConfig) -> Box<dyn ServiceRouter>,
        String,
    ) {
        (new_service_router, DEFAULT_ROUTER_RULE.to_string())
    }
}

impl Plugin for RuleRouter {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        DEFAULT_ROUTER_RULE.to_string()
    }
}

#[async_trait::async_trait]
impl ServiceRouter for RuleRouter {
    /// choose_instances 实例路由
    async fn choose_instances(
        &self,
        route_info: RouteContext,
        instances: ServiceInstances,
    ) -> Result<RouteResult, PolarisError> {
        Ok(RouteResult {
            instances,
            state: RouteState::Next,
        })
    }

    /// enable 是否启用
    async fn enable(&self, route_info: RouteInfo) -> bool {
        return true;
    }
}
