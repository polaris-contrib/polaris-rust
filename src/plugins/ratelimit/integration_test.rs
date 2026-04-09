#[cfg(test)]
mod integration_tests {
    use std::sync::Arc;

    use crate::core::model::error::PoleError;
    use crate::core::plugin::ratelimit::ServiceRateLimiter;
    use crate::plugins::ratelimit::unirate::unirate::UnirateLimiter;
    use crate::plugins::ratelimit::concurrency::concurrency::ConcurrencyLimiter;
    use crate::plugins::ratelimit::reject::reject::RejectLimiter;

    #[test]
    fn test_limiter_creation() {
        // 测试所有限流器工厂方法是否正常工作
        let (unirate_factory, unirate_name) = UnirateLimiter::builder();
        let unirate_limiter = unirate_factory();
        assert_eq!(unirate_limiter.name(), "unirate");

        let (concurrency_factory, concurrency_name) = ConcurrencyLimiter::builder();
        let concurrency_limiter = concurrency_factory();
        assert_eq!(concurrency_limiter.name(), "concurrency");

        let (reject_factory, reject_name) = RejectLimiter::builder();
        let reject_limiter = reject_factory();
        assert_eq!(reject_limiter.name(), "reject");
    }

    #[tokio::test]
    async fn test_all_limiters_allocate_quota() {
        // 测试所有限流器的基础功能
        let (unirate_factory, _) = UnirateLimiter::builder();
        let unirate_limiter = unirate_factory();
        assert!(matches!(unirate_limiter.allocate_quota().await, Ok(true)));

        let (concurrency_factory, _) = ConcurrencyLimiter::builder();
        let concurrency_limiter = concurrency_factory();
        assert!(matches!(concurrency_limiter.allocate_quota().await, Ok(result) if result));

        let (reject_factory, _) = RejectLimiter::builder();
        let reject_limiter = reject_factory();
        assert!(matches!(reject_limiter.allocate_quota().await, Ok(true)));
    }
}