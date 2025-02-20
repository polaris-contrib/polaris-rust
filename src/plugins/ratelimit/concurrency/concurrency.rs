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
    model::error::PolarisError,
    plugin::{plugins::Plugin, ratelimit::ServiceRateLimiter},
};

static PLUGIN_NAME: &str = "concurrency";

pub struct ConcurrencyLimiter {}

impl ConcurrencyLimiter {
    pub fn builder() -> (fn() -> Box<dyn ServiceRateLimiter>, String) {
        (new_instance, PLUGIN_NAME.to_string())
    }
}

fn new_instance() -> Box<dyn ServiceRateLimiter> {
    Box::new(ConcurrencyLimiter {})
}

impl Plugin for ConcurrencyLimiter {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        PLUGIN_NAME.to_string()
    }
}

#[async_trait::async_trait]
impl ServiceRateLimiter for ConcurrencyLimiter {
    async fn allocate_quota(&self) -> Result<(), PolarisError> {
        Ok(())
    }

    async fn return_quota(&self) -> Result<(), PolarisError> {
        Ok(())
    }

    async fn on_remote_update(&self) -> Result<(), PolarisError> {
        Ok(())
    }

    async fn fetch_local_usage(&self) -> Result<(), PolarisError> {
        Ok(())
    }

    async fn get_amount(&self) -> Result<(), PolarisError> {
        Ok(())
    }
}
