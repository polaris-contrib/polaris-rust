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
    config::default::{DefaultConfigFileAPI, DefaultConfigGroupAPI},
    core::{
        context::SDKContext,
        model::{
            config::{ConfigFile, ConfigGroup},
            error::PolarisError,
        },
    },
};

use super::req::{
    CreateConfigFileRequest, GetConfigFileRequest, GetConfigGroupRequest, PublishConfigFileRequest,
    UpdateConfigFileRequest, UpsertAndPublishConfigFileRequest, WatchConfigFileRequest,
    WatchConfigFileResponse, WatchConfigGroupRequest, WatchConfigGroupResponse,
};

/// new_config_file_api
pub fn new_config_file_api() -> Result<impl ConfigFileAPI, PolarisError> {
    let start_time = std::time::Instant::now();
    let context_ret = SDKContext::default();
    tracing::info!("create sdk context cost: {:?}", start_time.elapsed());
    if context_ret.is_err() {
        return Err(context_ret.err().unwrap());
    }
    Ok(DefaultConfigFileAPI::new_raw(context_ret.unwrap()))
}

/// new_config_file_api_by_context
pub fn new_config_file_api_by_context(
    context: Arc<SDKContext>,
) -> Result<impl ConfigFileAPI, PolarisError> {
    Ok(DefaultConfigFileAPI::new(context))
}

/// ConfigFileAPI 配置文件API
#[async_trait::async_trait]
pub trait ConfigFileAPI
where
    Self: Send + Sync,
{
    /// get_config_file 获取配置文件
    async fn get_config_file(&self, req: GetConfigFileRequest) -> Result<ConfigFile, PolarisError>;

    /// create_config_file 创建配置文件
    async fn create_config_file(&self, req: CreateConfigFileRequest) -> Result<bool, PolarisError>;

    /// update_config_file 更新配置文件
    async fn update_config_file(&self, req: UpdateConfigFileRequest) -> Result<bool, PolarisError>;

    /// publish_config_file 发布配置文件
    async fn publish_config_file(
        &self,
        req: PublishConfigFileRequest,
    ) -> Result<bool, PolarisError>;

    /// upsert_publish_config_file 创建/更新配置文件后并发布
    async fn upsert_publish_config_file(
        &self,
        req: UpsertAndPublishConfigFileRequest,
    ) -> Result<bool, PolarisError>;

    /// watch_config_file 监听配置文件变更
    async fn watch_config_file(
        &self,
        req: WatchConfigFileRequest,
    ) -> Result<WatchConfigFileResponse, PolarisError>;
}

/// new_config_group_api
pub fn new_config_group_api() -> Result<impl ConfigGroupAPI, PolarisError> {
    let start_time = std::time::Instant::now();
    let context_ret = SDKContext::default();
    tracing::info!("create sdk context cost: {:?}", start_time.elapsed());
    if context_ret.is_err() {
        return Err(context_ret.err().unwrap());
    }
    Ok(DefaultConfigGroupAPI::new_raw(context_ret.unwrap()))
}

/// new_config_group_api_by_context
pub fn new_config_group_api_by_context(
    context: Arc<SDKContext>,
) -> Result<impl ConfigGroupAPI, PolarisError> {
    Ok(DefaultConfigGroupAPI::new(context))
}

/// ConfigGroupAPI 配置组API
#[async_trait::async_trait]
pub trait ConfigGroupAPI
where
    Self: Send + Sync,
{
    /// get_publish_config_files 获取发布的配置文件列表
    async fn get_publish_config_files(
        &self,
        req: GetConfigGroupRequest,
    ) -> Result<ConfigGroup, PolarisError>;

    /// watch_publish_config_files 监听发布的配置文件变更
    async fn watch_publish_config_files(
        &self,
        req: WatchConfigGroupRequest,
    ) -> Result<WatchConfigGroupResponse, PolarisError>;
}
