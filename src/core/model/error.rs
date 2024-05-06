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

use std::fmt;
use std::fmt::Display;

#[derive(Debug)]
pub enum ErrorCode {
    Success = 0,
    ApiInvalidArgument = 1001,
    InvalidConfig = 1002,
    PluginError = 1003,
    ApiTimeout = 1004,
    InvalidState = 1005,
    ServerUserError = 1006,
    NetworkError = 1007,
    CircuitBreakError = 1008,
    InstanceInfoError = 1009,
    InstanceNotFound = 1010,
    InvalidRule = 1011,
    RouteRuleNotMatch = 1012,
    InvalidResponse = 1013,
    InternalError = 1014,
    ServiceNotFound = 1015,
    ServerException = 1016,
    LocationNotFound = 1017,
    LocationMismatch = 1018,
    MetadataMismatch = 1019,
    ClientCircuitBreaking = 1020,
    ConnectError = 2001,
    ServerError = 2002,
    RpcError = 2003,
    RpcTimeout = 2004,
    InvalidServerResponse = 2005,
    InvalidRequest = 2006,
    UNAUTHORIZED = 2007,
    RequestLimit = 2008,
    CmdbNotFound = 2009,
    UnknownServerError = 2100,
    NotSupport = 20010,
    RsaKeyGenerateError = 30001,
    RsaEncryptError = 30002,
    RsaDecryptError = 30003,
    AesKeyGenerateError = 30004,
    AesEncryptError = 30005,
    AesDecryptError = 30006,
    ParameterError = 40000,
}

impl Default for ErrorCode {
    fn default() -> Self {
        Self::UNAUTHORIZED
    }
}

#[derive(Debug)]
pub struct PolarisError {
    err_msg: String,
    err_code: ErrorCode,
}

impl Display for PolarisError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl PolarisError {
    pub fn new(code: ErrorCode, err_msg: String) -> Self {
        PolarisError {
            err_msg,
            err_code: code,
        }
    }
}
