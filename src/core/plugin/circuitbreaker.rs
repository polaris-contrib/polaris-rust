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

use crate::core::model::circuitbreaker::{Resource, ResourceStat};
use crate::core::model::{circuitbreaker::CircuitBreakerStatus, error::PolarisError};

use crate::core::plugin::plugins::Plugin;

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
