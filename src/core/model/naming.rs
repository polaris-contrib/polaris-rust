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

use std::iter::Map;

pub struct ServiceKey {
    pub namespace: String,
    pub name: String,
}

pub struct ServiceInfo {
    pub id: String,
    pub namespace: String,
    pub name: String,
    pub metadata: Map<String, String>,
    pub revision: String,
}

pub struct ServiceInstances {
    pub service: ServiceInfo,
    pub instances: Vec<Instance>,
}
pub struct Instance {
    pub id: String,
    pub namespace: String,
    pub service: String,
    pub ip: String,
    pub port: u32,
    pub vpc_id: String,
    pub version: String,
    pub health: bool,
    pub isolated: bool,
    pub weight: u32,
    pub priority: u32,
    pub metadata: Map<String, String>,
    pub location: Location,
    pub revision: String,
}

pub struct Location {
    pub region: String,
    pub zone: String,
    pub campus: String,
}
