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

use once_cell::sync::Lazy;
use std::collections::HashMap;

use crate::core::{
    config::consumer::ServiceRouterPluginConfig,
    model::{
        error::{ErrorCode, PolarisError},
        naming::{Instance, Location, ServiceInstances},
        router::{RouteResult, RouteState, DEFAULT_ROUTER_NEARBY},
    },
    plugin::{
        location::LocationSupplier,
        plugins::Plugin,
        router::{RouteContext, ServiceRouter},
    },
};

static KEY_METADATA_NEARBY: &str = "internal-enable-nearby";
static DEFAULT_NEARBY_MATCH_LEVEL: &str = "zone";
static DEFAULT_NEARBY_MAX_MATCH_LEVEL: &str = "all";
static MATCH_LEVEL: Lazy<HashMap<String, u16>> = Lazy::new(|| {
    [
        ("unknown".to_string(), 0),
        ("campus".to_string(), 1),
        ("zone".to_string(), 2),
        ("region".to_string(), 3),
        ("all".to_string(), 4),
    ]
    .iter()
    .cloned()
    .collect()
});

static ORDER_MATCH_LEVEL: Lazy<HashMap<u16, String>> = Lazy::new(|| {
    [
        (0, "unknown".to_string()),
        (1, "campus".to_string()),
        (2, "zone".to_string()),
        (3, "region".to_string()),
        (4, "all".to_string()),
    ]
    .iter()
    .cloned()
    .collect()
});

pub fn new_service_router(conf: &ServiceRouterPluginConfig) -> Box<dyn ServiceRouter> {
    if conf.options.is_none() {
        // 默认就近区域：默认城市 matchLevel: zone # 最大就近区域，默认为空（全匹配） maxMatchLevel: all #
        // 假如开启了严格就近，插件的初始化会等待地域信息获取成功才返回，假如获取失败（server获取失败或者IP地域信息缺失），则会初始化失败，而且必须按照 strictNearby: false #
        // 是否启用按服务不健康实例比例进行降级 enableDegradeByUnhealthyPercent: true，假如不启用，则不会降级#
        // 需要进行降级的实例比例，不健康实例达到百分之多少才进行降级。值(0, 100]。 # 默认100，即全部不健康才进行切换。
        return Box::new(NearbyRouter {
            strict_nearby: false,
            match_level: "zone".to_string(),
            max_match_level: "all".to_string(),
            enable_degrade_unhealthy_percent: false,
            unhealthy_percent_to_degrade: 100,
        });
    }
    // #描述: 就近路由的最小匹配级别。region(大区)、zone(区域)、campus(园区)
    // matchLevel: zone
    // #描述: 最大匹配级别
    // maxMatchLevel: all
    // #描述: 强制就近
    // strictNearby: false
    // #描述: 全部实例不健康时是否降级其他地域
    // enableDegradeByUnhealthyPercent: false
    // #描述: 达到降级标准的不健康实例百分比
    // unhealthyPercentToDegrade: 100
    // #描述: 是否通过上报方式获取地域信息
    // enableReportLocalAddress: false
    let options = conf.options.clone().unwrap();
    Box::new(NearbyRouter {
        strict_nearby: options
            .get("strictNearby")
            .unwrap()
            .parse()
            .expect("strictNearby must be a boolean"),
        match_level: options.get("matchLevel").unwrap().to_string(),
        max_match_level: options.get("maxMatchLevel").unwrap().to_string(),
        enable_degrade_unhealthy_percent: options
            .get("enableDegradeByUnhealthyPercent")
            .unwrap()
            .parse()
            .expect("enableDegradeByUnhealthyPercent must be a boolean"),
        unhealthy_percent_to_degrade: options
            .get("unhealthyPercentToDegrade")
            .unwrap()
            .parse()
            .expect("unhealthyPercentToDegrade must be a number, range [0, 100]"),
    })
}

pub struct NearbyRouter {
    pub strict_nearby: bool,
    pub match_level: String,
    pub max_match_level: String,
    pub enable_degrade_unhealthy_percent: bool,
    pub unhealthy_percent_to_degrade: u16,
}

impl NearbyRouter {
    pub fn builder() -> (
        fn(&ServiceRouterPluginConfig) -> Box<dyn ServiceRouter>,
        String,
    ) {
        (new_service_router, DEFAULT_ROUTER_NEARBY.to_string())
    }

