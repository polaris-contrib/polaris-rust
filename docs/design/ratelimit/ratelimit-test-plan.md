# Ratelimit 模块测试设计

## 概述

本文档描述了 Ratelimit 模块的测试设计方案，覆盖单元测试、集成测试和性能测试。

## 模块架构分析

### 核心组件

1. **限流器插件** (`src/plugins/ratelimit/`)
   - `ConcurrencyLimiter`: 并发限流器
   - `UnirateLimiter`: 单一速率限流器
   - `RejectLimiter`: 拒绝策略限流器

2. **API 层** (`src/ratelimit/`)
   - `RateLimitAPI`: 限流 API trait
   - `DefaultRateLimitAPI`: 默认实现

3. **流程层** (`src/core/flow.rs`)
   - `RatelimitFlow`: 限流核心流程

4. **数据模型** (`src/core/model/ratelimit.rs`)
   - `LimiterConfig`: 限流器配置
   - `WindowConfig`: 窗口配置
   - `QuotaConfig`: 配额配置
   - `ResponseConfig`: 响应配置
   - `RemoteUpdate`: 远程更新类型
   - `QuotaSyncData`: 配额同步数据

---

## 1. 单元测试

### 1.1 限流器插件测试

#### 1.1.1 ConcurrencyLimiter 并发限流器

**测试文件**: `tests/ratelimit/concurrency_limiter_test.rs`

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_concurrency_limiter_allocate_quota` | 测试配额分配 | 返回 `Ok(true)` |
| `test_concurrency_limiter_return_quota` | 测试配额归还 | 返回 `Ok(true)` |
| `test_concurrency_limiter_name` | 测试插件名称 | 返回 "concurrency" |
| `test_concurrency_limiter_plugin_trait` | 测试 Plugin trait 实现 | init/destroy 正常执行 |
| `test_concurrency_limiter_remote_update` | 测试远程配置更新 | 返回 `Ok(())` |
| `test_concurrency_limiter_fetch_usage` | 测试本地使用量获取 | 返回 `Ok(())` |
| `test_concurrency_limiter_get_amount` | 测试配额获取 | 返回 `Ok(())` |

#### 1.1.2 UnirateLimiter 单一速率限流器

**测试文件**: `tests/ratelimit/unirate_limiter_test.rs`

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_unirate_limiter_allocate_quota` | 测试配额分配 | 返回 `Ok(true)` |
| `test_unirate_limiter_return_quota` | 测试配额归还 | 返回 `Ok(true)` |
| `test_unirate_limiter_name` | 测试插件名称 | 返回 "unirate" |
| `test_unirate_limiter_plugin_trait` | 测试 Plugin trait 实现 | init/destroy 正常执行 |
| `test_unirate_limiter_remote_update` | 测试远程配置更新 | 返回 `Ok(())` |
| `test_unirate_limiter_fetch_usage` | 测试本地使用量获取 | 返回 `Ok(())` |
| `test_unirate_limiter_get_amount` | 测试配额获取 | 返回 `Ok(())` |

#### 1.1.3 RejectLimiter 拒绝策略限流器

**测试文件**: `tests/ratelimit/reject_limiter_test.rs`

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_reject_limiter_allocate_quota` | 测试配额分配 | 返回 `Ok(true)` |
| `test_reject_limiter_return_quota` | 测试配额归还 | 返回 `Ok(true)` |
| `test_reject_limiter_name` | 测试插件名称 | 返回 "reject" |
| `test_reject_limiter_plugin_trait` | 测试 Plugin trait 实现 | init/destroy 正常执行 |
| `test_reject_limiter_remote_update` | 测试远程配置更新 | 返回 `Ok(())` |
| `test_reject_limiter_fetch_usage` | 测试本地使用量获取 | 返回 `Ok(())` |
| `test_reject_limiter_get_amount` | 测试配额获取 | 返回 `Ok(())` |

### 1.2 数据模型测试

**测试文件**: `tests/ratelimit/model_test.rs`

#### 1.2.1 LimiterConfig 测试

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_limiter_config_default` | 测试默认配置 | 字段正确初始化 |
| `test_limiter_config_clone` | 测试克隆 | 克隆前后值一致 |
| `test_limiter_config_debug` | 测试 Debug 格式化 | 输出完整信息 |

