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
    model::{config::ConfigFile, error::PolarisError},
};

use super::{
    api::{ConfigFileAPI, ConfigGroupAPI},
    req::{
        DeleteConfigFileRequest, GetConfigFileRequest, GetConfigGroupRequest,
        PublishConfigFileRequest, UpdateConfigFileRequest, WatchConfigFileRequest,
        WatchConfigFileResponse, WatchConfigGroupRequest, WatchConfigGroupResponse,
    },
};

/// DefaultConfigFileAPI
pub struct DefaultConfigFileAPI {
    context: Arc<SDKContext>,
    manage_sdk: bool,
}

impl DefaultConfigFileAPI {
    pub fn new(context: Arc<SDKContext>, manage_sdk: bool) -> Self {
        Self {
            context,
            manage_sdk: manage_sdk,
        }
    }
}

#[async_trait::async_trait]
impl ConfigFileAPI for DefaultConfigFileAPI {
    async fn get_config_file(
        &self,
        req: GetConfigFileRequest,
    ) -> Result<ConfigFile, PolarisError> {
        let engine = self.context.get_engine();
        engine.get_config_file(req).await
    }

    async fn update_config_file(
        &self,
        _req: UpdateConfigFileRequest,
    ) -> Result<bool, PolarisError> {
        Ok(true)
    }

    async fn delete_config_file(
        &self,
        _req: DeleteConfigFileRequest,
    ) -> Result<bool, PolarisError> {
        Ok(true)
    }

    async fn publish_config_file(
        &self,
        _req: PublishConfigFileRequest,
    ) -> Result<bool, PolarisError> {
        Ok(true)
    }

    async fn watch_config_file(
        &self,
        _req: WatchConfigFileRequest,
    ) -> Result<WatchConfigFileResponse, PolarisError> {
        todo!()
    }
}

/// DefaultConfigGroupAPI
pub struct DefaultConfigGroupAPI {}

#[async_trait::async_trait]
impl ConfigGroupAPI for DefaultConfigGroupAPI {
    async fn get_publish_config_files(
        &self,
        req: GetConfigGroupRequest,
    ) -> Result<Vec<ConfigFile>, PolarisError> {
        todo!()
    }

    async fn watch_publish_config_files(
        &self,
        req: WatchConfigGroupRequest,
    ) -> Result<WatchConfigGroupResponse, PolarisError> {
        todo!()
    }
}
