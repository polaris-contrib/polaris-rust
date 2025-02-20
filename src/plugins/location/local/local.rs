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
    config::global::LocationProviderConfig,
    model::naming::Location,
    plugin::{location::LocationSupplier, plugins::Plugin},
};

static PLUGIN_NAME: &str = "local";

pub struct LocalLocationSupplier {
    loc_cache: Location,
}

impl LocalLocationSupplier {
    pub fn new(opt: LocationProviderConfig) -> Self {
        let options = opt.options;

        let region = options.get("region");
        let zone = options.get("zone");
        let campus = options.get("campus");

        let mut loc_ret = Location::default();
        if region.is_some() {
            loc_ret.region = region.unwrap().to_string();
        }
        if zone.is_some() {
            loc_ret.zone = zone.unwrap().to_string();
        }
        if campus.is_some() {
            loc_ret.campus = campus.unwrap().to_string();
        }
        Self { loc_cache: loc_ret }
    }
}

impl Plugin for LocalLocationSupplier {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        PLUGIN_NAME.to_string()
    }
}

impl LocationSupplier for LocalLocationSupplier {
    fn get_location(&self) -> crate::core::model::naming::Location {
        self.loc_cache.clone()
    }
}