#### 1.2.2 WindowConfig 测试

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_window_config_default` | 测试默认窗口配置 | 默认值正确 |
| `test_window_config_with_duration` | 测试 Duration 设置 | 值正确设置 |
| `test_window_config_clone` | 测试克隆 | 克隆前后值一致 |

#### 1.2.3 QuotaConfig 测试

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_quota_config_default` | 测试默认配额配置 | 默认值正确 |
| `test_quota_config_amount` | 测试配额数量设置 | 值正确设置 |
| `test_quota_config_period` | 测试统计周期设置 | 值正确设置 |

#### 1.2.4 RemoteUpdate 枚举测试

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_remote_update_quota_sync` | 测试配额同步变体 | 正确构造 |
| `test_remote_update_config_update` | 测试配置更新变体 | 正确构造 |
| `test_remote_update_debug` | 测试 Debug 格式化 | 输出正确格式 |

#### 1.2.5 QuotaSyncData 测试

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_quota_sync_data_default` | 测试默认构造 | 字段初始化正确 |
| `test_quota_sync_data_clone` | 测试克隆 | 数据一致 |
| `test_quota_sync_data_timestamp` | 测试时间戳设置 | 值正确 |

### 1.3 请求/响应模型测试

**测试文件**: `tests/ratelimit/request_response_test.rs`

#### 1.3.1 QuotaRequest 测试

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_quota_request_valid` | 测试有效请求 | 验证通过 |
| `test_quota_request_empty_service` | 测试空服务名 | 返回错误 |
| `test_quota_request_empty_namespace` | 测试空命名空间 | 返回错误 |
| `test_quota_request_with_labels` | 测试带标签请求 | 正确解析 |
| `test_quota_request_timeout` | 测试超时设置 | 值正确 |

#### 1.3.2 QuotaResponse 测试

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_quota_response_allowed` | 测试允许响应 | allowed=true |
| `test_quota_response_rejected` | 测试拒绝响应 | allowed=false |
| `test_quota_response_message` | 测试消息设置 | 消息正确 |
| `test_quota_response_clone` | 测试克隆 | 数据一致 |

---

## 2. 集成测试

### 2.1 API 层集成测试

**测试文件**: `tests/ratelimit/api_integration_test.rs`

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_new_ratelimit_api` | 测试 API 创建 | 成功创建 |
| `test_new_ratelimit_api_by_context` | 测试通过 Context 创建 | 成功创建 |
| `test_get_quota_valid_request` | 测试获取配额-有效请求 | 返回响应 |
| `test_get_quota_invalid_request` | 测试获取配额-无效请求 | 返回错误 |

### 2.2 规则匹配测试

**测试文件**: `tests/ratelimit/rule_matching_test.rs`

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_rule_snapshot_cache` | 测试规则快照缓存 | 缓存正确 |
| `test_namespace_service_key` | 测试命名空间服务键 | 格式正确 |
| `test_rule_update_listener` | 测试规则更新监听 | 正确触发 |

### 2.3 配置更新流程测试

