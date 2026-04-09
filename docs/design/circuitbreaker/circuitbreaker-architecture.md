# pole-client-rust 熔断（Circuit Breaker）当前设计

本文描述仓库当前已实现的熔断整体设计，基于 `main` 代码行为整理，不包含未来规划。

## 1. 目标与范围

当前熔断设计目标：

- 在调用前判断资源是否允许放通（`check_resource`）。
- 在调用后上报统计数据（`report_stat`），驱动状态机迁移。
- 将实例级熔断状态回写到运行时统计，供路由/负载均衡跳过异常实例。

当前内置实现仅有一个熔断插件：`composite`。

## 2. 架构分层

### 2.1 API 层（业务接入）

- `src/circuitbreaker/api.rs`
- `src/circuitbreaker/default.rs`

关键点：

- `CircuitBreakerAPI` 暴露 `check_resource` / `report_stat` / `make_invoke_handler`。
- `InvokeHandler::acquire_permission` 在请求前做服务级熔断检查，不通过则返回 `CallAbortedError`。
- `InvokeHandler::on_success` / `on_error` 在请求后上报统计：
  - 一次服务级（`ServiceResource`）
  - 若有 path，再补一次方法级（`MethodResource`）

### 2.2 Flow 层（统一流程封装）

- `src/core/flow.rs`

关键点：

- `CircuitBreakerFlow` 从 `Extensions` 取熔断插件。
- 若未启用熔断插件，`check_resource` 直接放通，`report_stat` 直接返回 `Ok(())`。
- `CircuitBreakerStatus -> CheckResult` 转换规则：仅 `Status::Open` 视为拦截；`Close/HalfOpen` 都放通。

### 2.3 Plugin 层（策略实现）

- 接口：`src/core/plugin/circuitbreaker.rs`
- 实现：`src/plugins/circuitbreaker/composite/circuitbreaker.rs`
- 注册与加载：`src/core/plugin/plugins.rs`

关键点：

- `CircuitBreaker` trait 要求实现异步 `check_resource` / `report_stat`。
- 当前仅注册 `CompositeCircuitBreaker::builder`，插件名 `composite`。
- `load_circuit_breaker` 在 `consumer.circuit_breaker.enable = true` 时加载该插件，并注入：
  - `ResourceCache`
  - `InstanceStatManager`

### 2.4 规则与缓存层

- 缓存接口：`src/core/plugin/cache.rs`
- 内存缓存实现：`src/plugins/cache/memory/memory.rs`
- 缓存模型：`src/core/model/cache.rs`

关键点：

- 熔断规则通过 `ResourceCache::load_service_circuitbreaker` 按 `namespace + service` 拉取。
- 缓存键：`"{namespace}#{service}"`。
- 首次访问会注册 watch，等待初始化（超时返回错误）。
- 规则变更通过 cache 的 watch 更新，`CompositeCircuitBreaker` 每次检查/上报都会重新读取当前规则快照。

### 2.5 运行时实例统计联动

- `src/core/model/instance_stat.rs`

关键点：

- `CompositeCircuitBreaker` 仅在 `Resource::InstanceResource` 场景更新实例熔断状态（Open/HalfOpen/Close）。
- 状态存入 `InstanceStatManager.circuit_status`（`AtomicU8`）。
- 负载均衡插件会过滤 `Open` 实例（见 leastConnections / leastRT 实现）。

## 3. 资源模型与键设计

- 定义：`src/core/model/circuitbreaker.rs`

支持资源：

- `ServiceResource`
- `MethodResource`
- `InstanceResource`
- `CustomResource`

统一资源键 `resource_key()` 用于状态隔离与统计聚合：

- service: `service_resource:{ns}:{svc}`
- method: `interface_resource:{ns}:{svc}:{proto}:{method}`
- instance: `instance_resource:{ns}:{svc}:{ip}:{port}`
- custom: `{type}_resource:{segments...}`

`CompositeCircuitBreaker` 的状态键：

- `state_key = "{rule_identity}#{resource_key}"`
- 即“按规则 + 按资源”独立维护状态机。

## 4. Composite 状态机

状态定义：

- `Close`
- `Open`
- `HalfOpen`
- `Destroy`（保留态，当前无外部触发销毁逻辑）

内部状态结构（`RuleRuntimeState`）：

- `status`
- `open_start_ms`
- `half_open_inflight`（半开探测并发闸门）
- `half_open_success`
- `consecutive_error`
- `samples: VecDeque<(timestamp_ms, is_error)>`

### 4.1 检查路径（on_check）

- `Close`：放通。
- `Open`：
  - 未到 `sleep_window`：拒绝。
  - 到了 `sleep_window`：转 `HalfOpen` 并继续判定。
- `HalfOpen`：
  - 若已有探测请求在飞，拒绝（对外表现为 Open）。
  - 否则放通一个探测请求并置 `half_open_inflight = true`。

### 4.2 上报路径（record_stat）

