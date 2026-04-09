use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
    Arc,
};
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::core::model::error::{ErrorCode, PoleError};
use crate::core::model::ratelimit::{RemoteQuotaInfo, RemoteUpdate, WindowStatus};
use crate::core::plugin::ratelimit::ServiceRateLimiter;
use crate::ratelimit::connector::async_connector::{
    AsyncRateLimitConnector, ConnectorState, ServiceIdentifier, SyncTarget,
};

pub struct PoleRemoteSyncTask {
    status: AtomicU8,
    connector: Arc<AsyncRateLimitConnector>,
    limiter: Arc<Box<dyn ServiceRateLimiter>>,
    client_key: Arc<RwLock<Option<String>>>,
    target: SyncTarget,
    reporting_started: Arc<AtomicBool>,
    shutdown_flag: Arc<AtomicBool>,
    reporting_task: std::sync::Mutex<Option<JoinHandle<()>>>,
    last_manual_acquire_ms: AtomicU64,
}

impl PoleRemoteSyncTask {
    const REPORT_INTERVAL_MS: u64 = 500;
    const STREAM_STALE_TIMEOUT_MS: u64 = 5_000;
    const RECONNECT_BASE_BACKOFF_MS: u64 = 200;
    const RECONNECT_MAX_BACKOFF_MS: u64 = 5_000;
    const MANUAL_ACQUIRE_MIN_INTERVAL_MS: u64 = 300;

