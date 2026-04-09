// Tencent is pleased to support the open source community by making Pole available.
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

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use dashmap::DashMap;
use parking_lot::RwLock;

use super::naming::Instance;

const EWMA_ALPHA_NUM: u64 = 2; // 0.2
const EWMA_ALPHA_DEN: u64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CircuitStatus {
    Close = 0,
    HalfOpen = 1,
    Open = 2,
}

impl CircuitStatus {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => CircuitStatus::HalfOpen,
            2 => CircuitStatus::Open,
            _ => CircuitStatus::Close,
        }
    }
}

#[derive(Default)]
pub struct InstanceStatManager {
    stats: DashMap<String, Arc<InstanceRuntimeStat>>,
}

impl InstanceStatManager {
    pub fn new() -> Self {
        Self {
            stats: DashMap::new(),
        }
    }

    pub fn make_key(namespace: &str, service: &str, instance: &Instance) -> String {
        let ins_key = if !instance.id.is_empty() {
            instance.id.clone()
        } else {
            format!("{}:{}", instance.ip, instance.port)
        };
        format!("{}#{}#{}", namespace, service, ins_key)
    }

    fn get_or_create(&self, key: &str) -> Arc<InstanceRuntimeStat> {
        self.stats
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(InstanceRuntimeStat::default()))
            .clone()
    }

    pub fn on_instance_selected(&self, namespace: &str, service: &str, instance: &Instance) {
        let key = Self::make_key(namespace, service, instance);
        let stat = self.get_or_create(&key);
        stat.increase_active_connections();
        stat.total_request.fetch_add(1, Ordering::Relaxed);
        stat.last_update_ms.store(now_ms(), Ordering::Relaxed);
    }

    pub fn report_call(
        &self,
        namespace: &str,
        service: &str,
        instance: &Instance,
        duration: Duration,
        success: bool,
        error_code: Option<&str>,
    ) {
        let key = Self::make_key(namespace, service, instance);
        let stat = self.get_or_create(&key);
        stat.decrease_active_connections();

        let rt_ms = duration.as_millis().min(u128::from(u64::MAX)) as u64;
        stat.total_rt_ms.fetch_add(rt_ms, Ordering::Relaxed);
        stat.total_complete.fetch_add(1, Ordering::Relaxed);

        if success {
            stat.total_success.fetch_add(1, Ordering::Relaxed);
        } else {
            stat.total_error.fetch_add(1, Ordering::Relaxed);
            if let Some(code) = error_code {
                let mut guard = stat.error_code_counter.write();
                let counter = guard.entry(code.to_string()).or_insert(0);
                *counter += 1;
            }
        }

        let old = stat.ewma_rt_ms.load(Ordering::Relaxed);
        let new_val = if old == 0 {
            rt_ms
        } else {
            ((old * (EWMA_ALPHA_DEN - EWMA_ALPHA_NUM)) + rt_ms * EWMA_ALPHA_NUM) / EWMA_ALPHA_DEN
        };
        stat.ewma_rt_ms.store(new_val, Ordering::Relaxed);
        stat.last_update_ms.store(now_ms(), Ordering::Relaxed);
    }

    pub fn set_circuit_open(
        &self,
        namespace: &str,
        service: &str,
        instance: &Instance,
        is_open: bool,
    ) {
        let status = if is_open {
            CircuitStatus::Open
        } else {
            CircuitStatus::Close
        };
        self.set_circuit_status(namespace, service, instance, status);
    }

    pub fn set_circuit_status(
        &self,
        namespace: &str,
        service: &str,
        instance: &Instance,
        status: CircuitStatus,
    ) {
        let key = Self::make_key(namespace, service, instance);
        let stat = self.get_or_create(&key);
        stat.circuit_status.store(status as u8, Ordering::Relaxed);
        stat.last_update_ms.store(now_ms(), Ordering::Relaxed);
    }

    pub fn get_active_connections(
        &self,
        namespace: &str,
        service: &str,
        instance: &Instance,
    ) -> Option<u64> {
        let key = Self::make_key(namespace, service, instance);
        self.stats
            .get(&key)
            .map(|v| v.active_connections.load(Ordering::Relaxed))
    }

    pub fn get_ewma_rt_ms(
        &self,
        namespace: &str,
        service: &str,
        instance: &Instance,
    ) -> Option<u64> {
        let key = Self::make_key(namespace, service, instance);
        self.stats
            .get(&key)
            .map(|v| v.ewma_rt_ms.load(Ordering::Relaxed))
    }

    pub fn get_error_code_counters(
        &self,
        namespace: &str,
        service: &str,
        instance: &Instance,
    ) -> Option<HashMap<String, u64>> {
        let key = Self::make_key(namespace, service, instance);
        self.stats
            .get(&key)
            .map(|v| v.error_code_counter.read().clone())
    }

    pub fn get_error_rate(
        &self,
        namespace: &str,
        service: &str,
        instance: &Instance,
    ) -> Option<f64> {
        let key = Self::make_key(namespace, service, instance);
        self.stats.get(&key).map(|v| {
            let total = v.total_complete.load(Ordering::Relaxed);
            if total == 0 {
                return 0_f64;
            }
            let err = v.total_error.load(Ordering::Relaxed);
            err as f64 / total as f64
        })
    }

    pub fn get_circuit_status(
        &self,
        namespace: &str,
        service: &str,
        instance: &Instance,
    ) -> Option<CircuitStatus> {
        let key = Self::make_key(namespace, service, instance);
        self.stats.get(&key).map(|v| {
            let raw = v.circuit_status.load(Ordering::Relaxed);
            CircuitStatus::from_u8(raw)
        })
    }
}

