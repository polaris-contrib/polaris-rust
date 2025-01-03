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

use std::time::Duration;

use crate::core::model::{error::{ErrorCode, PolarisError}, ArgumentType};

/// QuotaRequest 获取请求配额
#[derive(Clone, Debug)]
pub struct QuotaRequest {
    pub flow_id: String,
    pub timeout: Duration,
    // service 服务名
    pub service: String,
    // namespace 命名空间
    pub namespace: String,
    // method 方法名
    pub method: String,
    // traffic_label_provider 流量标签提供者
    pub traffic_label_provider: fn(ArgumentType, &str) -> Option<String>,
    // 北极星内部治理规则执行时，会识别规则中的参数来源类别，如果发现规则中的参数来源指定为外部数据源时，会调用本接口进行获取
    pub external_parameter_supplier: fn(&str) -> Option<String>,
}

impl QuotaRequest {
    pub fn check_valid(&self) -> Result<(), PolarisError> {
        if self.service.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::ApiInvalidArgument,
                "service is empty".to_string(),
            ));
        }

        if self.namespace.is_empty() {
            return Err(PolarisError::new(
                ErrorCode::ApiInvalidArgument,
                "namespace is empty".to_string(),
            ));
        }
        Ok(())
    }
}


/// QuotaResponse 配额响应
#[derive(Clone, Debug)]
pub struct QuotaResponse {
    pub allowed: bool,
    pub message: String,
}
