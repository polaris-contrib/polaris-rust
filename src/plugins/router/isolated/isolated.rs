// Tencent is pleased to support the open source community by making Pole available.
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
        error::PoleError,
        naming::ServiceInstances,
        router::{RouteResult, RouteState, DEFAULT_ROUTER_ISOLATED},
    },
    plugin::{
        plugins::Plugin,
        router::{RouteContext, ServiceRouter},
    },
};

pub fn new_service_router(_conf: &ServiceRouterPluginConfig) -> Box<dyn ServiceRouter> {
    Box::new(IsolatedRouter {})
}

pub struct IsolatedRouter {}

impl IsolatedRouter {
    pub fn builder() -> (
        fn(&ServiceRouterPluginConfig) -> Box<dyn ServiceRouter>,
        String,
    ) {
        (new_service_router, DEFAULT_ROUTER_ISOLATED.to_string())
    }
}

impl Plugin for IsolatedRouter {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        DEFAULT_ROUTER_ISOLATED.to_string()
    }
}

#[async_trait::async_trait]
impl ServiceRouter for IsolatedRouter {
    async fn choose_instances(
        &self,
        _route_ctx: RouteContext,
        instances: ServiceInstances,
    ) -> Result<RouteResult, PoleError> {
        Ok(RouteResult {
            instances,
            state: RouteState::Next,
        })
    }

    async fn enable(&self, _route_ctx: RouteContext, _instances: ServiceInstances) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{
        model::{
            naming::{Instance, ServiceInstances},
            router::RouteInfo,
        },
        plugin::router::{RouteContext, ServiceRouter},
    };

    use super::IsolatedRouter;

    #[tokio::test]
    async fn test_isolated_router_is_noop() {
        let router = IsolatedRouter {};
        let mut input = ServiceInstances::default();
        input.instances.push(Instance {
            id: "a".to_string(),
            weight: 100,
            health: true,
            ..Default::default()
        });

        let result = router
            .choose_instances(
                RouteContext {
                    route_info: RouteInfo::default(),
                    extensions: None,
                },
                input.clone(),
            )
            .await
            .unwrap();

        assert_eq!(result.instances.instances.len(), 1);
        assert_eq!(result.instances.instances[0].id, "a");
        assert_eq!(result.instances.total_weight, input.total_weight);
    }
}
