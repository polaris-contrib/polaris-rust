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

use std::sync::Arc;

use tokio::runtime::{Builder, Runtime};

use crate::core::config::config::Configuration;
use crate::core::config::global::DISCOVER_SERVER_CONNECTOR;
use crate::core::model::error::PolarisError;
use crate::core::model::naming::InstanceRequest;
use crate::core::plugin::plugins::Extensions;
use crate::discovery::req::{
    InstanceDeregisterRequest, InstanceHeartbeatRequest, InstanceRegisterRequest,
    InstanceRegisterResponse,
};

use super::model::pb::lib::InstanceHeartbeat;

pub struct Engine {
    pub runtime: Arc<Runtime>,
    extensions: Extensions,
}

impl Engine {
    pub fn new(conf: Configuration) -> Result<Self, PolarisError> {
        let runtime = Arc::new(
            Builder::new_multi_thread()
                .enable_all()
                .thread_name("polaris-client-thread-pool")
                .worker_threads(4)
                .build()
                .unwrap(),
        );

        let ret = Extensions::build(conf, runtime.clone());
        match ret {
            Ok(extensions) => Ok(Self {
                extensions: extensions,
                runtime: runtime,
            }),
            Err(err) => Err(err),
        }
    }

    /// sync_register_instance 同步注册实例
    pub async fn sync_register_instance(
        &self,
        req: InstanceRegisterRequest,
    ) -> Result<InstanceRegisterResponse, PolarisError> {
        let connector = self
            .extensions
            .get_active_connector(DISCOVER_SERVER_CONNECTOR);
        let rsp = connector
            .register_instance(InstanceRequest {
                flow_id: req.flow_id.clone(),
                ttl: req.ttl.clone(),
                instance: req.convert_instance(),
            })
            .await;

        return match rsp {
            Ok(ins_rsp) => Ok(InstanceRegisterResponse {
                instance_id: ins_rsp.instance.id.clone(),
                exist: ins_rsp.exist.clone(),
            }),
            Err(err) => Err(err),
        };
    }

    /// sync_deregister_instance 同步注销实例
    pub async fn sync_deregister_instance(
        &self,
        req: InstanceDeregisterRequest,
    ) -> Result<(), PolarisError> {
        let connector = self
            .extensions
            .get_active_connector(DISCOVER_SERVER_CONNECTOR);
        let rsp = connector
            .deregister_instance(InstanceRequest {
                flow_id: req.flow_id.clone(),
                ttl: 0,
                instance: req.convert_instance(),
            })
            .await;

        return match rsp {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        };
    }

    /// sync_instance_heartbeat 同步实例心跳
    pub async fn sync_instance_heartbeat(
        &self,
        req: InstanceHeartbeatRequest,
    ) -> Result<(), PolarisError> {
        let connector = self
            .extensions
            .get_active_connector(DISCOVER_SERVER_CONNECTOR);
        let rsp = connector
            .heartbeat_instance(InstanceRequest {
                flow_id: req.flow_id.clone(),
                ttl: 0,
                instance: req.convert_instance(),
            })
            .await;

        return match rsp {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        };
    }

    pub fn get_executor(&self) -> Arc<Runtime> {
        self.runtime.clone()
    }
}
