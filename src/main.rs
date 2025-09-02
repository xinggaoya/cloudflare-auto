mod config;
mod services;
mod utils;
mod api;

use axum::Router;
use std::net::SocketAddr;
use std::str::FromStr;
use std::env;
use tokio::{net::TcpListener, signal};
use tracing::{info, error, warn};
use crate::services::{config_service::ConfigService, monitor_service::MonitorService};
use crate::utils::logger::{init_logger, start_log_cleanup_task};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志系统 - 支持控制台和文件同步输出
    let _guard = init_logger()?;
    
    info!("🚀 启动Cloudflare自动IPv6更新服务...");
    info!("📝 日志系统已初始化，支持控制台和文件同步输出");
    
    // 启动日志清理定时任务
    if let Err(e) = start_log_cleanup_task().await {
        warn!("⚠️ 启动日志清理任务失败: {}", e);
    }
    
    // 初始化配置服务
    info!("⚙️ 初始化配置服务...");
    let config_service = ConfigService::new()?;
    info!("✅ 配置服务初始化完成");
    
    // 初始化监控服务
    info!("🔍 初始化监控服务...");
    let mut monitor_service = MonitorService::new(config_service.clone()).await?;
    info!("✅ 监控服务初始化完成");
    
    // 启动监控服务
    info!("🔄 启动监控服务...");
    if let Err(e) = monitor_service.start().await {
        error!("❌ 启动监控服务失败: {}", e);
        return Err(e);
    }
    info!("✅ 监控服务启动成功");

    // 程序启动时立即执行一次检查更新
    info!("🔍 程序启动，立即执行首次IP检查...");
    if let Err(e) = monitor_service.check_and_update_now().await {
        warn!("⚠️ 首次IP检查失败: {}", e);
    } else {
        info!("✅ 首次IP检查完成");
    }
    
    // 创建Web服务器
    info!("🌐 创建Web服务器...");
    let app = Router::new()
        .merge(api::configure_routes())
        .with_state(config_service);
    
    // 读取监听地址，优先使用环境变量 BIND_ADDR（示例：0.0.0.0:3000），默认 127.0.0.1:3000
    let bind_addr_str = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let addr = SocketAddr::from_str(&bind_addr_str)
        .map_err(|e| anyhow::anyhow!("无效的 BIND_ADDR 格式：{} ({})", bind_addr_str, e))?;
    info!("🌐 Web服务启动在: http://{}", addr);
    info!("📱 可通过浏览器访问Web管理界面");
    
    // 启动服务器
    info!("🚀 启动HTTP服务器...");
    let listener = TcpListener::bind(addr).await?;
    info!("✅ HTTP服务器启动成功，等待连接...");
    
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    
    info!("👋 服务已正常关闭");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("📡 收到关闭信号，正在停止服务...");
}