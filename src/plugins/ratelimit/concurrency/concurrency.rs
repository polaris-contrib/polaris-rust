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

use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering},
};

use tokio::sync::RwLock;

use crate::core::{
    model::{
        error::PoleError,
        ratelimit::{AmountInfo, LimiterConfig, LocalQuotaInfo, RemoteUpdate},
    },
    plugin::{plugins::Plugin, ratelimit::ServiceRateLimiter},
};

static PLUGIN_NAME: &str = "concurrency";

/// ConcurrencyLimiter 并发限流器，使用原子计数器实现
pub struct ConcurrencyLimiter {
    /// 当前活跃的请求数
    pub active_count: AtomicUsize,
    /// 最大并发数
    pub max_concurrency: AtomicUsize,
    /// 当前配置的哈希值，用于检测是否需要更新
    pub config_hash: AtomicU64,
    /// 是否启用限流
    pub enabled: AtomicBool,
    /// 当前配置
    pub current_config: RwLock<Option<LimiterConfig>>,
    /// 本地使用统计
    pub quota_used: AtomicI64,
    pub quota_limited: AtomicI64,
}

impl ConcurrencyLimiter {
    pub fn builder() -> (fn() -> Box<dyn ServiceRateLimiter>, String) {
        (new_instance, PLUGIN_NAME.to_string())
    }
}

fn new_instance() -> Box<dyn ServiceRateLimiter> {
    Box::new(ConcurrencyLimiter {
        active_count: AtomicUsize::new(0),
        max_concurrency: AtomicUsize::new(100), // 默认最大并发数
        config_hash: AtomicU64::new(0),
        enabled: AtomicBool::new(true),
        current_config: RwLock::new(None),
        quota_used: AtomicI64::new(0),
        quota_limited: AtomicI64::new(0),
    })
}

impl Plugin for ConcurrencyLimiter {
    fn init(&mut self) {
        // 初始化配置
    }

    fn destroy(&self) {
        // 清理资源
    }

    fn name(&self) -> String {
        PLUGIN_NAME.to_string()
    }
}

#[async_trait::async_trait]
impl ServiceRateLimiter for ConcurrencyLimiter {
    async fn allocate_quota(&self) -> Result<bool, PoleError> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(true); // 如果未启用限流，则始终允许
        }

        let max = self.max_concurrency.load(Ordering::Relaxed);

        // 使用CAS循环来尝试增加活跃计数
        loop {
            let current = self.active_count.load(Ordering::Relaxed);

            if current >= max {
                // 已达到最大并发数，拒绝请求
                self.quota_limited.fetch_add(1, Ordering::Relaxed);
                return Ok(false);
            }

            match self.active_count.compare_exchange_weak(
                current,
                current + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // 成功增加计数，允许请求
                    self.quota_used.fetch_add(1, Ordering::Relaxed);
                    return Ok(true);
                }
                Err(_) => {
                    // 并发冲突，重试
                    continue;
                }
            }
        }
    }

    async fn return_quota(&self) -> Result<bool, PoleError> {
        // 使用 CAS 保证在 0 时不会下溢到 usize::MAX。
        loop {
            let current = self.active_count.load(Ordering::Acquire);
            if current == 0 {
                return Ok(true);
            }

            if self
                .active_count
                .compare_exchange_weak(current, current - 1, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                return Ok(true);
            }
        }
    }

    async fn on_remote_update(&self, update: RemoteUpdate<'_>) -> Result<(), PoleError> {
        match update {
            RemoteUpdate::ConfigUpdate(config) => {
                let new_hash = config.config_hash;
                let current_hash = self.config_hash.load(Ordering::Relaxed);

                if new_hash != current_hash {
                    self.update_config(config).await?;
                    self.config_hash.store(new_hash, Ordering::Relaxed);
                }

                Ok(())
            }
            RemoteUpdate::RemoteQuotaUpdate(remote) => {
                let allocated = if remote.client_count > 0 {
                    remote.left_quota / remote.client_count as i64
                } else {
                    remote.left_quota
                };
                let global_max = allocated.max(0) as usize;
                self.max_concurrency.store(global_max, Ordering::Relaxed);
                Ok(())
            }
            RemoteUpdate::QuotaSync(_) => Ok(()),
        }
    }

    async fn fetch_local_usage_info(&self, _cur_time_ms: u64) -> HashMap<i32, LocalQuotaInfo> {
        let guard = self.current_config.read().await;
        let Some(config) = guard.as_ref() else {
            return HashMap::new();
        };
        let duration_key = config.quota.period.as_secs() as i32;
        drop(guard);

        let mut usage = HashMap::new();
        usage.insert(
            duration_key.max(1),
            LocalQuotaInfo {
                quota_used: self.quota_used.swap(0, Ordering::Relaxed),
                quota_limited: self.quota_limited.swap(0, Ordering::Relaxed),
            },
        );
        usage
    }

    fn get_amount_info(&self) -> HashMap<i32, AmountInfo> {
        let Some(config) = self
            .current_config
            .try_read()
            .ok()
            .and_then(|guard| guard.clone())
        else {
            return HashMap::new();
        };

        let duration_key = config.quota.period.as_secs() as i32;
        let mut amount = HashMap::new();
        amount.insert(
            duration_key.max(1),
            AmountInfo {
                max_amount: config.quota.amount,
                duration_secs: config.quota.period.as_secs() as i64,
                mode: config.mode,
            },
        );
        amount
    }

    async fn fetch_local_usage(&self) -> Result<(), PoleError> {
        // 并发限流不需要上报本地使用情况
        Ok(())
    }

    async fn get_amount(&self) -> Result<(), PoleError> {
        // 返回当前活跃的并发数
        Ok(())
    }
}

impl ConcurrencyLimiter {
    /// 更新配置
    async fn update_config(&self, config: &LimiterConfig) -> Result<(), PoleError> {
        let mut cfg = self.current_config.write().await;
        *cfg = Some(config.clone());
        drop(cfg);

        let new_max_concurrency = (config.quota.amount.max(1)) as usize; // 至少为1
        self.max_concurrency
            .store(new_max_concurrency, Ordering::Relaxed);
        Ok(())
    }
}
