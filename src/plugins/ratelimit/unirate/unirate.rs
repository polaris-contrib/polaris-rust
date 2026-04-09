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
    sync::{
        atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
        Arc,
    },
};

use tokio::sync::RwLock;

use crate::core::{
    model::{
        error::PoleError,
        ratelimit::{AmountInfo, LimiterConfig, LocalQuotaInfo, RemoteUpdate},
    },
    plugin::{plugins::Plugin, ratelimit::ServiceRateLimiter},
};

static PLUGIN_NAME: &str = "unirate";

pub struct TokenBucket {
    pub rate: AtomicI64,
    pub burst: AtomicI64,
    pub tokens: AtomicI64,
    pub last_time: AtomicU64,
    pub enabled: AtomicBool,
    pub quota_used: AtomicI64,
    pub quota_limited: AtomicI64,
}

impl TokenBucket {
    pub fn new(rate: i64, burst: i64) -> Self {
        Self {
            rate: AtomicI64::new(rate),
            burst: AtomicI64::new(burst),
            tokens: AtomicI64::new(burst),
            last_time: AtomicU64::new(now_nanos()),
            enabled: AtomicBool::new(true),
            quota_used: AtomicI64::new(0),
            quota_limited: AtomicI64::new(0),
        }
    }

    fn update_config(&self, config: &crate::core::model::ratelimit::QuotaConfig) {
        let amount = config.amount;
        self.rate.store(amount, Ordering::SeqCst);
        self.burst.store(amount, Ordering::SeqCst);
        let current_tokens = self.tokens.load(Ordering::SeqCst);
        if current_tokens > amount {
            self.tokens.store(amount, Ordering::SeqCst);
        }
    }

    fn apply_remote_quota(&self, left_quota: i64, client_count: i32) {
        if !self.enabled.load(Ordering::SeqCst) {
            return;
        }

        let allocated_tokens = if client_count > 0 {
            left_quota / client_count as i64
        } else {
            left_quota
        };

        let burst = self.burst.load(Ordering::SeqCst);
        let final_tokens = allocated_tokens.min(burst).max(0);

        self.tokens.store(final_tokens, Ordering::SeqCst);
        self.last_time.store(now_nanos(), Ordering::SeqCst);
    }

    fn try_acquire(&self, num: i64) -> bool {
        if !self.enabled.load(Ordering::SeqCst) {
            return true;
        }

        if num <= 0 {
            return true;
        }

        let burst = self.burst.load(Ordering::SeqCst);
        if num > burst {
            return false;
        }

        let now = now_nanos();
        let last_time = self.last_time.load(Ordering::SeqCst);

        let elapsed_nanos = now.saturating_sub(last_time) as f64;
        let period_nanos = self.get_period_nanos();
        let new_tokens = if period_nanos > 0 {
            (elapsed_nanos / period_nanos as f64) * self.rate.load(Ordering::SeqCst) as f64
        } else {
            0.0
        };

        let current_tokens = self.tokens.load(Ordering::SeqCst);
        let updated_tokens = (current_tokens as f64 + new_tokens).min(burst as f64) as i64;

        if updated_tokens >= num {
            loop {
                let current = self.tokens.load(Ordering::SeqCst);
                if current < num {
                    return false;
                }

                let new_count = current - num;
                match self.tokens.compare_exchange_weak(
                    current,
                    new_count,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => {
                        self.last_time.store(now, Ordering::SeqCst);
                        return true;
                    }
                    Err(_) => {
                        continue;
                    }
                }
            }
        } else {
            false
        }
    }

    fn get_period_nanos(&self) -> u64 {
        1_000_000_000
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }
}

pub struct UnirateLimiter {
    pub token_bucket: Arc<TokenBucket>,
    pub config: RwLock<Option<LimiterConfig>>,
}

impl UnirateLimiter {
    pub fn builder() -> (fn() -> Box<dyn ServiceRateLimiter>, String) {
        (new_instance, PLUGIN_NAME.to_string())
    }
}

fn new_instance() -> Box<dyn ServiceRateLimiter> {
    Box::new(UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    })
}

impl Plugin for UnirateLimiter {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        PLUGIN_NAME.to_string()
    }
}

#[async_trait::async_trait]
impl ServiceRateLimiter for UnirateLimiter {
    async fn allocate_quota(&self) -> Result<bool, PoleError> {
        let allowed = self.token_bucket.try_acquire(1);
        if allowed {
            self.token_bucket.quota_used.fetch_add(1, Ordering::Relaxed);
        } else {
            self.token_bucket
                .quota_limited
                .fetch_add(1, Ordering::Relaxed);
        }
        Ok(allowed)
    }

    async fn return_quota(&self) -> Result<bool, PoleError> {
        Ok(true)
    }

    async fn on_remote_update(&self, update: RemoteUpdate<'_>) -> Result<(), PoleError> {
        match update {
            RemoteUpdate::ConfigUpdate(config) => {
                let mut config_guard = self.config.write().await;
                *config_guard = Some(config.clone());
                drop(config_guard);

                self.token_bucket.update_config(&config.quota);
                Ok(())
            }
            RemoteUpdate::RemoteQuotaUpdate(quota_info) => {
                self.token_bucket
                    .apply_remote_quota(quota_info.left_quota, quota_info.client_count);
                Ok(())
            }
            RemoteUpdate::QuotaSync(_) => Ok(()),
        }
    }

    async fn fetch_local_usage_info(&self, _cur_time_ms: u64) -> HashMap<i32, LocalQuotaInfo> {
        let config_guard = self.config.read().await;
        let Some(config) = config_guard.as_ref() else {
            return HashMap::new();
        };
        let duration_key = config.quota.period.as_secs() as i32;
        drop(config_guard);

        let mut usage = HashMap::new();
        usage.insert(
            duration_key.max(1),
            LocalQuotaInfo {
                quota_used: self.token_bucket.quota_used.swap(0, Ordering::Relaxed),
                quota_limited: self.token_bucket.quota_limited.swap(0, Ordering::Relaxed),
            },
        );
        usage
    }

    fn get_amount_info(&self) -> HashMap<i32, AmountInfo> {
        let Some(config) = self.config.try_read().ok().and_then(|guard| guard.clone()) else {
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

fn now_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}
