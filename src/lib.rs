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

#[cfg(test)]
mod tests {
    use std::{borrow::BorrowMut, collections::HashMap, pin::Pin, sync::Arc, time::Duration};

    use crate::{
        core::model::{error::PolarisError, naming::Location},
        discovery::{
            api::{new_provider_api, ProviderAPI},
            req::{InstanceDeregisterRequest, InstanceHeartbeatRequest, InstanceRegisterRequest},
        },
    };

    use std::sync::Once;

    use tracing::metadata::LevelFilter;

    static LOGGER_INIT: Once = Once::new();

    pub(crate) fn setup_log() {
        LOGGER_INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_thread_names(true)
                .with_file(true)
                .with_level(true)
                .with_line_number(true)
                .with_thread_ids(true)
                .with_max_level(LevelFilter::DEBUG)
                .init()
        });
    }

    #[tokio::test]
    async fn test_create_provider() {
        setup_log();
        let start_time = std::time::Instant::now();
        let provider_ret = new_provider_api();
        tracing::info!("create provider cost: {:?}", start_time.elapsed());
        match provider_ret {
            Err(err) => {
                tracing::error!("create provider fail: {}", err.to_string());
            }
            Ok(provier) => {
                let metadata = HashMap::new();

                let req = InstanceRegisterRequest {
                    flow_id: uuid::Uuid::new_v4().to_string(),
                    timeout: Duration::from_secs(1),
                    id: None,
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
                    metadata: metadata,
                    location: Location {
                        region: "1".to_string(),
                        zone: "1".to_string(),
                        campus: "1".to_string(),
                    },
                    ttl: 5,
                    // 这里开启心跳的自动上报能力
                    auto_heartbeat: true,
                };
                let _ret = provier.register(req).await;
                match _ret {
                    Err(err) => {
                        tracing::error!("register fail: {}", err.to_string());
                    }
                    Ok(_) => {}
                }

                let arc_provider = Arc::new(provier);

                // 主动做一次心跳上报
                async {
                    let cloned_provider = arc_provider.clone();
                    for _ in 0..10 {
                        let _ = cloned_provider
                            .heartbeat(InstanceHeartbeatRequest {
                                timeout: Duration::from_secs(1),
                                flow_id: "1".to_string(),
                                id: None,
                                namespace: "rust-demo".to_string(),
                                service: "polaris-rust-provider".to_string(),
                                ip: "1.1.1.1".to_string(),
                                port: 8080,
                                vpc_id: "1".to_string(),
                            })
                            .await;
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
                .await;

                // 反注册
                let deregister_req = InstanceDeregisterRequest {
                    flow_id: uuid::Uuid::new_v4().to_string(),
                    timeout: Duration::from_secs(1),
                    namespace: "rust-demo".to_string(),
                    service: "polaris-rust-provider".to_string(),
                    ip: "1.1.1.1".to_string(),
                    port: 8080,
                    vpc_id: "1".to_string(),
                };

                let _ret = arc_provider.clone().deregister(deregister_req).await;
                match _ret {
                    Err(err) => {
                        tracing::error!("deregister fail: {}", err.to_string());
                    }
                    Ok(_) => {}
                }

                std::mem::forget(arc_provider);
            }
        }
    }
}
