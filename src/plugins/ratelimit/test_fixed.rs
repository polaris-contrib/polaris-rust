#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use tokio;

    use crate::core::model::ratelimit::{LimiterConfig, QuotaConfig, RemoteUpdate, ResponseConfig, WindowConfig};
    use crate::plugins::ratelimit::unirate::unirate::{TokenBucket, UnirateLimiter};
    use crate::plugins::ratelimit::concurrency::concurrency::ConcurrencyLimiter;
    use crate::plugins::ratelimit::reject::reject::RejectLimiter;

    #[tokio::test]
    async fn test_unirate_limiter_memory_ordering() {
        let limiter = UnirateLimiter {
            token_bucket: Arc::new(TokenBucket::new(10, 10)), // 每秒10个令牌，桶容量10
            config: tokio::sync::RwLock::new(None),
        };

        // 测试高并发下的原子操作安全性
        let mut handles = vec![];
        for _ in 0..100 {
            let limiter_arc = Arc::new(limiter.clone());
            let handle = tokio::spawn(async move {
                limiter_arc.allocate_quota().await.unwrap()
            });
            handles.push(handle);
        }

        let results = futures::future::join_all(handles).await;
        let allowed_count = results.into_iter()
            .map(|r| if r.unwrap() { 1 } else { 0 })
            .sum::<usize>();

        // 确保并发访问的安全性
        assert!(allowed_count > 0); // 至少有一些请求被允许
    }

    #[tokio::test]
    async fn test_concurrency_limiter_correct_logic() {
        let limiter = ConcurrencyLimiter {
            active_count: AtomicUsize::new(0),
            max_concurrency: AtomicI64::new(2), // 最大并发数为2
            config_hash: AtomicU64::new(0),
            enabled: AtomicBool::new(true),
        };

        // 应该能够获取2个并发许可
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);

        // 第3个应该失败
        assert_eq!(limiter.allocate_quota().await.unwrap(), false);

        // 释放一个许可
        assert_eq!(limiter.return_quota().await.unwrap(), true);

        // 现在应该又能获取一个许可
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);

        // 再次达到上限
        assert_eq!(limiter.allocate_quota().await.unwrap(), false);
    }

    #[tokio::test]
    async fn test_reject_limiter_with_conditions() {
        let limiter = RejectLimiter {
            enabled: AtomicBool::new(true),
            config_hash: AtomicU64::new(0),
            reject_all: AtomicBool::new(false),
            current_config: tokio::sync::RwLock::new(None),
        };

        // 正常情况下应该允许
        assert_eq!(limiter.allocate_quota().await.unwrap(), true);

        // 创建一个包含紧急拒绝消息的配置
        let emergency_config = LimiterConfig {
            window: WindowConfig {
                window_size: Duration::from_secs(1),
                expire_duration: Duration::from_secs(60),
            },
            quota: QuotaConfig {
                amount: 10,
                period: Duration::from_secs(1),
            },
            response: ResponseConfig {
                message: "EMERGENCY REJECT".to_string(), // 包含紧急拒绝关键词
                status: Some(429),
            },
            config_hash: 12345,
            limiter_type: "reject".to_string(),
        };

        // 更新配置，触发拒绝模式
        limiter.on_remote_update(RemoteUpdate::ConfigUpdate(&emergency_config)).await.unwrap();

        // 应该拒绝请求
        assert_eq!(limiter.allocate_quota().await.unwrap(), false);
    }

    #[tokio::test]
    async fn test_concurrency_limiter_config_update() {
        let limiter = ConcurrencyLimiter {
            active_count: AtomicUsize::new(0),
            max_concurrency: AtomicI64::new(1), // 初始最大并发数为1
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
}