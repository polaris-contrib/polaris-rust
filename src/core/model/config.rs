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

use super::pb::lib::ConfigFileRelease;

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

#[derive(Clone, Debug)]
pub struct ConfigReleaseRequest {
    pub flow_id: String,
    pub config_file: ConfigFileRelease,
}

impl ConfigFileRequest {
    pub fn convert_spec(&self) -> crate::core::model::pb::lib::ConfigFile {
        todo!()
    }

    pub fn convert_spec_release(&self) -> crate::core::model::pb::lib::ConfigFileRelease {
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

#[derive(Default, Debug, Clone)]
pub struct ConfigGroup {
    pub namespace: String,
    pub group: String,
    pub files: Vec<ConfigFile>,
}

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
        return "".to_string();
    }

    pub fn get_encrypt_algo(&self) -> String {
        for (_k, v) in self.tags.iter().enumerate() {
            let label_key = v.key.clone().unwrap();
            if label_key == CONFIG_FILE_TAG_KEY_ENCRYPT_ALGO {
                return v.value.clone().unwrap();
            }
        }
        return "".to_string();
    }
}
