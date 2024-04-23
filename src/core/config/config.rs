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

use std::{fs, io};
use serde::Deserialize;
use crate::core::config::config_file::ConfigFileConfig;
use crate::core::config::consumer::ConsumerConfig;
use crate::core::config::global::GlobalConfig;
use crate::core::config::provider::ProviderConfig;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    pub global: GlobalConfig,
    pub consumer: ConsumerConfig,
    pub provider: ProviderConfig,
    pub config: ConfigFileConfig,
}

pub fn load(path: String) -> Result<*const Configuration, io::Error> {
    let val = fs::read_to_string(path);
    if val.is_ok() {
        let data = val.ok().unwrap();
        let config: Configuration = serde_yaml::from_str(&*data).expect(&format!("failure to format yaml str {}", &data));
        return Ok(&config)
    }
    return Err(val.err().unwrap())
}

#[cfg(test)]
mod tests {
    use std::env;
    use crate::core::config::config::load;

    #[test]
    fn test_load() {
        let ret = load(env::var("TEST_LOAD_FILE").unwrap());
        if ret.is_ok() {
            let conf = ret.ok().unwrap();
            println!("{:?}", conf)
        } else {
            println!("{:?}", ret.err().unwrap())
        }
    }
}