pub struct InstanceRuntimeStat {
    pub active_connections: AtomicU64,
    pub total_request: AtomicU64,
    pub total_complete: AtomicU64,
    pub total_success: AtomicU64,
    pub total_error: AtomicU64,
    pub total_rt_ms: AtomicU64,
    pub ewma_rt_ms: AtomicU64,
    pub circuit_status: AtomicU8,
    pub error_code_counter: RwLock<HashMap<String, u64>>,
    pub last_update_ms: AtomicU64,
}

impl Default for InstanceRuntimeStat {
    fn default() -> Self {
        Self {
            active_connections: AtomicU64::new(0),
            total_request: AtomicU64::new(0),
            total_complete: AtomicU64::new(0),
            total_success: AtomicU64::new(0),
            total_error: AtomicU64::new(0),
            total_rt_ms: AtomicU64::new(0),
            ewma_rt_ms: AtomicU64::new(0),
            circuit_status: AtomicU8::new(CircuitStatus::Close as u8),
            error_code_counter: RwLock::new(HashMap::new()),
            last_update_ms: AtomicU64::new(0),
        }
    }
}

impl InstanceRuntimeStat {
    fn increase_active_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    fn decrease_active_connections(&self) {
        let _ = self
            .active_connections
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                Some(v.saturating_sub(1))
            });
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{CircuitStatus, InstanceStatManager};
    use crate::core::model::naming::Instance;

    #[test]
    fn test_instance_stat_manager_record_and_read() {
        let mgr = InstanceStatManager::new();
        let mut ins = Instance::default();
        ins.id = "ins-1".to_string();

        mgr.on_instance_selected("default", "svc-a", &ins);
        assert_eq!(
            mgr.get_active_connections("default", "svc-a", &ins),
            Some(1)
        );

        mgr.report_call(
            "default",
            "svc-a",
            &ins,
            Duration::from_millis(25),
            false,
            Some("500"),
        );

        assert_eq!(
            mgr.get_active_connections("default", "svc-a", &ins),
            Some(0)
        );
        assert_eq!(mgr.get_ewma_rt_ms("default", "svc-a", &ins), Some(25));

        let counter = mgr
            .get_error_code_counters("default", "svc-a", &ins)
            .unwrap();
        assert_eq!(counter.get("500").copied(), Some(1));
        assert_eq!(mgr.get_error_rate("default", "svc-a", &ins), Some(1_f64));
        assert_eq!(
            mgr.get_circuit_status("default", "svc-a", &ins),
            Some(CircuitStatus::Close)
        );

        mgr.set_circuit_status("default", "svc-a", &ins, CircuitStatus::Open);
        assert_eq!(
            mgr.get_circuit_status("default", "svc-a", &ins),
            Some(CircuitStatus::Open)
        );
    }
}
