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

use crate::core::config::config::{load_default, Configuration};
use crate::core::engine::Engine;
use crate::core::model::error::{ErrorCode, PolarisError};

pub struct SDKContext {
    pub conf: Arc<Configuration>,
    engine: Arc<Engine>,
}

impl Drop for SDKContext {
    fn drop(&mut self) {}
}

impl SDKContext {
    // default
    pub fn default() -> Result<SDKContext, PolarisError> {
        let cfg_opt = load_default();
        match cfg_opt {
            Ok(conf) => SDKContext::create_by_configuration(conf),
            Err(err) => Err(PolarisError::new(ErrorCode::InternalError, err.to_string())),
        }
    }

    // create_by_addresses
    pub fn create_by_addresses(addresses: Vec<String>) -> Result<SDKContext, PolarisError> {
        let cfg_opt = load_default();
        if cfg_opt.is_err() {
            return Err(PolarisError::new(
                ErrorCode::InternalError,
                cfg_opt.err().unwrap().to_string(),
            ));
        }
        let mut conf = cfg_opt.unwrap();

        conf.global.update_server_connector_address(addresses);

        SDKContext::create_by_configuration(conf)
    }

    // create_by_configuration
    pub fn create_by_configuration(cfg: Configuration) -> Result<SDKContext, PolarisError> {
        let start_time = std::time::Instant::now();
        let cfg = Arc::new(cfg);
        let ret = Engine::new(cfg.clone());
        tracing::info!("create engine cost: {:?}", start_time.elapsed());
        if ret.is_err() {
            return Err(ret.err().unwrap());
        }
        Ok(Self {
            conf: cfg,
            engine: Arc::new(ret.ok().unwrap()),
        })
    }

    pub fn get_engine(&self) -> Arc<Engine> {
        self.engine.clone()
    }
}

mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
