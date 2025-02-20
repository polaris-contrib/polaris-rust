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
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

use tokio::sync::RwLock;

use crate::core::{
    context::SDKContext,
    model::{
        cache::{EventType, ServerEvent},
        config::{ConfigFile, ConfigFileChangeEvent, ConfigGroup, ConfigGroupChangeEvent},
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

pub struct ConfigFileResourceListener {
    // watcher_id: 用于生成唯一的 watcher_id
    watcher_id: Arc<AtomicU64>,
    // watchers: namespace#service -> ConfigFileWatcher
    watchers: Arc<RwLock<HashMap<String, HashMap<u64, ConfigFileWatcher>>>>,
}

impl ConfigFileResourceListener {
    pub async fn cancel_watch(&self, watch_key: &str, watch_id: u64) {
        let mut watchers = self.watchers.write().await;
        let items = watchers.get_mut(watch_key);
        if let Some(vals) = items {
            vals.remove(&watch_id);
        }
    }
}

#[async_trait::async_trait]
impl ResourceListener for ConfigFileResourceListener {
    // 处理事件
    async fn on_event(&self, _action: Action, val: ServerEvent) {
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
                        (watcher.1.req.call_back)(ConfigFileChangeEvent {
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
    // manage_sdk: 是否管理 sdk_context 的生命周期
    manage_sdk: bool,
    // watchers: namespace#service -> ConfigFileWatcher
    watchers: Arc<ConfigFileResourceListener>,
    // register_resource_watcher: 是否已经注册资源监听器
    register_resource_watcher: AtomicBool,
}

impl DefaultConfigFileAPI {
    pub fn new_raw(context: SDKContext) -> Self {
        Self {
            context: Arc::new(context),
            manage_sdk: true,
            watchers: Arc::new(ConfigFileResourceListener {
                watcher_id: Arc::new(AtomicU64::new(0)),
                watchers: Arc::new(RwLock::new(HashMap::new())),
            }),
            register_resource_watcher: AtomicBool::new(false),
        }
    }

    pub fn new(context: Arc<SDKContext>) -> Self {
        Self {
            context,
            manage_sdk: false,
            watchers: Arc::new(ConfigFileResourceListener {
                watcher_id: Arc::new(AtomicU64::new(0)),
                watchers: Arc::new(RwLock::new(HashMap::new())),
            }),
            register_resource_watcher: AtomicBool::new(false),
        }
    }
}

impl Drop for DefaultConfigFileAPI {
    fn drop(&mut self) {
        if !self.manage_sdk {
            return;
        }
        let ctx = self.context.to_owned();
        let ret = Arc::try_unwrap(ctx);

        match ret {
            Ok(ctx) => {
                drop(ctx);
            }
            Err(_) => {
                // do nothing
            }
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

        let watch_id = self.watchers.watcher_id.fetch_add(1, Ordering::Acquire);
        let mut watchers = self.watchers.watchers.write().await;

        let watch_key = req.get_key();
        let items = watchers
            .entry(watch_key.clone())
            .or_insert_with(HashMap::new);

        items.insert(watch_id, ConfigFileWatcher { req });
        Ok(WatchConfigFileResponse::new(
            watch_id,
            watch_key,
            self.watchers.clone(),
        ))
    }
}

/// DefaultConfigGroupAPI
pub struct DefaultConfigGroupAPI {
    context: Arc<SDKContext>,
    // manage_sdk: 是否管理 sdk_context 的生命周期
    manage_sdk: bool,
    // watchers: namespace#service -> ConfigFileWatcher
    watchers: Arc<ConfigGroupResourceListener>,
    // register_resource_watcher: 是否已经注册资源监听器
    register_resource_watcher: AtomicBool,
}

impl DefaultConfigGroupAPI {
    pub fn new_raw(context: SDKContext) -> Self {
        Self {
            context: Arc::new(context),
            manage_sdk: true,
            watchers: Arc::new(ConfigGroupResourceListener {
                watcher_id: Arc::new(AtomicU64::new(0)),
                watchers: Arc::new(RwLock::new(HashMap::new())),
            }),
            register_resource_watcher: AtomicBool::new(false),
        }
    }

    pub fn new(context: Arc<SDKContext>) -> Self {
        Self {
            context,
            manage_sdk: false,
            watchers: Arc::new(ConfigGroupResourceListener {
                watcher_id: Arc::new(AtomicU64::new(0)),
                watchers: Arc::new(RwLock::new(HashMap::new())),
            }),
            register_resource_watcher: AtomicBool::new(false),
        }
    }
}

impl Drop for DefaultConfigGroupAPI {
    fn drop(&mut self) {
        if !self.manage_sdk {
            return;
        }
        let ctx = self.context.to_owned();
        let ret = Arc::try_unwrap(ctx);

        match ret {
            Ok(ctx) => {
                drop(ctx);
            }
            Err(_) => {
                // do nothing
            }
        }
    }
}

#[async_trait::async_trait]
impl ConfigGroupAPI for DefaultConfigGroupAPI {
    async fn get_publish_config_files(
        &self,
        req: GetConfigGroupRequest,
    ) -> Result<ConfigGroup, PolarisError> {
        self.context.get_engine().get_config_group_files(req).await
    }

    async fn watch_publish_config_files(
        &self,
        req: WatchConfigGroupRequest,
    ) -> Result<WatchConfigGroupResponse, PolarisError> {
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

        let watch_id = self.watchers.watcher_id.fetch_add(1, Ordering::Acquire);
        let mut watchers = self.watchers.watchers.write().await;

        let watch_key = req.get_key();
        let items = watchers
            .entry(watch_key.clone())
            .or_insert_with(HashMap::new);

        items.insert(watch_id, ConfigGroupWatcher { req });
        Ok(WatchConfigGroupResponse::new(
            watch_id,
            watch_key,
            self.watchers.clone(),
        ))
    }
}

struct ConfigGroupWatcher {
    req: WatchConfigGroupRequest,
}

pub struct ConfigGroupResourceListener {
    // watcher_id: 用于生成唯一的 watcher_id
    watcher_id: Arc<AtomicU64>,
    // watchers: namespace#group -> ConfigGroupWatcher
    watchers: Arc<RwLock<HashMap<String, HashMap<u64, ConfigGroupWatcher>>>>,
}

impl ConfigGroupResourceListener {
    pub async fn cancel_watch(&self, watch_key: &str, watch_id: u64) {
        let mut watchers = self.watchers.write().await;
        let items = watchers.get_mut(watch_key);
        if let Some(vals) = items {
            vals.remove(&watch_id);
        }
    }
}

#[async_trait::async_trait]
impl ResourceListener for ConfigGroupResourceListener {
    // 处理事件
    async fn on_event(&self, _action: Action, val: ServerEvent) {
        let event_key = val.event_key;
        let mut watch_key = event_key.namespace.clone();
        let group = event_key.filter.get("group");
        watch_key.push('#');
        watch_key.push_str(group.unwrap().as_str());

        let watchers = self.watchers.read().await;
        if let Some(watchers) = watchers.get(&watch_key) {
            let cfg_cache_opt = val.value.to_config_group();
            match cfg_cache_opt {
                Some(cfg_cache_val) => {
                    for watcher in watchers {
                        (watcher.1.req.call_back)(ConfigGroupChangeEvent {
                            config_group: cfg_cache_val.to_config_group().await,
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
