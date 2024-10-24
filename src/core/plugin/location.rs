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

use crate::{
    core::{
        config::global::LocationConfig,
        model::{
            error::{ErrorCode, PolarisError},
            naming::Location,
        },
    },
    plugins::location::{
        local::local::LocalLocationSupplier, remotehttp::remotehttp::RemoteHttpLocationSupplier,
    },
};

use super::plugins::Plugin;

/// LocationProvider 获取位置信息
pub trait LocationSupplier: Plugin {
    // get_location 获取位置信息
    fn get_location(&self) -> Location;
}

pub enum LocationType {
    Local,
    Http,
    Service,
}

impl LocationType {
    pub fn parse(name: &str) -> LocationType {
        match name {
            "local" => LocationType::Local,
            "http" => LocationType::Http,
            "service" => LocationType::Service,
            _ => LocationType::Local,
        }
    }
}

/// LocationProvider 位置信息提供者, 位置获取优先顺序 本地 > http
pub struct LocationProvider {
    chain: Vec<Box<dyn LocationSupplier>>,
}

pub fn new_location_provider(opt: &LocationConfig) -> Result<LocationProvider, PolarisError> {
    let mut chain = Vec::<Box<dyn LocationSupplier>>::new();
    let providers = opt.clone().providers;
    if providers.is_none() {
        return Err(PolarisError::new(
            ErrorCode::ApiInvalidArgument,
            "".to_string(),
        ));
    }

    providers.unwrap().iter().for_each(|provider| {
        let name: String = provider.name.clone();
        match LocationType::parse(name.as_str()) {
            LocationType::Local => {
                chain.push(Box::new(LocalLocationSupplier::new(provider.clone())));
            }
            LocationType::Http => {
                chain.push(Box::new(RemoteHttpLocationSupplier::new(provider.clone())));
            }
            LocationType::Service => {}
        }
    });

    Ok(LocationProvider { chain })
}

impl Plugin for LocationProvider {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        "chain".to_string()
    }
}

impl LocationSupplier for LocationProvider {
    fn get_location(&self) -> Location {
        for supplier in self.chain.iter() {
            let loc = supplier.get_location();
            if !loc.is_empty() {
                return loc;
            }
        }
        Location::default()
    }
}
