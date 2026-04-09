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

use pole_client_rust::core::model::error::PoleError;
use pole_client_rust::ratelimit::req::{QuotaRequest, QuotaResponse};
use std::time::Duration;

// ==================== QuotaRequest Tests ====================

#[test]
fn test_quota_request_valid() {
    let req = QuotaRequest {
        flow_id: "test-flow-1".to_string(),
        timeout: Duration::from_secs(5),
        service: "test-service".to_string(),
        namespace: "test-namespace".to_string(),
        method: "test-method".to_string(),
        traffic_label_provider: |_, _| None,
        external_parameter_supplier: |_| None,
    };

    let result = req.check_valid();
    assert!(result.is_ok());
}

#[test]
fn test_quota_request_empty_service() {
    let req = QuotaRequest {
        flow_id: "test-flow-1".to_string(),
        timeout: Duration::from_secs(5),
        service: "".to_string(),
        namespace: "test-namespace".to_string(),
        method: "test-method".to_string(),
        traffic_label_provider: |_, _| None,
        external_parameter_supplier: |_| None,
    };

    let result = req.check_valid();
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error.to_string().contains("service is empty"));
}

#[test]
fn test_quota_request_empty_namespace() {
    let req = QuotaRequest {
        flow_id: "test-flow-1".to_string(),
        timeout: Duration::from_secs(5),
        service: "test-service".to_string(),
        namespace: "".to_string(),
        method: "test-method".to_string(),
        traffic_label_provider: |_, _| None,
        external_parameter_supplier: |_| None,
    };

    let result = req.check_valid();
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error.to_string().contains("namespace is empty"));
}

#[test]
fn test_quota_request_both_empty() {
    let req = QuotaRequest {
        flow_id: "test-flow-1".to_string(),
        timeout: Duration::from_secs(5),
        service: "".to_string(),
        namespace: "".to_string(),
        method: "test-method".to_string(),
        traffic_label_provider: |_, _| None,
        external_parameter_supplier: |_| None,
    };

    let result = req.check_valid();
    assert!(result.is_err());

    let error = result.unwrap_err();
    // Should fail on service first
    assert!(error.to_string().contains("service is empty"));
}

use pole_client_rust::core::model::ArgumentType;

#[test]
fn test_quota_request_with_labels() {
    let req = QuotaRequest {
        flow_id: "test-flow-1".to_string(),
        timeout: Duration::from_secs(5),
        service: "test-service".to_string(),
        namespace: "test-namespace".to_string(),
        method: "test-method".to_string(),
        traffic_label_provider: |arg_type, key| {
            if arg_type == ArgumentType::Header && key == "x-user-id" {
                Some("user123".to_string())
            } else {
                None
            }
        },
        external_parameter_supplier: |key| {
            if key == "remote_addr" {
                Some("192.168.1.1".to_string())
            } else {
                None
            }
        },
    };

    let result = req.check_valid();
    assert!(result.is_ok());

    // Test traffic_label_provider
    let label = (req.traffic_label_provider)(ArgumentType::Header, "x-user-id");
    assert_eq!(label, Some("user123".to_string()));

    // Test external_parameter_supplier
    let param = (req.external_parameter_supplier)("remote_addr");
    assert_eq!(param, Some("192.168.1.1".to_string()));
}

#[test]
fn test_quota_request_timeout() {
    let req = QuotaRequest {
        flow_id: "test-flow-1".to_string(),
        timeout: Duration::from_millis(500),
        service: "test-service".to_string(),
        namespace: "test-namespace".to_string(),
        method: "test-method".to_string(),
        traffic_label_provider: |_, _| None,
        external_parameter_supplier: |_| None,
    };

    assert_eq!(req.timeout, Duration::from_millis(500));
}