- 先写入 `samples`，并按最大触发窗口裁剪历史。
- 更新 `consecutive_error`。
- 按当前状态处理：
  - `Close`：若满足触发条件则 `open()`。
  - `HalfOpen`：
    - 探测失败：立即 `open()`。
    - 探测成功：累积成功数，达到 `recover_condition.consecutive_success` 后 `close()`。
  - `Open/Destroy`：不上移。

### 4.3 默认阈值（规则未配置触发条件时）

`is_triggered` 会注入两条默认触发条件：

1. 错误率：60s 窗口，最少 20 请求，错误率 >= 50%
2. 连续错误：连续错误 >= 5

`recover_condition.sleep_window` 默认 30s。
`recover_condition.consecutive_success` 默认 1。

## 5. 规则匹配与错误判定

### 5.1 规则匹配

入口：`rule_matches_resource`。

匹配维度：

- `rule.enable` 必须为 true。
- `rule.level` 与资源类型一致（service/method/instance 等）。
- `rule_matcher.destination` 匹配被调服务。
- 若配置了 `rule_matcher.source`，则要求 caller 存在且匹配；若请求无 caller，source 必须是空或通配。
- method 级规则在 `Level::Method` 下额外匹配 api（protocol/method/path）。

### 5.2 错误判定

入口：`is_error_by_rule`。

- 默认：`RetSuccess` 以外都视为错误。
- 若规则配置 `error_conditions`（含 block 级），则按条件匹配：
  - 输入类型支持 `ret_code` / `delay`
  - 比较类型支持 `Exact` / `NotEquals` / `Regex` / `In` / `NotIn` / `Range`

## 6. 调用时序

### 6.1 请求前

1. 业务侧 `InvokeHandler::acquire_permission`
2. `CircuitBreakerFlow.check_resource`
3. `CompositeCircuitBreaker.check_resource`
4. 读取规则 -> 按优先级遍历 -> 命中后执行状态机检查
5. 若拦截，返回 `CallAbortedError(rule_name, fallback_info)`

### 6.2 请求后

1. `InvokeHandler::on_success/on_error`
2. 构造 `ResourceStat`（服务级，可能再加方法级）
3. `CircuitBreakerFlow.report_stat`
4. `CompositeCircuitBreaker.report_stat`
5. 命中规则后更新运行时状态机

### 6.3 实例选择阶段

- `HealthRouter` 会对每个实例做 `InstanceResource` 检查，不通过则剔除。
- `leastConnections` / `leastRT` 也会依据 `InstanceStatManager` 中的 `CircuitStatus::Open` 过滤实例。

## 7. 配置与开关

配置结构：`src/core/config/consumer.rs`

```yaml
consumer:
  circuitBreaker:
    enable: true
    enableRemotePull: true
```

当前行为：

- `enable` 生效：控制是否加载熔断插件。
- `enableRemotePull` 字段已存在，但当前代码路径未看到被消费的分支逻辑（规则拉取仍通过资源缓存统一流程进行）。

## 8. 并发与性能特征

- 规则状态容器：`DashMap<String, Arc<Mutex<RuleRuntimeState>>>`
- 每个 `state_key` 细粒度互斥，避免全局锁。
- `samples` 使用 `VecDeque`，按时间窗口裁剪。
- 每次 check/report 都会读取当前规则快照；行为更实时，但会增加缓存读取频次。

## 9. 已有测试覆盖

当前熔断相关主要为单元测试，覆盖点包括：

- `CircuitBreakerFlow` 的状态到放通判定转换（Open 阻断，HalfOpen 放通）
- `Composite` 的字符串匹配、连续错误触发、error_condition 匹配

缺口（当前仓库状态）：

- 尚未看到完整的端到端 integration tests 覆盖服务级/实例级熔断联动。
- `Destroy` 状态没有明显外部生命周期测试。

## 10. 主动探测（Fault Detector）现状检查

主动探测相关代码目前主要在“规则缓存链路”，不在“执行链路”：

- 已有能力：
  - 资源模型/事件类型支持 `FaultDetectRule`（`EventType`、`DiscoverRequestType::FaultDetector`）。
  - grpc connector 可以接收 `DiscoverResponseType::FaultDetector` 并分发到 cache watch。
  - `MemoryResourceCache` 提供 `load_service_faultdetect`，并维护 `faultdetect_rules` 缓存。
- 当前缺失：
  - 未看到独立的 Fault Detector 插件/flow/后台任务消费这些规则并主动发起探测。
  - 也未看到主动探测结果回写实例可用性/熔断状态的完整执行链路。

当前实现是单事件监听：

- `load_service_faultdetect` 初始化 watch 时仅监听 `FaultDetectRule`。

## 11. 现状结论

当前熔断实现是“规则驱动 + 本地状态机 + 运行时实例状态联动”的组合设计：

- 通过 `composite` 实现统一承载服务级/方法级/实例级熔断。
- 通过请求前检查与请求后上报形成闭环。
- 通过 `InstanceStatManager` 将实例熔断结果传递到路由/负载均衡。

整体设计已经可运行，且与现有插件化框架一致；后续若要增强，可优先补齐 integration tests 与 `enableRemotePull` 语义收敛。
