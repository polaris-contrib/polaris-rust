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

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use pole_client_rust::core::model::error::PoleError;
use pole_client_rust::core::model::ratelimit::{
    AmountInfo, LocalQuotaInfo, QuotaMode, RemoteQuotaInfo, RemoteUpdate, WindowStatus,
};
use pole_client_rust::core::plugin::plugins::Plugin;
use pole_client_rust::core::plugin::ratelimit::ServiceRateLimiter;
use pole_client_rust::ratelimit::connector::async_connector::{
    AsyncRateLimitConnector, ConnectorState, ServiceIdentifier, SyncTarget,
};
use pole_client_rust::ratelimit::connector::grpc_metric::rate_limit_grpcv2_server::{
    RateLimitGrpcv2, RateLimitGrpcv2Server,
};
use pole_client_rust::ratelimit::connector::grpc_metric::{
    Mode, QuotaCounter, QuotaLeft, RateLimitCmd, RateLimitInitResponse, RateLimitReportResponse,
    RateLimitResponse, TimeAdjustRequest, TimeAdjustResponse,
};
use pole_client_rust::ratelimit::connector::stream::{DurationBaseCallback, StreamResource};
use pole_client_rust::ratelimit::sync::pole::PoleRemoteSyncTask;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{transport::Server, Request, Response, Status};

#[derive(Default)]
struct MockRateLimitService;

#[tonic::async_trait]
impl RateLimitGrpcv2 for MockRateLimitService {
    type ServiceStream = ReceiverStream<Result<RateLimitResponse, Status>>;

