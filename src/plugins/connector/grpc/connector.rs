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
use crate::core::model::pb::lib::polaris_config_grpc_client::PolarisConfigGrpcClient;
use crate::core::model::pb::lib::polaris_grpc_client::PolarisGrpcClient;
use crate::core::model::pb::lib::Code::{ExecuteSuccess, ExistedResource};
use crate::core::model::pb::lib::{Code, DiscoverRequest, DiscoverResponse};
use crate::core::plugin::connector::{Connector, ResourceHandler};
use crate::core::plugin::plugins::{Extensions, Plugin};
use crate::plugins::connector::grpc::manager::ConnectionManager;
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;
use tonic::metadata::{AsciiMetadataKey, MetadataValue};
use tonic::service::interceptor::InterceptedService;
use tonic::service::Interceptor;
use tonic::transport::{Channel, Endpoint};
use tonic::Streaming;
use tracing::Instrument;

use super::manager::EmptyConnectionSwitchListener;

pub struct GrpcConnector {
    discover_channel: Channel,
    config_channel: Channel,
    label: String,
    extensions: Arc<Extensions>,
    connection_manager: Arc<ConnectionManager>,
    discover_grpc_client: PolarisGrpcClient<Channel>,
    config_grpc_client: PolarisConfigGrpcClient<Channel>,
}

fn new_connector(
    label: String,
    conf: &ServerConnectorConfig,
    extensions: Arc<Extensions>,
) -> Box<dyn Connector> {
    let (discover_channel, config_channel) = create_channel(label.clone(), conf);

    let client_id = extensions.get_client_id();
    let connect_timeout = conf.connect_timeout;
    let server_switch_interval = conf.server_switch_interval;

    let discover_grpc_client = create_discover_grpc_client(label.clone(), discover_channel.clone());
    let config_grpc_client = create_config_grpc_client(label.clone(), config_channel.clone());

    let c = GrpcConnector {
        discover_channel: discover_channel.clone(),
        config_channel: config_channel.clone(),
        label: label.clone(),
        extensions: extensions,
        connection_manager: Arc::new(ConnectionManager::new(
            connect_timeout,
            server_switch_interval,
            client_id,
            Arc::new(EmptyConnectionSwitchListener::new()),
        )),
        discover_grpc_client: discover_grpc_client,
        config_grpc_client: config_grpc_client,
    };

    Box::new(c) as Box<dyn Connector + 'static>
}

fn create_channel(label: String, conf: &ServerConnectorConfig) -> (Channel, Channel) {
    let addresses = conf.addresses.clone();
    let mut discover_address: Vec<String> = Vec::new();
    let mut config_address: Vec<String> = Vec::new();

    for ele in addresses {
        if ele.starts_with("discover://") {
            discover_address.push(format!(
                "http://{}",
                ele.trim_start_matches("discover://").to_string()
            ));
        } else if ele.starts_with("config://") {
            config_address.push(format!(
                "http://{}",
                ele.trim_start_matches("config://").to_string()
            ));
        }
    }

    let connect_timeout = conf.connect_timeout;

    let discover_endpoints = discover_address.iter().map(|item| {
        Endpoint::from_shared(item.to_string())
            .unwrap()
            .connect_timeout(connect_timeout.clone())
    });
    let config_endpoints = discover_address.iter().map(|item| {
        Endpoint::from_shared(item.to_string())
            .unwrap()
            .connect_timeout(connect_timeout.clone())
    });

    let discover_channel = Channel::balance_list(discover_endpoints);
    let config_channel = Channel::balance_list(config_endpoints);

    (discover_channel, config_channel)
}

fn create_discover_grpc_client(label: String, channel: Channel) -> PolarisGrpcClient<Channel> {
    PolarisGrpcClient::new(channel)
}

fn create_config_grpc_client(label: String, channel: Channel) -> PolarisConfigGrpcClient<Channel> {
    PolarisConfigGrpcClient::new(channel)
}

impl Plugin for GrpcConnector {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        "grpc".to_string()
    }
}

impl GrpcConnector {
    fn wait_server_ready(&self) {}

    pub fn builder() -> (
        fn(String, &ServerConnectorConfig, Arc<Extensions>) -> Box<dyn Connector>,
        String,
    ) {
        return (new_connector, "grpc".to_string());
    }

    fn create_discover_grpc_stub(
        &self,
        flow: String,
    ) -> PolarisGrpcClient<InterceptedService<Channel, GrpcConnectorInterceptor>> {
        let interceptor = GrpcConnectorInterceptor {
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("request-id".to_string(), flow.to_string());
                metadata
            },
        };
        PolarisGrpcClient::with_interceptor(self.discover_channel.clone(), interceptor)
    }
}

