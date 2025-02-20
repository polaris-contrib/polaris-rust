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

use crate::core::{context::SDKContext, model::error::PolarisError};

use super::{
    default::DefaultRateLimitAPI,
    req::{QuotaRequest, QuotaResponse},
};

/// new_ratelimit_api
pub fn new_ratelimit_api() -> Result<impl RateLimitAPI, PolarisError> {
    let context_ret = SDKContext::default();
    if context_ret.is_err() {
        return Err(context_ret.err().unwrap());
    }

    Ok(DefaultRateLimitAPI::new_raw(context_ret.unwrap()))
}

/// new_ratelimit_api_by_context
pub fn new_ratelimit_api_by_context(
    context: Arc<SDKContext>,
) -> Result<impl RateLimitAPI, PolarisError> {
    Ok(DefaultRateLimitAPI::new(context))
}

#[async_trait::async_trait]
pub trait RateLimitAPI
where
    Self: Send + Sync,
{
    async fn get_quota(&self, req: QuotaRequest) -> Result<QuotaResponse, PolarisError>;
}
