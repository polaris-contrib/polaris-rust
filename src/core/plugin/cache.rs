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

use crate::core::model::cache::{RegistryCacheValue, ResourceEventKey};
use crate::core::model::config::{ConfigFile, ConfigGroup};
use crate::core::model::error::PolarisError;
use crate::core::model::naming::{ServiceKey, ServiceRule, Services};
use crate::core::plugin::plugins::Plugin;
use std::collections::HashMap;
use std::sync::Arc;

use super::plugins::Extensions;

pub struct Filter {
    pub resource_key: ResourceEventKey,
    pub internal_request: bool,
    pub include_cache: bool,
}

enum Action {
    Add,
    Update,
    Delete,
}

pub trait ResourceListener {
    fn on_event(&self, action: Action, key: ResourceEventKey, val: Arc<dyn RegistryCacheValue>);
}

pub trait ResourceCache: Plugin {
    fn get_services(&self) -> Vec<ServiceKey>;

    fn load_service_rule(&self, filter: Filter) -> Result<ServiceRule, PolarisError>;

    fn load_services(&self, filter: Filter) -> Result<Services, PolarisError>;

    fn load_service_instances(&self, filter: Filter) -> Result<Services, PolarisError>;

    fn load_config_file(&self, filter: Filter) -> Result<ConfigFile, PolarisError>;

    fn load_config_groups(&self, filter: Filter) -> Result<ConfigGroup, PolarisError>;

    fn load_config_group_files(&self, filter: Filter) -> Result<ConfigGroup, PolarisError>;

    fn register_resource_listener(&self, listener: Arc<dyn ResourceListener>);

    fn watch_resource(&self, key: ResourceEventKey);

    fn un_watch_resource(&self, key: ResourceEventKey);
}

pub struct NoopResourceCache {}

impl Default for NoopResourceCache {
    fn default() -> Self {
        Self {}
    }
}

impl Plugin for NoopResourceCache {
    fn init(&mut self, extensions: Arc<Extensions>) {
        todo!()
    }

    fn destroy(&self) {
        todo!()
    }

    fn name(&self) -> String {
        todo!()
    }
}

impl ResourceCache for NoopResourceCache {
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
