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
    AmountInfo, LimiterConfig, LocalQuotaInfo, QuotaConfig, QuotaMode, QuotaSyncData,
    RemoteQuotaInfo, RemoteUpdate, ResponseConfig, WindowConfig, WindowStatus,
};
use std::time::Duration;

// ==================== LimiterConfig Tests ====================

#[test]
fn test_limiter_config_default() {
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

    assert_eq!(config.window.window_size, Duration::from_secs(60));
    assert_eq!(config.quota.amount, 1000);
    assert_eq!(config.response.status, Some(429));
    assert_eq!(config.limiter_type, "concurrency");
}

#[test]
fn test_limiter_config_clone() {
    let config1 = LimiterConfig {
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

    let config2 = config1.clone();

    assert_eq!(config1.window.window_size, config2.window.window_size);
    assert_eq!(config1.quota.amount, config2.quota.amount);
    assert_eq!(config1.limiter_type, config2.limiter_type);
}

#[test]
fn test_limiter_config_debug() {
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

    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("LimiterConfig"));
    assert!(debug_str.contains("window_size"));
    assert!(debug_str.contains("amount"));
}

// ==================== WindowConfig Tests ====================

#[test]
fn test_window_config_default() {
    let window = WindowConfig {
        window_size: Duration::from_secs(60),
        expire_duration: Duration::from_secs(300),
    };

    assert_eq!(window.window_size, Duration::from_secs(60));
    assert_eq!(window.expire_duration, Duration::from_secs(300));
}

#[test]
fn test_window_config_with_duration() {
    let window = WindowConfig {
        window_size: Duration::from_secs(30),
        expire_duration: Duration::from_secs(180),
    };

    assert!(window.window_size < window.expire_duration);
}

#[test]
fn test_window_config_clone() {
    let window1 = WindowConfig {
        window_size: Duration::from_secs(60),
        expire_duration: Duration::from_secs(300),
    };

    let window2 = window1.clone();

    assert_eq!(window1.window_size, window2.window_size);
    assert_eq!(window1.expire_duration, window2.expire_duration);
}

// ==================== QuotaConfig Tests ====================

#[test]
fn test_quota_config_default() {
    let quota = QuotaConfig {
        amount: 1000,
        period: Duration::from_secs(1),
    };

    assert_eq!(quota.amount, 1000);
    assert_eq!(quota.period, Duration::from_secs(1));
}

#[test]
fn test_quota_config_amount() {
    let quota = QuotaConfig {
        amount: 5000,
        period: Duration::from_secs(1),
    };

    assert!(quota.amount > 0);
}

#[test]
fn test_quota_config_period() {
    let quota = QuotaConfig {
        amount: 100,
        period: Duration::from_secs(60),
    };

    assert_eq!(quota.period, Duration::from_secs(60));
}

#[test]
fn test_quota_config_clone() {
    let quota1 = QuotaConfig {
        amount: 1000,
        period: Duration::from_secs(1),
    };

    let quota2 = quota1.clone();

    assert_eq!(quota1.amount, quota2.amount);
    assert_eq!(quota1.period, quota2.period);
}

// ==================== ResponseConfig Tests ====================

#[test]
fn test_response_config_default() {
    let response = ResponseConfig {
        message: "Rate limit exceeded".to_string(),
        status: Some(429),
    };

    assert_eq!(response.message, "Rate limit exceeded");
    assert_eq!(response.status, Some(429));
}

#[test]
fn test_response_config_without_status() {
    let response = ResponseConfig {
        message: "Rate limit exceeded".to_string(),
        status: None,
    };

    assert!(response.message.len() > 0);
    assert!(response.status.is_none());
}

#[test]
fn test_response_config_clone() {
    let response1 = ResponseConfig {
        message: "Rate limit exceeded".to_string(),
        status: Some(429),
    };

    let response2 = response1.clone();

    assert_eq!(response1.message, response2.message);
    assert_eq!(response1.status, response2.status);
}

// ==================== RemoteUpdate Tests ====================

#[test]
fn test_remote_update_quota_sync() {
    let sync_data = QuotaSyncData {
        remote_quota: 100,
        sync_timestamp: 1234567890,
    };

    let update = RemoteUpdate::QuotaSync(&sync_data);

    match update {
        RemoteUpdate::QuotaSync(data) => {
            assert_eq!(data.remote_quota, 100);
            assert_eq!(data.sync_timestamp, 1234567890);
        }
        _ => panic!("Expected QuotaSync variant"),
    }
}

