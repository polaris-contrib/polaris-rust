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

use crate::core::config::global::ServerConnectorConfig;
use crate::core::model::cache::{EventType, RemoteData};
use crate::core::model::config::{ConfigFileRequest, ConfigReleaseRequest};
use crate::core::model::error::ErrorCode::{ServerError, ServerUserError};
use crate::core::model::error::PoleError;
use crate::core::model::naming::{
    InstanceRequest, InstanceResponse, ServiceContract, ServiceContractRequest,
};
use crate::core::model::ReportClientRequest;
use crate::core::plugin::connector::{Connector, InitConnectorOption, ResourceHandler};
use crate::core::plugin::plugins::Plugin;
use crate::{debug, error, info};
use pole_specification::v1::config_grpc_client::ConfigGrpcClient;
use pole_specification::v1::discover_grpc_client::DiscoverGrpcClient;
use pole_specification::v1::Code::{ExecuteSuccess, ExistedResource};
use pole_specification::v1::{
    self, Code, ConfigDiscoverRequest, ConfigDiscoverResponse, DiscoverRequest, DiscoverResponse,
    HeartbeatsRequest, HeartbeatsResponse, InstanceHeartbeat,
};
use prost::Message;
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;
use tonic::service::interceptor::InterceptedService;
use tonic::service::Interceptor;
use tonic::transport::{Channel, Endpoint};
use tonic::Streaming;
use tracing::Instrument;

struct ResourceHandlerWrapper {
    handler: Box<dyn ResourceHandler>,
    revision: String,
}

static PLUGIN_NAME: &str = "grpc";

#[derive(Clone)]
pub struct GrpcConnector {
    opt: InitConnectorOption,
    discover_channel: Channel,
    config_channel: Channel,

    beat_spec_sender: Arc<UnboundedSender<HeartbeatsRequest>>,
    discover_spec_sender: Arc<UnboundedSender<DiscoverRequest>>,
    config_spec_sender: Arc<UnboundedSender<ConfigDiscoverRequest>>,

    watch_resources: Arc<RwLock<HashMap<String, ResourceHandlerWrapper>>>,
}

fn new_connector(opt: InitConnectorOption) -> Box<dyn Connector> {
    let conf = &opt.conf.global.server_connectors.clone();
    let (discover_channel, config_channel) = create_channel(conf);

    let (discover_sender, mut discover_reciver) =
        run_discover_spec_stream(discover_channel.clone(), opt.runtime.clone()).unwrap();

    let beat_sender =
        run_heartbeat_spec_stream(discover_channel.clone(), opt.runtime.clone()).unwrap();

    let (config_sender, mut config_reciver) =
        run_config_spec_stream(config_channel.clone(), opt.runtime.clone()).unwrap();

    let c = GrpcConnector {
        opt,
        discover_channel: discover_channel.clone(),
        config_channel: config_channel.clone(),

        beat_spec_sender: Arc::new(beat_sender),
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
                        receive_c.receive_config_response(config_ret.unwrap()).await;
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
                    debug!(
                        "[pole][discovery][connector] send discover request: {:?} filter: {:?}",
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
            tokio::time::sleep(Duration::from_secs(2)).await;
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
            discover_address.push(format!("http://{}", ele.trim_start_matches("discover://")));
        } else if ele.starts_with("config://") {
            config_address.push(format!("http://{}", ele.trim_start_matches("config://")));
        }
    }

    info!(
        "[pole][server_connector] discover_address: {:?} config_address: {:?}",
        discover_address, config_address
    );

    let connect_timeout = conf.connect_timeout;

    let discover_endpoints = discover_address.iter().map(|item| {
        Endpoint::from_shared(item.to_string())
            .unwrap()
            .connect_timeout(connect_timeout)
    });
    let config_endpoints = config_address.iter().map(|item| {
        Endpoint::from_shared(item.to_string())
            .unwrap()
            .connect_timeout(connect_timeout)
    });

    let discover_channel = Channel::balance_list(discover_endpoints);
    let config_channel = Channel::balance_list(config_endpoints);

    (discover_channel, config_channel)
}

impl Plugin for GrpcConnector {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        PLUGIN_NAME.to_string()
    }
}

impl GrpcConnector {
    pub fn builder() -> (fn(opt: InitConnectorOption) -> Box<dyn Connector>, String) {
        (new_connector, PLUGIN_NAME.to_string())
    }

    fn create_discover_grpc_stub(
        &self,
        flow: String,
    ) -> DiscoverGrpcClient<InterceptedService<Channel, impl Interceptor>> {
        let interceptor = make_interceptor(flow);
        DiscoverGrpcClient::with_interceptor(self.discover_channel.clone(), interceptor)
    }