**测试文件**: `tests/ratelimit/config_update_test.rs`

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_config_update_trigger` | 测试配置更新触发 | 正确处理 |
| `test_config_hash_change` | 测试配置哈希变化 | 检测到变更 |
| `test_limiter_type_switch` | 测试限流器类型切换 | 切换成功 |

### 2.4 流程集成测试

**测试文件**: `tests/ratelimit/flow_integration_test.rs`

| 测试用例 | 描述 | 预期结果 |
|---------|------|---------|
| `test_ratelimit_flow_creation` | 测试流程创建 | 创建成功 |
| `test_ratelimit_flow_check_quota` | 测试配额检查 | 正确执行 |
| `test_ratelimit_flow_with_cache` | 测试带缓存的流程 | 缓存生效 |

---

## 3. 性能测试

### 3.1 高并发场景压测

**测试文件**: `benches/ratelimit/concurrency_benchmark.rs`

| 测试场景 | 并发数 | 持续时间 | 指标 |
|---------|--------|---------|------|
| `benchmark_concurrent_quota_request` | 100 | 10s | QPS |
| `benchmark_concurrent_quota_request` | 500 | 10s | QPS |
| `benchmark_concurrent_quota_request` | 1000 | 10s | QPS |
| `benchmark_concurrent_quota_request` | 5000 | 10s | QPS |

### 3.2 内存使用测试

**测试文件**: `benches/ratelimit/memory_benchmark.rs`

| 测试场景 | 描述 | 监控指标 |
|---------|------|---------|
| `benchmark_memory_single_limiter` | 单限流器内存 | RSS, Heap |
| `benchmark_memory_rule_cache` | 规则缓存内存 | 缓存大小 |
| `benchmark_memory_concurrent_requests` | 并发请求内存 | 内存峰值 |

### 3.3 延迟分布测试

**测试文件**: `benches/ratelimit/latency_benchmark.rs`

| 测试场景 | 请求数 | 百分位 |
|---------|--------|--------|
| `benchmark_latency_p50` | 10000 | p50 |
| `benchmark_latency_p90` | 10000 | p90 |
| `benchmark_latency_p95` | 10000 | p95 |
| `benchmark_latency_p99` | 10000 | p99 |
| `benchmark_latency_p999` | 10000 | p999 |

---

## 4. 测试覆盖目标

### 4.1 代码覆盖率目标

| 模块 | 目标覆盖率 |
|------|-----------|
| `src/plugins/ratelimit/*` | > 90% |
| `src/ratelimit/*` | > 85% |
| `src/core/model/ratelimit.rs` | 100% |
| 总体覆盖率 | > 80% |

### 4.2 关键路径覆盖率

以下路径必须达到 100% 覆盖率：

- `QuotaRequest::check_valid()`
- `RateLimitAPI::get_quota()`
- `DefaultRateLimitAPI::get_quota()`
- 所有限流器的 `allocate_quota()` 和 `return_quota()`

### 4.3 测试组织结构

```
tests/
└── ratelimit/
    ├── mod.rs
    ├── concurrency_limiter_test.rs
    ├── unirate_limiter_test.rs
    ├── reject_limiter_test.rs
    ├── model_test.rs
    ├── request_response_test.rs
    ├── api_integration_test.rs
    ├── rule_matching_test.rs
    ├── config_update_test.rs
    └── flow_integration_test.rs

benches/
└── ratelimit/
    ├── concurrency_benchmark.rs
    ├── memory_benchmark.rs
    └── latency_benchmark.rs
```

---

## 5. 测试依赖

在 `Cargo.toml` 的 `[dev-dependencies]` 中添加：

```toml
[dev-dependencies]
criterion = "0.5"
tokio-test = "0.4"
proptest = "1.4"
mockall = "0.12"

[profile.bench]
lto = true
codegen-units = 1
```

---

## 6. 测试执行

### 6.1 运行单元测试

```bash
cargo test --test ratelimit
```

### 6.2 运行集成测试

```bash
cargo test --test ratelimit_integration
```

### 6.3 运行性能测试

```bash
cargo bench --bench ratelimit
```

### 6.4 生成覆盖率报告

```bash
cargo tarpaulin --output-dir coverage
```

---

## 7. 测试优先级

### P0 - 必须实现

1. 限流器插件基本功能测试
2. 数据模型单元测试
3. API 集成测试

### P1 - 高优先级

4. 规则匹配测试
5. 配置更新流程测试
6. 延迟分布测试

### P2 - 中优先级

7. 高并发压测
8. 内存使用测试

---

## 8. 后续完善

当前模块实现为占位符（返回 `Ok(true)`），测试设计基于接口契约。当限流算法实现后，需补充：

1. **算法正确性测试**: 验证令牌桶、滑动窗口等算法实现的准确性
2. **边界条件测试**: 配额为 0、过期时间边界等
3. **错误恢复测试**: 网络中断、配置错误等异常场景
4. **分布式场景测试**: 多节点配额同步测试