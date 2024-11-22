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

use crate::core::{
    config::consumer::ServiceRouterPluginConfig,
    model::{
        error::PolarisError,
        naming::ServiceInstances,
        router::{RouteInfo, RouteResult, RouteState, DEFAULT_ROUTER_METADATA},
    },
    plugin::{
        plugins::Plugin,
        router::{RouteContext, ServiceRouter},
    },
};

pub fn new_service_router(_conf: &ServiceRouterPluginConfig) -> Box<dyn ServiceRouter> {
    Box::new(MetadataRouter {})
}

/// 正常场景: 选出的实例子集不为空, 那么优先返回健康子集, 如果全部不健康则进行全死全活返回不健康子集。
/// <p>
/// 异常场景: 需要根据GetOneInstanceRequest的请求策略进行降级决策
/// <p>
/// 不降级(默认): 返回未找到实例错误
/// 返回所有节点: 优先返回服务下的健康子集, 如果全部不健康则全死全活返回不健康子集
/// 返回实例元数据不包含请求metadata的key的节点: 优先返回筛选出的健康子集, 如果全部不健康则返回不健康子集
/// 例如: ip1 set=1 ; ip2 set=2 ; ip3 ; 请求时 set=0 返回的 ip3 (这个时候只判断key)
/// 降级使用指定metadata进行实例筛选。(未实现)
pub struct MetadataRouter {}

impl MetadataRouter {
    pub fn builder() -> (
        fn(&ServiceRouterPluginConfig) -> Box<dyn ServiceRouter>,
        String,
    ) {
        (new_service_router, DEFAULT_ROUTER_METADATA.to_string())
    }
}

impl Plugin for MetadataRouter {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        DEFAULT_ROUTER_METADATA.to_string()
    }
}

#[async_trait::async_trait]
impl ServiceRouter for MetadataRouter {
    /// choose_instances 实例路由
    async fn choose_instances(
        &self,
        _route_ctx: RouteContext,
        instances: ServiceInstances,
    ) -> Result<RouteResult, PolarisError> {
        Ok(RouteResult {
            instances,
            state: RouteState::Next,
        })
    }

    /// enable 是否启用
    async fn enable(&self, route_info: RouteInfo) -> bool {
        return true;
    }
}