    fn select_instances(
        &self,
        local_loc: Location,
        match_level: &str,
        instances: &ServiceInstances,
    ) -> (ServiceInstances, u32) {
        let mut ret = Vec::<Instance>::with_capacity(instances.instances.len());

        let mut total_weight: u64 = 0;
        let mut health_ins_cnt = 0 as u32;
        for (_, ins) in instances.instances.iter().enumerate() {
            if ins.health {
                health_ins_cnt += 1;
            }
            match match_level {
                "campus" => {
                    if local_loc.campus == "" || ins.location.campus == local_loc.campus {
                        total_weight += ins.weight as u64;
                        ret.push(ins.clone());
                    }
                }
                "zone" => {
                    if local_loc.zone == "" || ins.location.zone == local_loc.zone {
                        total_weight += ins.weight as u64;
                        ret.push(ins.clone());
                    }
                }
                "region" => {
                    if local_loc.region == "" || ins.location.region == local_loc.region {
                        total_weight += ins.weight as u64;
                        ret.push(ins.clone());
                    }
                }
                _ => {
                    total_weight += ins.weight as u64;
                    ret.push(ins.clone());
                }
            }
        }

        (
            ServiceInstances {
                instances: ret,
                service: instances.service.clone(),
                total_weight: total_weight,
            },
            health_ins_cnt,
        )
    }
}

impl Plugin for NearbyRouter {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        DEFAULT_ROUTER_NEARBY.to_string()
    }
}

#[async_trait::async_trait]
impl ServiceRouter for NearbyRouter {
    /// choose_instances 实例路由
    async fn choose_instances(
        &self,
        route_info: RouteContext,
        instances: ServiceInstances,
    ) -> Result<RouteResult, PolarisError> {
        let mut min_available_level = self.match_level.clone();
        if min_available_level.is_empty() {
            min_available_level = DEFAULT_NEARBY_MATCH_LEVEL.to_string();
        }
        let mut max_match_level = self.max_match_level.clone();
        if max_match_level.is_empty() {
            max_match_level = DEFAULT_NEARBY_MAX_MATCH_LEVEL.to_string();
        }

        let locatin_provider = route_info
            .extensions
            .clone()
            .unwrap()
            .get_location_provider();

        let location = locatin_provider.get_location();

        if grater_match_level(min_available_level.as_str(), max_match_level.as_str()) {
            let (ret_ins, _health_cnt) =
                self.select_instances(location.clone(), min_available_level.as_str(), &instances);
            if ret_ins.instances.is_empty() {
                return Err(PolarisError::new(ErrorCode::LocationMismatch, format!("")));
            }
            return Ok(RouteResult {
                instances: ret_ins,
                state: RouteState::Next,
            });
        }

        let min_level_ord = MATCH_LEVEL
            .get(min_available_level.as_str())
            .unwrap()
            .clone();
        let max_level_ord = MATCH_LEVEL.get(max_match_level.as_str()).unwrap().clone();

        let mut cur_level = min_available_level.clone();
        let mut ret_ins: Option<ServiceInstances> = None;
        for i in min_level_ord..=max_level_ord {
            let cur_match_level = ORDER_MATCH_LEVEL.get(&i).unwrap();
            let (tmp_ins, health_cnt) =
                self.select_instances(location.clone(), cur_match_level, &instances);
            cur_level = cur_match_level.to_string().clone();
            if !tmp_ins.instances.is_empty() {
                ret_ins = Some(tmp_ins);
            } else {
                min_available_level = cur_match_level.to_string().clone();
            }
        }

        if ret_ins.is_none() {
            return Err(PolarisError::new(
                ErrorCode::LocationMismatch,
                format!("can not find any instance by level {}", cur_level),
            ));
        }

        // if !self.enable_degrade_unhealthy_percent
        //     || cur_level == DEFAULT_NEARBY_MAX_MATCH_LEVEL.to_string()
        // {
        //     return Ok(RouteResult {
        //         instances: ret_ins.unwrap(),
        //         state: RouteState::Next,
        //     });
        // }

        //TODO 需要做健康降级检查判断

        return Ok(RouteResult {
            instances: ret_ins.unwrap(),
            state: RouteState::Next,
        });
    }

    /// enable 是否启用
    async fn enable(&self, route_info: RouteContext, instances: ServiceInstances) -> bool {
        let svc_info = instances.service.clone();
        let meta_val = svc_info.metadata.get(KEY_METADATA_NEARBY);
        if meta_val.is_none() {
            return false;
        }
        return meta_val.unwrap() != "true";
    }
}

fn grater_match_level(a: &str, b: &str) -> bool {
    let a_level = MATCH_LEVEL.get(a).unwrap();
    let b_level = MATCH_LEVEL.get(b).unwrap();
    a_level > b_level
}
