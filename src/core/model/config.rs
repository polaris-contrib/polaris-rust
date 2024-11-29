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
    pub fn convert_spec(&self) -> polaris_specification::v1::ConfigFile {
        let mut tags = Vec::<polaris_specification::v1::ConfigFileTag>::new();
        self.config_file.labels.iter().for_each(|(k, v)| {
            tags.push(polaris_specification::v1::ConfigFileTag {
                key: Some(k.clone()),
                value: Some(v.clone()),
            });
        });

        polaris_specification::v1::ConfigFile {
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
    pub fn convert_spec(&self) -> polaris_specification::v1::ConfigFileRelease {
        polaris_specification::v1::ConfigFileRelease {
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
    pub fn convert_spec(&self) -> polaris_specification::v1::ConfigFilePublishInfo {
        todo!()
    }
}

/// ConfigFile 配置文件
#[derive(Default, Debug, Clone)]
pub struct ConfigFile {
    // namespace 命名空间
    pub namespace: String,
    // group 配置分组
    pub group: String,
    // name 配置文件名
    pub name: String,
    // version 版本号
    pub version: u64,
    // content 配置内容
    pub content: String,
    // labels 配置标签
    pub labels: HashMap<String, String>,
    // encrypt_algo 配置加解密标识
    pub encrypt_algo: String,
    // encrypt_key 加密密钥
    pub encrypt_key: String,
}

impl ConfigFile {
    pub fn convert_from_spec(f: polaris_specification::v1::ClientConfigFileInfo) -> ConfigFile {
        ConfigFile {
            namespace: f.namespace.clone().unwrap(),
            group: f.group.clone().unwrap(),
            name: f.name.clone().unwrap(),
            version: f.version.unwrap(),
            content: f.content.clone().unwrap(),
            labels: f
                .tags
                .iter()
                .map(|tag| (tag.key.clone().unwrap(), tag.value.clone().unwrap()))
                .collect(),
            encrypt_algo: get_encrypt_algo(&f),
            encrypt_key: get_encrypt_data_key(&f),
        }
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

pub fn get_encrypt_data_key(file: &polaris_specification::v1::ClientConfigFileInfo) -> String {
    for (_k, v) in file.tags.iter().enumerate() {
        let label_key = v.key.clone().unwrap();
        if label_key == CONFIG_FILE_TAG_KEY_DATA_KEY {
            return v.value.clone().unwrap();
        }
    }
    "".to_string()
}

pub fn get_encrypt_algo(file: &polaris_specification::v1::ClientConfigFileInfo) -> String {
    for (_k, v) in file.tags.iter().enumerate() {
        let label_key = v.key.clone().unwrap();
        if label_key == CONFIG_FILE_TAG_KEY_ENCRYPT_ALGO {
            return v.value.clone().unwrap();
        }
    }
    "".to_string()
}
