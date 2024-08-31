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
use std::sync::Arc;
use crate::core::model::cache::ResourceEventKey;
use crate::core::model::config::{ConfigFile, ConfigGroup};
use crate::core::model::error::PolarisError;
use crate::core::model::naming::{ServiceKey, ServiceRule, Services};
use crate::core::plugin::cache::{Filter, ResourceCache, ResourceListener};
use crate::core::plugin::plugins::{Extensions, Plugin};

pub struct MemoryCache {

}

impl Default for MemoryCache {
    fn default() -> Self {
        MemoryCache{}
    }
}

impl Plugin for MemoryCache {
    fn init(&mut self, options: HashMap<String, String>, extensions: Arc<Extensions>) {
        todo!()
    }

    fn destroy(&self) {
        todo!()
    }

    fn name(&self) -> String {
        "memory".to_string()
    }
}

impl ResourceCache for MemoryCache {
    fn get_services(&self) -> Vec<ServiceKey> {
        todo!()
    }

    fn load_service_rule(&self, filter: Filter) -> Result<ServiceRule, PolarisError> {
        todo!()
    }

    fn load_services(&self, filter: Filter) -> Result<Services, PolarisError> {
        todo!()
    }

    fn load_service_instances(&self, filter: Filter) -> Result<Services, PolarisError> {
        todo!()
    }

    fn load_config_file(&self, filter: Filter) -> Result<ConfigFile, PolarisError> {
        todo!()
    }

    fn load_config_groups(&self, filter: Filter) -> Result<ConfigGroup, PolarisError> {
        todo!()
    }

    fn load_config_group_files(&self, filter: Filter) -> Result<ConfigGroup, PolarisError> {
        todo!()
    }

    fn register_resource_listener(&self, listener: Arc<dyn ResourceListener>) {
        todo!()
    }

    fn watch_resource(&self, key: ResourceEventKey) {
        todo!()
    }

    fn un_watch_resource(&self, key: ResourceEventKey) {
        todo!()
    }
}