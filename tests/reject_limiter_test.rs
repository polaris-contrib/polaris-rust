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
use pole_client_rust::plugins::ratelimit::reject::reject::RejectLimiter;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64};
use tokio::sync::RwLock;

#[tokio::test]
async fn test_reject_limiter_allocate_quota() {
    let limiter = RejectLimiter::builder().0();
    let result = limiter.allocate_quota().await;
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_reject_limiter_return_quota() {
    let limiter = RejectLimiter::builder().0();
    let result = limiter.return_quota().await;
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_reject_limiter_name() {
    let limiter = RejectLimiter::builder().0();
    assert_eq!(limiter.name(), "reject");
}

#[test]
fn test_reject_limiter_plugin_init() {
    let mut limiter = RejectLimiter::builder().0();
    limiter.init();
}

#[test]
fn test_reject_limiter_plugin_destroy() {
    let limiter = RejectLimiter::builder().0();
    limiter.destroy();
}

#[tokio::test]
async fn test_reject_limiter_remote_update_quota_sync() {
    use pole_client_rust::core::model::ratelimit::{QuotaSyncData, RemoteUpdate};

    let limiter = RejectLimiter::builder().0();
    let sync_data = QuotaSyncData {
        remote_quota: 0,
        sync_timestamp: 1111111111,
    };
    let update = RemoteUpdate::QuotaSync(&sync_data);
    let result = limiter.on_remote_update(update).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reject_limiter_remote_update_config_update() {
    use pole_client_rust::core::model::ratelimit::{
        LimiterConfig, QuotaConfig, QuotaMode, RemoteUpdate, ResponseConfig, WindowConfig,
    };
    use std::time::Duration;

    let limiter = RejectLimiter::builder().0();
    let config = LimiterConfig {
        window: WindowConfig {
            window_size: Duration::from_secs(10),
            expire_duration: Duration::from_secs(60),
        },
        quota: QuotaConfig {
            amount: 0,
            period: Duration::from_secs(1),
        },
        response: ResponseConfig {
            message: "Request rejected".to_string(),
            status: Some(403),
        },
        config_hash: 99999,
        limiter_type: "reject".to_string(),
        mode: QuotaMode::Local,
    };
    let update = RemoteUpdate::ConfigUpdate(&config);
    let result = limiter.on_remote_update(update).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reject_limiter_fetch_local_usage() {
    let limiter = RejectLimiter::builder().0();
    let result = limiter.fetch_local_usage().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reject_limiter_get_amount() {
    let limiter = RejectLimiter::builder().0();
    let result = limiter.get_amount().await;
    assert!(result.is_ok());
}

#[test]
fn test_reject_limiter_builder() {
    let (factory, name) = RejectLimiter::builder();
    assert_eq!(name, "reject");
    let limiter = factory();
    assert_eq!(limiter.name(), "reject");
}

#[tokio::test]
async fn test_reject_limiter_multiple_allocate() {
    let limiter = RejectLimiter::builder().0();
    for _ in 0..10 {
        let result = limiter.allocate_quota().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}

#[tokio::test]
async fn test_reject_limiter_allocate_and_return() {
    let limiter = RejectLimiter::builder().0();
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
async fn test_reject_limiter_remote_quota_update_switches_reject_mode() {
    use pole_client_rust::core::model::ratelimit::{RemoteQuotaInfo, RemoteUpdate};

    let limiter = RejectLimiter {
        enabled: AtomicBool::new(true),
        config_hash: AtomicU64::new(0),
        reject_all: AtomicBool::new(false),
        current_config: RwLock::new(None),
        quota_used: AtomicI64::new(0),
        quota_limited: AtomicI64::new(0),
    };

    let deny_update = RemoteQuotaInfo {
        left_quota: 0,
        client_count: 1,
        timestamp_ms: 0,
        duration_ms: 1000,
    };
    limiter
        .on_remote_update(RemoteUpdate::RemoteQuotaUpdate(&deny_update))
        .await
        .unwrap();
    assert!(!limiter.allocate_quota().await.unwrap());

    let allow_update = RemoteQuotaInfo {
        left_quota: 10,
        client_count: 1,
        timestamp_ms: 0,
        duration_ms: 1000,
    };
    limiter
        .on_remote_update(RemoteUpdate::RemoteQuotaUpdate(&allow_update))
        .await
        .unwrap();
    assert!(limiter.allocate_quota().await.unwrap());
}
