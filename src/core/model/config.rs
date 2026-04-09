// Tencent is pleased to support the open source community by making Pole available.
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

use pole_specification::v1::{config_file::ConfigFileType, ConfigFileSupportedClient};

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
    pub fn convert_spec(&self) -> pole_specification::v1::ConfigFile {
        pole_specification::v1::ConfigFile {
            id: self.config_file.id.clone(),
            name: self.config_file.name.clone(),
            namespace: self.config_file.namespace.clone(),
            group: self.config_file.group.clone(),
            content: self.config_file.content.clone(),
            format: "".to_string(),
            comment: "".to_string(),
            status: "".to_string(),
            labels: self.config_file.labels.clone(),
            ctime: "".to_string(),
            mtime: "".to_string(),
            rtime: "".to_string(),
            encrypted: false,
            encrypt_algo: "".to_string(),
            supported_client: ConfigFileSupportedClient::ClientAll.into(),
            persistent: None,
            config_type: ConfigFileType::ConfigFile.into(),
            placeholder_value_map: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConfigReleaseRequest {
    pub flow_id: String,
    pub config_file: ConfigFileRelease,
}

impl ConfigReleaseRequest {
    pub fn convert_spec(&self) -> pole_specification::v1::ConfigFileRelease {
        pole_specification::v1::ConfigFileRelease {
            id: self.config_file.id.clone(),
            name: self.config_file.release_name.clone(),
            namespace: self.config_file.namespace.clone(),
            group: self.config_file.group.clone(),
            content: "".to_string(),
            format: "".to_string(),
            comment: "".to_string(),
            file_name: self.config_file.file_name.clone(),
            version: 0,
            md5: self.config_file.md5.clone(),
            labels: HashMap::new(),
            active: true,
            encrypt_algo: "".to_string(),
            encrypted: false,
            release_description: "".to_string(),
            release_type: "".to_string(),
            beta_labels: Vec::new(),
            create_by: "".to_string(),
            modify_by: "".to_string(),
            ctime: "".to_string(),
            mtime: "".to_string(),
            supported_client: ConfigFileSupportedClient::ClientAll.into(),
            persistent: None,
            config_type: ConfigFileType::ConfigFile.into(),
            placeholder_value_map: HashMap::new(),
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
    pub fn convert_spec(&self) -> pole_specification::v1::ConfigFilePublishInfo {
        todo!()
    }
}

/// ConfigFile 配置文件
#[derive(Default, Debug, Clone)]
pub struct ConfigFile {
    pub id: String,
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
    pub fn convert_from_spec(f: pole_specification::v1::ConfigFileRelease) -> ConfigFile {
        let encrypt_algo = get_encrypt_algo(&f);
        let encrypt_key = get_encrypt_data_key(&f);
        ConfigFile {
            id: f.id,
            namespace: f.namespace,
            group: f.group,
            name: f.name,
            version: f.version,
            content: f.content,
            labels: f.labels,
            encrypt_algo,
            encrypt_key,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConfigFileRelease {
    pub id: String,
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
    pub revision: String,
}

#[derive(Clone, Debug)]
pub struct ConfigFileChangeEvent {
    pub config_file: ConfigFile,
}

#[derive(Clone, Debug)]
pub struct ConfigGroupChangeEvent {
    pub config_group: ConfigGroup,
}

pub fn get_encrypt_data_key(file: &pole_specification::v1::ConfigFileRelease) -> String {
    for (key, val) in file.labels.iter() {
        let label_key = key.clone();
        if label_key == CONFIG_FILE_TAG_KEY_DATA_KEY {
            return val.clone();
        }
    }
    "".to_string()
}

pub fn get_encrypt_algo(file: &pole_specification::v1::ConfigFileRelease) -> String {
    for (key, val) in file.labels.iter() {
        let label_key = key.clone();
        if label_key == CONFIG_FILE_TAG_KEY_ENCRYPT_ALGO {
            return val.clone();
        }
    }
    "".to_string()
}
