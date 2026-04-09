#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use tokio;

    use crate::core::model::ratelimit::{LimiterConfig, QuotaConfig, RemoteUpdate, ResponseConfig, WindowConfig};
    use crate::plugins::ratelimit::concurrency::concurrency::ConcurrencyLimiter;

    #[tokio::test]
    async fn test_concurrency_limiter_correct_logic_atomic() {
        let limiter = ConcurrencyLimiter {
            active_count: AtomicUsize::new(0),
            max_concurrency: AtomicUsize::new(2), // 最大并发数为2
            config_hash: AtomicU64::new(0),
            enabled: AtomicBool::new(true),
        };

        // 应该能够获取2个并发许可
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);  // 第1个请求
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);  // 第2个请求

        // 第3个应该失败，因为已达到最大并发数
        assert_eq!(limiter.allocate_quota().await.unwrap(), false);

        // 现在释放一个许可
        assert_eq!(limiter.return_quota().await.unwrap(), true);

        // 现在应该又能获取一个许可
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);

        // 再次达到上限
        assert_eq!(limiter.allocate_quota().await.unwrap(), false);

        // 再释放一个
        assert_eq!(limiter.return_quota().await.unwrap(), true);

        // 又能获取一个
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);
    }

    #[tokio::test]
    async fn test_concurrency_limiter_high_concurrency() {
        let limiter = Arc::new(ConcurrencyLimiter {
            active_count: AtomicUsize::new(0),
            max_concurrency: AtomicUsize::new(5), // 最大并发数为5
            config_hash: AtomicU64::new(0),
            enabled: AtomicBool::new(true),
        });

        // 并发发起大量请求
        let mut handles = vec![];
        for _ in 0..20 {
            let limiter_clone = limiter.clone();
            let handle = tokio::spawn(async move {
                limiter_clone.allocate_quota().await.unwrap()
            });
            handles.push(handle);
        }

        let results = futures::future::join_all(handles).await;
        let allowed_count = results.into_iter()
            .map(|r| if r.unwrap() { 1 } else { 0 })
            .sum::<usize>();

        // 确保只允许最多5个请求通过
        assert_eq!(allowed_count, 5);

        // 释放所有占用的许可
        for _ in 0..5 {
            assert_eq!(limiter.return_quota().await.unwrap(), true);
        }
    }

    #[tokio::test]
    async fn test_concurrency_limiter_config_update() {
        let limiter = ConcurrencyLimiter {
            active_count: AtomicUsize::new(0),
            max_concurrency: AtomicUsize::new(1), // 初始最大并发数为1
            config_hash: AtomicU64::new(0),
            enabled: AtomicBool::new(true),
        };

        // 初始只能获取1个
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);
        assert_eq!(limiter.allocate_quota().await.unwrap(), false);

        // 释放许可
        limiter.return_quota().await.unwrap();

        // 更新配置到更高的并发数
        let new_config = LimiterConfig {
            window: WindowConfig {
                window_size: Duration::from_secs(1),
                expire_duration: Duration::from_secs(60),
            },
            quota: QuotaConfig {
                amount: 3,  // 新的最大并发数
                period: Duration::from_secs(1),
            },
            response: ResponseConfig {
                message: "Updated config".to_string(),
                status: Some(429),
            },
            config_hash: 54321,
            limiter_type: "concurrency".to_string(),
        };

        limiter.on_remote_update(RemoteUpdate::ConfigUpdate(&new_config)).await.unwrap();

        // 现在应该能获取3个
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);
        assert_eq!(limiter.allocate_quota().await.unwrap(), false);
    }

    #[tokio::test]
    async fn test_concurrency_limiter_disabled() {
        let limiter = ConcurrencyLimiter {
            active_count: AtomicUsize::new(0),
            max_concurrency: AtomicUsize::new(1), // 最大并发数为1
            config_hash: AtomicU64::new(0),
            enabled: AtomicBool::new(false), // 禁用限流器
        };

        // 即使达到最大并发数，由于限流器被禁用，也应该总是允许
        for _ in 0..10 {
            assert_eq!(limiter.allocate_quota().await.unwrap(), true);
        }
    }
}