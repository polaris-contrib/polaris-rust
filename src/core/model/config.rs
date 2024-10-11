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