#[test]
fn test_remote_update_config_update() {
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

    match update {
        RemoteUpdate::ConfigUpdate(cfg) => {
            assert_eq!(cfg.limiter_type, "concurrency");
            assert_eq!(cfg.quota.amount, 1000);
        }
        _ => panic!("Expected ConfigUpdate variant"),
    }
}

#[test]
fn test_remote_update_debug() {
    let sync_data = QuotaSyncData {
        remote_quota: 100,
        sync_timestamp: 1234567890,
    };
    let update = RemoteUpdate::QuotaSync(&sync_data);

    let debug_str = format!("{:?}", update);
    assert!(debug_str.contains("QuotaSync"));
}

// ==================== QuotaSyncData Tests ====================

#[test]
fn test_quota_sync_data_default() {
    let sync_data = QuotaSyncData {
        remote_quota: 100,
        sync_timestamp: 1234567890,
    };

    assert_eq!(sync_data.remote_quota, 100);
    assert_eq!(sync_data.sync_timestamp, 1234567890);
}

#[test]
fn test_quota_sync_data_clone() {
    let sync_data1 = QuotaSyncData {
        remote_quota: 100,
        sync_timestamp: 1234567890,
    };

    let sync_data2 = sync_data1.clone();

    assert_eq!(sync_data1.remote_quota, sync_data2.remote_quota);
    assert_eq!(sync_data1.sync_timestamp, sync_data2.sync_timestamp);
}

#[test]
fn test_quota_sync_data_timestamp() {
    let sync_data = QuotaSyncData {
        remote_quota: 0,
        sync_timestamp: 0,
    };

    assert_eq!(sync_data.sync_timestamp, 0);
}

#[test]
fn test_quota_sync_data_debug() {
    let sync_data = QuotaSyncData {
        remote_quota: 100,
        sync_timestamp: 1234567890,
    };

    let debug_str = format!("{:?}", sync_data);
    assert!(debug_str.contains("QuotaSyncData"));
    assert!(debug_str.contains("remote_quota"));
}

// ==================== Boundary Tests ====================

#[test]
fn test_limiter_config_zero_amount() {
    let config = LimiterConfig {
        window: WindowConfig {
            window_size: Duration::from_secs(60),
            expire_duration: Duration::from_secs(300),
        },
        quota: QuotaConfig {
            amount: 0,
            period: Duration::from_secs(1),
        },
        response: ResponseConfig {
            message: "Rate limit exceeded".to_string(),
            status: Some(429),
        },
        config_hash: 0,
        limiter_type: "reject".to_string(),
        mode: QuotaMode::Local,
    };

    assert_eq!(config.quota.amount, 0);
}

#[test]
fn test_limiter_config_large_values() {
    let config = LimiterConfig {
        window: WindowConfig {
            window_size: Duration::from_secs(86400),
            expire_duration: Duration::from_secs(604800),
        },
        quota: QuotaConfig {
            amount: i64::MAX,
            period: Duration::from_secs(3600),
        },
        response: ResponseConfig {
            message: "x".repeat(1000),
            status: Some(429),
        },
        config_hash: u64::MAX,
        limiter_type: "concurrency".to_string(),
        mode: QuotaMode::Local,
    };

    assert_eq!(config.quota.amount, i64::MAX);
    assert_eq!(config.config_hash, u64::MAX);
}

// ==================== QuotaMode Tests ====================

#[test]
fn test_quota_mode_default_is_local() {
    let mode: QuotaMode = QuotaMode::default();
    assert_eq!(mode, QuotaMode::Local);
}

#[test]
fn test_quota_mode_clone_eq() {
    let a = QuotaMode::Global;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn test_quota_mode_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(QuotaMode::Local);
    set.insert(QuotaMode::Global);
    assert_eq!(set.len(), 2);
    assert!(set.contains(&QuotaMode::Local));
    assert!(set.contains(&QuotaMode::Global));
}

#[test]
fn test_quota_mode_debug() {
    let debug_str = format!("{:?}", QuotaMode::Global);
    assert!(debug_str.contains("Global"));
}

// ==================== WindowStatus Tests ====================

#[test]
fn test_window_status_default_is_created() {
    let status: WindowStatus = WindowStatus::default();
    assert_eq!(status, WindowStatus::Created);
}

#[test]
fn test_window_status_variants() {
    let statuses = [
        WindowStatus::Created,
        WindowStatus::Initializing,
        WindowStatus::Initialized,
        WindowStatus::Deleted,
    ];
    for (i, a) in statuses.iter().enumerate() {
        for (j, b) in statuses.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
    }
}

#[test]
fn test_window_status_debug() {
    let debug_str = format!("{:?}", WindowStatus::Initializing);
    assert!(debug_str.contains("Initializing"));
}

// ==================== RemoteQuotaInfo Tests ====================

