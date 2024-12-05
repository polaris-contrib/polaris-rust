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

use polaris_specification::v1::{
    config_discover_response::ConfigDiscoverResponseType, discover_response::DiscoverResponseType,
    ConfigDiscoverResponse, DiscoverResponse,
};
use prost::Message;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::core::{
    config::global::LocalCacheConfig,
    model::{
        cache::EventType,
        error::{ErrorCode, PolarisError},
    },
    plugin::cache::{Filter, ResourceCacheFailover},
};

pub struct DiskCacheFailover {
    conf: LocalCacheConfig,
}

impl DiskCacheFailover {
    pub fn new(conf: LocalCacheConfig) -> Self {
        Self { conf }
    }
}

#[async_trait::async_trait]
impl ResourceCacheFailover for DiskCacheFailover {
    // failover_naming_load 兜底加载
    async fn failover_naming_load(&self, filter: Filter) -> Result<DiscoverResponse, PolarisError> {
        let mut persist_file = self.conf.persist_dir.clone();
        let event_type = filter.get_event_type();
        let resource_key = filter.resource_key;
        match event_type {
            EventType::Service => {
                persist_file = format!(
                    "{}/svc#{}#services.data",
                    persist_file,
                    resource_key.namespace.clone(),
                );
            }
            EventType::Instance
            | EventType::RouterRule
            | EventType::LaneRule
            | EventType::CircuitBreakerRule
            | EventType::FaultDetectRule
            | EventType::RateLimitRule => {
                persist_file = format!(
                    "{}/svc#{}#{}#{}",
                    persist_file,
                    resource_key.namespace.clone(),
                    resource_key.filter["service"].clone(),
                    event_type.to_persist_file(),
                );
            }
            _ => {
                return Err(PolarisError::new(
                    ErrorCode::InternalError,
                    format!("unsupported event type"),
                ));
            }
        }

        let ret = File::open(persist_file.clone()).await;
        match ret {
            Ok(mut file) => {
                let mut buf = Vec::new();
                let ret = file.read_to_end(&mut buf).await;
                if let Err(e) = ret {
                    return Err(PolarisError::new(
                        ErrorCode::InternalError,
                        format!("read file:{:?} error: {:?}", persist_file, e),
                    ));
                }
                let ret = DiscoverResponse::decode(buf.as_slice());
                match ret {
                    Ok(value) => Ok(value),
                    Err(e) => Err(PolarisError::new(
                        ErrorCode::InternalError,
                        format!("decode file:{:?} error: {:?}", persist_file, e),
                    )),
                }
            }
            Err(e) => Err(PolarisError::new(
                ErrorCode::InternalError,
                format!("open file:{:?} error: {:?}", persist_file, e),
            )),
        }
    }

    // save_failover 保存容灾数据
    async fn save_naming_failover(&self, value: DiscoverResponse) -> Result<(), PolarisError> {
        let mut buf = Vec::new();
        let mut persist_file = self.conf.persist_dir.clone();
        let svc = value.service.clone().unwrap();

        match value.r#type().clone() {
            polaris_specification::v1::discover_response::DiscoverResponseType::Services => {
                persist_file = format!(
                    "{}/svc#{}#services.data",
                    persist_file,
                    svc.namespace.clone().unwrap()
                );
            }
            DiscoverResponseType::Instance
            | DiscoverResponseType::Routing
            | DiscoverResponseType::RateLimit
            | DiscoverResponseType::CircuitBreaker
            | DiscoverResponseType::FaultDetector
            | DiscoverResponseType::Lane => {
                persist_file = format!(
                    "{}/svc#{}#{}#{}",
                    persist_file,
                    svc.namespace.clone().unwrap(),
                    svc.name.clone().unwrap(),
                    EventType::naming_spec_to_persist_file(value.r#type().clone())
                );
            }
            _ => {
                return Err(PolarisError::new(
                    ErrorCode::InternalError,
                    format!("unsupported discover response type"),
                ));
            }
        }

        let ret = DiscoverResponse::encode(&value, &mut buf);
        if let Err(e) = ret {
            return Err(PolarisError::new(
                ErrorCode::InternalError,
                format!("encode discover response error: {:?}", e),
            ));
        }

        match File::create(persist_file).await {
            Ok(mut file) => {
                let ret = file.write_all(buf.as_slice()).await;
                if let Err(e) = ret {
                    return Err(PolarisError::new(
                        ErrorCode::InternalError,
                        format!("write file error: {:?}", e),
                    ));
                }
                Ok(())
            }
            Err(e) => Err(PolarisError::new(
                ErrorCode::InternalError,
                format!("create file error: {:?}", e),
            )),
        }
    }

