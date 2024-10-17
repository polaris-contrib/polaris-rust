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
use crate::core::model::cache::{EventType, RemoteData};
use crate::core::model::config::{ConfigFileRequest, ConfigReleaseRequest};
use crate::core::model::error::ErrorCode::{ServerError, ServerUserError};
use crate::core::model::error::PolarisError;
use crate::core::model::naming::{InstanceRequest, InstanceResponse};
use crate::core::model::pb::lib::polaris_config_grpc_client::PolarisConfigGrpcClient;
use crate::core::model::pb::lib::polaris_grpc_client::PolarisGrpcClient;
use crate::core::model::pb::lib::Code::{ExecuteSuccess, ExistedResource};
use crate::core::model::pb::lib::{
    Code, ConfigDiscoverRequest, ConfigDiscoverResponse, DiscoverRequest, DiscoverResponse,
};
use crate::core::plugin::connector::{Connector, InitConnectorOption, ResourceHandler};
use crate::core::plugin::plugins::Plugin;
use crate::plugins::connector::grpc::manager::ConnectionManager;
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::thread::sleep;
use std::time::{self, Duration};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;
use tonic::metadata::{AsciiMetadataKey, MetadataValue};
use tonic::service::interceptor::InterceptedService;
use tonic::service::Interceptor;
use tonic::transport::{Channel, Endpoint};
use tonic::Streaming;
use tracing::Instrument;

use super::manager::EmptyConnectionSwitchListener;

struct ResourceHandlerWrapper {
    handler: Box<dyn ResourceHandler>,
    revision: String,
}

#[derive(Clone)]
pub struct GrpcConnector {
    opt: InitConnectorOption,
    discover_channel: Channel,
    config_channel: Channel,
    connection_manager: Arc<ConnectionManager>,
    discover_grpc_client: PolarisGrpcClient<Channel>,
    config_grpc_client: PolarisConfigGrpcClient<Channel>,

    discover_spec_sender: Arc<UnboundedSender<DiscoverRequest>>,
    config_spec_sender: Arc<UnboundedSender<ConfigDiscoverRequest>>,

    watch_resources: Arc<RwLock<HashMap<String, ResourceHandlerWrapper>>>,
}

fn new_connector(opt: InitConnectorOption) -> Box<dyn Connector> {
    let conf = &opt.conf.global.server_connectors.clone();
    let (discover_channel, config_channel) = create_channel(conf);

    let client_id = opt.client_id.clone();

    let connect_timeout = conf.connect_timeout;
    let server_switch_interval = conf.server_switch_interval;

    let discover_grpc_client = create_discover_grpc_client(discover_channel.clone());
    let config_grpc_client = create_config_grpc_client(config_channel.clone());

    let (discover_sender, mut discover_reciver) =
        run_discover_spec_stream(discover_channel.clone(), opt.runtime.clone()).unwrap();

    let (config_sender, mut config_reciver) =
        run_config_spec_stream(config_channel.clone(), opt.runtime.clone()).unwrap();

    let c = GrpcConnector {
        opt: opt,
        discover_channel: discover_channel.clone(),
        config_channel: config_channel.clone(),
        connection_manager: Arc::new(ConnectionManager::new(
            connect_timeout,
            server_switch_interval,
            client_id,
            Arc::new(EmptyConnectionSwitchListener::new()),
        )),
        discover_grpc_client: discover_grpc_client,
        config_grpc_client: config_grpc_client,

        discover_spec_sender: Arc::new(discover_sender),
        config_spec_sender: Arc::new(config_sender),

        watch_resources: Arc::new(RwLock::new(HashMap::new())),
    };

    let receive_c = c.clone();
    // 创建一个新的线程，用于处理grpc的消息
    c.opt.runtime.spawn(async move {
        loop {
            tokio::select! {
                discover_ret = discover_reciver.recv() => {
                    if discover_ret.is_some() {
                        receive_c.receive_discover_response(discover_ret.unwrap()).await;
                    }
                }
                config_ret = config_reciver.recv() => {
                    if config_ret.is_some() {
                        receive_c.receive_config_response(config_ret.unwrap());
                    }
                }
            }
        }
    });

    let send_c = c.clone();
    // 开启一个异步任务，定期发送请求到服务端
    c.opt.runtime.spawn(async move {
        loop {
            {
                // 额外一个方法块，减少 lock 的占用时间
                let watch_resources = send_c.watch_resources.read().await;
                watch_resources.iter().for_each(|(_key, handler)| {
                    let key = handler.handler.interest_resource();
                    let filter = key.clone().filter;
                    tracing::debug!(
                        "[polaris][discovery][connector] send discover request: {:?} filter: {:?}",
                        key.clone(),
                        filter.clone()
                    );

                    let discover_request = key.to_discover_request(handler.revision.clone());
                    let config_request = key.to_config_request(handler.revision.clone());

                    if discover_request.is_some() {
                        send_c.send_naming_discover_request(discover_request.unwrap());
                    } else {
                        send_c.send_config_discover_request(config_request.unwrap());
                    }
                });
            }
            sleep(Duration::from_secs(2));
        }
    });

    Box::new(c) as Box<dyn Connector + 'static>
}