#[async_trait::async_trait]
impl Connector for GrpcConnector {
    async fn register_resource_handler(
        &self,
        handler: Arc<dyn ResourceHandler>,
    ) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn deregister_resource_handler(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn register_instance(
        &self,
        req: InstanceRequest,
    ) -> Result<InstanceResponse, PolarisError> {
        self.wait_server_ready();

        tracing::debug!("[polaris][discovery][connector] send register instance request={req:?}");

        let mut client = self.create_discover_grpc_stub(req.flow_id.clone());
        let ret = client
            .register_instance(tonic::Request::new(req.convert_spec()))
            .in_current_span()
            .await;
        return match ret {
            Ok(rsp) => {
                let rsp = rsp.into_inner();
                if rsp.instance.is_none() {
                    return Err(PolarisError::new(
                        ServerUserError,
                        "[polaris][discovery][connector] invalid register response: missing instance".to_string(),
                    ));
                }
                let recv_code: Code = unsafe { std::mem::transmute(rsp.code.unwrap()) };
                if ExecuteSuccess.eq(&recv_code) {
                    let ins_id = rsp.instance.unwrap().id.unwrap();
                    tracing::info!(
                        "[polaris][discovery][connector] register instance to server success id={}",
                        ins_id.clone(),
                    );
                    let mut ins = InstanceResponse::default();
                    ins.instance.id = ins_id.clone();
                    return Ok(ins);
                }
                if ExistedResource.eq(&recv_code) {
                    return Ok(InstanceResponse::exist_resource());
                }
                Err(PolarisError::new(ServerError, rsp.info.unwrap()))
            }
            Err(err) => {
                tracing::error!(
                    "[polaris][discovery][connector] send register request to server fail: {}",
                    err
                );
                Err(PolarisError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn deregister_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError> {
        self.wait_server_ready();

        tracing::debug!("[polaris][discovery][connector] send deregister instance request={req:?}");

        let mut client = self.create_discover_grpc_stub(req.flow_id.clone());
        let ret = client
            .deregister_instance(tonic::Request::new(req.convert_spec()))
            .in_current_span()
            .await;
        return match ret {
            Ok(rsp) => {
                let rsp = rsp.into_inner();
                let recv_code: Code = unsafe { std::mem::transmute(rsp.code.unwrap()) };
                if ExecuteSuccess.eq(&recv_code) {
                    return Ok(true);
                }
                tracing::error!(
                    "[polaris][discovery][connector] send deregister request to server receive fail: code={} info={}",
                    rsp.code.unwrap().clone(),
                    rsp.info.clone().unwrap(),
                );
                Err(PolarisError::new(ServerError, rsp.info.unwrap()))
            }
            Err(err) => {
                tracing::error!(
                    "[polaris][discovery][connector] send deregister request to server fail: {}",
                    err
                );
                Err(PolarisError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn heartbeat_instance(&self, req: InstanceRequest) -> Result<bool, PolarisError> {
        self.wait_server_ready();

        tracing::debug!("[polaris][discovery][connector] send heartbeat instance request={req:?}");

        let mut client = self.create_discover_grpc_stub(req.flow_id.clone());
        let ret = client
            .heartbeat(tonic::Request::new(req.convert_beat_spec()))
            .in_current_span()
            .await;
        return match ret {
            Ok(rsp) => {
                let rsp = rsp.into_inner();
                let recv_code: Code = unsafe { std::mem::transmute(rsp.code.unwrap()) };
                if ExecuteSuccess.eq(&recv_code) {
                    return Ok(true);
                }
                tracing::error!(
                    "[polaris][discovery][connector] send heartbeat request to server receive fail: code={} info={}",
                    rsp.code.unwrap().clone(),
                    rsp.info.clone().unwrap(),
                );
                Err(PolarisError::new(ServerError, rsp.info.unwrap()))
            }
            Err(err) => {
                tracing::error!(
                    "[polaris][discovery][connector] send heartbeat request to server fail: {}",
                    err
                );
                Err(PolarisError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn report_client(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn report_service_contract(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn get_service_contract(&self) -> Result<String, PolarisError> {
        todo!()
    }

    async fn create_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn update_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn release_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }

    async fn upsert_publish_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }
}

struct DiscoverStreamClient {
    sender: UnboundedSender<DiscoverRequest>,
    reciver: Streaming<DiscoverResponse>
}

impl DiscoverStreamClient {
    async fn build(client: &mut PolarisGrpcClient<Channel>) -> Result<Self, PolarisError> {
        let (tx, rx) = mpsc::unbounded_channel::<DiscoverRequest>();
        let reciver = UnboundedReceiverStream::new(rx);
        let discover_rt = client.discover(tonic::Request::new(reciver)).await;

        if discover_rt.is_err() {
            return Err(PolarisError::new(crate::core::model::error::ErrorCode::PluginError, discover_rt.err().unwrap().to_string()));
        }

        let stream_recv = discover_rt.ok().unwrap().into_inner();
        Ok(Self{
            sender: tx,
            reciver: stream_recv
        })
    }
}

struct ConfigStreamClient {}

struct GrpcConnectorInterceptor {
    metadata: HashMap<String, String>,
}

impl Interceptor for GrpcConnectorInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        let metadata = self.metadata.clone();
        for ele in metadata {
            let meta_key = AsciiMetadataKey::from_str(ele.0.to_string().as_str());
            if meta_key.is_err() {
                return Err(tonic::Status::internal("invalid metadata value"));
            }
            let meta_val: Result<MetadataValue<_>, _> = ele.1.to_string().parse();
            if meta_val.is_err() {
                return Err(tonic::Status::internal("invalid metadata value"));
            }
            request
                .metadata_mut()
                .insert(meta_key.unwrap(), meta_val.unwrap());
        }
        Ok(request)
    }
}
