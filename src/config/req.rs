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

use std::{sync::Arc, time::Duration};

use crate::core::model::config::{
    ConfigFile, ConfigFileChangeEvent, ConfigFileRelease, ConfigFileRequest,
    ConfigGroupChangeEvent, ConfigPublishRequest, ConfigReleaseRequest,
};

use super::default::{ConfigFileResourceListener, ConfigGroupResourceListener};

#[derive(Clone, Debug)]
pub struct GetConfigFileRequest {
    pub namespace: String,
    pub group: String,
    pub file: String,
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct CreateConfigFileRequest {
    pub flow_id: String,
    pub timeout: Duration,
    pub file: ConfigFile,
}

impl CreateConfigFileRequest {
    pub fn to_config_request(&self) -> ConfigFileRequest {
        let mut flow_id = self.flow_id.clone();
        if flow_id.is_empty() {
            flow_id = uuid::Uuid::new_v4().to_string();
        }
        ConfigFileRequest {
            flow_id,
            config_file: self.file.clone(),
        }
    }
}

/// UpdateConfigFileRequest 配置更新请求
#[derive(Clone, Debug)]
pub struct UpdateConfigFileRequest {
    // flow_id 流水号
    pub flow_id: String,
    // timeout 超时时间
    pub timeout: Duration,
    // file 配置文件
    pub file: ConfigFile,
}

impl UpdateConfigFileRequest {
    pub fn to_config_request(&self) -> ConfigFileRequest {
        let mut flow_id = self.flow_id.clone();
        if flow_id.is_empty() {
            flow_id = uuid::Uuid::new_v4().to_string();
        }
        ConfigFileRequest {
            flow_id,
            config_file: self.file.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PublishConfigFileRequest {
    pub flow_id: String,
    pub timeout: Duration,
    pub config_file: ConfigFileRelease,
}

impl PublishConfigFileRequest {
    pub fn to_config_request(&self) -> ConfigReleaseRequest {
        let mut flow_id = self.flow_id.clone();
        if flow_id.is_empty() {
            flow_id = uuid::Uuid::new_v4().to_string();
        }
        ConfigReleaseRequest {
            flow_id,
            config_file: self.config_file.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct UpsertAndPublishConfigFileRequest {
    pub flow_id: String,
    pub timeout: Duration,
    pub release_name: String,
    pub md5: String,
    pub config_file: ConfigFile,
}

impl UpsertAndPublishConfigFileRequest {
    pub fn to_config_request(&self) -> ConfigPublishRequest {
        let mut flow_id = self.flow_id.clone();
        if flow_id.is_empty() {
            flow_id = uuid::Uuid::new_v4().to_string();
        }
        ConfigPublishRequest {
            flow_id,
            md5: self.md5.clone(),
            release_name: self.release_name.clone(),
            config_file: self.config_file.clone(),
        }
    }
}

#[derive(Clone)]
pub struct WatchConfigFileRequest {
    pub namespace: String,
    pub group: String,
    pub file: String,
    pub call_back: Arc<dyn Fn(ConfigFileChangeEvent) + Send + Sync>,
}

impl WatchConfigFileRequest {
    pub fn get_key(&self) -> String {
        format!("{}#{}#{}", self.namespace, self.group, self.file)
    }
}

pub struct WatchConfigFileResponse {
    pub watch_id: u64,
    watch_key: String,
    owner: Arc<ConfigFileResourceListener>,
}

impl WatchConfigFileResponse {
    pub fn new(watch_id: u64, watch_key: String, owner: Arc<ConfigFileResourceListener>) -> Self {
        WatchConfigFileResponse {
            watch_id,
            watch_key,
            owner,
        }
    }

    pub async fn cancel_watch(&self) {
        self.owner
            .cancel_watch(&self.watch_key, self.watch_id)
            .await;
    }
}

#[derive(Clone, Debug)]
pub struct GetConfigGroupRequest {
    pub flow_id: String,
    pub timeout: Duration,
    // namespace 命名空间
    pub namespace: String,
    // group 配置分组
    pub group: String,
}

#[derive(Clone)]
pub struct WatchConfigGroupRequest {
    pub flow_id: String,
    pub timeout: Duration,
    // namespace 命名空间
    pub namespace: String,
    // group 配置分组
    pub group: String,
    pub call_back: Arc<dyn Fn(ConfigGroupChangeEvent) + Send + Sync>,
}

impl WatchConfigGroupRequest {
    pub fn get_key(&self) -> String {
        format!("{}#{}", self.namespace, self.group)
    }
}

pub struct WatchConfigGroupResponse {
    pub watch_id: u64,
    watch_key: String,
    owner: Arc<ConfigGroupResourceListener>,
}

impl WatchConfigGroupResponse {
    pub fn new(watch_id: u64, watch_key: String, owner: Arc<ConfigGroupResourceListener>) -> Self {
        WatchConfigGroupResponse {
            watch_id,
            watch_key,
            owner,
        }
    }

    pub async fn cancel_watch(&self) {
        self.owner
            .cancel_watch(&self.watch_key, self.watch_id)
            .await;
    }
}