fn create_channel(conf: &ServerConnectorConfig) -> (Channel, Channel) {
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

    tracing::info!(
        "[polaris][server_connector] discover_address: {:?} config_address: {:?}",
        discover_address,
        config_address
    );

    let connect_timeout = conf.connect_timeout;

    let discover_endpoints = discover_address.iter().map(|item| {
        Endpoint::from_shared(item.to_string())
            .unwrap()
            .connect_timeout(connect_timeout.clone())
    });
    let config_endpoints = config_address.iter().map(|item| {
        Endpoint::from_shared(item.to_string())
            .unwrap()
            .connect_timeout(connect_timeout.clone())
    });

    let discover_channel = Channel::balance_list(discover_endpoints);
    let config_channel = Channel::balance_list(config_endpoints);

    (discover_channel, config_channel)
}

fn create_discover_grpc_client(channel: Channel) -> PolarisGrpcClient<Channel> {
    PolarisGrpcClient::new(channel)
}

fn create_config_grpc_client(channel: Channel) -> PolarisConfigGrpcClient<Channel> {
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
    pub fn builder() -> (fn(opt: InitConnectorOption) -> Box<dyn Connector>, String) {
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

    fn create_config_grpc_stub(
        &self,
        flow: String,
    ) -> PolarisConfigGrpcClient<InterceptedService<Channel, GrpcConnectorInterceptor>> {
        let interceptor = GrpcConnectorInterceptor {
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("request-id".to_string(), flow.to_string());
                metadata
            },
        };
        PolarisConfigGrpcClient::with_interceptor(self.config_channel.clone(), interceptor)
    }

    async fn receive_discover_response(&self, resp: DiscoverResponse) {
        let remote_rsp = resp.clone();
        if remote_rsp.code.unwrap() == Code::DataNoChange as u32 {
            tracing::debug!(
                "[polaris][discovery][connector] receive naming_discover no_change response: {:?}",
                resp
            );
            return;
        }

        if remote_rsp.code.unwrap() != Code::ExecuteSuccess as u32 {
            tracing::error!(
                "[polaris][discovery][connector] receive naming_discover failure response: {:?}",
                resp
            );
            return;
        }

        let mut watch_key = "".to_string();
        match resp.r#type() {
            crate::core::model::pb::lib::discover_response::DiscoverResponseType::Services => {
                watch_key = resp.service.unwrap().namespace.clone().unwrap();
            },
            crate::core::model::pb::lib::discover_response::DiscoverResponseType::Instance => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!("{:?}#{}#{}", EventType::Instance, svc.namespace.clone().unwrap(), svc.name.clone().unwrap());
            },
            crate::core::model::pb::lib::discover_response::DiscoverResponseType::Routing => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!("{:?}#{}#{}", EventType::RouterRule, svc.namespace.clone().unwrap(), svc.name.clone().unwrap());
            },
            crate::core::model::pb::lib::discover_response::DiscoverResponseType::RateLimit => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!("{:?}#{}#{}", EventType::RateLimitRule, svc.namespace.clone().unwrap(), svc.name.clone().unwrap());
            },
            crate::core::model::pb::lib::discover_response::DiscoverResponseType::CircuitBreaker => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!("{:?}#{}#{}", EventType::CircuitBreakerRule, svc.namespace.clone().unwrap(), svc.name.clone().unwrap());
            },
            crate::core::model::pb::lib::discover_response::DiscoverResponseType::FaultDetector => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!("{:?}#{}#{}", EventType::FaultDetectRule, svc.namespace.clone().unwrap(), svc.name.clone().unwrap());
            },
            crate::core::model::pb::lib::discover_response::DiscoverResponseType::Lane => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!("{:?}#{}#{}", EventType::LaneRule, svc.namespace.clone().unwrap(), svc.name.clone().unwrap());
            },
            _ => {},
        }
        let mut handlers = self.watch_resources.write().await;
        if let Some(handle) = handlers.get_mut(watch_key.as_str()) {
            handle.revision = remote_rsp
                .service
                .clone()
                .unwrap()
                .revision
                .clone()
                .unwrap();
            handle.handler.handle_event(RemoteData {
                event_key: handle.handler.interest_resource(),
                discover_value: Some(remote_rsp),
                config_value: None,
            });
        }
    }

    fn receive_config_response(&self, mut resp: ConfigDiscoverResponse) {
        tracing::info!(
            "[polaris][config][connector] receive config_discover response: {:?}",
            resp
        );

        let filters = self.opt.config_filters.clone();
        for (_i, filter) in filters.iter().enumerate() {
            let ret = filter.response_process(
                crate::core::model::DiscoverResponseInfo::Configuration(resp.clone()),
            );
            match ret {
                Ok(rsp) => {
                    resp = rsp.to_config_response();
                }
                Err(err) => {
                    tracing::error!(
                        "[polaris][config][connector] filter response_process fail: {}",
                        err.to_string()
                    );
                    return;
                }
            }
        }

        match resp.r#type() {
            crate::core::model::pb::lib::config_discover_response::ConfigDiscoverResponseType::ConfigFile => todo!(),
            crate::core::model::pb::lib::config_discover_response::ConfigDiscoverResponseType::ConfigFileNames => todo!(),
            crate::core::model::pb::lib::config_discover_response::ConfigDiscoverResponseType::ConfigFileGroups => todo!(),
            _ => todo!()
        }
    }

    fn send_naming_discover_request(&self, req: DiscoverRequest) {
        let _ = self.discover_spec_sender.send(req);
    }

    fn send_config_discover_request(&self, mut req: ConfigDiscoverRequest) {
        let filters = self.opt.config_filters.clone();
        for (_i, filter) in filters.iter().enumerate() {
            let ret = filter.request_process(
                crate::core::model::DiscoverRequestInfo::Configuration(req.clone()),
            );
            match ret {
                Ok(rsp) => {
                    req = rsp.to_config_request();
                }
                Err(err) => {
                    tracing::error!(
                        "[polaris][config][connector] filter request_process fail: {}",
                        err.to_string()
                    );
                    return;
                }
            }
        }
        let _ = self.config_spec_sender.send(req);
    }
}

