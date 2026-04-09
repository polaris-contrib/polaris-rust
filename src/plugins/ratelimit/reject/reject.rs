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
    sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
};

use tokio::sync::RwLock;

use crate::core::{
    model::{
        error::PoleError,
        ratelimit::{AmountInfo, LimiterConfig, LocalQuotaInfo, RemoteUpdate},
    },
    plugin::{plugins::Plugin, ratelimit::ServiceRateLimiter},
};

static PLUGIN_NAME: &str = "reject";

/// RejectLimiter 拒绝限流器，根据配置决定是否拒绝所有请求
pub struct RejectLimiter {
    /// 是否启用拒绝限流
    pub enabled: AtomicBool,
    /// 当前配置的哈希值，用于检测是否需要更新
    pub config_hash: AtomicU64,
    /// 拒绝所有请求的模式
    pub reject_all: AtomicBool,
    /// 存储当前配置（可用于后续扩展）
    pub current_config: RwLock<Option<LimiterConfig>>,
    /// 本地使用统计
    pub quota_used: AtomicI64,
    pub quota_limited: AtomicI64,
}

impl RejectLimiter {
    pub fn builder() -> (fn() -> Box<dyn ServiceRateLimiter>, String) {
        (new_instance, PLUGIN_NAME.to_string())
    }
}

fn new_instance() -> Box<dyn ServiceRateLimiter> {
    Box::new(RejectLimiter {
        enabled: AtomicBool::new(true),
        config_hash: AtomicU64::new(0),
        reject_all: AtomicBool::new(false),
        current_config: RwLock::new(None),
        quota_used: AtomicI64::new(0),
        quota_limited: AtomicI64::new(0),
    })
}

impl Plugin for RejectLimiter {
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
impl ServiceRateLimiter for RejectLimiter {
    async fn allocate_quota(&self) -> Result<bool, PoleError> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(true);
        }

        let should_reject = self.reject_all.load(Ordering::Relaxed);
        if should_reject {
            self.quota_limited.fetch_add(1, Ordering::Relaxed);
            Ok(false)
        } else {
            self.quota_used.fetch_add(1, Ordering::Relaxed);
            Ok(true)
        }
    }

    async fn return_quota(&self) -> Result<bool, PoleError> {
        Ok(true)
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
                self.reject_all.store(allocated <= 0, Ordering::Relaxed);
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
        Ok(())
    }

    async fn get_amount(&self) -> Result<(), PoleError> {
        Ok(())
    }
}

impl RejectLimiter {
    async fn update_config(&self, config: &LimiterConfig) -> Result<(), PoleError> {
        {
            let mut config_guard = self.current_config.write().await;
            *config_guard = Some(config.clone());
        }

        let should_reject_all = self.should_reject_all(config);
        self.reject_all.store(should_reject_all, Ordering::Relaxed);

        Ok(())
    }

    fn should_reject_all(&self, config: &LimiterConfig) -> bool {
        if config.response.message.contains("emergency")
            || config.response.message.contains("EMERGENCY")
        {
            return true;
        }

        if config.limiter_type == "reject" && config.quota.amount < 0 {
            return true;
        }

        false
    }
}
