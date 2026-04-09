// Tencent is pleased to support the open source community by making Pole available.
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

use std::collections::HashMap;

use crate::core::model::error::PoleError;
use crate::core::model::ratelimit::{AmountInfo, LocalQuotaInfo, RemoteUpdate};

use super::plugins::Plugin;

/// ServiceRateLimiter 服务速率限制器
#[async_trait::async_trait]
pub trait ServiceRateLimiter: Plugin {
    // allocate_quota 申请配额
    async fn allocate_quota(&self) -> Result<bool, PoleError>;
    // return_quota 归还配额
    async fn return_quota(&self) -> Result<bool, PoleError>;

    /// on_remote_update 处理远程更新
    /// - QuotaSync: 分布式场景下的配额同步
    /// - RemoteQuotaUpdate: 分布式场景下完整的远程配额信息
    /// - ConfigUpdate: 规则变化引起的配置更新
    async fn on_remote_update(&self, update: RemoteUpdate<'_>) -> Result<(), PoleError>;

    /// fetch_local_usage 获取本地配额使用情况
    /// 默认实现返回空 HashMap（供本地限流器使用）
    /// 分布式限流器应覆盖此方法返回实际使用数据
    async fn fetch_local_usage_info(&self, _cur_time_ms: u64) -> HashMap<i32, LocalQuotaInfo> {
        HashMap::new()
    }

    /// get_amount_info 获取限流阈值信息
    /// 默认实现返回空 HashMap
    /// 分布式限流器应覆盖此方法返回实际阈值数据
    fn get_amount_info(&self) -> HashMap<i32, AmountInfo> {
        HashMap::new()
    }

    // fetch_local_usage 获取本地使用情况（旧接口，保持兼容）
    async fn fetch_local_usage(&self) -> Result<(), PoleError>;
    // get_amount 获取数量（旧接口，保持兼容）
    async fn get_amount(&self) -> Result<(), PoleError>;
}
