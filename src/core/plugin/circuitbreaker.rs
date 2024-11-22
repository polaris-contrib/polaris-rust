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

use crate::core::model::circuitbreaker::{CheckResult, Resource, ResourceStat, Status};
use crate::core::model::{circuitbreaker::CircuitBreakerStatus, error::PolarisError};

use crate::core::plugin::plugins::Plugin;

use super::plugins::Extensions;

#[async_trait::async_trait]
pub trait CircuitBreaker: Plugin {
    /// check_resource 检查资源
    async fn check_resource(
        &self,
        resource: Resource,
    ) -> Result<CircuitBreakerStatus, PolarisError>;
    /// report_stat 上报统计信息
    async fn report_stat(&self, stat: ResourceStat) -> Result<(), PolarisError>;
}

/// CircuitBreakerFlow
pub struct CircuitBreakerFlow {
    extensions: Arc<Extensions>,
}

impl CircuitBreakerFlow {
    pub fn new(extensions: Arc<Extensions>) -> Self {
        CircuitBreakerFlow { extensions }
    }

    pub async fn check_resource(&self, resource: Resource) -> Result<CheckResult, PolarisError> {
        let circuit_breaker_opt = self.extensions.circuit_breaker.clone();
        if circuit_breaker_opt.is_none() {
            return Ok(CheckResult::pass());
        }

        let circuit_breaker = circuit_breaker_opt.unwrap();
        let status = circuit_breaker.check_resource(resource).await?;

        Ok(CircuitBreakerFlow::convert_from_status(status))
    }

    pub async fn report_stat(&self, stat: ResourceStat) -> Result<(), PolarisError> {
        let circuit_breaker_opt = self.extensions.circuit_breaker.clone();
        if circuit_breaker_opt.is_none() {
            return Ok(());
        }

        let circuit_breaker = circuit_breaker_opt.unwrap();
        circuit_breaker.report_stat(stat).await
    }

    fn convert_from_status(ret: CircuitBreakerStatus) -> CheckResult {
        let status = ret.status;
        CheckResult {
            pass: status == Status::Open,
            rule_name: ret.circuit_breaker,
            fallback_info: ret.fallback_info.clone(),
        }
    }
}