    pub fn new(
        connector: Arc<AsyncRateLimitConnector>,
        limiter: Arc<Box<dyn ServiceRateLimiter>>,
        target: SyncTarget,
    ) -> Self {
        Self {
            status: AtomicU8::new(WindowStatus::Created as u8),
            connector,
            limiter,
            client_key: Arc::new(RwLock::new(None)),
            target,
            reporting_started: Arc::new(AtomicBool::new(false)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            reporting_task: std::sync::Mutex::new(None),
            last_manual_acquire_ms: AtomicU64::new(0),
        }
    }

    pub fn get_status(&self) -> WindowStatus {
        match self.status.load(Ordering::SeqCst) {
            0 => WindowStatus::Created,
            1 => WindowStatus::Initializing,
            2 => WindowStatus::Initialized,
            3 => WindowStatus::Deleted,
            _ => WindowStatus::Created,
        }
    }

    fn set_status(&self, status: WindowStatus) {
        self.status.store(status as u8, Ordering::SeqCst);
    }

    pub async fn get_client_key(&self) -> Option<String> {
        self.client_key.read().await.clone()
    }

    pub fn get_target(&self) -> &SyncTarget {
        &self.target
    }

    pub async fn do_remote_init(&self, redo_init: bool) -> Result<(), PoleError> {
        if self.get_status() == WindowStatus::Deleted {
            return Err(PoleError::new(
                ErrorCode::InvalidState,
                "remote sync task already deleted".to_string(),
            ));
        }
        let current_status = self.get_status();
        if !redo_init && current_status == WindowStatus::Initialized {
            return Ok(());
        }

        self.shutdown_flag.store(false, Ordering::SeqCst);
        self.set_status(WindowStatus::Initializing);
        if redo_init {
            let mut key = self.client_key.write().await;
            *key = None;
        }
        self.connector.adjust_time().await?;
        let amount_info = self.limiter.get_amount_info();
        let result = self.connector.do_init(&self.target, &amount_info).await;

        match result {
            Ok(client_key) => {
                let mut key = self.client_key.write().await;
                *key = Some(client_key);
                self.set_status(WindowStatus::Initialized);
                self.start_periodic_reporting();
                Ok(())
            }
            Err(err) => {
                self.set_status(WindowStatus::Created);
                Err(err)
            }
        }
    }

    pub async fn do_remote_acquire(&self) -> Result<(), PoleError> {
        let current_status = self.get_status();
        if current_status == WindowStatus::Deleted {
            return Err(PoleError::new(
                ErrorCode::InvalidState,
                "cannot acquire quota: window deleted".to_string(),
            ));
        }
        if current_status != WindowStatus::Initialized {
            return Err(PoleError::new(
                ErrorCode::InvalidState,
                format!(
                    "cannot acquire quota: window not initialized, current status: {:?}",
                    current_status
                ),
            ));
        }

        if self.connector.get_state() == ConnectorState::Disconnected
            || self.missing_stream()
            || self.stream_unhealthy(current_time_ms())
        {
            self.do_remote_init(true).await?;
        }

        let client_key = self.client_key.read().await;
        let client_key = client_key.as_ref().ok_or_else(|| {
            PoleError::new(
                ErrorCode::InvalidState,
                "client_key not available".to_string(),
            )
        })?;

        let cur_time_ms = current_time_ms();
        let last_manual = self.last_manual_acquire_ms.load(Ordering::Relaxed);
        if cur_time_ms.saturating_sub(last_manual) < Self::MANUAL_ACQUIRE_MIN_INTERVAL_MS {
            return self.apply_remote_quota_update(cur_time_ms).await;
        }
        self.last_manual_acquire_ms
            .store(cur_time_ms, Ordering::Relaxed);

        let local_usage = self.limiter.fetch_local_usage_info(cur_time_ms).await;
        let amount_info = self.limiter.get_amount_info();

        let server_time_ms = self.connector.get_server_time_ms(cur_time_ms);
        self.connector
            .do_acquire(client_key, server_time_ms, &local_usage, &amount_info)
            .await?;

        self.apply_remote_quota_update(cur_time_ms).await
    }

    fn start_periodic_reporting(&self) {
        if self
            .reporting_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let connector = self.connector.clone();
        let limiter = self.limiter.clone();
        let target = self.target.clone();
        let client_key = self.client_key.clone();
        let shutdown_flag = self.shutdown_flag.clone();
        let reporting_started = self.reporting_started.clone();
        let handle = tokio::spawn(async move {
            let mut reconnect_backoff_ms = Self::RECONNECT_BASE_BACKOFF_MS;
            loop {
                if shutdown_flag.load(Ordering::SeqCst) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(Self::REPORT_INTERVAL_MS)).await;

                let service_id = ServiceIdentifier {
                    namespace: target.namespace.clone(),
                    service: target.service.clone(),
                };

                if connector.get_state() == ConnectorState::Disconnected {
                    if let Err(err) = connector.adjust_time().await {
                        crate::warn!("adjust time failed before reconnect: {}", err);
                        tokio::time::sleep(Duration::from_millis(reconnect_backoff_ms)).await;
                        reconnect_backoff_ms =
                            (reconnect_backoff_ms * 2).min(Self::RECONNECT_MAX_BACKOFF_MS);
                        continue;
                    }
                    let amount_info = limiter.get_amount_info();
                    match connector.do_init(&target, &amount_info).await {
                        Ok(key) => {
                            let mut key_guard = client_key.write().await;
                            *key_guard = Some(key);
                            reconnect_backoff_ms = Self::RECONNECT_BASE_BACKOFF_MS;
                        }
                        Err(err) => {
                            crate::warn!("rate-limit reconnect init failed: {}", err);
                            tokio::time::sleep(Duration::from_millis(reconnect_backoff_ms)).await;
                            reconnect_backoff_ms =
                                (reconnect_backoff_ms * 2).min(Self::RECONNECT_MAX_BACKOFF_MS);
                            continue;
                        }
                    }
                }

                let Some(counter_set) = connector.get_stream_counter_set(&service_id) else {
                    reconnect_backoff_ms =
                        (reconnect_backoff_ms * 2).min(Self::RECONNECT_MAX_BACKOFF_MS);
                    continue;
                };

                let Some(client_key) = counter_set.stream.get_client_key().await else {
                    reconnect_backoff_ms =
                        (reconnect_backoff_ms * 2).min(Self::RECONNECT_MAX_BACKOFF_MS);
                    continue;
                };

                if !counter_set
                    .stream
                    .is_alive(current_time_ms(), Self::STREAM_STALE_TIMEOUT_MS)
                {
                    connector.set_state(ConnectorState::Disconnected);
                    reconnect_backoff_ms =
                        (reconnect_backoff_ms * 2).min(Self::RECONNECT_MAX_BACKOFF_MS);
                    continue;
                }

                let cur_time_ms = current_time_ms();
                let local_usage = limiter.fetch_local_usage_info(cur_time_ms).await;
                let amount_info = limiter.get_amount_info();
                let server_time_ms = connector.get_server_time_ms(cur_time_ms);

                if connector
                    .do_acquire(&client_key, server_time_ms, &local_usage, &amount_info)
                    .await
                    .is_ok()
                {
                    reconnect_backoff_ms = Self::RECONNECT_BASE_BACKOFF_MS;
                    let _ = apply_stream_remote_update(&limiter, &counter_set.stream, cur_time_ms)
                        .await;
                } else {
                    connector.set_state(ConnectorState::Disconnected);
                    reconnect_backoff_ms =
                        (reconnect_backoff_ms * 2).min(Self::RECONNECT_MAX_BACKOFF_MS);
                }
            }
            reporting_started.store(false, Ordering::SeqCst);
        });
        if let Ok(mut task_guard) = self.reporting_task.lock() {
            *task_guard = Some(handle);
        }
    }

