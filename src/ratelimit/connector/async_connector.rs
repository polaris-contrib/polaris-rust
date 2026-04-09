use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;

use crate::core::model::error::{ErrorCode, PoleError};
use crate::core::model::ratelimit::{AmountInfo, LocalQuotaInfo, QuotaMode};

use super::grpc_metric::rate_limit_grpcv2_client::RateLimitGrpcv2Client;
use super::grpc_metric::{
    LimitTarget, Mode, QuotaMode as MetricQuotaMode, QuotaSum, QuotaTotal, RateLimitCmd,
    RateLimitInitRequest, RateLimitReportRequest, RateLimitRequest, TimeAdjustRequest,
};
use super::stream::StreamResource;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ServiceIdentifier {
    pub namespace: String,
    pub service: String,
}

#[derive(Clone, Debug)]
pub struct SyncTarget {
    pub namespace: String,
    pub service: String,
    pub labels: String,
}

pub struct StreamCounterSet {
    pub service_id: ServiceIdentifier,
    pub stream: Arc<StreamResource>,
}

impl StreamCounterSet {
    pub fn new(service_id: ServiceIdentifier, stream: Arc<StreamResource>) -> Self {
        Self { service_id, stream }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ConnectorState {
    Created = 0,
    Connecting = 1,
    Connected = 2,
    Disconnected = 3,
}

pub struct PoleAsyncRateLimitConnector {
    stream_counter_sets: DashMap<ServiceIdentifier, Arc<StreamCounterSet>>,
    state: Arc<AtomicU8>,
    time_diff_ms: Arc<AtomicI64>,
    client_id: String,
    grpc_endpoint: String,
}

impl PoleAsyncRateLimitConnector {
    pub fn new() -> Self {
        Self::new_with_endpoint(resolve_ratelimit_endpoint())
    }

    pub fn new_with_endpoint(grpc_endpoint: String) -> Self {
        Self {
            stream_counter_sets: DashMap::new(),
            state: Arc::new(AtomicU8::new(ConnectorState::Created as u8)),
            time_diff_ms: Arc::new(AtomicI64::new(0)),
            client_id: format!("ratelimit-{}", uuid::Uuid::new_v4()),
            grpc_endpoint,
        }
    }

    pub fn get_state(&self) -> ConnectorState {
        match self.state.load(Ordering::SeqCst) {
            0 => ConnectorState::Created,
            1 => ConnectorState::Connecting,
            2 => ConnectorState::Connected,
            3 => ConnectorState::Disconnected,
            _ => ConnectorState::Created,
        }
    }

    pub fn set_state(&self, state: ConnectorState) {
        self.state.store(state as u8, Ordering::SeqCst);
    }

    pub fn get_time_diff_ms(&self) -> i64 {
        self.time_diff_ms.load(Ordering::SeqCst)
    }

    pub fn set_time_diff_ms(&self, diff: i64) {
        self.time_diff_ms.store(diff, Ordering::SeqCst);
    }

    pub fn get_or_create_stream_counter_set(
        &self,
        service_id: &ServiceIdentifier,
    ) -> Arc<StreamCounterSet> {
        if let Some(existing) = self.stream_counter_sets.get(service_id) {
            return existing.clone();
        }

        let stream = Arc::new(StreamResource::new(service_id.clone()));
        let counter_set = Arc::new(StreamCounterSet::new(service_id.clone(), stream));
        self.stream_counter_sets
            .insert(service_id.clone(), counter_set.clone());
        counter_set
    }

    pub fn get_stream_counter_set(
        &self,
        service_id: &ServiceIdentifier,
    ) -> Option<Arc<StreamCounterSet>> {
        self.stream_counter_sets.get(service_id).map(|v| v.clone())
    }

    pub fn remove_stream_counter_set(&self, service_id: &ServiceIdentifier) {
        self.stream_counter_sets.remove(service_id);
    }

    pub async fn adjust_time(&self) -> Result<(), PoleError> {
        let mut client = RateLimitGrpcv2Client::connect(self.grpc_endpoint.clone())
            .await
            .map_err(|err| PoleError::new(ErrorCode::ConnectError, err.to_string()))?;

        let mut best_rtt = i64::MAX;
        let mut best_diff = 0i64;

        for _ in 0..3 {
            let start = current_time_ms() as i64;
            let rsp = client
                .time_adjust(TimeAdjustRequest {})
                .await
                .map_err(|err| PoleError::new(ErrorCode::RpcError, err.to_string()))?
                .into_inner();
            let end = current_time_ms() as i64;
            let rtt = end.saturating_sub(start);
            let local_mid = start.saturating_add(rtt / 2);
            let diff = rsp.server_timestamp.saturating_sub(local_mid);
            if rtt < best_rtt {
                best_rtt = rtt;
                best_diff = diff;
            }
        }
        self.time_diff_ms.store(best_diff, Ordering::SeqCst);
        Ok(())
    }

    pub fn get_server_time_ms(&self, local_time_ms: u64) -> u64 {
        let diff = self.time_diff_ms.load(Ordering::SeqCst);
        if diff >= 0 {
            local_time_ms.saturating_add(diff as u64)
        } else {
            local_time_ms.saturating_sub((-diff) as u64)
        }
    }

    async fn init_grpc(&self, target: &SyncTarget) -> Result<Arc<StreamCounterSet>, PoleError> {
        let service_id = ServiceIdentifier {
            namespace: target.namespace.clone(),
            service: target.service.clone(),
        };

        let counter_set = self.get_or_create_stream_counter_set(&service_id);
        if counter_set.stream.get_request_sender().await.is_some() {
            return Ok(counter_set);
        }

        self.set_state(ConnectorState::Connecting);

        let mut client = RateLimitGrpcv2Client::connect(self.grpc_endpoint.clone())
            .await
            .map_err(|err| PoleError::new(ErrorCode::ConnectError, err.to_string()))?;

        let (tx, rx) = mpsc::unbounded_channel::<RateLimitRequest>();
        let response_stream = client
            .service(UnboundedReceiverStream::new(rx))
            .await
            .map_err(|err| PoleError::new(ErrorCode::RpcError, err.to_string()))?
            .into_inner();

        counter_set.stream.set_request_sender(tx).await;
        self.set_state(ConnectorState::Connected);

        let stream = counter_set.stream.clone();
        let time_diff = self.time_diff_ms.clone();
        let state = self.state.clone();
        tokio::spawn(async move {
            if process_response_stream(stream, response_stream, time_diff)
                .await
                .is_err()
            {
                state.store(ConnectorState::Disconnected as u8, Ordering::SeqCst);
            }
        });

        Ok(counter_set)
    }

    pub async fn do_init(
        &self,
        target: &SyncTarget,
        amount_info: &HashMap<i32, AmountInfo>,
    ) -> Result<String, PoleError> {
        let counter_set = self.init_grpc(target).await?;

        if let Some(client_key) = counter_set.stream.get_client_key().await {
            return Ok(client_key);
        }

        let totals = amount_info
            .iter()
            .map(|(duration_key, amount)| QuotaTotal {
                mode: match amount.mode {
                    QuotaMode::Local => MetricQuotaMode::Whole as i32,
                    QuotaMode::Global => MetricQuotaMode::Divide as i32,
                },
                duration: (*duration_key).max(0) as u32,
                max_amount: amount.max_amount.max(0) as u32,
            })
            .collect::<Vec<_>>();

        let request = RateLimitRequest {
            cmd: RateLimitCmd::Init as i32,
            rate_limit_init_request: Some(RateLimitInitRequest {
                target: Some(LimitTarget {
                    namespace: target.namespace.clone(),
                    service: target.service.clone(),
                    labels: target.labels.clone(),
                    labels_list: Vec::new(),
                }),
                client_id: self.client_id.clone(),
                totals,
                slide_count: 0,
                mode: Mode::Adaptive as i32,
            }),
            rate_limit_report_request: None,
            rate_limit_batch_init_request: None,
        };

        let sender = counter_set
            .stream
            .get_request_sender()
            .await
            .ok_or_else(|| {
                PoleError::new(
                    ErrorCode::InvalidState,
                    "rate-limit stream not available".to_string(),
                )
            })?;

        sender
            .send(request)
            .map_err(|err| PoleError::new(ErrorCode::RpcError, err.to_string()))?;

        let wait_deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(client_key) = counter_set.stream.get_client_key().await {
                return Ok(client_key);
            }
            if tokio::time::Instant::now() >= wait_deadline {
                return Err(PoleError::new(
                    ErrorCode::RpcTimeout,
                    "wait init response timeout".to_string(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    pub async fn do_acquire(
        &self,
        client_key: &str,
        server_time_ms: u64,
        local_usage: &HashMap<i32, LocalQuotaInfo>,
        _amount_info: &HashMap<i32, AmountInfo>,
    ) -> Result<(), PoleError> {
        let stream = self
            .stream_counter_sets
            .iter()
            .map(|entry| entry.value().stream.clone())
            .collect::<Vec<_>>();

        let mut target_stream: Option<Arc<StreamResource>> = None;
        for stream_resource in stream {
            let found = stream_resource
                .get_client_key()
                .await
                .is_some_and(|key| key == client_key);
            if found {
                target_stream = Some(stream_resource);
                break;
            }
        }

        let stream_resource = target_stream.ok_or_else(|| {
            PoleError::new(
                ErrorCode::InvalidState,
                format!("client_key({client_key}) not initialized"),
            )
        })?;

        let sender = stream_resource.get_request_sender().await.ok_or_else(|| {
            PoleError::new(
                ErrorCode::InvalidState,
                "rate-limit stream sender unavailable".to_string(),
            )
        })?;

        let quota_uses = local_usage
            .iter()
            .map(|(duration_key, usage)| QuotaSum {
                counter_key: stream_resource
                    .get_counter_key_by_duration(*duration_key)
                    .unwrap_or((*duration_key).max(0) as u32),
                used: usage.quota_used.max(0) as u32,
                limited: usage.quota_limited.max(0) as u32,
            })
            .collect::<Vec<_>>();

        sender
            .send(RateLimitRequest {
                cmd: RateLimitCmd::Acquire as i32,
                rate_limit_init_request: None,
                rate_limit_report_request: Some(RateLimitReportRequest {
                    client_key: client_key.parse::<u32>().map_err(|err| {
                        PoleError::new(
                            ErrorCode::InvalidRequest,
                            format!("invalid client_key({client_key}): {err}"),
                        )
                    })?,
                    quota_uses,
                    timestamp: server_time_ms as i64,
                }),
                rate_limit_batch_init_request: None,
            })
            .map_err(|err| PoleError::new(ErrorCode::RpcError, err.to_string()))?;
        Ok(())
    }

    pub fn service_count(&self) -> usize {
        self.stream_counter_sets.len()
    }
}

impl Default for PoleAsyncRateLimitConnector {
    fn default() -> Self {
        Self::new()
    }
}

pub type AsyncRateLimitConnector = PoleAsyncRateLimitConnector;

fn resolve_ratelimit_endpoint() -> String {
    if let Ok(value) = std::env::var("POLE_RATELIMIT_ADDR") {
        return normalize_endpoint(&value);
    }
    normalize_endpoint("http://127.0.0.1:8091")
}

fn normalize_endpoint(raw: &str) -> String {
    let trimmed = raw
        .trim_start_matches("discover://")
        .trim_start_matches("config://")
        .trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

async fn process_response_stream(
    stream: Arc<StreamResource>,
    mut response_stream: tonic::codec::Streaming<super::grpc_metric::RateLimitResponse>,
    time_diff: Arc<AtomicI64>,
) -> Result<(), PoleError> {
    while let Some(item) = response_stream.next().await {
        let response = item.map_err(|err| PoleError::new(ErrorCode::RpcError, err.to_string()))?;
        let now_ms = current_time_ms();
        stream.update_last_recv_time(now_ms);

        if let Some(init_rsp) = response.rate_limit_init_response {
            stream.set_client_key(init_rsp.client_key.to_string()).await;
            for counter in init_rsp.counters {
                let duration_key = counter.duration as i32;
                stream.map_counter_key(duration_key, counter.counter_key);
                stream.update_counter(
                    duration_key,
                    super::stream::DurationBaseCallback {
                        duration_secs: duration_key as i64,
                        left_quota: counter.left,
                        client_count: counter.client_count as i32,
                        last_update_ms: now_ms,
                    },
                );
            }
            calibrate_time_diff(init_rsp.timestamp, now_ms, &time_diff);
        }

        if let Some(report_rsp) = response.rate_limit_report_response {
            for quota_left in report_rsp.quota_lefts {
                if let Some(duration_key) =
                    stream.get_duration_by_counter_key(quota_left.counter_key)
                {
                    stream.update_counter(
                        duration_key,
                        super::stream::DurationBaseCallback {
                            duration_secs: duration_key as i64,
                            left_quota: quota_left.left,
                            client_count: quota_left.client_count as i32,
                            last_update_ms: now_ms,
                        },
                    );
                }
            }
            calibrate_time_diff(report_rsp.timestamp, now_ms, &time_diff);
        }
    }
    Ok(())
}

fn calibrate_time_diff(server_timestamp: i64, local_timestamp: u64, time_diff: &AtomicI64) {
    if server_timestamp <= 0 {
        return;
    }
    let diff = server_timestamp.saturating_sub(local_timestamp as i64);
    time_diff.store(diff, Ordering::SeqCst);
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
