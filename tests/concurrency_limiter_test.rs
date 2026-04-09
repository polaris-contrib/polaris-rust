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

use pole_client_rust::core::plugin::plugins::Plugin;
use pole_client_rust::core::plugin::ratelimit::ServiceRateLimiter;
use pole_client_rust::plugins::ratelimit::concurrency::concurrency::ConcurrencyLimiter;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize};
use tokio::sync::RwLock;

#[tokio::test]
async fn test_concurrency_limiter_allocate_quota() {
    let limiter = ConcurrencyLimiter::builder().0();
    let result = limiter.allocate_quota().await;
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_concurrency_limiter_return_quota() {
    let limiter = ConcurrencyLimiter::builder().0();
    let result = limiter.return_quota().await;
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_concurrency_limiter_return_quota_should_not_underflow() {
    let limiter = ConcurrencyLimiter {
        active_count: AtomicUsize::new(0),
        max_concurrency: AtomicUsize::new(1),
        config_hash: AtomicU64::new(0),
        enabled: AtomicBool::new(true),
        current_config: RwLock::new(None),
        quota_used: AtomicI64::new(0),
        quota_limited: AtomicI64::new(0),
    };

    limiter.return_quota().await.unwrap();
    assert_eq!(
        limiter
            .active_count
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn test_concurrency_limiter_name() {
    let limiter = ConcurrencyLimiter::builder().0();
    assert_eq!(limiter.name(), "concurrency");
}

#[test]
fn test_concurrency_limiter_plugin_init() {
    let mut limiter = ConcurrencyLimiter::builder().0();
    limiter.init();
}

#[test]
fn test_concurrency_limiter_plugin_destroy() {
    let limiter = ConcurrencyLimiter::builder().0();
    limiter.destroy();
}

#[tokio::test]
async fn test_concurrency_limiter_remote_update_quota_sync() {
    use pole_client_rust::core::model::ratelimit::{QuotaSyncData, RemoteUpdate};

    let limiter = ConcurrencyLimiter::builder().0();
    let sync_data = QuotaSyncData {
        remote_quota: 100,
        sync_timestamp: 1234567890,
    };
    let update = RemoteUpdate::QuotaSync(&sync_data);
    let result = limiter.on_remote_update(update).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_concurrency_limiter_remote_update_config_update() {
    use pole_client_rust::core::model::ratelimit::{
        LimiterConfig, QuotaConfig, QuotaMode, RemoteUpdate, ResponseConfig, WindowConfig,
    };
    use std::time::Duration;

    let limiter = ConcurrencyLimiter::builder().0();
    let config = LimiterConfig {
        window: WindowConfig {
            window_size: Duration::from_secs(60),
            expire_duration: Duration::from_secs(300),
        },
        quota: QuotaConfig {
            amount: 1000,
            period: Duration::from_secs(1),
        },
        response: ResponseConfig {
            message: "Rate limit exceeded".to_string(),
            status: Some(429),
        },
        config_hash: 12345,
        limiter_type: "concurrency".to_string(),
        mode: QuotaMode::Local,
    };
    let update = RemoteUpdate::ConfigUpdate(&config);
    let result = limiter.on_remote_update(update).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_concurrency_limiter_fetch_local_usage() {
    let limiter = ConcurrencyLimiter::builder().0();
    let result = limiter.fetch_local_usage().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_concurrency_limiter_get_amount() {
    let limiter = ConcurrencyLimiter::builder().0();
    let result = limiter.get_amount().await;
    assert!(result.is_ok());
}

#[test]
fn test_concurrency_limiter_builder() {
    let (factory, name) = ConcurrencyLimiter::builder();
    assert_eq!(name, "concurrency");
    let limiter = factory();
    assert_eq!(limiter.name(), "concurrency");
}

#[tokio::test]
async fn test_concurrency_limiter_multiple_allocate() {
    let limiter = ConcurrencyLimiter::builder().0();
    for _ in 0..10 {
        let result = limiter.allocate_quota().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}

#[tokio::test]
async fn test_concurrency_limiter_allocate_and_return() {
    let limiter = ConcurrencyLimiter::builder().0();
    for _ in 0..5 {
        let allocate_result = limiter.allocate_quota().await;
        assert!(allocate_result.is_ok());
        assert!(allocate_result.unwrap());

        let return_result = limiter.return_quota().await;
        assert!(return_result.is_ok());
        assert!(return_result.unwrap());
    }
}

#[tokio::test]
async fn test_concurrency_limiter_limit_enforced() {
    // 测试并发限制是否真正生效
    let limiter = ConcurrencyLimiter::builder().0();

    // 获取所有可用的配额
    for _ in 0..100 {
        let result = limiter.allocate_quota().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    // 下一次应该被拒绝
    let result = limiter.allocate_quota().await;
    assert!(result.is_ok());
    assert!(!result.unwrap(), "应该拒绝超过最大并发数的请求");
}

#[tokio::test]
async fn test_concurrency_limiter_remote_quota_update_enforced() {
    use pole_client_rust::core::model::ratelimit::{RemoteQuotaInfo, RemoteUpdate};

    let limiter = ConcurrencyLimiter {
        active_count: AtomicUsize::new(0),
        max_concurrency: AtomicUsize::new(100),
        config_hash: AtomicU64::new(0),
        enabled: AtomicBool::new(true),
        current_config: RwLock::new(None),
        quota_used: AtomicI64::new(0),
        quota_limited: AtomicI64::new(0),
    };

    let update = RemoteQuotaInfo {
        left_quota: 2,
        client_count: 1,
        timestamp_ms: 0,
        duration_ms: 1000,
    };
    limiter
        .on_remote_update(RemoteUpdate::RemoteQuotaUpdate(&update))
        .await
        .unwrap();

    assert!(limiter.allocate_quota().await.unwrap());
    assert!(limiter.allocate_quota().await.unwrap());
    assert!(!limiter.allocate_quota().await.unwrap());
}