    // failover_config_load 兜底加载
    async fn failover_config_load(
        &self,
        filter: Filter,
    ) -> Result<ConfigDiscoverResponse, PolarisError> {
        let mut persist_file = self.conf.persist_dir.clone();
        let event_type = filter.get_event_type();
        let resource_key = filter.resource_key;
        match event_type {
            EventType::ConfigFile => {
                persist_file = format!(
                    "{}/config#{}#{}#{}#config_file.data",
                    persist_file,
                    resource_key.namespace.clone(),
                    resource_key.filter["group"].clone(),
                    resource_key.filter["file"].clone(),
                );
            }
            EventType::ConfigGroup => {
                persist_file = format!(
                    "{}/config#{}#{}#config_group.data",
                    persist_file,
                    resource_key.namespace.clone(),
                    resource_key.filter["group"].clone(),
                );
            }
            _ => {
                return Err(PolarisError::new(
                    ErrorCode::InternalError,
                    format!("unsupported event type"),
                ));
            }
        }

        let ret = File::open(persist_file.clone()).await;
        match ret {
            Ok(mut file) => {
                let mut buf = Vec::new();
                let ret = file.read_to_end(&mut buf).await;
                if let Err(e) = ret {
                    return Err(PolarisError::new(
                        ErrorCode::InternalError,
                        format!("read file:{:?} error: {:?}", persist_file, e),
                    ));
                }
                let ret = ConfigDiscoverResponse::decode(buf.as_slice());
                match ret {
                    Ok(value) => Ok(value),
                    Err(e) => Err(PolarisError::new(
                        ErrorCode::InternalError,
                        format!("decode file:{:?} error: {:?}", persist_file, e),
                    )),
                }
            }
            Err(e) => Err(PolarisError::new(
                ErrorCode::InternalError,
                format!("open file:{:?} error: {:?}", persist_file, e),
            )),
        }
    }

    // save_config_failover 保存容灾数据
    async fn save_config_failover(
        &self,
        value: ConfigDiscoverResponse,
    ) -> Result<(), PolarisError> {
        let mut buf = Vec::new();
        let mut persist_file = self.conf.persist_dir.clone();
        let conf = value.config_file.clone().unwrap();

        match value.r#type().clone() {
            ConfigDiscoverResponseType::ConfigFile => {
                persist_file = format!(
                    "{}/config#{}#{}#{}#config_file.data",
                    persist_file,
                    conf.namespace.clone().unwrap(),
                    conf.group.clone().unwrap(),
                    conf.file_name.clone().unwrap(),
                );
            }
            ConfigDiscoverResponseType::ConfigFileNames => {
                persist_file = format!(
                    "{}/config#{}#{}#config_group.data",
                    persist_file,
                    conf.namespace.clone().unwrap(),
                    conf.group.clone().unwrap(),
                );
            }
            _ => {
                return Err(PolarisError::new(
                    ErrorCode::InternalError,
                    format!("unsupported config response type"),
                ));
            }
        }

        let ret = ConfigDiscoverResponse::encode(&value, &mut buf);
        if let Err(e) = ret {
            return Err(PolarisError::new(
                ErrorCode::InternalError,
                format!("encode config response error: {:?}", e),
            ));
        }

        match File::create(persist_file).await {
            Ok(mut file) => {
                let ret = file.write_all(buf.as_slice()).await;
                if let Err(e) = ret {
                    return Err(PolarisError::new(
                        ErrorCode::InternalError,
                        format!("write file error: {:?}", e),
                    ));
                }
                Ok(())
            }
            Err(e) => Err(PolarisError::new(
                ErrorCode::InternalError,
                format!("create file error: {:?}", e),
            )),
        }
    }
}
