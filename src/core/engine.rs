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

use crate::core::config::config::Configuration;
use crate::core::config::global::DISCOVER_SERVER_CONNECTOR;
use crate::core::model::error::PolarisError;
use crate::core::model::naming::InstanceRequest;
use crate::core::plugin::connector::Connector;
use crate::core::plugin::plugins::Extensions;
use crate::discovery::req::{InstanceDeregisterRequest, InstanceRegisterRequest, InstanceRegisterResponse};

pub struct Engine {
    extensions: Extensions
}

impl Engine {

    pub fn new() -> Self {
        Self {
            extensions: Extensions::default(),
        }
    }

    pub(crate) fn init(&mut self, conf: &Configuration) -> Result<bool, PolarisError> {
        let ret = self.extensions.init(conf);
        if ret.is_err() {
            return ret;
        }
        return Ok(true);
    }

    pub(crate) fn sync_register_instance(&mut self, req: InstanceRegisterRequest) -> Result<InstanceRegisterResponse, PolarisError> {
        let mut connector = self.extensions.get_active_connector(DISCOVER_SERVER_CONNECTOR);
        let rsp = connector.register_instance(InstanceRequest{
            flow_id: req.flow_id.clone(),
            ttl: req.ttl.clone(),
            instance: req.convert_instance(),
        });

        return match rsp {
            Ok(ins_rsp) => {
                Ok(InstanceRegisterResponse{ instance_id: ins_rsp.instance.ip.clone(), exist: ins_rsp.exist.clone() })
            }
            Err(err) => {
                Err(err)
            }
        }
    }

    pub(crate) fn sync_deregister_instance(&mut self, req: InstanceDeregisterRequest) -> Result<(), PolarisError> {
        let mut connector = self.extensions.get_active_connector(DISCOVER_SERVER_CONNECTOR);
        let rsp = connector.deregister_instance(InstanceRequest{
            flow_id: req.flow_id.clone(),
            ttl: 0,
            instance: req.convert_instance(),
        });

        return match rsp {
            Ok(ins_rsp) => {
                Ok(())
            }
            Err(err) => {
                Err(err)
            }
        }
    }

}
