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

use crate::core::model::cluster::{
    ClusterType, ServerServiceInfo, BUILDIN_SERVER_NAMESPACE, BUILDIN_SERVER_SERVICE,
};
use crate::core::model::error::{ErrorCode, PolarisError};
use crate::core::model::naming::{Instance, ServiceKey};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc::Sender;
use tonic::transport::{Channel, Endpoint};
use tower::discover::Change;
use tracing::{debug, error, info};
use uuid::Uuid;

pub trait ConnectionSwitchListener
where
    Self: Sync + Send,
{
    fn on_switch(&self, id: ConnID);
}

pub struct EmptyConnectionSwitchListener {}

impl EmptyConnectionSwitchListener {
    pub fn new() -> Self {
        Self {}
    }
}

impl ConnectionSwitchListener for EmptyConnectionSwitchListener {
    fn on_switch(&self, id: ConnID) {
        todo!()
    }
}

pub struct ConnectionManager
where
    Self: Sync + Send,
{
    pub connect_timeout: Duration,
    pub switch_interval: Duration,
    pub client_id: String,
    pub server_address: HashMap<String, ServerAddress>,
    pub on_switch: Arc<dyn ConnectionSwitchListener>,
}

impl ConnectionManager {
    pub fn empty() -> Self {
        Self {
            connect_timeout: Default::default(),
            switch_interval: Default::default(),
            client_id: "".to_string(),
            server_address: Default::default(),
            on_switch: Arc::new(EmptyConnectionSwitchListener::new()),
        }
    }

    pub fn new(
        connect_timeout: Duration,
        switch_interval: Duration,
        client_id: String,
        on_switch: Arc<dyn ConnectionSwitchListener>,
    ) -> Self {
        Self {
            connect_timeout,
            switch_interval,
            client_id,
            server_address: Default::default(),
            on_switch,
        }
    }

    // set_on_switch just for unit test
    pub fn set_on_switch(&mut self, on_switch: Arc<dyn ConnectionSwitchListener>) {
        self.on_switch = on_switch;
    }

    pub fn get_connection(
        &self,
        op_key: &str,
        cluster_type: ClusterType,
    ) -> Result<Arc<Connection>, PolarisError> {
        loop {
            let ret = self.try_get_connection(op_key, cluster_type);
            match ret {
                Ok(conn) => {
                    if conn.acquire(op_key) {
                        return Ok(conn);
                    }
                }
                Err(err) => {
                    return Err(PolarisError::new(ErrorCode::ServerError, err.to_string()));
                }
            }
        }
    }

    fn try_get_connection(
        &self,
        op_key: &str,
        cluster_type: ClusterType,
    ) -> Result<Arc<Connection>, PolarisError> {
        let server_address = self.server_address.get(cluster_type.to_string().as_str());
        if server_address.is_none() {
            return Err(PolarisError::new(
                ErrorCode::ServerError,
                format!("{} server_address is empty", cluster_type).to_string(),
            ));
        }
        match server_address {
            Some(server_address) => server_address.try_get_connection(op_key),
            None => Err(PolarisError::new(ErrorCode::ServerError, "".to_string())),
        }
    }
}

pub struct ServerAddress {
    lock: Mutex<u32>,
    cluster: ClusterType,
    cur_index: AtomicUsize,
    active_conn: Arc<Connection>,
    ready: AtomicBool,
    service_info: Option<ServerServiceInfo>,
    conn_mgr: Arc<ConnectionManager>,
    // connect_timeout 连接超时时间
    connect_timeout: Duration,
    endpoints: Vec<crate::core::model::naming::Endpoint>,
    // sender 负责通知底层 gRPC 连接 IP 切换
    sender: Sender<Change<String, Endpoint>>,
}

impl ServerAddress {
    fn new(
        conn_mgr: Arc<ConnectionManager>,
        sender: Sender<Change<String, Endpoint>>,
    ) -> ServerAddress {
        Self {
            lock: Mutex::new(0),
            cluster: ClusterType::default(),
            cur_index: AtomicUsize::new(0),
            active_conn: Arc::new(Connection::empty_conn(conn_mgr.clone())),
            ready: Default::default(),
            endpoints: vec![],
            service_info: None,
            conn_mgr: conn_mgr,
            sender,
            connect_timeout: Duration::from_secs(1),
        }
    }

