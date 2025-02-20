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

use crate::core::{
    config::consumer::ServiceRouterPluginConfig,
    model::{
        error::PolarisError,
        naming::{Instance, ServiceInfo, ServiceInstances},
        router::{
            MetadataFailoverType, RouteInfo, RouteResult, RouteState, DEFAULT_ROUTER_METADATA,
        },
    },
    plugin::{
        plugins::Plugin,
        router::{RouteContext, ServiceRouter},
    },
};

pub fn new_service_router(_conf: &ServiceRouterPluginConfig) -> Box<dyn ServiceRouter> {
    Box::new(MetadataRouter {})
}

static KEY_METADATA_FAILOVER: &str = "internal-metadata-failover-type";

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
        route_ctx: RouteContext,
        instances: ServiceInstances,
    ) -> Result<RouteResult, PolarisError> {
        let mut failover_type = route_ctx.route_info.metadata_failover.clone();
        let svc_info = instances.service.clone();
        if let Some(_custom_failover_type) = svc_info.metadata.get(KEY_METADATA_FAILOVER) {
            failover_type = paese_custom_failover_type(_custom_failover_type);
        }
        let req_meta = route_ctx.route_info.metadata.clone();
        let mut ret = Vec::<Instance>::with_capacity(instances.instances.len());

        let mut total_weight = 0 as u64;

        for instance in instances.instances.iter() {
            let mut match_count = 0;
            for (key, value) in req_meta.iter() {
                if let Some(v) = instance.metadata.get(key) {
                    if v == value {
                        match_count += 1;
                    } else {
                        break;
                    }
                }
            }
            if match_count != 0 && match_count == req_meta.len() {
                total_weight += instance.weight as u64;
                ret.push(instance.clone());
            }
        }

        if !ret.is_empty() {
            return Ok(RouteResult {
                instances: ServiceInstances {
                    service: instances.service.clone(),
                    instances: ret,
                    total_weight: total_weight,
                },
                state: RouteState::Next,
            });
        }

        total_weight = 0;

        match failover_type {
            MetadataFailoverType::MetadataFailoverAll => {
                for instance in instances.instances.iter() {
                    total_weight += instance.weight as u64;
                    ret.push(instance.clone());
                }
            }
            MetadataFailoverType::MetadataFailoverNoKey => {
                for instance in instances.instances.iter() {
                    let mut exist_meta = !instance.metadata.is_empty();
                    for (key, _value) in req_meta.iter() {
                        if instance.metadata.contains_key(key) {
                            exist_meta = true;
                            break;
                        }
                    }
                    if !exist_meta {
                        total_weight += instance.weight as u64;
                        ret.push(instance.clone());
                    }
                }
            }
            _ => {
                return Err(PolarisError::new(
                    crate::core::model::error::ErrorCode::MetadataMismatch,
                    format!(
                        "can not find any instance by service namespace({}) name({})",
                        svc_info.namespace, svc_info.name
                    ),
                ));
            }
        }

        Ok(RouteResult {
            instances: ServiceInstances {
                service: instances.service.clone(),
                instances: ret,
                total_weight: total_weight,
            },
            state: RouteState::Next,
        })
    }

    /// enable 是否启用
    async fn enable(&self, route_info: RouteContext, instances: ServiceInstances) -> bool {
        return true;
    }
}

fn paese_custom_failover_type(v: &str) -> MetadataFailoverType {
    match v {
        "none" => MetadataFailoverType::MetadataFailoverNone,
        "all" => MetadataFailoverType::MetadataFailoverAll,
        "others" => MetadataFailoverType::MetadataFailoverNoKey,
        _ => MetadataFailoverType::MetadataFailoverNone,
    }
}

#[cfg(test)]
mod tests {
    use crate::core::plugin::plugins::Extensions;
    use std::sync::Arc;
    use std::sync::Once;

    use super::*;

    use tracing::metadata::LevelFilter;
    use crate::info;

    static LOGGER_INIT: Once = Once::new();

    pub(crate) fn setup_log() {
        LOGGER_INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_thread_names(true)
                .with_file(true)
                .with_level(true)
                .with_line_number(true)
                .with_thread_ids(true)
                .with_max_level(LevelFilter::DEBUG)
                .init()
        });
    }

    #[tokio::test]
    async fn test_choose_instances_no_failover() {
        setup_log();
        let router = MetadataRouter {};
        let route_ctx = RouteContext {
            route_info: RouteInfo {
                metadata_failover: MetadataFailoverType::MetadataFailoverNone,
                ..Default::default()
            },
            extensions: None,
        };
        let instances = ServiceInstances {
            service: ServiceInfo {
                namespace: "default".to_string(),
                name: "test_service".to_string(),
                metadata: HashMap::new(),
                id: "".to_string(),
                revision: "".to_string(),
            },
            instances: vec![Instance {
                metadata: HashMap::new(),
                weight: 1,
                ..Default::default()
            }],
            total_weight: 1,
        };

        let result = router.choose_instances(route_ctx, instances).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_choose_instances_failover_all() {
        setup_log();
        let route_ctx = RouteContext {
            route_info: RouteInfo {
                metadata_failover: MetadataFailoverType::MetadataFailoverAll,
                metadata: [("key1".to_string(), "value1".to_string())]
                    .iter()
                    .cloned()
                    .collect(),
                ..Default::default()
            },
            extensions: None,
        };
        let instances = ServiceInstances {
            service: ServiceInfo {
                namespace: "default".to_string(),
                name: "test_service".to_string(),
                metadata: HashMap::new(),
                id: "".to_string(),
                revision: "".to_string(),
            },
            instances: vec![Instance {
                metadata: [("key2".to_string(), "value2".to_string())]
                    .iter()
                    .cloned()
                    .collect(),
                weight: 1,
                ..Default::default()
            }],
            total_weight: 1,
        };

        let result = MetadataRouter {}
            .choose_instances(route_ctx, instances)
            .await;
        assert!(result.is_ok());
        let route_result = result.unwrap();
        assert_eq!(route_result.instances.instances.len(), 1);
    }

    #[tokio::test]
    async fn test_choose_instances_failover_no_key() {
        setup_log();
        let router = MetadataRouter {};
        let route_ctx = RouteContext {
            route_info: RouteInfo {
                metadata_failover: MetadataFailoverType::MetadataFailoverNoKey,
                metadata: [("key1".to_string(), "value1".to_string())]
                    .iter()
                    .cloned()
                    .collect(),
                ..Default::default()
            },
            extensions: None,
        };
        let instances = ServiceInstances {
            service: ServiceInfo {
                namespace: "default".to_string(),
                name: "test_service".to_string(),
                metadata: HashMap::new(),
                id: "".to_string(),
                revision: "".to_string(),
            },
            instances: vec![
                Instance {
                    metadata: [("key1".to_string(), "value2".to_string())]
                        .iter()
                        .cloned()
                        .collect(),
                    weight: 1,
                    ..Default::default()
                },
                Instance {
                    metadata: HashMap::new(),
                    weight: 1,
                    ..Default::default()
                },
            ],
            total_weight: 2,
        };

        let result = router.choose_instances(route_ctx, instances).await;
        assert!(result.is_ok());
        let route_result = result.unwrap();
        assert_eq!(route_result.instances.instances.len(), 1);
        info!("{:?}", route_result.instances.instances[0]);
        assert!(route_result.instances.instances[0].metadata.is_empty());
    }
}