    fn create_config_grpc_stub(
        &self,
        flow: String,
    ) -> ConfigGrpcClient<InterceptedService<Channel, impl Interceptor>> {
        let interceptor = make_interceptor(flow);
        ConfigGrpcClient::with_interceptor(self.config_channel.clone(), interceptor)
    }

    async fn receive_discover_response(&self, resp: DiscoverResponse) {
        let remote_rsp = resp.clone();
        if remote_rsp.code == Code::DataNoChange as u32 {
            debug!(
                "[pole][discovery][connector] receive naming_discover no_change response: {:?}",
                resp
            );
            return;
        }

        if remote_rsp.code != Code::ExecuteSuccess as u32 {
            error!(
                "[pole][discovery][connector] receive naming_discover failure response: {:?}",
                resp
            );
            return;
        }

        let mut watch_key = "".to_string();
        match resp.r#type() {
            pole_specification::v1::discover_response::DiscoverResponseType::Services => {
                watch_key = resp.service.unwrap().namespace.clone();
            }
            pole_specification::v1::discover_response::DiscoverResponseType::Instance => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!(
                    "{:?}#{}#{}",
                    EventType::Instance,
                    svc.namespace.clone(),
                    svc.name.clone()
                );
            }
            pole_specification::v1::discover_response::DiscoverResponseType::CustomRouteRule => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!(
                    "{:?}#{}#{}",
                    EventType::RouterRule,
                    svc.namespace.clone(),
                    svc.name.clone()
                );
            }
            pole_specification::v1::discover_response::DiscoverResponseType::NearbyRouteRule => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!(
                    "{:?}#{}#{}",
                    EventType::RouterRule,
                    svc.namespace.clone(),
                    svc.name.clone()
                );
            }
            pole_specification::v1::discover_response::DiscoverResponseType::RateLimit => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!(
                    "{:?}#{}#{}",
                    EventType::RateLimitRule,
                    svc.namespace.clone(),
                    svc.name.clone()
                );
            }
            pole_specification::v1::discover_response::DiscoverResponseType::CircuitBreaker => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!(
                    "{:?}#{}#{}",
                    EventType::CircuitBreakerRule,
                    svc.namespace.clone(),
                    svc.name.clone()
                );
            }
            pole_specification::v1::discover_response::DiscoverResponseType::FaultDetector => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!(
                    "{:?}#{}#{}",
                    EventType::FaultDetectRule,
                    svc.namespace.clone(),
                    svc.name.clone()
                );
            }
            pole_specification::v1::discover_response::DiscoverResponseType::Lane => {
                let svc = resp.service.unwrap().clone();
                watch_key = format!(
                    "{:?}#{}#{}",
                    EventType::LaneRule,
                    svc.namespace.clone(),
                    svc.name.clone()
                );
            }
            _ => {}
        }
        let mut handlers = self.watch_resources.write().await;
        if let Some(handle) = handlers.get_mut(watch_key.as_str()) {
            handle.revision = remote_rsp.service.clone().unwrap().revision.clone();
            handle.handler.handle_event(RemoteData {
                event_key: handle.handler.interest_resource(),
                discover_value: Some(remote_rsp),
                config_value: None,
            });
        }
    }

    async fn receive_config_response(&self, mut resp: ConfigDiscoverResponse) {
        info!(
            "[pole][config][connector] receive config_discover response: {:?}",
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
                    error!(
                        "[pole][config][connector] filter response_process fail: {}",
                        err.to_string()
                    );
                    return;
                }
            }
        }

        let remote_rsp = resp.clone();

        let mut watch_key = "".to_string();
        match resp.r#type() {
            pole_specification::v1::config_discover_response::ConfigDiscoverResponseType::ConfigFile => {
                let ret = resp.file.unwrap().clone();
                watch_key = format!("{:?}#{}#{}#{}", EventType::ConfigFile, ret.namespace.clone(), 
                ret.group.clone(),  ret.file_name.clone());
            },
            pole_specification::v1::config_discover_response::ConfigDiscoverResponseType::ConfigFileNames => {
                let ret = resp.file.unwrap().clone();
                watch_key = format!("{:?}#{}#{}", EventType::ConfigGroup, ret.namespace.clone(), 
                ret.group.clone());
            },
            pole_specification::v1::config_discover_response::ConfigDiscoverResponseType::ConfigFileGroups => {
                error!("[pole][config][connector] not support ConfigFileGroups");
            },
            _ => {}
        }

        let mut handlers = self.watch_resources.write().await;
        if let Some(handle) = handlers.get_mut(watch_key.as_str()) {
            handle.revision = remote_rsp.revision.clone();
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
                    error!(
                        "[pole][config][connector] filter request_process fail: {}",
                        err.to_string()
                    );
                    return;
                }
            }
        }

        // 添加客户端标签，用于配置灰度发布读取
        let mut file = req.file.unwrap();
        let mut client_tags = HashMap::<String, String>::new();
        let client_labels = self.opt.client_ctx.labels.clone();
        for (k, v) in client_labels.iter() {
            client_tags.insert(k.clone(), v.clone());
        }
        file.labels = client_tags;
        req.file = Some(file);
        let _ = self.config_spec_sender.send(req);
    }
}