    async fn apply_remote_quota_update(&self, now_ms: u64) -> Result<(), PoleError> {
        let service_id = ServiceIdentifier {
            namespace: self.target.namespace.clone(),
            service: self.target.service.clone(),
        };

        let counter_set = self
            .connector
            .get_stream_counter_set(&service_id)
            .ok_or_else(|| {
                PoleError::new(
                    ErrorCode::InvalidState,
                    "stream counter set not found".to_string(),
                )
            })?;

        apply_stream_remote_update(&self.limiter, &counter_set.stream, now_ms).await
    }

    pub fn shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
        self.set_status(WindowStatus::Deleted);
        if let Ok(mut task_guard) = self.reporting_task.lock() {
            if let Some(handle) = task_guard.take() {
                handle.abort();
            }
        }
        self.reporting_started.store(false, Ordering::SeqCst);
        let service_id = ServiceIdentifier {
            namespace: self.target.namespace.clone(),
            service: self.target.service.clone(),
        };
        self.connector.remove_stream_counter_set(&service_id);
    }

    fn missing_stream(&self) -> bool {
        let service_id = ServiceIdentifier {
            namespace: self.target.namespace.clone(),
            service: self.target.service.clone(),
        };
        self.connector.get_stream_counter_set(&service_id).is_none()
    }

    fn stream_unhealthy(&self, now_ms: u64) -> bool {
        let service_id = ServiceIdentifier {
            namespace: self.target.namespace.clone(),
            service: self.target.service.clone(),
        };
        let Some(counter_set) = self.connector.get_stream_counter_set(&service_id) else {
            return true;
        };
        !counter_set
            .stream
            .is_alive(now_ms, Self::STREAM_STALE_TIMEOUT_MS)
    }
}

