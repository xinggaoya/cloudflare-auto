# Cloudflare IPv6自动更新服务

一个使用Rust编写的现代化工具，用于监控本地IPv6地址变化并自动更新到Cloudflare DNS记录。

## 功能特性

- 🌐 实时监控本地IPv6地址变化
- 🔄 自动更新Cloudflare DNS AAAA记录
- 💾 嵌入式SQLite数据库存储配置
- 🧪 配置测试功能（测试成功才能保存）
- 🖥️ 现代化Web管理界面
- ⚡ 基于Rust和Axum的高性能后端
- 🔧 支持多子域名选择更新

## 快速开始

### 1. 安装和运行

```bash
# 克隆项目
git clone <项目地址>
cd cloudflare-auto

# 运行服务
cargo run
```

服务将在 `http://localhost:3000` 启动

### 2. 配置Cloudflare

1. 打开Web管理界面
2. 输入您的Cloudflare配置：
   - **API密钥**: `YOUR_CLOUDFLARE_API_KEY`
   - **区域ID**: `YOUR_ZONE_ID`
   - **根域名**: `example.com`

3. 点击"测试配置"验证连接
4. 获取域名列表并选择要自动更新的子域名
5. 设置检查间隔（默认300秒）
6. 保存配置

### 3. 监控服务

配置保存后，监控服务将自动启动：
- 每5分钟检查一次IPv6地址变化
- 检测到变化时自动更新所有选中的DNS记录
- 支持创建新的AAAA记录（如果不存在）

## API接口

### 测试配置
```
POST /api/test-config
{
  "api_key": "your_api_key",
  "zone_id": "your_zone_id", 
  "root_domain": "example.com"
}
```

### 获取域名列表
```
POST /api/domain-list
{
  "api_key": "your_api_key",
  "zone_id": "your_zone_id",
  "root_domain": "example.com"
}
```

### 保存配置
```
POST /api/save-config
{
  "api_key": "your_api_key",
  "zone_id": "your_zone_id",
  "root_domain": "example.com",
  "selected_subdomains": ["sub1", "sub2"],
  "check_interval": 300
}
```

### 获取配置状态
```
GET /api/config-status
```

### 获取当前IP
```
GET /api/current-ip
```

## 技术栈

- **后端**: Rust + Axum + Tokio
- **数据库**: SQLite (rusqlite)
- **HTTP客户端**: Reqwest
- **定时任务**: tokio-cron-scheduler
- **前端**: 原生HTML/CSS/JavaScript

## 项目结构

```
src/
├── main.rs          # 主程序入口
├── lib.rs           # 库模块导出
├── config/          # 配置管理
│   ├── mod.rs
│   └── database.rs  # 数据库操作
├── services/        # 业务服务
│   ├── mod.rs
│   ├── cloudflare.rs # Cloudflare API客户端
│   ├── config_service.rs # 配置服务
│   └── monitor_service.rs # 监控服务
├── utils/           # 工具函数
│   ├── mod.rs
│   └── network.rs   # 网络功能
├── api/             # Web API
│   ├── mod.rs
│   ├── handlers.rs  # 请求处理
│   └── routes.rs    # 路由配置
└── static/          # 静态文件
    ├── index.html   # 前端页面
    ├── css/
    │   └── style.css
    └── js/
        └── app.js
```

## 开发说明

### 构建项目
```bash
cargo build
```

### 运行测试
```bash
cargo test
```

### 发布构建
```bash
cargo build --release
```

## 注意事项

1. 确保本地网络支持IPv6
2. Cloudflare API密钥需要适当的权限
3. 服务需要持续运行以保持监控
4. 建议在生产环境中使用系统服务管理（如systemd）

## 许可证

MIT License