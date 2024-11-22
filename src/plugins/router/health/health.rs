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
        circuitbreaker::{InstanceResource, Resource},
        error::PolarisError,
        naming::ServiceInstances,
        router::{RouteResult, RouteState, DEFAULT_ROUTER_RECOVER},
    },
    plugin::{
        circuitbreaker::CircuitBreakerFlow,
        plugins::Plugin,
        router::{RouteContext, ServiceRouter},
    },
};

pub fn new_service_router(_conf: &ServiceRouterPluginConfig) -> Box<dyn ServiceRouter> {
    Box::new(HealthRouter {})
}

pub struct HealthRouter {}

impl HealthRouter {
    pub fn builder() -> (
        fn(&ServiceRouterPluginConfig) -> Box<dyn ServiceRouter>,
        String,
    ) {
        (new_service_router, DEFAULT_ROUTER_RECOVER.to_string())
    }
}

impl Plugin for HealthRouter {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        DEFAULT_ROUTER_RECOVER.to_string()
    }
}

#[async_trait::async_trait]
impl ServiceRouter for HealthRouter {
    async fn choose_instances(
        &self,
        route_ctx: RouteContext,
        instances: ServiceInstances,
    ) -> Result<RouteResult, PolarisError> {
        let mut final_instances = Vec::with_capacity(instances.instances.len());

        let circuit_breaker_flow = CircuitBreakerFlow::new(route_ctx.extensions);
        let mut total_weight = 0 as u64;

        for (_, ins) in instances.instances.iter().enumerate() {
            if !ins.is_available() {
                continue;
            }
            let ret = circuit_breaker_flow
                .check_resource(Resource::InstanceResource(InstanceResource {}))
                .await;
            match ret {
                Ok(check_ret) => {
                    if !check_ret.pass {
                        continue;
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }

            total_weight += ins.weight as u64;
        }

        // 重新算一次
        total_weight = 0 as u64;
        for instance in instances.instances {
            let weight = instance.weight as u64;
            if !instance.is_available() && total_weight != 0 {
                continue;
            }
            total_weight += weight as u64;
            final_instances.push(instance);
        }

        Ok(RouteResult {
            instances: ServiceInstances {
                service: instances.service,
                instances: final_instances,
                total_weight: total_weight as u64,
            },
            state: RouteState::Next,
        })
    }

    async fn enable(&self, _route_info: crate::core::model::router::RouteInfo) -> bool {
        true
    }
}
