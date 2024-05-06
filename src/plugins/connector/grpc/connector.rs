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

use std::cmp::PartialEq;
use std::collections::HashMap;
use std::f32::consts::E;
use std::sync::Arc;
use log::error;
use tonic::IntoRequest;
use tonic::transport::Endpoint;
use tower::discover::Change;
use crate::core::model::cluster::ClusterType;
use crate::core::model::error::ErrorCode::{ServerError, ServerUserError};
use crate::core::model::error::PolarisError;
use crate::core::model::naming::{InstanceRequest, InstanceResponse};
use crate::core::model::pb::lib::Code;
use crate::core::model::pb::lib::Code::{ExecuteSuccess, ExistedResource};
use crate::core::model::pb::lib::polaris_grpc_client::PolarisGrpcClient;
use crate::core::plugin::connector::Connector;
use crate::core::plugin::plugins::{Extensions, Plugin};
use crate::plugins::connector::grpc::manager::ConnectionManager;

pub struct GrpcConnector {
    connection_manager: ConnectionManager
}

impl GrpcConnector {
    fn get_conn_mgr(&mut self) -> &mut ConnectionManager {
        &mut self.connection_manager
    }
}

impl Default for GrpcConnector {
    fn default() -> Self {
        todo!()
    }
}

impl Plugin for GrpcConnector {
    fn init(&self, options: HashMap<String, String>, extensions: Arc<Extensions>) {
        todo!()
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

    fn register_instance(&mut self, req: InstanceRequest) -> Result<InstanceResponse, PolarisError> {
        let mut conn_mgr_ref = self.get_conn_mgr();
        let conn_ret = conn_mgr_ref.get_connection("register_instance", ClusterType::Discover);
        return match conn_ret {
            Ok(mut conn) => {
                let client = conn.get_channel();
                return futures::executor::block_on(async {
                    let ret = client.register_instance(req.convert_spec()).await;
                    return match ret {
                        Ok(rsp) => {
                            let rsp = rsp.into_inner();
                            if rsp.instance.is_none() {
                                return Err(PolarisError::new(ServerUserError, "invalid register response: missing instance".to_string()))
                            }
                            let recv_code: Code = unsafe { std::mem::transmute(rsp.code.unwrap()) };
                            if ExecuteSuccess.eq(&recv_code) {
                                return Ok(InstanceResponse::default())
                            }
                            if ExistedResource.eq(&recv_code) {
                                return Ok(InstanceResponse::exist_resource())
                            }
                            Err(PolarisError::new(ServerError, rsp.info.unwrap()))
                        }
                        Err(err) => {
                            error!("send register instance request to server fail: {}", err);
                            Err(PolarisError::new(ServerError, "".to_string()))
                        }
                    }
                });
            }
            Err(err) => {
                error!("try get one connection to do register instance action fail: {}", err);
                Err(err)
            }
        }
    }

    fn deregister_instance(&mut self, req: InstanceRequest) -> Result<bool, PolarisError> {
        let mut conn_mgr_ref = &self.connection_manager;
        let conn = conn_mgr_ref.get_connection("deregister_instance", ClusterType::Discover);
        return match conn {
            Ok(mut conn) => {
                let mut client = conn.get_channel();
                return futures::executor::block_on(async {
                    let ret = client.deregister_instance(req.convert_spec()).await;
                    return match ret {
                        Ok(rsp) => {
                            let rsp = rsp.into_inner();
                            if rsp.instance.is_none() {
                                return Err(PolarisError::new(ServerUserError, "invalid deregister response: missing instance".to_string()))
                            }
                            let recv_code: Code = unsafe { std::mem::transmute(rsp.code.unwrap()) };
                            if ExecuteSuccess.eq(&recv_code) {
                                return Ok(true)
                            }
                            Err(PolarisError::new(ServerError, rsp.info.unwrap()))
                        }
                        Err(err) => {
                            error!("send deregister instance request to server fail: {}", err);
                            Err(PolarisError::new(ServerError, "".to_string()))
                        }
                    }
                });
            }
            Err(err) => {
                error!("try get one connection to do deregister instance action fail: {}", err);
                Err(err)
            }
        }
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