    fn with_endpoints(mut self, endpoints: Vec<crate::core::model::naming::Endpoint>) -> Self {
        self.endpoints = endpoints;
        Self { ..self }
    }

    fn with_service_info(mut self, service_info: Option<ServerServiceInfo>) -> Self {
        self.service_info = service_info;
        Self { ..self }
    }

    fn with_connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = connect_timeout;
        Self { ..self }
    }

    fn run(&mut self) {}

    fn try_get_connection(&self, op_key: &str) -> Result<Arc<Connection>, PolarisError> {
        if self.active_conn.is_empty {
            return Err(PolarisError::new(ErrorCode::InvalidState, "".to_string()));
        }
        return Ok(self.active_conn.clone());
    }

    fn get_server_address(&self) -> Result<crate::core::model::naming::Endpoint, PolarisError> {
        if self.service_info.is_none() {
            let cur_index = self.cur_index.load(SeqCst);
            let cur_index = cur_index % self.endpoints.len();
            let endpoint = self.endpoints.get(cur_index).unwrap().clone();
            self.cur_index.store(cur_index + 1, SeqCst);
            return Ok(endpoint);
        }
        Ok(crate::core::model::naming::Endpoint::default())
    }

    fn switch_client_on_fail(&mut self, cur_conn_opt: Option<Connection>, last_conn_id: &ConnID) {
        drop(self.lock.lock().unwrap());
        match cur_conn_opt {
            Some(cur_conn) => {
                if !cur_conn.conn_id.eq(last_conn_id) {
                    return;
                }
                self.do_switch_client(Some(&cur_conn))
            }
            None => {}
        }
    }

    fn switch_client(&mut self, cur_conn_opt: Option<&Connection>) {
        if !available_cur_connection(cur_conn_opt) {
            return;
        }
        drop(self.lock.lock().unwrap());
        if !available_cur_connection(cur_conn_opt) {
            return;
        }
        self.do_switch_client(cur_conn_opt)
    }

    fn do_switch_client(&mut self, cur_conn: Option<&Connection>) {
        let server_addr_ret = self.get_server_address();
        match server_addr_ret {
            Ok(server_addr) => {
                match cur_conn {
                    Some(conn) => conn.lazy_destroy(),
                    None => {}
                }
                notify_change(self, server_addr)
            }
            Err(err) => {
                tracing::error!("failed to get server address: {}", err);
            }
        }
    }

    fn discover_instances() -> Result<Instance, PolarisError> {
        Ok(Instance::new())
    }
}

pub fn notify_change(
    conn_mgr: &mut ServerAddress,
    server_addr: crate::core::model::naming::Endpoint,
) {
    let mut ns = String::from(BUILDIN_SERVER_NAMESPACE);
    let mut svc = String::from(BUILDIN_SERVER_SERVICE);

    match conn_mgr.service_info.as_ref() {
        Some(svc_info) => {
            ns = svc_info.namespace.clone().unwrap();
            svc = svc_info.service.clone().unwrap();
        }
        None => {}
    }

    let cur_conn_id = ConnID::default()
        .with_id(Uuid::new_v4().to_string())
        .with_svc_key(ServiceKey::new(ns, svc))
        .with_cluster_type(conn_mgr.cluster.clone())
        .with_ip_port(server_addr.clone())
        .build();

    let uri_str = server_addr.format_address();
    futures::executor::block_on(async {
        let ret = conn_mgr
            .sender
            .send(Change::Insert(
                server_addr.clone().format_address(),
                Endpoint::try_from(uri_str.to_string()).ok().unwrap(),
            ))
            .await;
        match ret {
            Ok(_) => {}
            Err(err) => {
                error!(
                    "notify tonic insert polaris-server node address fail: {}",
                    err
                )
            }
        }
    });

    let cur_conn = Connection::new(
        cur_conn_id,
        conn_mgr.sender.clone(),
        Arc::clone(&conn_mgr.conn_mgr),
    );
    conn_mgr.active_conn = Arc::new(cur_conn);
}

