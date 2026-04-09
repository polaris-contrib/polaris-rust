#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use tokio;
    use tokio::sync::{RwLock, Semaphore};

    use crate::core::model::ratelimit::{LimiterConfig, QuotaConfig, RemoteUpdate, ResponseConfig, WindowConfig};
    use crate::plugins::ratelimit::unirate::unirate::{TokenBucket, UnirateLimiter};
    use crate::plugins::ratelimit::concurrency::concurrency::ConcurrencyLimiter;
    use crate::plugins::ratelimit::reject::reject::RejectLimiter;

    #[tokio::test]
    async fn test_unirate_limiter_basic() {
        let limiter = UnirateLimiter {
            token_bucket: Arc::new(TokenBucket::new(10, 10)), // 每秒10个令牌，桶容量10
            config: RwLock::new(None),
        };

        // 初始状态下，应该能获取一些令牌
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);

        // 快速消费多个令牌
        for _ in 0..8 {
            assert_eq!(limiter.allocate_quota().await.unwrap(), true);
        }

        // 应该还能获取一个令牌（总共10个）
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);

        // 第12个应该失败（超过了桶容量）
        assert_eq!(limiter.allocate_quota().await.unwrap(), false);
    }

    #[tokio::test]
    async fn test_unirate_limiter_config_update() {
        let limiter = UnirateLimiter {
            token_bucket: Arc::new(TokenBucket::new(1, 1)), // 低配额
            config: RwLock::new(None),
        };

        // 在低配额下只能获取一次
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);
        assert_eq!(limiter.allocate_quota().await.unwrap(), false);

        // 更新配置到高配额
        let new_config = LimiterConfig {
            window: WindowConfig {
                window_size: Duration::from_secs(1),
                expire_duration: Duration::from_secs(60),
            },
            quota: QuotaConfig {
                amount: 100,
                period: Duration::from_secs(1),
            },
            response: ResponseConfig {
                message: "Limited".to_string(),
                status: Some(429),
            },
            config_hash: 12345,
            limiter_type: "unirate".to_string(),
        };

        limiter.on_remote_update(RemoteUpdate::ConfigUpdate(&new_config)).await.unwrap();

        // 应该能够获取多次（直到达到100）
        for _ in 0..10 {
            assert_eq!(limiter.allocate_quota().await.unwrap(), true);
        }
    }

    #[tokio::test]
    async fn test_concurrency_limiter_basic() {
        let limiter = ConcurrencyLimiter {
            semaphore: Semaphore::new(2), // 最大并发数为2
            max_concurrency: AtomicI64::new(2),
            config_hash: AtomicU64::new(0),
            enabled: AtomicBool::new(true),
        };

        // 可以获取2个并发许可
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);

        // 第3个应该失败
        assert_eq!(limiter.allocate_quota().await.unwrap(), false);
    }

    #[tokio::test]
    async fn test_reject_limiter_basic() {
        let limiter = RejectLimiter {
            enabled: AtomicBool::new(true),
            config_hash: AtomicU64::new(0),
            reject_all: AtomicBool::new(false),
            current_config: RwLock::new(None),
        };

        // 普通模式下应该允许请求
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);

        // 设置为全拒绝模式
        limiter.reject_all.store(true, Ordering::Relaxed);
        assert_eq!(limiter.allocate_quota().await.unwrap(), false);

        // 恢复普通模式
        limiter.reject_all.store(false, Ordering::Relaxed);
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);
    }

    #[tokio::test]
    async fn test_disabled_limiters() {
        // 测试禁用状态下的限流器
        let unirate_limiter = UnirateLimiter {
            token_bucket: Arc::new(TokenBucket::new(1, 1)),
            config: RwLock::new(None),
        };
        unirate_limiter.token_bucket.set_enabled(false);

        // 禁用状态下应该总是允许
        for _ in 0..10 {
            assert_eq!(unirate_limiter.allocate_quota().await.unwrap(), true);
        }

        let concurrency_limiter = ConcurrencyLimiter {
            semaphore: Semaphore::new(0), // 0容量
            max_concurrency: AtomicI64::new(0),
            config_hash: AtomicU64::new(0),
            enabled: AtomicBool::new(false), // 禁用
        };

        // 禁用状态下应该总是允许
        assert_eq!(concurrency_limiter.allocate_quota().await.unwrap(), true);
    }
}