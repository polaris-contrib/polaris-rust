# 分布式限流架构设计文档

## 1. 概述

本文档描述 Rust 微服务治理 SDK 的分布式限流架构设计，基于 Java Pole 限流实现，支持单机限流和分布式限流两种模式，对外提供统一的 API 接口。

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                         LimitAPI                                 │
│                    (限流 API 入口接口)                             │
│                                                                  │
│  + getQuota(QuotaRequest) -> QuotaResponse                       │
│  + destroy()                                                     │
└─────────────────────────────────┬─────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                      DefaultLimitAPI                             │
│                    (限流 API 默认实现)                            │
│                                                                  │
│  - limitFlow: Arc<LimitFlow>                                     │
└─────────────────────────────────┬─────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                        LimitFlow                                 │
│                    (限流流程核心)                                 │
│                                                                  │
│  - quotaFlow: Arc<QuotaFlow>                                     │
│  - connector: Arc<AsyncRateLimitConnector>                       │
└─────────────────────────────────┬─────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                        QuotaFlow                                 │
│                    (配额分配流程)                                 │
│                                                                  │
│  - lookupWindow()  - 查找限流窗口                                │
│  - allocateQuota() - 分配配额                                    │
└─────────────────────────────────┬─────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                     RateLimitWindow                              │
│                    (限流窗口，核心调度单元)                        │
│                                                                  │
│  - mode: QuotaMode (Local/Global)                                │
│  - buckets: HashMap<i32, Arc<dyn QuotaBucket>>                   │
│  - syncTask: Arc<RemoteSyncTask>                                 │
└─────────────────────────────────┬─────────────────────────────────┘
                                  │
                    ┌─────────────┼─────────────┐
                    │             │             │
                    ▼             ▼             ▼
         ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
         │   QuotaBucket   │ │   QuotaBucket   │ │  RemoteSyncTask │
         │   (Unirate)     │ │  (Concurrency)  │ │                 │
         └─────────────────┘ └─────────────────┘ └─────────────────┘
```

### 2.2 限流模式

```rust
/// QuotaMode 限流模式
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotaMode {
    /// 本地限流模式：单机限流，无需与远程服务器通信
    Local,
    /// 分布式限流模式：全局限流，需要与限流服务器交互获取配额
    Global,
}
```

### 2.3 核心组件

| 组件 | 职责 |
|------|------|
| **LimitAPI** | 限流 API 入口，提供统一的对外接口 |
| **LimitFlow** | 限流流程编排，协调各组件工作 |
| **QuotaFlow** | 配额分配核心逻辑，处理规则匹配和配额分配 |
| **RateLimitWindow** | 限流窗口，管理具体限流器的生命周期 |
| **QuotaBucket** | 配额桶接口，封装具体限流算法实现 |
| **RemoteSyncTask** | 远程同步任务，负责与限流服务器通信 |
| **AsyncRateLimitConnector** | 异步连接器，管理 gRPC 流式连接 |

## 3. 接口定义

### 3.1 LimitAPI 主入口

```rust
/// LimitAPI 限流 API 主入口
#[async_trait::async_trait]
pub trait LimitAPI: Send + Sync {
    /// 获取配额检查结果
    async fn get_quota(&self, request: QuotaRequest) -> Result<QuotaResponse, PoleError>;

    /// 销毁并释放资源
    fn destroy(&self);
}
```

### 3.2 QuotaBucket 配额桶接口

```rust
/// QuotaBucket 配额桶接口（统一单机和分布式限流）
#[async_trait::async_trait]
pub trait QuotaBucket: Send + Sync {
    /// 分配配额
    ///
    /// # 参数
    /// - `count`: 请求的配额数量
    /// - `cur_time_ms`: 当前时间戳（毫秒）
    ///
    /// # 返回
    /// - `Ok(QuotaResult)`: 配额分配结果
    /// - `Err(PoleError)`: 分配失败错误
    async fn allocate_quota(
        &self,
        count: i64,
        cur_time_ms: u64
    ) -> Result<QuotaResult, PoleError>;

    /// 归还配额（仅并发限流需要）
    async fn return_quota(
        &self,
        allocate_time_ms: u64,
        count: i64
    ) -> Result<(), PoleError>;

    /// 远程配额更新（分布式限流核心方法）
    ///
    /// # 参数
    /// - `remote_info`: 远程配额信息（剩余配额、客户端数量等）
    async fn on_remote_update(
        &self,
        remote_info: RemoteQuotaInfo
    ) -> Result<(), PoleError>;

    /// 拉取本地配额使用情况（用于上报）
    fn fetch_local_usage(
        &self,
        cur_time_ms: u64
    ) -> HashMap<i32, LocalQuotaInfo>;

