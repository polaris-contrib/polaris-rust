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

use crate::core::model::error::PolarisError;

use super::plugins::Plugin;

/// ServiceRateLimiter 服务速率限制器
#[async_trait::async_trait]
pub trait ServiceRateLimiter: Plugin {
    // allocate_quota 申请配额
    async fn allocate_quota(&self) -> Result<(), PolarisError>;
    // return_quota 归还配额
    async fn return_quota(&self) -> Result<(), PolarisError>;
    // on_remote_update 远程更新
    async fn on_remote_update(&self) -> Result<(), PolarisError>;
    // fetch_local_usage 获取本地使用情况
    async fn fetch_local_usage(&self) -> Result<(), PolarisError>;
    // get_amount 获取数量
    async fn get_amount(&self) -> Result<(), PolarisError>;
}
