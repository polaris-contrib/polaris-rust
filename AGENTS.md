# AGENTS.md — pole-client-rust

本文件是本仓库中 agent（编码智能体）的工作指引。
请以本文件 + 现有代码模式 + CI 工作流作为执行准则。

## 1) 仓库概览

- 语言：Rust（edition 2021）
- Crate：`pole-client-rust`（见 `Cargo.toml`）
- 主要目录：
  - `src/`（库代码）
  - `tests/`（集成风格测试）
  - `examples/`（使用示例）
  - `docs/`（设计与测试文档）
  - `.github/workflows/`（CI 真相来源）
- 格式化配置：`rustfmt.toml`

## 2) 规则文件检查

- `.cursorrules`：未找到
- `.cursor/rules/`：未找到
- `.github/copilot-instructions.md`：未找到
- 因此请遵循本 `AGENTS.md` 与仓库内既有约定。

## 3) 常用命令（与 CI 对齐）

请在仓库根目录执行。

### 构建 / 检查

- `cargo check`（CI 编译检查）
- `cargo check --all-features`
- `cargo build`
- `cargo build --all-features`

### 格式化 / Lint

- `cargo fmt --all`
- `cargo fmt --all -- --check`（CI）
- `cargo clippy --all-features`（CI）
- `cargo clippy --all-targets --all-features -- -D warnings`（本地严格模式）

### 测试 / 文档 / 审计

- `cargo test --all-features`（CI）
- `cargo test`
- `cargo doc --no-deps --all-features`（CI 文档）
- `cargo audit`（本地对应 RustSec 审计）

## 4) 单测快速运行（重点）

用于快速迭代调试。

- 运行单个 integration target：
  - `cargo test --test ratelimit`
  - `cargo test --test concurrency_limiter_test`
  - `cargo test --test reject_limiter_test`
  - `cargo test --test unirate_limiter_test`
  - `cargo test --test request_response_test`
  - `cargo test --test model_test`
- 运行单个精确测试函数：
  - `cargo test test_concurrency_limiter_allocate_quota -- --exact`
- 列出某个 target 下所有测试：
  - `cargo test --test ratelimit -- --list`
- 使用名称过滤：
  - `cargo test limiter_allocate`
- 调试参数：
  - `cargo test <name> -- --nocapture`
  - `RUST_BACKTRACE=1 cargo test <name>`

## 5) CI 真相来源（不要偏离）

- `.github/workflows/check.yml`：
  - `cargo check`
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-features`
  - `cargo test --all-features`
- `.github/workflows/docs.yml`：`cargo doc --no-deps --all-features`
- `.github/workflows/rust-clippy.yml`：SARIF clippy 流程
- `.github/workflows/audit.yml`：RustSec 审计

若文档与实现不一致，以 CI 配置为准。

## 6) 代码风格规范

### Imports / Modules

- 优先 `std` 导入，再导入 crate/本地模块。
- 让 rustfmt 处理导入顺序（`reorder_imports = true`）。
- 优先显式 `crate::...` 路径，避免无边界 glob 导入。
- 模块/文件名使用 `snake_case`。
- 顶层模块在 `src/lib.rs` 中通过 `pub mod ...` 暴露。

### Formatting

`rustfmt.toml` 关键配置：
- `edition = "2021"`
- `max_width = 100`
- `newline_style = "Unix"`
- `reorder_imports = true`

### Types / APIs

- 使用明确的请求/响应结构体与强类型。
- 可失败 API 统一返回 `Result<T, PoleError>`。
- 异步共享所有权优先使用 `Arc<T>`。
- 保持 trait 边界清晰（如 `Plugin`、`Connector`、`ResourceCache`）。

### Naming

- 函数/变量/文件：`snake_case`
- 结构体/枚举/trait：`CamelCase`
- 常量/静态：`UPPER_SNAKE_CASE`（避免引入新的命名偏差）
- 测试命名建议：`test_<组件>_<行为>`

### Error Handling

- 使用统一错误模型：`PoleError` + `ErrorCode`（`src/core/model/error.rs`）。
- 优先错误传播/映射，避免 panic 路径。
- 生产代码中避免新增 `unwrap()` / `expect()`。
- 推荐使用 `ok_or_else`、`map_err`、显式 `match`。
- 错误信息保留上下文（可用 `format!(...)`）。

### Logging / Async / Tests

- 使用仓库日志宏：`trace!`、`debug!`、`info!`、`warn!`、`error!`。
- 日志链路位于 `src/macros.rs` 与 `src/core/logger/logger.rs`。
- 使用现有 `tokio` 原语，避免在 async 路径阻塞。
- async 测试使用 `#[tokio::test]`，断言保持可重复、稳定。

## 7) 开发流程与护栏

- 修改前先阅读邻近模块和一个相似实现。
- 变更尽量小而聚焦，保持与现有模式一致。
- 代码实现必须基于工程合理性与既有设计理念，不以“迎合指令”为目标；若需求可能引入实际 bug，需先明确提醒风险并发起讨论，只有在用户明确要求“必须这样改”时才按该方案执行。
- 非任务要求时，不要改动公共 API 形状。
- 不要用大范围 `#[allow(...)]` 仅为压警告。
- 不要通过删除失败测试来“修复”CI。
- 优先复用现有依赖/模式，再考虑引入新 crate。
- 变更后建议执行：
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-features`
  - 先跑相关测试，再在可行时执行 `cargo test --all-features`
- 若失败是仓库既有问题且与本次改动无关，请明确说明，避免顺手大重构。

## 8) 备注

- 仓库当前存在进行中的变更，请避免无关的大范围重构。
- `docs/design/ratelimit-test-plan.md` 可能包含规划性内容，运行行为以 `tests/` 现状为准。

## 9) 快速复制命令

```bash
# 本地模拟 CI 验证
cargo fmt --all -- --check
cargo clippy --all-features
cargo test --all-features

# 重点测试
cargo test --test ratelimit
cargo test test_concurrency_limiter_allocate_quota -- --exact
```