    async fn service(
        &self,
        request: Request<
            tonic::Streaming<pole_client_rust::ratelimit::connector::grpc_metric::RateLimitRequest>,
        >,
    ) -> Result<Response<Self::ServiceStream>, Status> {
        let mut in_stream = request.into_inner();
        let (tx, rx) = mpsc::channel(16);
        tokio::spawn(async move {
            while let Some(item) = in_stream.next().await {
                let Ok(req) = item else {
                    break;
                };
                match RateLimitCmd::try_from(req.cmd).ok() {
                    Some(RateLimitCmd::Init) => {
                        let rsp = RateLimitResponse {
                            cmd: RateLimitCmd::Init as i32,
                            rate_limit_init_response: Some(RateLimitInitResponse {
                                code: 0,
                                target: None,
                                client_key: 42,
                                counters: vec![QuotaCounter {
                                    duration: 1,
                                    counter_key: 1001,
                                    left: 50,
                                    mode: Mode::Adaptive as i32,
                                    client_count: 2,
                                }],
                                slide_count: 1,
                                timestamp: now_ms() as i64 + 120,
                            }),
                            rate_limit_report_response: None,
                            rate_limit_batch_init_response: None,
                        };
                        if tx.send(Ok(rsp)).await.is_err() {
                            break;
                        }
                    }
                    Some(RateLimitCmd::Acquire) => {
                        let report = req.rate_limit_report_request.unwrap_or_default();
                        let quota_lefts = report
                            .quota_uses
                            .iter()
                            .map(|usage| QuotaLeft {
                                counter_key: usage.counter_key,
                                left: 100 - usage.used as i64,
                                mode: Mode::Adaptive as i32,
                                client_count: 2,
                            })
                            .collect::<Vec<_>>();
                        let rsp = RateLimitResponse {
                            cmd: RateLimitCmd::Acquire as i32,
                            rate_limit_init_response: None,
                            rate_limit_report_response: Some(RateLimitReportResponse {
                                code: 0,
                                quota_lefts,
                                timestamp: now_ms() as i64 + 120,
                            }),
                            rate_limit_batch_init_response: None,
                        };
                        if tx.send(Ok(rsp)).await.is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn time_adjust(
        &self,
        _request: Request<TimeAdjustRequest>,
    ) -> Result<Response<TimeAdjustResponse>, Status> {
        Ok(Response::new(TimeAdjustResponse {
            server_timestamp: now_ms() as i64 + 120,
        }))
    }
}

struct RecordingState {
    remote_updates: Mutex<Vec<RemoteQuotaInfo>>,
    sync_count: AtomicUsize,
}

struct RecordingLimiter {
    state: Arc<RecordingState>,
}

impl Plugin for RecordingLimiter {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        "recording".to_string()
    }
}

#[async_trait::async_trait]
impl ServiceRateLimiter for RecordingLimiter {
    async fn allocate_quota(&self) -> Result<bool, PoleError> {
        Ok(true)
    }

    async fn return_quota(&self) -> Result<bool, PoleError> {
        Ok(true)
    }

    async fn on_remote_update(&self, update: RemoteUpdate<'_>) -> Result<(), PoleError> {
        match update {
            RemoteUpdate::RemoteQuotaUpdate(update) => {
                self.state.remote_updates.lock().await.push(update.clone());
            }
            RemoteUpdate::QuotaSync(_) => {
                self.state.sync_count.fetch_add(1, Ordering::SeqCst);
            }
            RemoteUpdate::ConfigUpdate(_) => {}
        }
        Ok(())
    }

    async fn fetch_local_usage_info(&self, _cur_time_ms: u64) -> HashMap<i32, LocalQuotaInfo> {
        let mut usage = HashMap::new();
        usage.insert(
            1,
            LocalQuotaInfo {
                quota_used: 10,
                quota_limited: 1,
            },
        );
        usage
    }

    fn get_amount_info(&self) -> HashMap<i32, AmountInfo> {
        let mut amount = HashMap::new();
        amount.insert(
            1,
            AmountInfo {
                max_amount: 100,
                duration_secs: 1,
                mode: QuotaMode::Global,
            },
        );
        amount
    }

    async fn fetch_local_usage(&self) -> Result<(), PoleError> {
        Ok(())
    }

    async fn get_amount(&self) -> Result<(), PoleError> {
        Ok(())
    }
}

async fn start_mock_server() -> (String, oneshot::Sender<()>) {
    let addr = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        let _ = Server::builder()
            .add_service(RateLimitGrpcv2Server::new(MockRateLimitService))
            .serve_with_shutdown(addr, async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    tokio::time::sleep(Duration::from_millis(80)).await;
    (format!("http://{addr}"), shutdown_tx)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[test]
fn test_connector_lifecycle() {
    let connector = AsyncRateLimitConnector::new();
    assert_eq!(connector.get_state(), ConnectorState::Created);

    connector.set_state(ConnectorState::Connecting);
    assert_eq!(connector.get_state(), ConnectorState::Connecting);

    connector.set_state(ConnectorState::Connected);
    assert_eq!(connector.get_state(), ConnectorState::Connected);

    assert_eq!(connector.service_count(), 0);
}

#[test]
fn test_connector_stream_counter_set_management() {
    let connector = AsyncRateLimitConnector::new();

    let svc1 = ServiceIdentifier {
        namespace: "production".to_string(),
        service: "order-service".to_string(),
    };
    let svc2 = ServiceIdentifier {
        namespace: "production".to_string(),
        service: "payment-service".to_string(),
    };

    let set1 = connector.get_or_create_stream_counter_set(&svc1);
    connector
        .get_or_create_stream_counter_set(&svc2)
        .stream
        .map_counter_key(1, 1001);
    assert_eq!(connector.service_count(), 2);

    let set1_again = connector.get_or_create_stream_counter_set(&svc1);
    assert!(Arc::ptr_eq(&set1, &set1_again));
    assert_eq!(connector.service_count(), 2);

    connector.remove_stream_counter_set(&svc1);
    assert_eq!(connector.service_count(), 1);
    assert!(connector.get_stream_counter_set(&svc1).is_none());
    assert!(connector.get_stream_counter_set(&svc2).is_some());
}

#[test]
fn test_connector_time_calibration_flow() {
    let connector = AsyncRateLimitConnector::new();
    assert_eq!(connector.get_time_diff_ms(), 0);

    let local_time = 1_700_000_000_000u64;
    assert_eq!(connector.get_server_time_ms(local_time), local_time);

    connector.set_time_diff_ms(500);
    assert_eq!(connector.get_server_time_ms(local_time), local_time + 500);

    connector.set_time_diff_ms(-200);
    assert_eq!(connector.get_server_time_ms(local_time), local_time - 200);
}

#[tokio::test]
async fn test_stream_resource_full_lifecycle() {
    let service_id = ServiceIdentifier {
        namespace: "default".to_string(),
        service: "test-svc".to_string(),
    };

    let stream = StreamResource::new(service_id.clone());
    assert!(stream.get_client_key().await.is_none());
    assert_eq!(stream.get_last_recv_time(), 0);
    assert_eq!(stream.counter_count(), 0);

    stream.set_client_key("client-abc".to_string()).await;
    assert_eq!(
        stream.get_client_key().await,
        Some("client-abc".to_string())
    );

    stream.update_counter(
        1,
        DurationBaseCallback {
            duration_secs: 1,
            left_quota: 100,
            client_count: 3,
            last_update_ms: 1000,
        },
    );
    stream.map_counter_key(1, 999);
    assert_eq!(stream.get_counter_key_by_duration(1), Some(999));
    assert_eq!(stream.get_duration_by_counter_key(999), Some(1));

    stream.update_counter(
        60,
        DurationBaseCallback {
            duration_secs: 60,
            left_quota: 6000,
            client_count: 3,
            last_update_ms: 1000,
        },
    );
    assert_eq!(stream.counter_count(), 2);

    let counter = stream.get_counter(&1).unwrap();
    assert_eq!(counter.left_quota, 100);

    stream.update_last_recv_time(5000);
    assert!(stream.is_alive(6000, 5000));
    assert!(!stream.is_alive(11000, 5000));

    stream.clear_counters();
    assert_eq!(stream.counter_count(), 0);
}

#[test]
fn test_quota_mode_values() {
    assert_eq!(QuotaMode::default(), QuotaMode::Local);
    assert_ne!(QuotaMode::Local, QuotaMode::Global);
}

#[test]
fn test_window_status_values() {
    assert_eq!(WindowStatus::default(), WindowStatus::Created);
    assert_ne!(WindowStatus::Created, WindowStatus::Initialized);
    assert_ne!(WindowStatus::Initializing, WindowStatus::Deleted);
}

#[tokio::test]
async fn test_async_connector_real_grpc_init_and_acquire() {
    let (endpoint, shutdown) = start_mock_server().await;
    let connector = AsyncRateLimitConnector::new_with_endpoint(endpoint);

    connector.adjust_time().await.unwrap();
    assert!(connector.get_time_diff_ms() > 0);

    let target = SyncTarget {
        namespace: "default".to_string(),
        service: "svc-a".to_string(),
        labels: "method:get".to_string(),
    };
    let mut amount = HashMap::new();
    amount.insert(
        1,
        AmountInfo {
            max_amount: 100,
            duration_secs: 1,
            mode: QuotaMode::Global,
        },
    );

    let client_key = connector.do_init(&target, &amount).await.unwrap();
    assert_eq!(client_key, "42");

    let mut usage = HashMap::new();
    usage.insert(
        1,
        LocalQuotaInfo {
            quota_used: 10,
            quota_limited: 1,
        },
    );

    connector
        .do_acquire(&client_key, now_ms(), &usage, &amount)
        .await
        .unwrap();

    let service_id = ServiceIdentifier {
        namespace: "default".to_string(),
        service: "svc-a".to_string(),
    };

    for _ in 0..20 {
        if let Some(set) = connector.get_stream_counter_set(&service_id) {
            if let Some(counter) = set.stream.get_counter(&1) {
                if counter.left_quota == 90 {
                    break;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let set = connector.get_stream_counter_set(&service_id).unwrap();
    let counter = set.stream.get_counter(&1).unwrap();
    assert_eq!(counter.left_quota, 90);

    let _ = shutdown.send(());
}

#[tokio::test]
async fn test_remote_sync_task_handles_remote_updates() {
    let (endpoint, shutdown) = start_mock_server().await;
    let connector = Arc::new(AsyncRateLimitConnector::new_with_endpoint(endpoint));

    let state = Arc::new(RecordingState {
        remote_updates: Mutex::new(Vec::new()),
        sync_count: AtomicUsize::new(0),
    });
    let limiter: Arc<Box<dyn ServiceRateLimiter>> = Arc::new(Box::new(RecordingLimiter {
        state: state.clone(),
    }));

    let task = PoleRemoteSyncTask::new(
        connector,
        limiter,
        SyncTarget {
            namespace: "default".to_string(),
            service: "svc-b".to_string(),
            labels: "method:post".to_string(),
        },
    );

    task.do_remote_init(false).await.unwrap();
    task.do_remote_acquire().await.unwrap();
    tokio::time::sleep(Duration::from_millis(80)).await;
    task.do_remote_acquire().await.unwrap();

    tokio::time::sleep(Duration::from_millis(120)).await;

    let updates = state.remote_updates.lock().await;
    assert!(!updates.is_empty());
    assert!(updates.iter().any(|item| item.left_quota == 90));
    drop(updates);

    assert!(state.sync_count.load(Ordering::SeqCst) > 0);

    let _ = shutdown.send(());
}
