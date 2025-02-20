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

use std::sync::Arc;

use crate::core::{context::SDKContext, flow::RatelimitFlow, model::error::PolarisError};

use super::{
    api::RateLimitAPI,
    req::{QuotaRequest, QuotaResponse},
};

pub struct DefaultRateLimitAPI {
    manage_sdk: bool,
    context: Arc<SDKContext>,
    flow: Arc<RatelimitFlow>,
}

impl DefaultRateLimitAPI {
    pub fn new_raw(context: SDKContext) -> Self {
        let ctx = Arc::new(context);
        let extensions = ctx.get_engine().get_extensions();
        Self {
            manage_sdk: true,
            context: ctx,
            flow: Arc::new(RatelimitFlow::new(extensions)),
        }
    }

    pub fn new(context: Arc<SDKContext>) -> Self {
        let extensions = context.get_engine().get_extensions();
        Self {
            manage_sdk: false,
            context: context,
            flow: Arc::new(RatelimitFlow::new(extensions)),
        }
    }
}

impl Drop for DefaultRateLimitAPI {
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

#[async_trait::async_trait]
impl RateLimitAPI for DefaultRateLimitAPI {
    async fn get_quota(&self, req: QuotaRequest) -> Result<QuotaResponse, PolarisError> {
        let check_ret = req.check_valid();
        check_ret?;

        todo!()
    }
}
