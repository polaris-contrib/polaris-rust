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

use crate::{
    config::default::DefaultConfigFileAPI,
    core::{
        context::SDKContext,
        model::{config::ConfigFile, error::PolarisError},
    },
};

use super::req::{
    DeleteConfigFileRequest, GetConfigFileRequest, GetConfigGroupRequest, PublishConfigFileRequest,
    UpdateConfigFileRequest, WatchConfigFileRequest, WatchConfigFileResponse,
    WatchConfigGroupRequest, WatchConfigGroupResponse,
};

/// new_config_file_api
pub fn new_config_file_api() -> Result<impl ConfigFileAPI, PolarisError> {
    let start_time = std::time::Instant::now();
    let context_ret = SDKContext::default();
    tracing::info!("create sdk context cost: {:?}", start_time.elapsed());
    if context_ret.is_err() {
        return Err(context_ret.err().unwrap());
    }
    Ok(DefaultConfigFileAPI::new(
        Arc::new(context_ret.unwrap()),
        true,
    ))
}

/// new_config_file_api_by_context
pub fn new_config_file_api_by_context(
    context: Arc<SDKContext>,
) -> Result<impl ConfigFileAPI, PolarisError> {
    Ok(DefaultConfigFileAPI::new(context, false))
}

#[async_trait::async_trait]
pub trait ConfigFileAPI
where
    Self: Send + Sync,
{
    async fn get_config_file(&self, req: GetConfigFileRequest) -> Result<ConfigFile, PolarisError>;

    async fn update_config_file(&self, req: UpdateConfigFileRequest) -> Result<bool, PolarisError>;

    async fn delete_config_file(&self, req: DeleteConfigFileRequest) -> Result<bool, PolarisError>;

    async fn publish_config_file(
        &self,
        req: PublishConfigFileRequest,
    ) -> Result<bool, PolarisError>;

    async fn watch_config_file(
        &self,
        req: WatchConfigFileRequest,
    ) -> Result<WatchConfigFileResponse, PolarisError>;
}

#[async_trait::async_trait]
pub trait ConfigGroupAPI
where
    Self: Send + Sync,
{
    async fn get_publish_config_files(
        &self,
        req: GetConfigGroupRequest,
    ) -> Result<Vec<ConfigFile>, PolarisError>;

    async fn watch_publish_config_files(
        &self,
        req: WatchConfigGroupRequest,
    ) -> Result<WatchConfigGroupResponse, PolarisError>;
}
