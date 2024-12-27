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

use polaris_rust::{config::{
    api::{new_config_file_api_by_context, ConfigFileAPI},
    req::{
        CreateConfigFileRequest, PublishConfigFileRequest, UpdateConfigFileRequest,
        UpsertAndPublishConfigFileRequest, WatchConfigFileRequest,
    },
}, core::{
    context::SDKContext,
    model::{
        config::{ConfigFile, ConfigFileRelease},
        error::PolarisError,
    },
}, info};
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

    let config_file_api_ret = new_config_file_api_by_context(arc_ctx.clone());
    if config_file_api_ret.is_err() {
        tracing::error!(
            "create config_file api fail: {}",
            config_file_api_ret.err().unwrap()
        );
        return Err(PolarisError::new(
            polaris_rust::core::model::error::ErrorCode::UnknownServerError,
            "".to_string(),
        ));
    }

    tracing::info!(
        "create config_file api client cost: {:?}",
        start_time.elapsed()
    );

    let config_file_api = config_file_api_ret.unwrap();

    let mut labels = HashMap::<String, String>::new();

    labels.insert("rust".to_string(), "rust".to_string());

    // 创建文件
    let ret = config_file_api
        .create_config_file(CreateConfigFileRequest {
            flow_id: uuid::Uuid::new_v4().to_string(),
            timeout: Duration::from_secs(1),
            file: ConfigFile {
                namespace: "rust".to_string(),
                group: "rust".to_string(),
                name: "rust.toml".to_string(),
                content: "test".to_string(),
                labels: labels.clone(),
                ..Default::default()
            },
        })
        .await;

    if ret.is_err() {
        tracing::error!("create config_file fail: {}", ret.err().unwrap());
        return Err(PolarisError::new(
            polaris_rust::core::model::error::ErrorCode::UnknownServerError,
            "".to_string(),
        ));
    }

    // 更新文件
    let ret = config_file_api
        .update_config_file(UpdateConfigFileRequest {
            flow_id: uuid::Uuid::new_v4().to_string(),
            timeout: Duration::from_secs(1),
            file: ConfigFile {
                namespace: "rust".to_string(),
                group: "rust".to_string(),
                name: "rust.toml".to_string(),
                content: "test".to_string(),
                labels: labels.clone(),
                ..Default::default()
            },
        })
        .await;

    if ret.is_err() {
        tracing::error!("update config_file fail: {}", ret.err().unwrap());
        return Err(PolarisError::new(
            polaris_rust::core::model::error::ErrorCode::UnknownServerError,
            "".to_string(),
        ));
    }

    // 发布文件
    let ret = config_file_api
        .publish_config_file(PublishConfigFileRequest {
            flow_id: uuid::Uuid::new_v4().to_string(),
            timeout: Duration::from_secs(1),
            config_file: ConfigFileRelease {
                namespace: "rust".to_string(),
                group: "rust".to_string(),
                file_name: "rust.toml".to_string(),
                release_name: "rust".to_string(),
                md5: "".to_string(),
            },
        })
        .await;

    if ret.is_err() {
        tracing::error!("publish config_file fail: {}", ret.err().unwrap());
        return Err(PolarisError::new(
            polaris_rust::core::model::error::ErrorCode::UnknownServerError,
            "".to_string(),
        ));
    }

    // 文件变更订阅
    let _ = config_file_api
        .watch_config_file(WatchConfigFileRequest {
            namespace: "rust".to_string(),
            group: "rust".to_string(),
            file: "rust.toml".to_string(),
            call_back: Arc::new(|event| {
                info!("receive config change event: {:?}", event);
            }),
        })
        .await;

    // 变更 10 次配置文件并发布
    for i in 0..10 {
        let ret = config_file_api
            .upsert_publish_config_file(UpsertAndPublishConfigFileRequest {
                flow_id: uuid::Uuid::new_v4().to_string(),
                timeout: Duration::from_secs(1),
                release_name: format!("rust-{}", i),
                md5: "".to_string(),
                config_file: ConfigFile {
                    namespace: "rust".to_string(),
                    group: "rust".to_string(),
                    name: "rust.toml".to_string(),
                    content: format!("test-{}", i),
                    labels: labels.clone(),
                    ..Default::default()
                },
            })
            .await;

        if ret.is_err() {
            tracing::error!(
                "upsert and publish config_file fail: {}",
                ret.err().unwrap()
            );
            return Err(PolarisError::new(
                polaris_rust::core::model::error::ErrorCode::UnknownServerError,
                "".to_string(),
            ));
        }

        std::thread::sleep(Duration::from_secs(10));
    }

    Ok(())
}