pub struct Connection {
    is_empty: bool,
    lock: Mutex<u32>,
    sender: Option<Sender<Change<String, Endpoint>>>,
    conn_id: ConnID,
    conn_mgr: Arc<ConnectionManager>,
    create_time: SystemTime,
    ref_cnt: AtomicU32,
    lazy_destroy: AtomicBool,
    closed: AtomicBool,
}

fn available_cur_connection(conn: Option<&Connection>) -> bool {
    match conn {
        Some(cur_conn) => !cur_conn.is_lazy_destroy(),
        None => false,
    }
}

impl Connection {
    fn empty_conn(conn_mgr: Arc<ConnectionManager>) -> Connection {
        Connection {
            is_empty: true,
            lock: Mutex::new(1),
            sender: None,
            conn_id: ConnID::default(),
            conn_mgr: conn_mgr,
            create_time: SystemTime::now(),
            ref_cnt: Default::default(),
            lazy_destroy: Default::default(),
            closed: AtomicBool::new(false),
        }
    }

    fn new(
        conn_id: ConnID,
        sender: Sender<Change<String, Endpoint>>,
        conn_mgr: Arc<ConnectionManager>,
    ) -> Connection {
        Connection {
            is_empty: false,
            lock: Mutex::new(1),
            sender: Some(sender),
            conn_id,
            conn_mgr,
            create_time: SystemTime::now(),
            ref_cnt: Default::default(),
            lazy_destroy: Default::default(),
            closed: AtomicBool::new(false),
        }
    }

    fn lazy_destroy(&self) {
        self.lazy_destroy.store(true, SeqCst);
        let cur_ref = self.ref_cnt.load(SeqCst);
        info!(
            "connection {}: lazyClose, curRef is {}",
            self.conn_id, cur_ref
        );
        if cur_ref <= 0 {
            self.close_connection();
        }
    }

    fn close_connection(&self) {
        drop(self.lock.lock().unwrap());
        let need_close = self.ref_cnt.load(SeqCst) <= 0 && !self.closed.load(SeqCst);
        if !need_close {
            return;
        }
        info!("connection {}: closed", self.conn_id);
        self.closed.store(true, SeqCst);

        match self.sender.as_ref() {
            Some(sender) => {
                futures::executor::block_on(async {
                    let ret = sender
                        .send(Change::Remove(self.conn_id.format_address()))
                        .await;
                    match ret {
                        Ok(_) => {}
                        Err(err) => {
                            error!(
                                "notify tonic insert polaris-server node address fail: {}",
                                err
                            )
                        }
                    }
                });
            }
            None => {}
        }
    }

    fn is_lazy_destroy(&self) -> bool {
        self.lazy_destroy.load(SeqCst)
    }

    fn acquire(&self, op: &str) -> bool {
        if self.is_lazy_destroy() {
            return false;
        }
        drop(self.lock.lock().unwrap());
        if self.is_lazy_destroy() {
            return false;
        }
        let cur_ref = self.ref_cnt.fetch_add(1, SeqCst);
        debug!(
            "connection {:?}: acquired for op {}, curRef is {}",
            self.conn_id, op, cur_ref
        );
        return true;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(self) struct ConnID {
    id: String,
    svc_key: ServiceKey,
    cluster_type: ClusterType,
    endpoint: crate::core::model::naming::Endpoint,
}

impl Display for ConnID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl ConnID {
    fn default() -> Self {
        Self {
            id: "".to_owned(),
            svc_key: ServiceKey::default(),
            cluster_type: ClusterType::default(),
            endpoint: crate::core::model::naming::Endpoint::default(),
        }
    }

    fn with_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    fn with_svc_key(mut self, svc_key: ServiceKey) -> Self {
        self.svc_key = svc_key;
        self
    }

    fn with_cluster_type(mut self, cluster_type: ClusterType) -> Self {
        self.cluster_type = cluster_type;
        self
    }

    fn with_ip_port(mut self, endpoint: crate::core::model::naming::Endpoint) -> Self {
        self.endpoint = endpoint;
        self
    }

    fn build(mut self) -> Self {
        self
    }

    fn format_address(&self) -> String {
        let addr = self.endpoint.format_address();
        addr
    }
}
