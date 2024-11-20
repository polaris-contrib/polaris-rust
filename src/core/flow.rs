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

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::{task::JoinHandle, time::sleep};

use super::{
    model::{ClientContext, ReportClientRequest},
    plugin::{location::LocationSupplier, plugins::Extensions},
};

pub struct Flow
where
    Self: Send + Sync,
{
    client: Arc<ClientContext>,
    extensions: Arc<Extensions>,

    futures: Vec<JoinHandle<()>>,

    closed: Arc<AtomicBool>,
}

impl Flow {
    pub fn new(client: Arc<ClientContext>, extensions: Extensions) -> Self {
        Flow {
            client,
            extensions: Arc::new(extensions),
            futures: vec![],
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn run_flow(&mut self) {
        let client = self.client.clone();
        let extensions = self.extensions.clone();
        let is_closed = self.closed.clone();

        let f: JoinHandle<()> = self.extensions.runtime.spawn(async move {
            loop {
                Flow::report_client(client.clone(), extensions.clone()).await;
                sleep(Duration::from_secs(60)).await;
                if is_closed.load(Ordering::Relaxed) {
                    return;
                }
            }
        });

        self.futures.push(f);
    }

    /// report_client 上报客户端信息数据
    pub async fn report_client(client: Arc<ClientContext>, extensions: Arc<Extensions>) {
        let server_connector = extensions.server_connector();
        let loc_provider = extensions.location_provider();
        let loc = loc_provider.get_location();
        let req = ReportClientRequest {
            client_id: client.client_id.clone(),
            host: client.host.clone(),
            version: client.version.clone(),
            location: loc,
        };
        let ret = server_connector.report_client(req).await;
        if let Err(e) = ret {
            tracing::error!("report client failed: {:?}", e);
        }
    }

    pub fn stop_flow(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
    }
}
