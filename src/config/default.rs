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

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use tokio::sync::RwLock;

use crate::core::{
    context::SDKContext,
    model::{
        cache::{EventType, ServerEvent},
        config::{ConfigFile, ConfigFileChangeEvent},
        error::PolarisError,
    },
    plugin::cache::{Action, ResourceListener},
};

use super::{
    api::{ConfigFileAPI, ConfigGroupAPI},
    req::{
        CreateConfigFileRequest, GetConfigFileRequest, GetConfigGroupRequest,
        PublishConfigFileRequest, UpdateConfigFileRequest, UpsertAndPublishConfigFileRequest,
        WatchConfigFileRequest, WatchConfigFileResponse, WatchConfigGroupRequest,
        WatchConfigGroupResponse,
    },
};

struct ConfigFileWatcher {
    req: WatchConfigFileRequest,
}

struct ConfigFileResourceListener {
    // watchers: namespace#service -> ConfigFileWatcher
    watchers: Arc<RwLock<HashMap<String, Vec<ConfigFileWatcher>>>>,
}

#[async_trait::async_trait]
impl ResourceListener for ConfigFileResourceListener {
    // 处理事件
    async fn on_event(&self, action: Action, val: ServerEvent) {
        let event_key = val.event_key;
        let mut watch_key = event_key.namespace.clone();
        let group = event_key.filter.get("group");
        let file = event_key.filter.get("file");
        watch_key.push('#');
        watch_key.push_str(group.unwrap().as_str());
        watch_key.push('#');
        watch_key.push_str(file.unwrap().as_str());

        let watchers = self.watchers.read().await;
        if let Some(watchers) = watchers.get(&watch_key) {
            let cfg_cache_opt = val.value.to_config_file();
            match cfg_cache_opt {
                Some(cfg_cache_val) => {
                    for watcher in watchers {
                        (watcher.req.call_back)(ConfigFileChangeEvent {
                            config_file: cfg_cache_val.to_config_file(),
                        })
                    }
                }
                None => {
                    // do nothing
                }
            }
        }
    }

    // 获取监听的key
    fn watch_key(&self) -> EventType {
        EventType::ConfigFile
    }
}

/// DefaultConfigFileAPI
pub struct DefaultConfigFileAPI {
    context: Arc<SDKContext>,
    manage_sdk: bool,
    // watchers: namespace#service -> ConfigFileWatcher
    watchers: Arc<ConfigFileResourceListener>,
    //
    register_resource_watcher: AtomicBool,
}

impl DefaultConfigFileAPI {
    pub fn new(context: Arc<SDKContext>, manage_sdk: bool) -> Self {
        Self {
            context,
            manage_sdk,
            watchers: Arc::new(ConfigFileResourceListener {
                watchers: Arc::new(RwLock::new(HashMap::new())),
            }),
            register_resource_watcher: AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl ConfigFileAPI for DefaultConfigFileAPI {
    async fn get_config_file(&self, req: GetConfigFileRequest) -> Result<ConfigFile, PolarisError> {
        self.context.get_engine().get_config_file(req).await
    }

    async fn create_config_file(&self, req: CreateConfigFileRequest) -> Result<bool, PolarisError> {
        self.context.get_engine().create_config_file(req).await
    }

    async fn update_config_file(&self, req: UpdateConfigFileRequest) -> Result<bool, PolarisError> {
        self.context.get_engine().update_config_file(req).await
    }

    async fn publish_config_file(
        &self,
        req: PublishConfigFileRequest,
    ) -> Result<bool, PolarisError> {
        self.context.get_engine().publish_config_file(req).await
    }

    async fn upsert_publish_config_file(
        &self,
        req: UpsertAndPublishConfigFileRequest,
    ) -> Result<bool, PolarisError> {
        self.context
            .get_engine()
            .upsert_publish_config_file(req)
            .await
    }

    async fn watch_config_file(
        &self,
        req: WatchConfigFileRequest,
    ) -> Result<WatchConfigFileResponse, PolarisError> {
        if self
            .register_resource_watcher
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::SeqCst)
            .is_ok()
        {
            // 延迟注册资源监听器
            self.context
                .get_engine()
                .register_resource_listener(self.watchers.clone())
                .await;
        }

        let mut watchers = self.watchers.watchers.write().await;

        let watch_key = req.get_key();
        let items = watchers.entry(watch_key.clone()).or_insert_with(Vec::new);

        items.push(ConfigFileWatcher { req });
        Ok(WatchConfigFileResponse {})
    }
}

/// DefaultConfigGroupAPI
pub struct DefaultConfigGroupAPI {}

#[async_trait::async_trait]
impl ConfigGroupAPI for DefaultConfigGroupAPI {
    async fn get_publish_config_files(
        &self,
        req: GetConfigGroupRequest,
    ) -> Result<Vec<ConfigFile>, PolarisError> {
        todo!()
    }

    async fn watch_publish_config_files(
        &self,
        req: WatchConfigGroupRequest,
    ) -> Result<WatchConfigGroupResponse, PolarisError> {
        todo!()
    }
}