    /// 获取限流阈值信息
    fn get_amount_info(&self) -> HashMap<i32, AmountInfo>;
}
```

### 3.3 数据结构定义

```rust
/// RemoteQuotaInfo 远程配额信息（用于分布式限流）
#[derive(Clone, Debug)]
pub struct RemoteQuotaInfo {
    /// 剩余配额数量
    pub left_quota: i64,
    /// 当前客户端数量
    pub client_count: i32,
    /// 服务器时间戳（毫秒）
    pub timestamp_ms: u64,
    /// 配额周期（毫秒）
    pub duration_ms: i64,
}

/// LocalQuotaInfo 本地配额使用信息
#[derive(Clone, Debug, Default)]
pub struct LocalQuotaInfo {
    /// 已使用的配额数量
    pub quota_used: i64,
    /// 被限流的配额数量（请求被拒绝的次数）
    pub quota_limited: i64,
}

/// AmountInfo 限流阈值信息
#[derive(Clone, Debug)]
pub struct AmountInfo {
    /// 最大配额数量
    pub max_amount: i64,
    /// 统计周期（秒）
    pub duration_secs: i64,
    /// 限流模式
    pub mode: QuotaMode,
}

/// QuotaResult 配额分配结果
#[derive(Clone, Debug)]
pub struct QuotaResult {
    /// 是否允许通过
    pub allowed: bool,
    /// 限流提示信息
    pub message: String,
    /// 限流规则名称
    pub rule_name: Option<String>,
}
```

### 3.4 QuotaRequest 和 QuotaResponse

```rust
/// QuotaRequest 配额检查请求
#[derive(Clone, Debug)]
pub struct QuotaRequest {
    /// 命名空间
    pub namespace: String,
    /// 服务名称
    pub service: String,
    /// 方法名
    pub method: String,
    /// 流量标签提供者
    pub traffic_label_provider: Arc<dyn Fn(ArgumentType, &str) -> Option<String> + Send + Sync>,
}

/// QuotaResponse 配额检查响应
#[derive(Clone, Debug)]
pub struct QuotaResponse {
    /// 是否允许通过
    pub allowed: bool,
    /// 响应消息
    pub message: String,
}
```

## 4. 分布式限流实现

### 4.1 限流窗口状态机

```rust
/// WindowStatus 限流窗口状态
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowStatus {
    /// 已创建，无需进行后台调度
    Created,
    /// 已获取调度权，准备开始初始化
    Initializing,
    /// 已在远程初始化完成
    Initialized,
    /// 已删除
    Deleted,
}
```

### 4.2 远程同步任务

```rust
/// RemoteSyncTask 远程同步任务 Trait
#[async_trait::async_trait]
pub trait RemoteSyncTask: Send + Sync {
    /// 执行远程初始化
    async fn do_remote_init(&self, redo_init: bool) -> Result<(), PoleError>;

    /// 执行配额上报
    async fn do_remote_acquire(&self) -> Result<(), PoleError>;
}
```

### 4.3 gRPC 消息定义

参考 Pole Protobuf 定义：

```protobuf
// 限流命令
enum RateLimitCmd {
    INIT = 0;      // 初始化
    ACQUIRE = 1;   // 配额获取/上报
    RELEASE = 2;   // 配额释放
}

// 初始化请求
message RateLimitInitRequest {
    string client_id = 1;
    LimitTarget target = 2;  // 限流目标（namespace/service/labels）
    repeated QuotaTotal totals = 3;  // 配额总量信息
}

// 配额上报请求
message RateLimitReportRequest {
    string client_key = 1;
    int64 timestamp = 2;  // 客户端时间戳
    repeated QuotaSum quota_uses = 3;  // 配额使用统计
}

// 配额总量
message QuotaTotal {
    int32 duration = 1;  // 统计周期（秒）
    QuotaMode mode = 2;  // 限流模式
    int32 max_amount = 3;  // 最大配额数
}

// 配额使用统计
message QuotaSum {
    int32 counter_key = 1;
    int32 used = 2;  // 已使用配额
    int32 limited = 3;  // 被限流配额
}

// 响应
message RateLimitResponse {
    RateLimitCmd cmd = 1;
    int32 code = 2;
    string info = 3;
    RateLimitInitResponse rate_limit_init_response = 4;
    RateLimitReportResponse rate_limit_report_response = 5;
}

// 初始化响应
message RateLimitInitResponse {
    int32 code = 1;
    string client_key = 2;
}

// 上报响应
message RateLimitReportResponse {
    int32 code = 1;
    int64 timestamp = 2;  // 服务器时间戳
    repeated QuotaLeft quota_lefts = 3;  // 剩余配额
}

