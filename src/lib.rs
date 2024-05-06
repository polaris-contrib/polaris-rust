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

use crate::discovery::api::new_provider_api;

pub mod discovery;
pub mod core;
pub mod ratelimit;
pub mod router;
pub mod config;
pub mod circuitbreaker;
pub mod plugins;

mod tests {
    use log::error;
    use crate::discovery::api::new_provider_api;

    #[test]
    fn test_create_provider() {
        let provider_ret = new_provider_api();
        match provider_ret {
            Err(err) => {
                error!("{}", err.to_string());
            }
            Ok(mut provier) => {

            }
        }
    }
}