#[async_trait::async_trait]
impl Connector for GrpcConnector {
    async fn register_resource_handler(
        &self,
        handler: Box<dyn ResourceHandler>,
    ) -> Result<bool, PoleError> {
        let watch_key = handler.interest_resource();
        let watch_key_str = watch_key.to_string();

        let mut handlers = self.watch_resources.write().await;
        if handlers.contains_key(watch_key_str.as_str()) {
            return Err(PoleError::new(
                ServerError,
                format!(
                    "[pole][discovery][connector] resource handler already exist: {}",
                    watch_key_str
                ),
            ));
        }

        handlers.insert(
            watch_key_str.clone(),
            ResourceHandlerWrapper {
                handler,
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

        info!(
            "[pole][discovery][connector] register resource handler: {}",
            watch_key_str.clone()
        );
        Ok(true)
    }

    async fn register_instance(&self, req: InstanceRequest) -> Result<InstanceResponse, PoleError> {
        debug!("[pole][discovery][connector] send register instance request={req:?}");

        let mut client = self.create_discover_grpc_stub(req.flow_id.clone());
        let ret = client
            .register_instance(tonic::Request::new(req.convert_spec()))
            .in_current_span()
            .await;
        return match ret {
            Ok(rsp) => {
                let rsp = rsp.into_inner();
                if rsp.data.is_none() {
                    return Err(PoleError::new(
                        ServerUserError,
                        "[pole][discovery][connector] invalid register response: missing instance"
                            .to_string(),
                    ));
                }
                let recv_code: Code = unsafe { std::mem::transmute(rsp.code) };
                if ExecuteSuccess.eq(&recv_code) {
                    let rsp_ins: Result<v1::Instance, prost::DecodeError> =
                        v1::Instance::decode(rsp.data.unwrap().value.as_slice());
                    let ins_id = rsp_ins.unwrap().id;
                    info!(
                        "[pole][discovery][connector] register instance to server success id={}",
                        ins_id.clone(),
                    );
                    let mut ins = InstanceResponse::default();
                    ins.instance.id.clone_from(&ins_id);
                    return Ok(ins);
                }
                if ExistedResource.eq(&recv_code) {
                    return Ok(InstanceResponse::exist_resource());
                }
                Err(PoleError::new(ServerError, rsp.info))
            }
            Err(err) => {
                error!(
                    "[pole][discovery][connector] send register request to server fail: {}",
                    err
                );
                Err(PoleError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn deregister_instance(&self, req: InstanceRequest) -> Result<bool, PoleError> {
        debug!("[pole][discovery][connector] send deregister instance request={req:?}");

        let mut client = self.create_discover_grpc_stub(req.flow_id.clone());
        let ret = client
            .deregister_instance(tonic::Request::new(req.convert_spec()))
            .in_current_span()
            .await;
        return match ret {
            Ok(rsp) => {
                let rsp = rsp.into_inner();
                let recv_code: Code = unsafe { std::mem::transmute(rsp.code) };
                if ExecuteSuccess.eq(&recv_code) {
                    return Ok(true);
                }
                error!(
                    "[pole][discovery][connector] send deregister request to server receive fail: code={} info={}",
                    rsp.code.clone(),
                    rsp.info.clone(),
                );
                Err(PoleError::new(ServerError, rsp.info))
            }
            Err(err) => {
                error!(
                    "[pole][discovery][connector] send deregister request to server fail: {}",
                    err
                );
                Err(PoleError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn heartbeat_instance(&self, req: InstanceRequest) -> Result<bool, PoleError> {
        debug!("[pole][discovery][connector] send heartbeat instance request={req:?}");

        let spec_req = req.convert_beat_spec();

        let ret = self.beat_spec_sender.send(HeartbeatsRequest {
            heartbeats: vec![spec_req],
        });
        return match ret {
            Ok(_) => {
                return Ok(true);
            }
            Err(err) => {
                error!(
                    "[pole][discovery][connector] send heartbeat request to server fail: {}",
                    err
                );
                Err(PoleError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn report_client(&self, req: ReportClientRequest) -> Result<bool, PoleError> {
        todo!()
    }

    async fn report_service_contract(
        &self,
        req: ServiceContractRequest,
    ) -> Result<bool, PoleError> {
        debug!("[pole][discovery][connector] send report service_contract request={req:?}");

        let mut client = self.create_discover_grpc_stub(req.flow_id.clone());
        let ret = client
            .report_service_contract(tonic::Request::new(req.contract.convert_spec()))
            .in_current_span()
            .await;
        return match ret {
            Ok(rsp) => {
                let rsp = rsp.into_inner();
                let recv_code: Code = unsafe { std::mem::transmute(rsp.code) };
                if ExecuteSuccess.eq(&recv_code) {
                    return Ok(true);
                }
                error!(
                    "[pole][discovery][connector] send report service_contract request to server receive fail: code={} info={}",
                    rsp.code.clone(),
                    rsp.info.clone(),
                );
                Err(PoleError::new(ServerError, rsp.info))
            }
            Err(err) => {
                error!(
                    "[pole][discovery][connector] send report service_contract request to server fail: {}",
                    err
                );
                Err(PoleError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn get_service_contract(
        &self,
        req: ServiceContractRequest,
    ) -> Result<ServiceContract, PoleError> {
        todo!()
    }

    async fn create_config_file(&self, req: ConfigFileRequest) -> Result<bool, PoleError> {
        debug!("[pole][config][connector] send create config_file request={req:?}");

        let mut client = self.create_config_grpc_stub(req.flow_id.clone());
        let ret = client
            .create_config_file(tonic::Request::new(req.convert_spec()))
            .in_current_span()
            .await;
        return match ret {
            Ok(rsp) => {
                let rsp = rsp.into_inner();
                let recv_code: Code = unsafe { std::mem::transmute(rsp.code) };
                if ExecuteSuccess.eq(&recv_code) {
                    return Ok(true);
                }
                error!(
                    "[pole][config][connector] send create config_file request to server receive fail: code={} info={}",
                    rsp.code.clone(),
                    rsp.info.clone(),
                );
                Err(PoleError::new(ServerError, rsp.info))
            }
            Err(err) => {
                error!(
                    "[pole][config][connector] send create config_file request to server fail: {}",
                    err
                );
                Err(PoleError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn update_config_file(&self, req: ConfigFileRequest) -> Result<bool, PoleError> {
        debug!("[pole][config][connector] send update config_file request={req:?}");

        let mut client = self.create_config_grpc_stub(req.flow_id.clone());
        let ret = client
            .update_config_file(tonic::Request::new(req.convert_spec()))
            .in_current_span()
            .await;
        return match ret {
            Ok(rsp) => {
                let rsp = rsp.into_inner();
                let recv_code: Code = unsafe { std::mem::transmute(rsp.code) };
                if ExecuteSuccess.eq(&recv_code) {
                    return Ok(true);
                }
                error!(
                    "[pole][config][connector] send update config_file request to server receive fail: code={} info={}",
                    rsp.code.clone(),
                    rsp.info.clone(),
                );
                Err(PoleError::new(ServerError, rsp.info))
            }
            Err(err) => {
                error!(
                    "[pole][config][connector] send update config_file request to server fail: {}",
                    err
                );
                Err(PoleError::new(ServerError, err.to_string()))
            }
        };
    }

    async fn release_config_file(&self, req: ConfigReleaseRequest) -> Result<bool, PoleError> {
        debug!("[pole][config][connector] send publish config_file request={req:?}");

        let mut client = self.create_config_grpc_stub(req.flow_id.clone());
        let ret = client
            .publish_config_file(tonic::Request::new(req.convert_spec()))
            .in_current_span()
            .await;
        return match ret {
            Ok(rsp) => {
                let rsp = rsp.into_inner();
                let recv_code: Code = unsafe { std::mem::transmute(rsp.code) };
                if ExecuteSuccess.eq(&recv_code) {
                    return Ok(true);
                }
                error!(
                    "[pole][config][connector] send publish config_file request to server receive fail: code={} info={}",
                    rsp.code.clone(),
                    rsp.info.clone(),
                );
                Err(PoleError::new(ServerError, rsp.info))
            }
            Err(err) => {
                error!(
                    "[pole][config][connector] send publish config_file request to server fail: {}",
                    err
                );
                Err(PoleError::new(ServerError, err.to_string()))
            }
        };
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
    PoleError,
> {
    let (discover_sender, rx) = mpsc::unbounded_channel::<DiscoverRequest>();
    let (rsp_sender, rsp_recv) = mpsc::unbounded_channel::<DiscoverResponse>();
    _ = executor.spawn(async move {
        info!("[pole][discovery][connector] start naming_discover grpc stream");
        let reciver = UnboundedReceiverStream::new(rx);

        let mut client = DiscoverGrpcClient::new(channel);

        let discover_future = client.discover(tonic::Request::new(reciver));

        let discover_rt = discover_future.await;
        if discover_rt.is_err() {
            let stream_err = discover_rt.err().unwrap();
            error!(
                "[pole][discovery][connector] naming_discover stream receive err: {}",
                stream_err.clone().to_string()
            );
            return Err(PoleError::new(
                crate::core::model::error::ErrorCode::PluginError,
                stream_err.clone().to_string(),
            ));
        }

        let mut stream_recv = discover_rt.unwrap().into_inner();
        while let Some(received) = stream_recv.next().await {
            match received {
                Ok(rsp) => match rsp_sender.send(rsp) {
                    Ok(_) => {}
                    Err(err) => {
                        error!(
                            "[pole][discovery][connector] send discover request receive fail: {}",
                            err.to_string()
                        );
                    }
                },
                Err(err) => {
                    error!(
                        "[pole][discovery][connector] naming_discover stream receive err: {}",
                        err.to_string()
                    );
                }
            }
        }
        Ok(())
    });

    Ok((discover_sender, rsp_recv))
}

fn run_heartbeat_spec_stream(
    channel: Channel,
    executor: Arc<Runtime>,
) -> Result<UnboundedSender<HeartbeatsRequest>, PoleError> {
    let (beat_sender, rx) = mpsc::unbounded_channel::<HeartbeatsRequest>();
    _ = executor.spawn(async move {
        info!("[pole][discovery][connector] start naming_heartbeat grpc stream");
        let reciver = UnboundedReceiverStream::new(rx);

        let mut client = DiscoverGrpcClient::new(channel);

        let beat_future = client.heartbeat(tonic::Request::new(reciver));

        let beat_rt = beat_future.await;
        if beat_rt.is_err() {
            let stream_err = beat_rt.err().unwrap();
            error!(
                "[pole][discovery][connector] naming_heartbeat stream receive err: {}",
                stream_err.clone().to_string()
            );
            return Err(PoleError::new(
                crate::core::model::error::ErrorCode::PluginError,
                stream_err.clone().to_string(),
            ));
        }

        let mut stream_recv = beat_rt.unwrap().into_inner();
        while let Some(received) = stream_recv.next().await {
            match received {
                Ok(rsp) => {
                    // 心跳响应暂时不处理
                }
                Err(err) => {
                    error!(
                        "[pole][discovery][connector] naming_heartbeat stream receive err: {}",
                        err.to_string()
                    );
                }
            }
        }
        Ok(())
    });

    Ok(beat_sender)
}

fn run_config_spec_stream(
    channel: Channel,
    executor: Arc<Runtime>,
) -> Result<
    (
        UnboundedSender<ConfigDiscoverRequest>,
        UnboundedReceiver<ConfigDiscoverResponse>,
    ),
    PoleError,
> {
    info!("[pole][config][connector] start config_discover grpc stream");
    let (config_sender, config_reciver) = mpsc::unbounded_channel::<ConfigDiscoverRequest>();
    let (rsp_sender, rsp_recv) = mpsc::unbounded_channel::<ConfigDiscoverResponse>();
    _ = executor.spawn(async move {
        let reciver = UnboundedReceiverStream::new(config_reciver);
        let mut client = ConfigGrpcClient::new(channel);

        let discover_future = client.discover(tonic::Request::new(reciver));

        let discover_rt = discover_future.await;
        if discover_rt.is_err() {
            let stream_err = discover_rt.err().unwrap();
            error!(
                "[pole][config][connector] config_discover stream receive err: {}",
                stream_err.clone().to_string()
            );
            return Err(PoleError::new(
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
                        error!(
                            "[pole][config][connector] send config request receive fail: {}",
                            err.to_string()
                        );
                    }
                },
                Err(err) => {
                    error!(
                        "[pole][config][connector] config_discover stream receive err: {}",
                        err.to_string()
                    );
                }
            }
        }
        Ok(())
    });

    Ok((config_sender, rsp_recv))
}

fn make_interceptor(flow: String) -> impl Interceptor {
    move |mut req: tonic::Request<()>| {
        req.metadata_mut().insert(
            "request-id",
            flow.parse()
                .map_err(|_| tonic::Status::invalid_argument("Invalid request-id"))?,
        );
        Ok(req)
    }
}
