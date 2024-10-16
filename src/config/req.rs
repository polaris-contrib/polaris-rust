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

use crate::core::model::{
    config::{ConfigFile, ConfigFileChangeEvent, ConfigFileRequest, ConfigReleaseRequest},
    naming::ServiceInstancesChangeEvent,
    pb::lib::ConfigFileRelease,
};

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
            flow_id: flow_id,
            config_file: self.file.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct UpdateConfigFileRequest {
    pub flow_id: String,
    pub timeout: Duration,
    pub file: ConfigFile,
}

impl UpdateConfigFileRequest {
    pub fn to_config_request(&self) -> ConfigFileRequest {
        let mut flow_id = self.flow_id.clone();
        if flow_id.is_empty() {
            flow_id = uuid::Uuid::new_v4().to_string();
        }
        ConfigFileRequest {
            flow_id: flow_id,
            config_file: self.file.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DeleteConfigFileRequest {
    pub flow_id: String,
    pub namespace: String,
    pub group: String,
    pub file: String,
    pub timeout: Duration,
}

impl DeleteConfigFileRequest {
    pub fn to_config_request(&self) -> ConfigFileRequest {
        let mut flow_id = self.flow_id.clone();
        if flow_id.is_empty() {
            flow_id = uuid::Uuid::new_v4().to_string();
        }
        let mut file = ConfigFile::default();
        file.namespace = self.namespace.clone();
        file.group = self.group.clone();
        file.name = self.file.clone();
        ConfigFileRequest {
            flow_id: flow_id,
            config_file: file,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PublishConfigFileRequest {
    pub flow_id: String,
    pub namespace: String,
    pub group: String,
    pub file: String,
    pub release_name: String,
    pub md5: String,
    pub timeout: Duration,
}

impl PublishConfigFileRequest {
    pub fn to_config_request(&self) -> ConfigReleaseRequest {
        let mut flow_id = self.flow_id.clone();
        if flow_id.is_empty() {
            flow_id = uuid::Uuid::new_v4().to_string();
        }
        ConfigReleaseRequest {
            flow_id: flow_id,
            config_file: ConfigFileRelease {
                id: None,
                name: Some(self.release_name.clone()),
                namespace: Some(self.namespace.clone()),
                group: Some(self.group.clone()),
                file_name: Some(self.file.clone()),
                content: None,
                comment: None,
                md5: Some(self.md5.clone()),
                version: None,
                create_time: None,
                create_by: None,
                modify_time: None,
                modify_by: None,
                tags: vec![],
                active: None,
                format: None,
                release_description: None,
                release_type: None,
                beta_labels: vec![],
            },
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

pub struct WatchConfigFileResponse {}

#[derive(Clone, Debug)]
pub struct GetConfigGroupRequest {}

#[derive(Clone, Debug)]
pub struct WatchConfigGroupRequest {}

pub struct WatchConfigGroupResponse {}