#[async_trait::async_trait]
impl Connector for GrpcConnector {
    async fn register_resource_handler(
        &self,
        handler: Box<dyn ResourceHandler>,
    ) -> Result<bool, PolarisError> {
        let watch_key = handler.interest_resource();
        let watch_key_str = watch_key.to_string();

        let mut handlers = self.watch_resources.write().await;
        if handlers.contains_key(watch_key_str.as_str()) {
            return Err(PolarisError::new(
                ServerError,
                format!(
                    "[polaris][discovery][connector] resource handler already exist: {}",
                    watch_key_str
                ),
            ));
        }

        handlers.insert(
            watch_key_str.clone(),
            ResourceHandlerWrapper {
                handler: handler,
                revision: String::new(),
            },
        );

        // 立即发送一个数据通知

        let discover_request = watch_key.to_discover_request("".to_string());
        let config_request = watch_key.to_config_request("".to_string());

        if discover_request.is_some() {
            self.send_naming_discover_request(discover_request.unwrap());
        } else {
            self.send_config_discover_request(config_request.unwrap());
        }

        tracing::info!(
            "[polaris][discovery][connector] register resource handler: {}",
            watch_key_str.clone()
        );
        Ok(true)
    }

    async fn register_instance(
        &self,
        req: InstanceRequest,
    ) -> Result<InstanceResponse, PolarisError> {
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
        tracing::debug!(
            "[polaris][discovery][connector] send heartbeat instance request={:?}",
            req.convert_beat_spec()
        );

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

    async fn create_config_file(&self, req: ConfigFileRequest) -> Result<bool, PolarisError> {
        tracing::debug!("[polaris][config][connector] send create config_file request={req:?}");

        let mut client = self.create_config_grpc_stub(req.flow_id.clone());
        let ret = client
            .create_config_file(tonic::Request::new(req.convert_spec()))
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
                    "[polaris][config][connector] send create config_file request to server receive fail: code={} info={}",
                    rsp.code.unwrap().clone(),
                    rsp.info.clone().unwrap(),
                );
                Err(PolarisError::new(ServerError, rsp.info.unwrap()))
            }
            Err(err) => {
                tracing::error!(
                    "[polaris][config][connector] send create config_file request to server fail: {}",
                    err
                );
                Err(PolarisError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn update_config_file(&self, req: ConfigFileRequest) -> Result<bool, PolarisError> {
        tracing::debug!("[polaris][config][connector] send update config_file request={req:?}");

        let mut client = self.create_config_grpc_stub(req.flow_id.clone());
        let ret = client
            .update_config_file(tonic::Request::new(req.convert_spec()))
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
                    "[polaris][config][connector] send update config_file request to server receive fail: code={} info={}",
                    rsp.code.unwrap().clone(),
                    rsp.info.clone().unwrap(),
                );
                Err(PolarisError::new(ServerError, rsp.info.unwrap()))
            }
            Err(err) => {
                tracing::error!(
                    "[polaris][config][connector] send update config_file request to server fail: {}",
                    err
                );
                Err(PolarisError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn release_config_file(&self, req: ConfigReleaseRequest) -> Result<bool, PolarisError> {
        tracing::debug!("[polaris][config][connector] send publish config_file request={req:?}");

        let mut client = self.create_config_grpc_stub(req.flow_id.clone());
        let ret = client
            .publish_config_file(tonic::Request::new(req.config_file))
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
                    "[polaris][config][connector] send publish config_file request to server receive fail: code={} info={}",
                    rsp.code.unwrap().clone(),
                    rsp.info.clone().unwrap(),
                );
                Err(PolarisError::new(ServerError, rsp.info.unwrap()))
            }
            Err(err) => {
                tracing::error!(
                    "[polaris][config][connector] send publish config_file request to server fail: {}",
                    err
                );
                Err(PolarisError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn upsert_publish_config_file(&self) -> Result<bool, PolarisError> {
        todo!()
    }
}

fn run_discover_spec_stream(
    channel: Channel,
    executor: Arc<Runtime>,
) -> Result<
    (
        UnboundedSender<DiscoverRequest>,
        UnboundedReceiver<DiscoverResponse>,
    ),
    PolarisError,
> {
    let (discover_sender, rx) = mpsc::unbounded_channel::<DiscoverRequest>();
    let (rsp_sender, rsp_recv) = mpsc::unbounded_channel::<DiscoverResponse>();
    _ = executor.spawn(async move {
        tracing::info!("[polaris][discovery][connector] start naming_discover grpc stream");
        let reciver = UnboundedReceiverStream::new(rx);

        let mut client = PolarisGrpcClient::new(channel);

        let discover_future = client.discover(tonic::Request::new(reciver));

        let discover_rt = discover_future.await;
        if discover_rt.is_err() {
            let stream_err = discover_rt.err().unwrap();
            tracing::error!(
                "[polaris][discovery][connector] naming_discover stream receive err: {}",
                stream_err.clone().to_string()
            );
            return Err(PolarisError::new(
                crate::core::model::error::ErrorCode::PluginError,
                stream_err.clone().to_string(),
            ));
        }

        let mut stream_recv = discover_rt.unwrap().into_inner();
        while let Some(received) = stream_recv.next().await {
            match received {
                Ok(rsp) => {
                    match rsp_sender.send(rsp) {
                        Ok(_) => {}
                        Err(err) => {
                            tracing::error!(
                                "[polaris][discovery][connector] send discover request receive fail: {}",
                                err.to_string()
                            );
                        }
                    }
                }
                Err(err) => {
                    tracing::error!(
                        "[polaris][discovery][connector] naming_discover stream receive err: {}",
                        err.to_string()
                    );
                }
            }
        }
        return Ok(());
    });

    Ok((discover_sender, rsp_recv))
}

fn run_config_spec_stream(
    channel: Channel,
    executor: Arc<Runtime>,
) -> Result<
    (
        UnboundedSender<ConfigDiscoverRequest>,
        UnboundedReceiver<ConfigDiscoverResponse>,
    ),
    PolarisError,
> {
    tracing::info!("[polaris][config][connector] start config_discover grpc stream");
    let (config_sender, config_reciver) = mpsc::unbounded_channel::<ConfigDiscoverRequest>();
    let (rsp_sender, rsp_recv) = mpsc::unbounded_channel::<ConfigDiscoverResponse>();
    _ = executor.spawn(async move {
        let reciver = UnboundedReceiverStream::new(config_reciver);
        let mut client = PolarisConfigGrpcClient::new(channel);

        let discover_future = client.discover(tonic::Request::new(reciver));

        let discover_rt = discover_future.await;
        if discover_rt.is_err() {
            let stream_err = discover_rt.err().unwrap();
            tracing::error!(
                "[polaris][config][connector] config_discover stream receive err: {}",
                stream_err.clone().to_string()
            );
            return Err(PolarisError::new(
                crate::core::model::error::ErrorCode::PluginError,
                stream_err.clone().to_string(),
            ));
        }

        let mut stream_recv: Streaming<ConfigDiscoverResponse> = discover_rt.unwrap().into_inner();
        while let Some(received) = stream_recv.next().await {
            match received {
                Ok(rsp) => match rsp_sender.send(rsp) {
                    Ok(_) => {}
                    Err(err) => {
                        tracing::error!(
                            "[polaris][config][connector] send config request receive fail: {}",
                            err.to_string()
                        );
                    }
                },
                Err(err) => {
                    tracing::error!(
                        "[polaris][config][connector] config_discover stream receive err: {}",
                        err.to_string()
                    );
                }
            }
        }
        return Ok(());
    });

    Ok((config_sender, rsp_recv))
}

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
                return Err(tonic::Status::internal("invalid metadata key"));
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
