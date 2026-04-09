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

use pole_client_rust::core::model::ratelimit::{
    LimiterConfig, QuotaConfig, QuotaMode, ResponseConfig, WindowConfig,
};
use pole_client_rust::core::plugin::plugins::Plugin;
use pole_client_rust::core::plugin::ratelimit::ServiceRateLimiter;
use pole_client_rust::plugins::ratelimit::unirate::unirate::{TokenBucket, UnirateLimiter};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_unirate_limiter_allocate_quota() {
    let limiter = UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    };
    let result = limiter.allocate_quota().await;
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_unirate_limiter_return_quota() {
    let limiter = UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    };
    let result = limiter.return_quota().await;
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_unirate_limiter_name() {
    let limiter = UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    };
    assert_eq!(limiter.name(), "unirate");
}

#[test]
fn test_unirate_limiter_plugin_init() {
    let mut limiter = UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    };
    limiter.init();
    // init should not panic
}

#[test]
fn test_unirate_limiter_plugin_destroy() {
    let limiter = UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    };
    limiter.destroy();
    // destroy should not panic
}

#[tokio::test]
async fn test_unirate_limiter_remote_update_quota_sync() {
    use pole_client_rust::core::model::ratelimit::{QuotaSyncData, RemoteUpdate};

    let limiter = UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    };
    let sync_data = QuotaSyncData {
        remote_quota: 200,
        sync_timestamp: 9876543210,
    };
    let update = RemoteUpdate::QuotaSync(&sync_data);
    let result = limiter.on_remote_update(update).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_unirate_limiter_remote_update_config_update() {
    use pole_client_rust::core::model::ratelimit::RemoteUpdate;

    let limiter = UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    };
    let config = LimiterConfig {
        window: WindowConfig {
            window_size: Duration::from_secs(30),
            expire_duration: Duration::from_secs(180),
        },
        quota: QuotaConfig {
            amount: 500,
            period: Duration::from_secs(1),
        },
        response: ResponseConfig {
            message: "QPS limit exceeded".to_string(),
            status: Some(429),
        },
        config_hash: 54321,
        limiter_type: "unirate".to_string(),
        mode: QuotaMode::Local,
    };
    let update = RemoteUpdate::ConfigUpdate(&config);
    let result = limiter.on_remote_update(update).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_unirate_limiter_fetch_local_usage() {
    let limiter = UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    };
    let result = limiter.fetch_local_usage().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_unirate_limiter_get_amount() {
    let limiter = UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    };
    let result = limiter.get_amount().await;
    assert!(result.is_ok());
}

#[test]
fn test_unirate_limiter_builder() {
    let (factory, name) = UnirateLimiter::builder();
    assert_eq!(name, "unirate");
    let limiter = factory();
    assert_eq!(limiter.name(), "unirate");
}

#[tokio::test]
async fn test_unirate_limiter_multiple_allocate() {
    let limiter = UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    };
    // Test multiple consecutive allocate calls
    for _ in 0..10 {
        let result = limiter.allocate_quota().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}

#[tokio::test]
async fn test_unirate_limiter_allocate_and_return() {
    let limiter = UnirateLimiter {
        token_bucket: Arc::new(TokenBucket::new(100, 100)),
        config: RwLock::new(None),
    };
    // Test allocate and return cycle
    for _ in 0..5 {
        let allocate_result = limiter.allocate_quota().await;
        assert!(allocate_result.is_ok());
        assert!(allocate_result.unwrap());

        let return_result = limiter.return_quota().await;
        assert!(return_result.is_ok());
        assert!(return_result.unwrap());
    }
}
