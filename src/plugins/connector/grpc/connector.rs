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

use crate::core::model::error::ErrorCode::{ServerError, ServerUserError};
use crate::core::model::error::PolarisError;
use crate::core::model::naming::{InstanceRequest, InstanceResponse};
use crate::core::model::pb::lib::polaris_grpc_client::PolarisGrpcClient;
use crate::core::model::pb::lib::Code;
use crate::core::model::pb::lib::Code::{ExecuteSuccess, ExistedResource};
use crate::core::plugin::connector::Connector;
use crate::core::plugin::plugins::{Extensions, Plugin};
use crate::plugins::connector::grpc::manager::ConnectionManager;
use log::error;
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tonic::transport::{Channel, Endpoint};
use tower::discover::Change;

use super::manager::EmptyConnectionSwitchListener;

pub struct GrpcConnector {
    extensions: Arc<Option<Extensions>>,
    channel: Channel,
    sender: Sender<Change<String, Endpoint>>,
    connection_manager: Arc<Option<ConnectionManager>>,
    buildin_client: Box<Option<PolarisGrpcClient<Channel>>>,
    discover_client: Box<Option<PolarisGrpcClient<Channel>>>,
    healtch_client: Box<Option<PolarisGrpcClient<Channel>>>,
    config_client: Box<Option<PolarisGrpcClient<Channel>>>,
}

impl GrpcConnector {
    fn wait_discover_ready(&self) {}
}

impl Default for GrpcConnector {
    fn default() -> Self {
        let (channel, sender) = Channel::balance_channel(64);
        Self {
            extensions: Arc::new(None),
            channel: channel,
            sender: sender,
            connection_manager: Arc::new(None),
            buildin_client: Box::new(None),
            discover_client: Box::new(None),
            healtch_client: Box::new(None),
            config_client: Box::new(None),
        }
    }
}

impl Plugin for GrpcConnector {
    fn init(&mut self, options: HashMap<String, String>, extensions: Arc<Extensions>) {
        self.connection_manager = Arc::new(Some(ConnectionManager::new(
            Duration::from_secs(10),
            Duration::from_secs(10),
            "1".to_string(),
            Arc::new(EmptyConnectionSwitchListener::new()),
        )));
    }

    fn destroy(&self) {
        todo!()
    }

    fn name(&self) -> String {
        "grpc".to_string()
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
        return futures::executor::block_on(async {
            let mut discover_client = self.discover_client.clone().unwrap();
            let ret = discover_client.register_instance(req.convert_spec()).await;
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
                    Err(PolarisError::new(ServerError, "".to_string()))
                }
            };
        });
    }

    fn deregister_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError> {
        self.wait_discover_ready();
        return futures::executor::block_on(async {
            let mut discover_client = self.discover_client.clone().unwrap();
            let ret = discover_client
                .deregister_instance(req.convert_spec())
                .await;
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
