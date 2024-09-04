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

use crate::core::config::global::ServerConnectorConfig;
use crate::core::model::error::ErrorCode::{ServerError, ServerUserError};
use crate::core::model::error::PolarisError;
use crate::core::model::naming::{InstanceRequest, InstanceResponse};
use crate::core::model::pb::lib::polaris_grpc_client::PolarisGrpcClient;
use crate::core::model::pb::lib::Code;
use crate::core::model::pb::lib::Code::{ExecuteSuccess, ExistedResource};
use crate::core::plugin::connector::Connector;
use crate::core::plugin::plugins::{Extensions, Plugin};
use crate::plugins::connector::grpc::manager::ConnectionManager;
use futures::future::BoxFuture;
use http::Uri;
use hyper::rt::Executor;
use log::error;
use std::cmp::PartialEq;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use tonic::transport::{Channel, Endpoint};

use super::manager::EmptyConnectionSwitchListener;

pub struct GrpcConnector {
    label: String,
    extensions: Arc<Extensions>,
    connection_manager: Arc<ConnectionManager>,
    grpc_client: PolarisGrpcClient<Channel>,
}

fn new_connector(label: String, conf: &ServerConnectorConfig, extensions: Arc<Extensions>) -> Box<dyn Connector> {
    let addresses = conf.addresses.clone();
    let connect_server = format!("http://{}", addresses.get(0).unwrap().clone());
    let uri_ret = Uri::from_str(connect_server.as_str());
    if uri_ret.is_err() {
        panic!("parse server connect info fail: {}", uri_ret.unwrap_err());
    }
    log::info!("connect to server: {:?}", connect_server);
    let connect_ret = extensions.runtime.block_on(async {
        return Channel::builder(uri_ret.unwrap()).connect().await;
    });

    if connect_ret.is_err() {
        panic!("connect to server fail: {}", connect_ret.unwrap_err());
    }

    let channel = connect_ret.unwrap();
    let grpc_client = PolarisGrpcClient::new(channel);
    let client_id = extensions.get_client_id();
    let connect_timeout = conf.connect_timeout;
    let server_switch_interval = conf.server_switch_interval;

    let c = GrpcConnector {
        label: label,
        extensions: extensions,
        connection_manager: Arc::new(ConnectionManager::new(
            connect_timeout,
            server_switch_interval,
            client_id,
            Arc::new(EmptyConnectionSwitchListener::new()),
        )),
        grpc_client: grpc_client,
    };
    Box::new(c) as Box<dyn Connector + 'static>
}

impl Plugin for GrpcConnector {
    fn init(&mut self, extensions: Extensions) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        "grpc".to_string()
    }
}

impl GrpcConnector {
    fn wait_discover_ready(&self) {}

    pub fn builder() -> (
        fn(String, &ServerConnectorConfig, Arc<Extensions>) -> Box<dyn Connector>,
        String,
    ) {
        return (new_connector, "grpc".to_string());
    }
}

impl Connector for GrpcConnector {
    fn register_resource_handler(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn deregister_resource_handler(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn register_instance(&self, req: InstanceRequest) -> Result<InstanceResponse, PolarisError> {
        self.wait_discover_ready();
        return self.extensions.runtime.block_on(async {
            let mut client = self.grpc_client.clone();
            let ret = client.register_instance(req.convert_spec()).await;
            return match ret {
                Ok(rsp) => {
                    let rsp = rsp.into_inner();
                    if rsp.instance.is_none() {
                        return Err(PolarisError::new(
                            ServerUserError,
                            "invalid register response: missing instance".to_string(),
                        ));
                    }
                    let recv_code: Code = unsafe { std::mem::transmute(rsp.code.unwrap()) };
                    if ExecuteSuccess.eq(&recv_code) {
                        return Ok(InstanceResponse::default());
                    }
                    if ExistedResource.eq(&recv_code) {
                        return Ok(InstanceResponse::exist_resource());
                    }
                    Err(PolarisError::new(ServerError, rsp.info.unwrap()))
                }
                Err(err) => {
                    error!("send register instance request to server fail: {}", err);
                    Err(PolarisError::new(ServerError, err.to_string()))
                }
            };
        });
    }

    fn deregister_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError> {
        self.wait_discover_ready();
        return self.extensions.runtime.block_on(async {
            let mut client: PolarisGrpcClient<Channel> = self.grpc_client.clone();
            let ret = client.deregister_instance(req.convert_spec()).await;
            return match ret {
                Ok(rsp) => {
                    let rsp = rsp.into_inner();
                    if rsp.instance.is_none() {
                        return Err(PolarisError::new(
                            ServerUserError,
                            "invalid deregister response: missing instance".to_string(),
                        ));
                    }
                    let recv_code: Code = unsafe { std::mem::transmute(rsp.code.unwrap()) };
                    if ExecuteSuccess.eq(&recv_code) {
                        return Ok(true);
                    }
                    Err(PolarisError::new(ServerError, rsp.info.unwrap()))
                }
                Err(err) => {
                    error!("send deregister instance request to server fail: {}", err);
                    Err(PolarisError::new(ServerError, "".to_string()))
                }
            };
        });
    }

    fn heartbeat_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError> {
        todo!()
    }

    fn report_client(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn report_service_contract(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn get_service_contract(&self) -> Result<String, PolarisError> {
        todo!()
    }

    fn create_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn update_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn release_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    fn upsert_publish_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }
}

#[derive(Copy, Clone)]
struct ConnectorExec;

impl<F> Executor<F> for ConnectorExec
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, fut: F) {
        thread::spawn(move || fut);
    }
}

#[derive(Clone)]
pub(crate) struct SharedExec {
    inner: Arc<dyn Executor<BoxFuture<'static, ()>> + Send + Sync + 'static>,
}

impl SharedExec {
    pub(crate) fn new<E>(exec: E) -> Self
    where
        E: Executor<Pin<Box<dyn std::future::Future<Output = ()> + Send>>> + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(exec),
        }
    }

    pub(crate) fn connector() -> Self {
        Self::new(ConnectorExec)
    }
}

impl Executor<BoxFuture<'static, ()>> for SharedExec {
    fn execute(&self, fut: BoxFuture<'static, ()>) {
        self.inner.execute(fut)
    }
}
