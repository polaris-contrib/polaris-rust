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
    model::{
        circuitbreaker::{CircuitBreakerStatus, Resource, ResourceStat, Status},
        error::PolarisError,
    },
    plugin::{circuitbreaker::CircuitBreaker, plugins::Plugin},
};

static PLUGIN_NAME: &str = "composite";

fn new_circuir_breaker() -> Box<dyn CircuitBreaker> {
    Box::new(CompositeCircuitBreaker {})
}

pub struct CompositeCircuitBreaker {}

impl CompositeCircuitBreaker {
    pub fn builder() -> (fn() -> Box<dyn CircuitBreaker>, String) {
        (new_circuir_breaker, PLUGIN_NAME.to_string())
    }
}

impl Plugin for CompositeCircuitBreaker {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        PLUGIN_NAME.to_string()
    }
}

#[async_trait::async_trait]
impl CircuitBreaker for CompositeCircuitBreaker {
    /// check_resource 检查资源
    async fn check_resource(
        &self,
        resource: Resource,
    ) -> Result<CircuitBreakerStatus, PolarisError> {
        Ok(CircuitBreakerStatus {
            status: Status::Close,
            start_ms: 0,
            circuit_breaker: "".to_string(),
            fallback_info: None,
            destroy: false,
        })
    }
    /// report_stat 上报统计信息
    async fn report_stat(&self, stat: ResourceStat) -> Result<(), PolarisError> {
        todo!()
    }
}