#[test]
fn test_remote_quota_info_construction() {
    let info = RemoteQuotaInfo {
        left_quota: 500,
        client_count: 3,
        timestamp_ms: 1234567890000,
        duration_ms: 1000,
    };
    assert_eq!(info.left_quota, 500);
    assert_eq!(info.client_count, 3);
    assert_eq!(info.timestamp_ms, 1234567890000);
    assert_eq!(info.duration_ms, 1000);
}

#[test]
fn test_remote_quota_info_clone() {
    let info1 = RemoteQuotaInfo {
        left_quota: 42,
        client_count: 5,
        timestamp_ms: 999,
        duration_ms: 60000,
    };
    let info2 = info1.clone();
    assert_eq!(info1.left_quota, info2.left_quota);
    assert_eq!(info1.client_count, info2.client_count);
    assert_eq!(info1.timestamp_ms, info2.timestamp_ms);
    assert_eq!(info1.duration_ms, info2.duration_ms);
}

#[test]
fn test_remote_quota_info_debug() {
    let info = RemoteQuotaInfo {
        left_quota: 0,
        client_count: 1,
        timestamp_ms: 0,
        duration_ms: 0,
    };
    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("RemoteQuotaInfo"));
    assert!(debug_str.contains("left_quota"));
}

// ==================== LocalQuotaInfo Tests ====================

#[test]
fn test_local_quota_info_default() {
    let info = LocalQuotaInfo::default();
    assert_eq!(info.quota_used, 0);
    assert_eq!(info.quota_limited, 0);
}

#[test]
fn test_local_quota_info_construction() {
    let info = LocalQuotaInfo {
        quota_used: 100,
        quota_limited: 5,
    };
    assert_eq!(info.quota_used, 100);
    assert_eq!(info.quota_limited, 5);
}

#[test]
fn test_local_quota_info_clone() {
    let info1 = LocalQuotaInfo {
        quota_used: 77,
        quota_limited: 3,
    };
    let info2 = info1.clone();
    assert_eq!(info1.quota_used, info2.quota_used);
    assert_eq!(info1.quota_limited, info2.quota_limited);
}

// ==================== AmountInfo Tests ====================

#[test]
fn test_amount_info_construction() {
    let info = AmountInfo {
        max_amount: 10000,
        duration_secs: 60,
        mode: QuotaMode::Global,
    };
    assert_eq!(info.max_amount, 10000);
    assert_eq!(info.duration_secs, 60);
    assert_eq!(info.mode, QuotaMode::Global);
}

#[test]
fn test_amount_info_clone() {
    let info1 = AmountInfo {
        max_amount: 500,
        duration_secs: 1,
        mode: QuotaMode::Local,
    };
    let info2 = info1.clone();
    assert_eq!(info1.max_amount, info2.max_amount);
    assert_eq!(info1.duration_secs, info2.duration_secs);
    assert_eq!(info1.mode, info2.mode);
}

#[test]
fn test_amount_info_debug() {
    let info = AmountInfo {
        max_amount: 0,
        duration_secs: 0,
        mode: QuotaMode::Local,
    };
    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("AmountInfo"));
}

// ==================== RemoteUpdate::RemoteQuotaUpdate Tests ====================

#[test]
fn test_remote_update_remote_quota_update() {
    let info = RemoteQuotaInfo {
        left_quota: 200,
        client_count: 4,
        timestamp_ms: 5555555555,
        duration_ms: 1000,
    };
    let update = RemoteUpdate::RemoteQuotaUpdate(&info);
    match update {
        RemoteUpdate::RemoteQuotaUpdate(data) => {
            assert_eq!(data.left_quota, 200);
            assert_eq!(data.client_count, 4);
        }
        _ => panic!("Expected RemoteQuotaUpdate variant"),
    }
}

// ==================== LimiterConfig with mode Tests ====================

#[test]
fn test_limiter_config_mode_field() {
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
        mode: QuotaMode::Global,
    };
    assert_eq!(config.mode, QuotaMode::Global);
}

#[test]
fn test_limiter_config_mode_clone() {
    let config1 = LimiterConfig {
        window: WindowConfig {
            window_size: Duration::from_secs(60),
            expire_duration: Duration::from_secs(300),
        },
        quota: QuotaConfig {
            amount: 1000,
            period: Duration::from_secs(1),
        },
        response: ResponseConfig {
            message: "test".to_string(),
            status: None,
        },
        config_hash: 0,
        limiter_type: "unirate".to_string(),
        mode: QuotaMode::Global,
    };
    let config2 = config1.clone();
    assert_eq!(config1.mode, config2.mode);
}