#[test]
fn test_quota_request_debug() {
    let req = QuotaRequest {
        flow_id: "test-flow-1".to_string(),
        timeout: Duration::from_secs(5),
        service: "test-service".to_string(),
        namespace: "test-namespace".to_string(),
        method: "test-method".to_string(),
        traffic_label_provider: |_, _| None,
        external_parameter_supplier: |_| None,
    };

    let debug_str = format!("{:?}", req);
    assert!(debug_str.contains("QuotaRequest"));
    assert!(debug_str.contains("test-service"));
    assert!(debug_str.contains("test-namespace"));
}

#[test]
fn test_quota_request_clone() {
    let req1 = QuotaRequest {
        flow_id: "test-flow-1".to_string(),
        timeout: Duration::from_secs(5),
        service: "test-service".to_string(),
        namespace: "test-namespace".to_string(),
        method: "test-method".to_string(),
        traffic_label_provider: |_, _| None,
        external_parameter_supplier: |_| None,
    };

    let req2 = req1.clone();

    assert_eq!(req1.flow_id, req2.flow_id);
    assert_eq!(req1.service, req2.service);
    assert_eq!(req1.namespace, req2.namespace);
    assert_eq!(req1.method, req2.method);
}

// ==================== QuotaResponse Tests ====================

#[test]
fn test_quota_response_allowed() {
    let resp = QuotaResponse {
        allowed: true,
        message: "OK".to_string(),
    };

    assert!(resp.allowed);
    assert_eq!(resp.message, "OK");
}

#[test]
fn test_quota_response_rejected() {
    let resp = QuotaResponse {
        allowed: false,
        message: "Rate limit exceeded".to_string(),
    };

    assert!(!resp.allowed);
    assert_eq!(resp.message, "Rate limit exceeded");
}

#[test]
fn test_quota_response_message() {
    let resp = QuotaResponse {
        allowed: true,
        message: "".to_string(),
    };

    assert!(resp.message.is_empty());
}

#[test]
fn test_quota_response_clone() {
    let resp1 = QuotaResponse {
        allowed: true,
        message: "OK".to_string(),
    };

    let resp2 = resp1.clone();

    assert_eq!(resp1.allowed, resp2.allowed);
    assert_eq!(resp1.message, resp2.message);
}

#[test]
fn test_quota_response_debug() {
    let resp = QuotaResponse {
        allowed: true,
        message: "OK".to_string(),
    };

    let debug_str = format!("{:?}", resp);
    assert!(debug_str.contains("QuotaResponse"));
    assert!(debug_str.contains("allowed"));
    assert!(debug_str.contains("OK"));
}

// ==================== Boundary Tests ====================

#[test]
fn test_quota_request_very_long_service_name() {
    let long_service = "a".repeat(1000);
    let req = QuotaRequest {
        flow_id: "test-flow-1".to_string(),
        timeout: Duration::from_secs(5),
        service: long_service,
        namespace: "test-namespace".to_string(),
        method: "test-method".to_string(),
        traffic_label_provider: |_, _| None,
        external_parameter_supplier: |_| None,
    };

    let result = req.check_valid();
    assert!(result.is_ok());
}

#[test]
fn test_quota_request_zero_timeout() {
    let req = QuotaRequest {
        flow_id: "test-flow-1".to_string(),
        timeout: Duration::from_secs(0),
        service: "test-service".to_string(),
        namespace: "test-namespace".to_string(),
        method: "test-method".to_string(),
        traffic_label_provider: |_, _| None,
        external_parameter_supplier: |_| None,
    };

    let result = req.check_valid();
    assert!(result.is_ok());
}

#[test]
fn test_quota_request_special_characters_in_method() {
    let req = QuotaRequest {
        flow_id: "test-flow-1".to_string(),
        timeout: Duration::from_secs(5),
        service: "test-service".to_string(),
        namespace: "test-namespace".to_string(),
        method: "/api/v1/users/{id}/posts".to_string(),
        traffic_label_provider: |_, _| None,
        external_parameter_supplier: |_| None,
    };

    let result = req.check_valid();
    assert!(result.is_ok());
}
