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

use std::{collections::HashMap, sync::Arc, time::Duration};

use polaris_rust::{
    core::{
        context::SDKContext,
        model::{error::PolarisError, naming::Location},
    },
    discovery::{
        api::{new_consumer_api_by_context, new_provider_api_by_context, ConsumerAPI, ProviderAPI},
        req::{GetAllInstanceRequest, InstanceDeregisterRequest, InstanceRegisterRequest},
    },
};
use tracing::level_filters::LevelFilter;

#[tokio::main]
async fn main() -> Result<(), PolarisError> {
    tracing_subscriber::fmt()
        // all spans/events with a level higher than TRACE (e.g, info, warn, etc.)
        // will be written to stdout.
        .with_thread_names(true)
        .with_file(true)
        .with_level(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_max_level(LevelFilter::INFO)
        // sets this to be the default, global collector for this application.
        .init();

    let start_time = std::time::Instant::now();

    let sdk_context_ret = SDKContext::default();
    if sdk_context_ret.is_err() {
        tracing::error!(
            "create sdk context fail: {}",
            sdk_context_ret.err().unwrap()
        );
        return Err(PolarisError::new(
            polaris_rust::core::model::error::ErrorCode::UnknownServerError,
            "".to_string(),
        ));
    }
    let arc_ctx = Arc::new(sdk_context_ret.unwrap());

    let provider_ret = new_provider_api_by_context(arc_ctx.clone());
    if provider_ret.is_err() {
        tracing::error!("create provider fail: {}", provider_ret.err().unwrap());
        return Err(PolarisError::new(
            polaris_rust::core::model::error::ErrorCode::UnknownServerError,
            "".to_string(),
        ));
    }

    let consumer_ret = new_consumer_api_by_context(arc_ctx);
    if consumer_ret.is_err() {
        tracing::error!("create consumer fail: {}", consumer_ret.err().unwrap());
        return Err(PolarisError::new(
            polaris_rust::core::model::error::ErrorCode::UnknownServerError,
            "".to_string(),
        ));
    }

    let provider = provider_ret.unwrap();
    let consumer = consumer_ret.unwrap();

    tracing::info!("create provider cost: {:?}", start_time.elapsed());
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
        ttl: 0,
        // 这里开启心跳的自动上报能力
        auto_heartbeat: false,
    };
    let _ret = provider.register(req).await;
    match _ret {
        Err(err) => {
            tracing::error!("register fail: {}", err.to_string());
        }
        Ok(_) => {}
    }

    // for _ in 0..120 {
    //     std::thread::sleep(Duration::from_secs(1));
    // }

    let instances_ret = consumer
        .get_all_instance(GetAllInstanceRequest {
            flow_id: uuid::Uuid::new_v4().to_string(),
            timeout: Duration::from_secs(10),
            namespace: "rust-demo".to_string(),
            service: "polaris-rust-provider".to_string(),
        })
        .await;

    match instances_ret {
        Err(err) => {
            tracing::error!("get all instance fail: {}", err.to_string());
        }
        Ok(instances) => {
            tracing::info!("get all instance: {:?}", instances);
        }
    }

    // 反注册
    let deregister_req = InstanceDeregisterRequest {
        flow_id: uuid::Uuid::new_v4().to_string(),
        timeout: Duration::from_secs(1),
        namespace: "rust-demo".to_string(),
        service: "polaris-rust-provider".to_string(),
        ip: "1.1.1.1".to_string(),
        port: 8080,
    };

    let _ret = provider.deregister(deregister_req).await;
    match _ret {
        Err(err) => {
            tracing::error!("deregister fail: {}", err.to_string());
        }
        Ok(_) => {}
    }

    Ok(())
}