async fn apply_stream_remote_update(
    limiter: &Arc<Box<dyn ServiceRateLimiter>>,
    stream: &Arc<crate::ratelimit::connector::stream::StreamResource>,
    now_ms: u64,
) -> Result<(), PoleError> {
    for (_duration_key, callback) in stream.get_all_counters() {
        let update = RemoteQuotaInfo {
            left_quota: callback.left_quota,
            client_count: callback.client_count,
            timestamp_ms: callback.last_update_ms,
            duration_ms: callback.duration_secs.saturating_mul(1000),
        };
        limiter
            .on_remote_update(RemoteUpdate::RemoteQuotaUpdate(&update))
            .await?;
    }

    if stream.get_last_recv_time() > 0 {
        let sync_data = crate::core::model::ratelimit::QuotaSyncData {
            remote_quota: 0,
            sync_timestamp: now_ms,
        };
        limiter
            .on_remote_update(RemoteUpdate::QuotaSync(&sync_data))
            .await?;
    }

    Ok(())
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::core::model::ratelimit::{AmountInfo, LocalQuotaInfo, RemoteUpdate};
    use crate::core::plugin::plugins::Plugin;
    use crate::ratelimit::connector::async_connector::PoleAsyncRateLimitConnector;

    struct MockLimiter;

    impl Plugin for MockLimiter {
        fn init(&mut self) {}
        fn destroy(&self) {}
        fn name(&self) -> String {
            "mock".to_string()
        }
    }

    #[async_trait::async_trait]
    impl ServiceRateLimiter for MockLimiter {
        async fn allocate_quota(&self) -> Result<bool, PoleError> {
            Ok(true)
        }

        async fn return_quota(&self) -> Result<bool, PoleError> {
            Ok(true)
        }

        async fn on_remote_update(&self, _update: RemoteUpdate<'_>) -> Result<(), PoleError> {
            Ok(())
        }

        async fn fetch_local_usage_info(&self, _cur_time_ms: u64) -> HashMap<i32, LocalQuotaInfo> {
            let mut map = HashMap::new();
            map.insert(
                1,
                LocalQuotaInfo {
                    quota_used: 10,
                    quota_limited: 2,
                },
            );
            map
        }

        fn get_amount_info(&self) -> HashMap<i32, AmountInfo> {
            let mut map = HashMap::new();
            map.insert(
                1,
                AmountInfo {
                    max_amount: 100,
                    duration_secs: 1,
                    mode: crate::core::model::ratelimit::QuotaMode::Global,
                },
            );
            map
        }

        async fn fetch_local_usage(&self) -> Result<(), PoleError> {
            Ok(())
        }

        async fn get_amount(&self) -> Result<(), PoleError> {
            Ok(())
        }
    }

    #[test]
    fn test_sync_target_creation() {
        let target = SyncTarget {
            namespace: "default".to_string(),
            service: "test-svc".to_string(),
            labels: "method:GET".to_string(),
        };

        assert_eq!(target.namespace, "default");
        assert_eq!(target.service, "test-svc");
        assert_eq!(target.labels, "method:GET");
    }

    #[test]
    fn test_pole_remote_sync_task_initial_status() {
        let connector = Arc::new(PoleAsyncRateLimitConnector::new());
        let limiter: Arc<Box<dyn ServiceRateLimiter>> = Arc::new(Box::new(MockLimiter));
        let target = SyncTarget {
            namespace: "default".to_string(),
            service: "test-svc".to_string(),
            labels: "".to_string(),
        };

        let task = PoleRemoteSyncTask::new(connector, limiter, target);
        assert_eq!(task.get_status(), WindowStatus::Created);
    }

    #[tokio::test]
    async fn test_pole_remote_sync_task_client_key_initially_none() {
        let connector = Arc::new(PoleAsyncRateLimitConnector::new());
        let limiter: Arc<Box<dyn ServiceRateLimiter>> = Arc::new(Box::new(MockLimiter));
        let target = SyncTarget {
            namespace: "default".to_string(),
            service: "test-svc".to_string(),
            labels: "".to_string(),
        };

        let task = PoleRemoteSyncTask::new(connector, limiter, target);
        assert!(task.get_client_key().await.is_none());
    }

    #[tokio::test]
    async fn test_pole_remote_sync_task_acquire_without_init() {
        let connector = Arc::new(PoleAsyncRateLimitConnector::new());
        let limiter: Arc<Box<dyn ServiceRateLimiter>> = Arc::new(Box::new(MockLimiter));
        let target = SyncTarget {
            namespace: "default".to_string(),
            service: "test-svc".to_string(),
            labels: "".to_string(),
        };

        let task = PoleRemoteSyncTask::new(connector, limiter, target);
        let result = task.do_remote_acquire().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_pole_remote_sync_task_shutdown_sets_deleted() {
        let connector = Arc::new(PoleAsyncRateLimitConnector::new());
        let limiter: Arc<Box<dyn ServiceRateLimiter>> = Arc::new(Box::new(MockLimiter));
        let target = SyncTarget {
            namespace: "default".to_string(),
            service: "test-svc".to_string(),
            labels: "".to_string(),
        };

        let task = PoleRemoteSyncTask::new(connector, limiter, target);
        task.shutdown();
        assert_eq!(task.get_status(), WindowStatus::Deleted);
    }

    #[tokio::test]
    async fn test_pole_remote_sync_task_init_after_shutdown_fails() {
        let connector = Arc::new(PoleAsyncRateLimitConnector::new());
        let limiter: Arc<Box<dyn ServiceRateLimiter>> = Arc::new(Box::new(MockLimiter));
        let target = SyncTarget {
            namespace: "default".to_string(),
            service: "test-svc".to_string(),
            labels: "".to_string(),
        };

        let task = PoleRemoteSyncTask::new(connector, limiter, target);
        task.shutdown();
        let ret = task.do_remote_init(false).await;
        assert!(ret.is_err());
    }

    #[tokio::test]
    async fn test_pole_remote_sync_task_acquire_after_shutdown_fails() {
        let connector = Arc::new(PoleAsyncRateLimitConnector::new());
        let limiter: Arc<Box<dyn ServiceRateLimiter>> = Arc::new(Box::new(MockLimiter));
        let target = SyncTarget {
            namespace: "default".to_string(),
            service: "test-svc".to_string(),
            labels: "".to_string(),
        };

        let task = PoleRemoteSyncTask::new(connector, limiter, target);
        task.shutdown();
        let ret = task.do_remote_acquire().await;
        assert!(ret.is_err());
    }
}
