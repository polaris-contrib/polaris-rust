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

use reqwest::blocking::Client;

pub struct RemoteHttpLocationSupplier {
    opt: LocationProviderConfig,
    access_url: Location,
}

impl RemoteHttpLocationSupplier {
    pub fn new(opt: LocationProviderConfig) -> Self {
        let copy_opt = opt.clone();

        let options = copy_opt.options;

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

        Self {
            opt,
            access_url: loc_ret,
        }
    }
}

impl Plugin for RemoteHttpLocationSupplier {
    fn init(&mut self) {}

    fn destroy(&self) {}

    fn name(&self) -> String {
        "remotehttp".to_string()
    }
}

impl LocationSupplier for RemoteHttpLocationSupplier {
    fn get_location(&self) -> crate::core::model::naming::Location {
        let access_url = &self.access_url;
        let region = RemoteHttpLocationSupplier::get_http_response(
            access_url.region.to_string(),
            "region".to_string(),
        );
        let zone = RemoteHttpLocationSupplier::get_http_response(
            access_url.zone.to_string(),
            "zone".to_string(),
        );
        let campus = RemoteHttpLocationSupplier::get_http_response(
            access_url.campus.to_string(),
            "campus".to_string(),
        );

        if region.is_empty() && zone.is_empty() && campus.is_empty() {
            tracing::error!("get location from remote http: all location is empty")
        }

        Location {
            region,
            zone,
            campus,
        }
    }
}

impl RemoteHttpLocationSupplier {
    fn get_http_response(url: String, label: String) -> String {
        let client = Client::new();
        let response = client.get(url).send();
        match response {
            Ok(res) => {
                let ret = res.text();
                match ret {
                    Ok(body) => body,
                    Err(e) => {
                        tracing::error!("get http response error: {}, label: {}", e, label);
                        "".to_string()
                    }
                }
            }
            Err(e) => {
                tracing::error!("get http response error: {}, label: {}", e, label);
                "".to_string()
            }
        }
    }
}
