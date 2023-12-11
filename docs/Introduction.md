
# Introduction

## 目录结构：
```html
polaris-rust
├── core
├── discovery
├── loadbalance   
├── circuitbreak
├── ratelimit
├── config
├── observability
├── middleware
├── polaris
│   └── src
├── examples
├── benches
├── scripts
├── Cargo.toml
└── README.md
```
```html
core: 公共依赖、抽象和工具
discovery: 服务发现和注册相关
circuitbreak: 熔断功能
ratelimit: 限流功能
config: 配置管理
observability: metrics 和 tracing
middleware: 各种中间件
examples: 示例程序
benches: 基准测试
scripts: 构建、发布等脚本
```

```html
core
// 公共依赖、抽象和工具

discovery
// 服务发现和注册

loadbalance   
// 负载均衡

circuitbreak
// 熔断

ratelimit
// 限流

config
// 配置管理

observability
// 指标上报和追踪

middleware
// 中间件，如日志、监控等

polaris
// SDK 统一出口

examples 
// 示例程序
        
benches
// 基准测试

scripts
// 构建和发布脚本
```