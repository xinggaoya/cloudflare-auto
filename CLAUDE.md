# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

这是一个使用Rust编写的Cloudflare IPv6自动更新服务，主要功能包括：
- 实时监控本地IPv6地址变化
- 自动更新Cloudflare DNS AAAA记录
- 提供Web管理界面进行配置
- 使用SQLite数据库存储配置

## 开发命令

### 构建和运行
```bash
# 开发模式运行
cargo run

# 发布构建
cargo build --release

# 检查编译错误
cargo check

# 运行所有测试
cargo test

# 运行特定测试
cargo test --test test_name

# 代码格式化
cargo fmt

# 代码检查
cargo clippy
```

### 测试相关
- 测试文件位于各模块的测试部分（使用 `#[cfg(test)]`）
- 使用 `mockito` 进行HTTP API模拟测试
- 测试需要网络连接来验证Cloudflare API功能

## 架构说明

### 核心模块结构
```
src/
├── main.rs          # 主程序入口，服务启动和初始化
├── lib.rs           # 库模块导出
├── config/          # 配置管理
│   └── database.rs  # SQLite数据库操作
├── services/        # 业务服务层
│   ├── cloudflare.rs      # Cloudflare API客户端
│   ├── config_service.rs  # 配置服务
│   └── monitor_service.rs # IPv6监控服务
├── api/             # Web API层
│   ├── handlers.rs  # HTTP请求处理器
│   └── routes.rs    # 路由配置
└── utils/           # 工具函数
    ├── network.rs   # 网络功能（IPv6检测）
    └── logger.rs    # 日志系统
```

### 关键技术栈
- **Web框架**: Axum + Tokio
- **数据库**: SQLite (rusqlite)
- **HTTP客户端**: Reqwest
- **定时任务**: tokio-cron-scheduler
- **模板引擎**: Askama (用于Web界面)
- **日志系统**: tracing + tracing-appender

### 数据流
1. 主程序启动时初始化配置服务和监控服务
2. 监控服务定期检查IPv6地址变化（默认5分钟）
3. 检测到变化时调用Cloudflare服务更新DNS记录
4. Web API提供配置管理和状态查询接口
5. 前端通过静态文件提供管理界面

## 配置说明

- 配置文件存储在SQLite数据库 (`config.db`)
- 包含Cloudflare API密钥、区域ID、域名等敏感信息
- 配置需要通过Web界面进行测试验证后才能保存
- 支持多子域名选择和自定义检查间隔

## 注意事项

1. **网络要求**: 需要IPv6网络支持
2. **API权限**: Cloudflare API密钥需要DNS编辑权限
3. **日志系统**: 支持控制台和文件输出，自动清理旧日志
4. **错误处理**: 使用anyhow和thiserror进行错误处理
5. **测试**: 包含单元测试和集成测试，需要网络连接

## 开发提示

- 修改网络功能时注意 `src/utils/network.rs` 中的IPv6检测逻辑
- 添加新API时需要在 `src/api/routes.rs` 中注册路由
- 数据库操作集中在 `src/config/database.rs`
- Cloudflare API调用封装在 `src/services/cloudflare.rs`
- 定时任务逻辑在 `src/services/monitor_service.rs`