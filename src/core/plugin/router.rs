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

use std::{collections::HashMap, sync::Arc};

use crate::core::{
    model::{
        error::PolarisError,
        naming::ServiceInstances,
        router::{RouteInfo, RouteResult},
    },
    plugin::plugins::Plugin,
};

use super::plugins::Extensions;

#[derive(Clone)]
pub struct RouterContainer {
    pub before_routers: HashMap<String, Arc<Box<dyn ServiceRouter>>>,
    pub core_routers: HashMap<String, Arc<Box<dyn ServiceRouter>>>,
    pub after_routers: HashMap<String, Arc<Box<dyn ServiceRouter>>>,
}

impl RouterContainer {
    pub fn new() -> Self {
        RouterContainer {
            before_routers: HashMap::new(),
            core_routers: HashMap::new(),
            after_routers: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct RouteContext {
    pub route_info: RouteInfo,
    pub extensions: Option<Arc<Extensions>>,
}

#[async_trait::async_trait]
pub trait ServiceRouter: Plugin {
    /// choose_instances 实例路由
    async fn choose_instances(
        &self,
        route_info: RouteContext,
        instances: ServiceInstances,
    ) -> Result<RouteResult, PolarisError>;

    /// enable 是否启用
    async fn enable(&self, route_info: RouteContext, instances: ServiceInstances) -> bool;
}
