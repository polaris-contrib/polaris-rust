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

pub mod circuitbreaker;
pub mod config;
pub mod core;
pub mod discovery;
pub mod plugins;
pub mod ratelimit;
pub mod router;

mod tests {
    use std::{collections::HashMap, time::Duration};

    use crate::{
        core::model::naming::Location,
        discovery::{
            api::{new_provider_api, ProviderAPI},
            req::InstanceRegisterRequest,
        },
    };

    #[test]
    fn test_create_provider() {
        let provider_ret = new_provider_api();
        match provider_ret {
            Err(err) => {
                log::error!("create provider fail: {}", err.to_string());
            }
            Ok(mut provier) => {
                let req = InstanceRegisterRequest {
                    flow_id: "1".to_string(),
                    timeout: Duration::from_secs(1),
                    namespace: "rust-demo".to_string(),
                    service: "polaris-rust-provider".to_string(),
                    ip: "1.1.1.1".to_string(),
                    port: 8080,
                    vpc_id: "1".to_string(),
                    version: "1".to_string(),
                    protocol: "1".to_string(),
                    health: true,
                    isolated: false,
                    weight: 100,
                    priority: 0,
                    metadata: HashMap::new(),
                    location: Location {
                        region: "1".to_string(),
                        zone: "1".to_string(),
                        campus: "1".to_string(),
                    },
                    ttl: 1,
                    auto_heartbeat: true,
                };
                let _ret = provier.register(req);
                match _ret {
                    Err(err) => {
                        log::error!("register fail: {}", err.to_string());
                    }
                    Ok(_) => {}
                }
            }
        }
    }
}
