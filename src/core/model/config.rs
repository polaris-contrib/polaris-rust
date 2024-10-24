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

use std::collections::HashMap;

// CONFIG_FILE_TAG_KEY_USE_ENCRYPTED 配置加密开关标识，value 为 boolean
const CONFIG_FILE_TAG_KEY_USE_ENCRYPTED: &str = "internal-encrypted";
// CONFIG_FILE_TAG_KEY_DATA_KEY 加密密钥 tag key
const CONFIG_FILE_TAG_KEY_DATA_KEY: &str = "internal-datakey";
// CONFIG_FILE_TAG_KEY_ENCRYPT_ALGO 加密算法 tag key
const CONFIG_FILE_TAG_KEY_ENCRYPT_ALGO: &str = "internal-encryptalgo";

#[derive(Clone, Debug)]
pub struct ConfigFileRequest {
    pub flow_id: String,
    pub config_file: ConfigFile,
}

impl ConfigFileRequest {
    pub fn convert_spec(&self) -> crate::core::model::pb::lib::ConfigFile {
        let mut tags = Vec::<crate::core::model::pb::lib::ConfigFileTag>::new();
        self.config_file.labels.iter().for_each(|(k, v)| {
            tags.push(crate::core::model::pb::lib::ConfigFileTag {
                key: Some(k.clone()),
                value: Some(v.clone()),
            });
        });

        crate::core::model::pb::lib::ConfigFile {
            id: None,
            name: Some(self.config_file.name.clone()),
            namespace: Some(self.config_file.namespace.clone()),
            group: Some(self.config_file.group.clone()),
            content: Some(self.config_file.content.clone()),
            format: None,
            comment: None,
            status: None,
            tags,
            create_time: None,
            create_by: None,
            modify_time: None,
            modify_by: None,
            release_time: None,
            release_by: None,
            encrypted: None,
            encrypt_algo: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConfigReleaseRequest {
    pub flow_id: String,
    pub config_file: ConfigFileRelease,
}

impl ConfigReleaseRequest {
    pub fn convert_spec(&self) -> crate::core::model::pb::lib::ConfigFileRelease {
        crate::core::model::pb::lib::ConfigFileRelease {
            id: None,
            name: Some(self.config_file.release_name.clone()),
            namespace: Some(self.config_file.namespace.clone()),
            group: Some(self.config_file.group.clone()),
            content: None,
            format: None,
            comment: None,
            file_name: Some(self.config_file.file_name.clone()),
            version: None,
            tags: Vec::new(),
            active: None,
            release_description: None,
            release_type: None,
            beta_labels: Vec::new(),
            create_time: None,
            create_by: None,
            modify_time: None,
            modify_by: None,
            md5: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConfigPublishRequest {
    pub flow_id: String,
    pub md5: String,
    pub release_name: String,
    pub config_file: ConfigFile,
}

impl ConfigPublishRequest {
    pub fn convert_spec(&self) -> crate::core::model::pb::lib::ConfigFilePublishInfo {
        todo!()
    }
}

#[derive(Default, Debug, Clone)]
pub struct ConfigFile {
    pub namespace: String,
    pub group: String,
    pub name: String,
    pub version: u64,
    pub content: String,
    pub labels: HashMap<String, String>,
    // 配置加解密标识
    pub encrypt_algo: String,
    pub encrypt_key: String,
}

impl ConfigFile {
    pub fn convert_from_spec(f: crate::core::model::pb::lib::ClientConfigFileInfo) -> ConfigFile {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub struct ConfigFileRelease {
    pub namespace: String,
    pub group: String,
    pub file_name: String,
    pub release_name: String,
    pub md5: String,
}

#[derive(Default, Debug, Clone)]
pub struct ConfigGroup {
    pub namespace: String,
    pub group: String,
    pub files: Vec<ConfigFile>,
}

#[derive(Clone, Debug)]
pub struct ConfigFileChangeEvent {
    pub config_file: ConfigFile,
}

impl crate::core::model::pb::lib::ClientConfigFileInfo {
    pub fn get_encrypt_data_key(&self) -> String {
        for (_k, v) in self.tags.iter().enumerate() {
            let label_key = v.key.clone().unwrap();
            if label_key == CONFIG_FILE_TAG_KEY_DATA_KEY {
                return v.value.clone().unwrap();
            }
        }
        "".to_string()
    }

    pub fn get_encrypt_algo(&self) -> String {
        for (_k, v) in self.tags.iter().enumerate() {
            let label_key = v.key.clone().unwrap();
            if label_key == CONFIG_FILE_TAG_KEY_ENCRYPT_ALGO {
                return v.value.clone().unwrap();
            }
        }
        "".to_string()
    }
}
