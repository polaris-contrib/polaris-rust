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

use std::{collections::HashMap, fs::File};

use serde::{Deserialize, Serialize};

use polaris_specification::v1::{ConfigDiscoverResponse, DiscoverResponse, Routing};

use crate::core::{
    config::global::LocalCacheConfig,
    model::error::{ErrorCode, PolarisError},
    plugin::cache::{Filter, ResourceCacheFailover},
};

pub struct DiskCacheFailover {
    conf: LocalCacheConfig,
}

#[async_trait::async_trait]
impl ResourceCacheFailover for DiskCacheFailover {
    // failover_naming_load 兜底加载
    async fn failover_naming_load(&self, filter: Filter) -> Result<DiscoverResponse, PolarisError> {
        todo!()
    }

    // save_failover 保存容灾数据
    async fn save_naming_failover(&self, value: DiscoverResponse) -> Result<(), PolarisError> {
        match value.r#type() {
            polaris_specification::v1::discover_response::DiscoverResponseType::Instance => {
                return Ok(());
            }
            polaris_specification::v1::discover_response::DiscoverResponseType::Cluster => {
                return Ok(());
            }
            polaris_specification::v1::discover_response::DiscoverResponseType::Routing => {
                return Ok(());
            }
            polaris_specification::v1::discover_response::DiscoverResponseType::RateLimit => {
                return Ok(());
            }
            polaris_specification::v1::discover_response::DiscoverResponseType::CircuitBreaker => {
                return Ok(());
            }
            polaris_specification::v1::discover_response::DiscoverResponseType::Services => {
                return Ok(());
            }
            polaris_specification::v1::discover_response::DiscoverResponseType::Namespaces => {
                return Ok(());
            }
            polaris_specification::v1::discover_response::DiscoverResponseType::FaultDetector => {
                return Ok(());
            }
            polaris_specification::v1::discover_response::DiscoverResponseType::Lane => {
                return Ok(());
            }
            _ => {
                return Err(PolarisError::new(
                    ErrorCode::InternalError,
                    format!("unsupported discover response type"),
                ));
            }
        }
    }

    // failover_config_load 兜底加载
    async fn failover_config_load(
        &self,
        filter: Filter,
    ) -> Result<ConfigDiscoverResponse, PolarisError> {
        todo!()
    }

    // save_config_failover 保存容灾数据
    async fn save_config_failover(
        &self,
        value: ConfigDiscoverResponse,
    ) -> Result<(), PolarisError> {
        todo!()
    }
}

impl DiskCacheFailover {
    pub fn new(conf: LocalCacheConfig) -> Self {
        Self { conf }
    }
}
