use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;

use crate::core::model::error::{ErrorCode, PoleError};

use super::async_connector::ServiceIdentifier;
use super::grpc_metric::RateLimitRequest;

#[derive(Clone, Debug)]
pub struct DurationBaseCallback {
    pub duration_secs: i64,
    pub left_quota: i64,
    pub client_count: i32,
    pub last_update_ms: u64,
}

pub struct PoleStreamResource {
    service_id: ServiceIdentifier,
    client_key: RwLock<Option<String>>,
    request_sender: RwLock<Option<UnboundedSender<RateLimitRequest>>>,
    last_recv_time: AtomicU64,
    counters: DashMap<i32, DurationBaseCallback>,
    duration_to_counter_key: DashMap<i32, u32>,
    counter_key_to_duration: DashMap<u32, i32>,
}

impl PoleStreamResource {
    pub fn new(service_id: ServiceIdentifier) -> Self {
        Self {
            service_id,
            client_key: RwLock::new(None),
            request_sender: RwLock::new(None),
            last_recv_time: AtomicU64::new(0),
            counters: DashMap::new(),
            duration_to_counter_key: DashMap::new(),
            counter_key_to_duration: DashMap::new(),
        }
    }

    pub fn get_service_id(&self) -> &ServiceIdentifier {
        &self.service_id
    }

    pub async fn set_client_key(&self, key: String) {
        let mut client_key = self.client_key.write().await;
        *client_key = Some(key);
    }

    pub async fn get_client_key(&self) -> Option<String> {
        self.client_key.read().await.clone()
    }

    pub async fn set_request_sender(&self, sender: UnboundedSender<RateLimitRequest>) {
        let mut tx = self.request_sender.write().await;
        *tx = Some(sender);
    }

    pub async fn get_request_sender(&self) -> Option<UnboundedSender<RateLimitRequest>> {
        self.request_sender.read().await.clone()
    }

    pub fn has_counter_key_mapping(&self) -> bool {
        !self.duration_to_counter_key.is_empty()
    }

    pub fn get_last_recv_time(&self) -> u64 {
        self.last_recv_time.load(Ordering::SeqCst)
    }

    pub fn update_last_recv_time(&self, time_ms: u64) {
        self.last_recv_time.store(time_ms, Ordering::SeqCst);
    }

    pub fn update_counter(&self, duration_key: i32, callback: DurationBaseCallback) {
        self.counters.insert(duration_key, callback);
    }

    pub fn map_counter_key(&self, duration_key: i32, counter_key: u32) {
        self.duration_to_counter_key
            .insert(duration_key, counter_key);
        self.counter_key_to_duration
            .insert(counter_key, duration_key);
    }

    pub fn get_counter_key_by_duration(&self, duration_key: i32) -> Option<u32> {
        self.duration_to_counter_key
            .get(&duration_key)
            .map(|entry| *entry.value())
    }

    pub fn get_duration_by_counter_key(&self, counter_key: u32) -> Option<i32> {
        self.counter_key_to_duration
            .get(&counter_key)
            .map(|entry| *entry.value())
    }

    pub fn get_counter(&self, duration_key: &i32) -> Option<DurationBaseCallback> {
        self.counters.get(duration_key).map(|v| v.clone())
    }

    pub fn remove_counter(&self, duration_key: &i32) {
        self.counters.remove(duration_key);
    }

    pub fn get_all_counters(&self) -> Vec<(i32, DurationBaseCallback)> {
        self.counters
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect()
    }

    pub fn counter_count(&self) -> usize {
        self.counters.len()
    }

    pub fn is_alive(&self, cur_time_ms: u64, timeout_ms: u64) -> bool {
        let last_recv = self.last_recv_time.load(Ordering::SeqCst);
        if last_recv == 0 {
            return false;
        }
        cur_time_ms.saturating_sub(last_recv) < timeout_ms
    }

    pub fn send_request(&self) -> Result<(), PoleError> {
        Err(PoleError::new(
            ErrorCode::NotSupport,
            "stream send not yet implemented".to_string(),
        ))
    }

    pub fn clear_counters(&self) {
        self.counters.clear();
        self.duration_to_counter_key.clear();
        self.counter_key_to_duration.clear();
    }
}

pub type StreamResource = PoleStreamResource;
