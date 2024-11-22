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

use crate::core::model::loadbalance::Criteria;
use crate::core::model::naming::{Instance, ServiceInstances};
use crate::core::model::router::RouteInfo;

// 负载均衡相关请求

pub struct ProcessLoadBalanceRequest {
    pub service_instances: ServiceInstances,
    pub criteria: Criteria,
}

pub struct ProcessLoadBalanceResponse {
    pub instance: Instance,
}

// 路由相关请求

pub struct ProcessRouteRequest {
    pub service_instances: ServiceInstances,
    pub route_info: RouteInfo,
}

pub struct ProcessRouteResponse {
    pub service_instances: ServiceInstances,
}