// 剩余配额
message QuotaLeft {
    int32 counter_key = 1;
    int32 left = 2;  // 剩余配额数
    int32 client_count = 3;  // 客户端数量
}
```

## 5. 连接管理

### 5.1 异步连接器

```rust
/// AsyncRateLimitConnector 异步限流连接器
/// 负责管理与限流服务器的 gRPC 流式连接
pub struct AsyncRateLimitConnector {
    /// gRPC 客户端
    client: RateLimitServiceClient<Channel>,
    /// 流计数器集合：按服务分组
    stream_counter_sets: DashMap<ServiceIdentifier, Arc<StreamCounterSet>>,
    /// 连接状态
    state: AtomicU8,
    /// 时间差（用于时间校准）
    time_diff_ms: AtomicI64,
}

impl AsyncRateLimitConnector {
    /// 获取或创建流计数器集合
    pub fn get_stream_counter_set(
        &self,
        service_identifier: &ServiceIdentifier,
        window: &RateLimitWindow,
    ) -> Result<Arc<StreamCounterSet>, PoleError>;

    /// 时间校准
    pub async fn adjust_time(&self) -> Result<(), PoleError>;

    /// 获取校准后的服务器时间
    pub fn get_server_time_ms(&self, local_time_ms: u64) -> u64;
}
```

### 5.2 流资源管理

```rust
/// StreamResource 流资源
/// 封装与特定服务的 gRPC 流连接
pub struct StreamResource {
    /// 客户端 key
    client_key: String,
    /// 发送通道
    request_tx: mpsc::Sender<RateLimitRequest>,
    /// 接收通道
    response_rx: mpsc::Receiver<RateLimitResponse>,
    /// 限流窗口
    window: Weak<RateLimitWindow>,
    /// 最后接收时间
    last_recv_time: AtomicU64,
    /// 计数器映射
    counters: DashMap<i32, DurationBaseCallback>,
}

impl StreamResource {
    /// 发送限流请求
    pub fn send_request(&self, request: RateLimitRequest) -> Result<(), PoleError>;

    /// 处理响应
    pub async fn process_response(&self, response: RateLimitResponse);
}
```

## 6. 限流模式对比

| 特性 | 本地限流 (Local) | 分布式限流 (Global) |
|------|-----------------|-------------------|
| **配额来源** | 本地配置 | 限流服务器分配 |
| **通信方式** | 无 | gRPC 流式连接 |
| **时间要求** | 本地时间 | 需要时间校准 |
| **精度** | 单机精度 | 全局精度 |
| **性能** | 高 | 中（有网络开销） |
| **适用场景** | 单机应用 | 分布式集群 |

## 7. 配额上报流程

### 7.1 初始化流程

```
1. 创建 RateLimitWindow
   │
   ▼
2. 检查限流模式 (QuotaMode)
   │
   ├─ Local ──> 使用本地限流器，无需初始化
   │
   └─ Global ──> 发起远程初始化
         │
         ▼
3. AsyncRateLimitConnector 建立 gRPC 连接
         │
         ▼
4. 发送 RateLimitInitRequest
   - client_id
   - target (namespace/service/labels)
   - totals (配额总量信息)
         │
         ▼
5. 等待 RateLimitInitResponse
   - client_key
   - code
         │
         ▼
6. 状态更新为 Initialized
```

### 7.2 配额分配流程

```
1. get_quota(QuotaRequest)
   │
   ▼
2. QuotaFlow.lookupWindow() 查找限流窗口
   │
   ▼
3. RateLimitWindow.allocateQuota()
   │
   ├─ Local ──> 直接调用 QuotaBucket.allocate_quota()
   │
   └─ Global ──>
       a. 检查本地剩余配额
       b. 如果配额不足，触发远程获取
       c. 调用 QuotaBucket.allocate_quota()
   │
   ▼
4. 返回 QuotaResult
```

### 7.3 配额上报流程

```
1. 定时任务触发（每秒）
   │
   ▼
2. RemoteSyncTask.do_remote_acquire()
   │
   ▼
3. 调用 QuotaBucket.fetch_local_usage()
   获取本地配额使用统计
   │
   ▼
4. 构建 RateLimitReportRequest
   - client_key
   - timestamp (校准后的服务器时间)
   - quota_uses (配额使用统计)
   │
   ▼
5. 通过 gRPC 流发送请求
   │
   ▼
6. 等待 RateLimitReportResponse
   - timestamp
   - quota_lefts (剩余配额列表)
   │
   ▼
7. 调用 QuotaBucket.on_remote_update()
   更新本地配额
