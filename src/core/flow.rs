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

use crate::core::plugin::router::ServiceRouter;

use super::{
    model::{
        circuitbreaker::{CheckResult, CircuitBreakerStatus, Resource, ResourceStat, Status},
        error::PolarisError,
        naming::ServiceInstances,
        ClientContext, ReportClientRequest,
    },
    plugin::{location::LocationSupplier, plugins::Extensions, router::RouteContext},
};

pub struct ClientFlow
where
    Self: Send + Sync,
{
    client: Arc<ClientContext>,
    extensions: Arc<Extensions>,

    futures: Vec<JoinHandle<()>>,

    closed: Arc<AtomicBool>,
}

impl ClientFlow {
    pub fn new(client: Arc<ClientContext>, extensions: Arc<Extensions>) -> Self {
        ClientFlow {
            client,
            extensions: extensions,
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
                ClientFlow::report_client(client.clone(), extensions.clone()).await;
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
        let server_connector = extensions.get_server_connector();
        let loc_provider = extensions.get_location_provider();
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

/// CircuitBreakerFlow
pub struct CircuitBreakerFlow {
    extensions: Arc<Extensions>,
}

impl CircuitBreakerFlow {
    pub fn new(extensions: Arc<Extensions>) -> Self {
        CircuitBreakerFlow { extensions }
    }

    pub async fn check_resource(&self, resource: Resource) -> Result<CheckResult, PolarisError> {
        let circuit_breaker_opt = self.extensions.circuit_breaker.clone();
        if circuit_breaker_opt.is_none() {
            return Ok(CheckResult::pass());
        }

        let circuit_breaker = circuit_breaker_opt.unwrap();
        let status = circuit_breaker.check_resource(resource).await?;

        Ok(CircuitBreakerFlow::convert_from_status(status))
    }

    pub async fn report_stat(&self, stat: ResourceStat) -> Result<(), PolarisError> {
        let circuit_breaker_opt = self.extensions.circuit_breaker.clone();
        if circuit_breaker_opt.is_none() {
            return Ok(());
        }

        let circuit_breaker = circuit_breaker_opt.unwrap();
        circuit_breaker.report_stat(stat).await
    }

    fn convert_from_status(ret: CircuitBreakerStatus) -> CheckResult {
        let status = ret.status;
        CheckResult {
            pass: status == Status::Open,
            rule_name: ret.circuit_breaker,
            fallback_info: ret.fallback_info.clone(),
        }
    }
}

pub struct RouterFlow {
    extensions: Arc<Extensions>,
}

impl RouterFlow {
    pub fn new(extensions: Arc<Extensions>) -> Self {
        RouterFlow { extensions }
    }

    pub async fn choose_instances(
        &self,
        route_ctx: RouteContext,
        instances: ServiceInstances,
    ) -> Result<ServiceInstances, PolarisError> {
        let router_container = self.extensions.get_router_container();

        let mut routers = Vec::<Arc<Box<dyn ServiceRouter>>>::new();
        let chain = &route_ctx.route_info.chain;

        // 处理前置路由
        chain.before.iter().for_each(|name| {
            if let Some(router) = router_container.before_routers.get(name) {
                routers.push(router.clone());
            }
        });

        let mut tmp_instance = instances;
        for (_, ele) in routers.iter().enumerate() {
            let ret = ele.choose_instances(route_ctx.clone(), tmp_instance).await;
            if let Err(e) = ret {
                return Err(e);
            }
            tmp_instance = ret.unwrap().instances;
        }
        routers.clear();

        // 处理核心路由
        chain.core.iter().for_each(|name| {
            if let Some(router) = router_container.core_routers.get(name) {
                routers.push(router.clone());
            }
        });

        for (_, ele) in routers.iter().enumerate() {
            let ret = ele.choose_instances(route_ctx.clone(), tmp_instance).await;
            if let Err(e) = ret {
                return Err(e);
            }
            tmp_instance = ret.unwrap().instances;
        }
        routers.clear();

        // 处理后置路由
        chain.after.iter().for_each(|name| {
            if let Some(router) = router_container.before_routers.get(name) {
                routers.push(router.clone());
            }
        });

        for (_, ele) in routers.iter().enumerate() {
            let ret = ele.choose_instances(route_ctx.clone(), tmp_instance).await;
            if let Err(e) = ret {
                return Err(e);
            }
            tmp_instance = ret.unwrap().instances;
        }

        Ok(tmp_instance)
    }
}
