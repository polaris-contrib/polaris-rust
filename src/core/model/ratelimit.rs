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

use std::time::Duration;

/// QuotaMode 限流模式
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QuotaMode {
    /// 本地限流模式：单机限流，无需与远程服务器通信
    Local,
    /// 分布式限流模式：全局限流，需要与限流服务器交互获取配额
    Global,
}

impl Default for QuotaMode {
    fn default() -> Self {
        QuotaMode::Local
    }
}

/// WindowStatus 限流窗口状态
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowStatus {
    /// 已创建，无需进行后台调度
    Created,
    /// 已获取调度权，准备开始初始化
    Initializing,
    /// 已在远程初始化完成
    Initialized,
    /// 已删除
    Deleted,
}

impl Default for WindowStatus {
    fn default() -> Self {
        WindowStatus::Created
    }
}

/// RemoteQuotaInfo 远程配额信息（用于分布式限流）
#[derive(Clone, Debug)]
pub struct RemoteQuotaInfo {
    /// 剩余配额数量
    pub left_quota: i64,
    /// 当前客户端数量
    pub client_count: i32,
    /// 服务器时间戳（毫秒）
    pub timestamp_ms: u64,
    /// 配额周期（毫秒）
    pub duration_ms: i64,
}

/// LocalQuotaInfo 本地配额使用信息
#[derive(Clone, Debug, Default)]
pub struct LocalQuotaInfo {
    /// 已使用的配额数量
    pub quota_used: i64,
    /// 被限流的配额数量（请求被拒绝的次数）
    pub quota_limited: i64,
}

/// AmountInfo 限流阈值信息
#[derive(Clone, Debug)]
pub struct AmountInfo {
    /// 最大配额数量
    pub max_amount: i64,
    /// 统计周期（秒）
    pub duration_secs: i64,
    /// 限流模式
    pub mode: QuotaMode,
}

/// QuotaSyncData 配额同步数据（用于分布式限流场景）
#[derive(Clone, Debug)]
pub struct QuotaSyncData {
    /// 远程配额分配信息
    pub remote_quota: i64,
    /// 同步时间戳
    pub sync_timestamp: u64,
    // 预留字段，供分布式限流扩展使用
}

/// LimiterConfig 限流器配置
#[derive(Clone, Debug)]
pub struct LimiterConfig {
    /// 窗口配置
    pub window: WindowConfig,
    /// 配额配置
    pub quota: QuotaConfig,
    /// 响应配置
    pub response: ResponseConfig,
    /// 配置哈希（用于变更检测）
    pub config_hash: u64,
    /// limiter 插件名称（如 "unirate", "concurrency", "reject"）
    pub limiter_type: String,
    /// 限流模式（Local / Global）
    pub mode: QuotaMode,
}

/// WindowConfig 窗口配置
#[derive(Clone, Debug)]
pub struct WindowConfig {
    /// 窗口大小
    pub window_size: Duration,
    /// 过期时长（无流量时多久后清理）
    pub expire_duration: Duration,
}

/// QuotaConfig 配额配置
#[derive(Clone, Debug)]
pub struct QuotaConfig {
    /// 配额数量（从 LimitTrigger.amounts 中解析）
    pub amount: i64,
    /// 统计周期
    pub period: Duration,
}

/// ResponseConfig 响应配置
#[derive(Clone, Debug)]
pub struct ResponseConfig {
    /// 限流提示信息
    pub message: String,
    /// HTTP 状态码（可选）
    pub status: Option<i32>,
}

/// RemoteUpdate 远程更新类型
#[derive(Debug)]
pub enum RemoteUpdate<'a> {
    /// 配额状态同步（分布式限流场景，旧格式，兼容已有代码）
    QuotaSync(&'a QuotaSyncData),
    /// 远程配额更新（分布式限流，完整的远程配额信息）
    RemoteQuotaUpdate(&'a RemoteQuotaInfo),
    /// 配置更新（规则变化）
    ConfigUpdate(&'a LimiterConfig),
}
