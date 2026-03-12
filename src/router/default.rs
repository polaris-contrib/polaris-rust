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

use std::{collections::HashMap, sync::Arc};

use super::{api::RouterAPI, req::ProcessRouteResponse};
use crate::core::{
    config::consumer::ServiceRouterConfig,
    context::SDKContext,
    flow::RouterFlow,
    model::{
        error::{ErrorCode, PoleError},
        router::{RouteInfo, RouterChain},
    },
};
use crate::debug;

const DISABLE_DEFAULT_CHAIN_KEY: &str = "polaris.disableDefaultChain";

pub struct DefaultRouterAPI {
    manage_sdk: bool,
    context: Arc<SDKContext>,
    flow: Arc<RouterFlow>,
}

impl DefaultRouterAPI {
    pub fn new_raw(context: SDKContext) -> Self {
        let ctx = Arc::new(context);
        let extensions = ctx.get_engine().get_extensions();
        Self {
            manage_sdk: true,
            context: ctx,
            flow: Arc::new(RouterFlow::new(extensions)),
        }
    }

    pub fn new(context: Arc<SDKContext>) -> Self {
        let extensions = context.get_engine().get_extensions();
        Self {
            manage_sdk: false,
            context: context,
            flow: Arc::new(RouterFlow::new(extensions)),
        }
    }
}

impl Drop for DefaultRouterAPI {
    fn drop(&mut self) {
        if !self.manage_sdk {
            return;
        }
        let ctx = self.context.to_owned();
        let ret = Arc::try_unwrap(ctx);

        match ret {
            Ok(ctx) => {
                drop(ctx);
            }
            Err(_) => {
                // do nothing
            }
        }
    }
}

fn is_empty_router_chain(chain: &RouterChain) -> bool {
    chain.before.is_empty() && chain.core.is_empty() && chain.after.is_empty()
}

fn is_default_chain_disabled(metadata: &HashMap<String, String>) -> bool {
    metadata
        .get(DISABLE_DEFAULT_CHAIN_KEY)
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
}

fn apply_default_router_chain(route_info: &mut RouteInfo, default_chain: &ServiceRouterConfig) {
    if is_default_chain_disabled(&route_info.metadata) || !is_empty_router_chain(&route_info.chain)
    {
        return;
    }

    route_info.chain = RouterChain {
        before: default_chain
            .before_chain
            .iter()
            .map(|plugin| plugin.name.clone())
            .collect(),
        core: default_chain
            .core_chain
            .iter()
            .map(|plugin| plugin.name.clone())
            .collect(),
        after: default_chain
            .after_chain
            .iter()
            .map(|plugin| plugin.name.clone())
            .collect(),
    };
}

#[async_trait::async_trait]
impl RouterAPI for DefaultRouterAPI {
    async fn router(
        &self,
        req: super::req::ProcessRouteRequest,
    ) -> Result<super::req::ProcessRouteResponse, PoleError> {
        debug!("[pole][router_api] route request {:?}", req);

        let mut route_info = req.route_info;
        apply_default_router_chain(&mut route_info, &self.context.conf.consumer.service_router);

        let ret = self
            .flow
            .choose_instances(route_info, req.service_instances)
            .await;

        match ret {
            Ok(result) => Ok(ProcessRouteResponse {
                service_instances: result,
            }),
            Err(e) => Err(e),
        }
    }

    async fn load_balance(
        &self,
        req: super::req::ProcessLoadBalanceRequest,
    ) -> Result<super::req::ProcessLoadBalanceResponse, PoleError> {
        debug!("[pole][router_api] load_balance request {:?}", req);

        let criteria = req.criteria.clone();
        let mut lb_policy = criteria.policy.clone();

        if lb_policy.is_empty() {
            lb_policy.clone_from(&self.context.conf.consumer.load_balancer.default_policy);
        }

        let lb = self.flow.lookup_loadbalancer(&lb_policy).await;

        if lb.is_none() {
            crate::error!("[pole][router_api] load balancer {} not found", lb_policy);
            return Err(PoleError::new(
                ErrorCode::PluginError,
                format!("load balancer {} not found", lb_policy),
            ));
        }

        let lb = lb.unwrap();
        let result = lb.choose_instance(req.criteria, req.service_instances);

        match result {
            Ok(instance) => {
                let manager = self.context.get_engine().get_instance_stat_manager();
                manager.on_instance_selected(&instance.namespace, &instance.service, &instance);
                Ok(super::req::ProcessLoadBalanceResponse { instance })
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::core::{
        config::consumer::{ServiceRouterConfig, ServiceRouterPluginConfig},
        model::router::{RouteInfo, RouterChain},
    };

    use super::{apply_default_router_chain, DISABLE_DEFAULT_CHAIN_KEY};

    fn build_router_config() -> ServiceRouterConfig {
        ServiceRouterConfig {
            before_chain: vec![ServiceRouterPluginConfig {
                name: "isolatedRouter".to_string(),
                options: None,
            }],
            core_chain: vec![
                ServiceRouterPluginConfig {
                    name: "laneRouter".to_string(),
                    options: None,
                },
                ServiceRouterPluginConfig {
                    name: "ruleBasedRouter".to_string(),
                    options: None,
                },
            ],
            after_chain: vec![ServiceRouterPluginConfig {
                name: "recoverRouter".to_string(),
                options: None,
            }],
        }
    }

    #[test]
    fn test_apply_default_router_chain_when_request_chain_empty() {
        let mut route_info = RouteInfo::default();
        let default_chain = build_router_config();

        apply_default_router_chain(&mut route_info, &default_chain);

        assert_eq!(route_info.chain.before, vec!["isolatedRouter".to_string()]);
        assert_eq!(
            route_info.chain.core,
            vec!["laneRouter".to_string(), "ruleBasedRouter".to_string()]
        );
        assert_eq!(route_info.chain.after, vec!["recoverRouter".to_string()]);
    }

    #[test]
    fn test_apply_default_router_chain_skip_when_request_chain_not_empty() {
        let mut route_info = RouteInfo::default();
        route_info.chain = RouterChain {
            before: vec!["customBefore".to_string()],
            core: vec![],
            after: vec![],
        };
        let default_chain = build_router_config();

        apply_default_router_chain(&mut route_info, &default_chain);

        assert_eq!(route_info.chain.before, vec!["customBefore".to_string()]);
        assert!(route_info.chain.core.is_empty());
        assert!(route_info.chain.after.is_empty());
    }

    #[test]
    fn test_apply_default_router_chain_skip_when_disabled_by_metadata() {
        let mut route_info = RouteInfo::default();
        route_info.metadata =
            HashMap::from([(DISABLE_DEFAULT_CHAIN_KEY.to_string(), "TrUe".to_string())]);
        let default_chain = build_router_config();

        apply_default_router_chain(&mut route_info, &default_chain);

        assert!(route_info.chain.before.is_empty());
        assert!(route_info.chain.core.is_empty());
        assert!(route_info.chain.after.is_empty());
    }
}