```

## 8. 时间校准机制

```rust
/// 时间校准请求
pub async fn adjust_time(&self) -> Result<(), PoleError> {
    let local_send_time = current_time_ms();

    // 发送时间校准请求
    let request = TimeAdjustRequest {};
    let response = self.client.time_adjust(request).await?;

    let local_recv_time = current_time_ms();
    let remote_send_time = response.server_timestamp;

    // 计算网络延迟
    let latency = local_recv_time - local_send_time;

    // 计算时间差（考虑网络延迟，使用 1/3 延迟补偿）
    let remote_recv_time = remote_send_time + latency / 3;
    let time_diff = remote_recv_time as i64 - local_recv_time as i64;

    self.time_diff_ms.store(time_diff, Ordering::SeqCst);
    Ok(())
}

/// 获取服务器时间
pub fn get_server_time_ms(&self, local_time_ms: u64) -> u64 {
    let diff = self.time_diff_ms.load(Ordering::SeqCst);
    (local_time_ms as i64 + diff) as u64
}
```

## 9. 实现优先级

> 状态更新：以下内容已按当前仓库代码（`src/core/flow.rs`、`src/ratelimit/sync/pole.rs`、`src/ratelimit/connector/*`、`tests/remote_sync_test.rs`）更新。

### P0 - 核心功能
- [x] LimitAPI 和 DefaultLimitAPI 实现
- [~] LimitFlow 和 QuotaFlow 实现（以 `RatelimitFlow + LimiterRegistry` 形态落地，未按文档中的 `flow/` 子目录拆分）
- [~] RateLimitWindow 实现（以 `LimiterInstance + LimiterKey` 近似实现窗口生命周期）
- [~] QuotaBucket Trait 定义（由 `ServiceRateLimiter` + `RemoteUpdate` 承担同等职责）
- [x] 本地限流模式实现

### P1 - 分布式限流
- [x] RemoteQuotaInfo 和 LocalQuotaInfo 实现
- [~] RemoteSyncTask Trait 定义（未单独 trait 化，已由 `PoleRemoteSyncTask` 具体实现承载）
- [x] AsyncRateLimitConnector 实现
- [x] StreamResource 实现
- [x] 分布式限流模式实现（Global 路径已接入远程初始化/上报/更新，并带本地降级；`unirate/concurrency/reject` 均支持远程配额更新）

### P2 - 增强功能
- [x] 时间校准机制
- [x] 连接健康检查（基于 `last_recv_time` 的流存活检查）
- [x] 断线重连机制（周期任务内指数退避重连）
- [x] 配额预分配优化（已实现手动上报节流与周期上报，优先使用本地已分配配额）

## 10. 文件结构

```
src/ratelimit/
├── mod.rs              # 模块导出
├── api.rs              # LimitAPI 定义
├── default.rs          # DefaultLimitAPI 实现
├── req.rs              # QuotaRequest/QuotaResponse
│
├── flow/
│   ├── mod.rs
│   ├── limit_flow.rs   # LimitFlow 实现
│   ├── quota_flow.rs   # QuotaFlow 实现
│   └── window.rs       # RateLimitWindow 实现
│
├── bucket/
│   ├── mod.rs
│   ├── trait.rs        # QuotaBucket Trait
│   ├── local.rs        # 本地限流实现
│   └── global.rs       # 分布式限流实现
│
├── sync/
│   ├── mod.rs
│   ├── task.rs         # RemoteSyncTask Trait
│   └── pole.rs      # Pole 远程同步实现
│
└── connector/
    ├── mod.rs
    ├── async.rs        # AsyncRateLimitConnector
    └── stream.rs       # StreamResource
```

当前仓库实际结构补充（与上面的“目标结构”并存）：

- 主流程在 `src/core/flow.rs`（`RatelimitFlow`、`LimiterRegistry`、规则匹配与模式分发）
- 分布式同步在 `src/ratelimit/sync/pole.rs`
- 连接器在 `src/ratelimit/connector/async_connector.rs` 与 `src/ratelimit/connector/stream.rs`
- 分布式集成测试在 `tests/remote_sync_test.rs`

## 11. 测试建议

### 单元测试
- QuotaBucket 各实现的功能测试
- RateLimitWindow 状态机测试
- RemoteQuotaInfo 序列化和反序列化测试

### 集成测试
- 本地限流端到端测试
- 分布式限流端到端测试（需要 Mock 限流服务器）
- 配额上报流程测试

### 性能测试
- 单机限流 QPS 测试
- 分布式限流延迟测试
- 并发连接数测试

## 12. 参考资料

- Java 实现：`/Users/liaochuntao/Github/polemesh/pole-java/pole-ratelimit/`
- Protobuf 定义：`pole-protocol/limit.proto`
