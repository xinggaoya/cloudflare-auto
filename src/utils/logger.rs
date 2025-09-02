use std::path::Path;
use std::fs;
use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};
use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{RollingFileAppender, Rotation},
};

/// 初始化日志系统
/// 支持控制台和文件同步输出，自动日志轮转
pub fn init_logger() -> anyhow::Result<WorkerGuard> {
    // 创建日志目录
    let log_dir = "logs";
    if !Path::new(log_dir).exists() {
        fs::create_dir_all(log_dir)?;
    }

    // 配置日志轮转 - 每天轮转一次，保留7天
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        log_dir,
        "cloudflare-auto.log",
    );

    // 创建非阻塞写入器
    let (non_blocking_appender, guard) = tracing_appender::non_blocking(file_appender);

    // 配置环境过滤器 - 默认INFO级别，可通过RUST_LOG环境变量调整
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // 配置控制台输出格式
    let console_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .compact();

    // 配置文件输出格式
    let file_layer = fmt::layer()
        .with_writer(non_blocking_appender)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .json();

    // 初始化订阅者
    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    Ok(guard)
}

/// 清理旧日志文件
/// 删除超过指定天数的日志文件
pub fn cleanup_old_logs(days_to_keep: u64) -> anyhow::Result<()> {
    let log_dir = Path::new("logs");
    if !log_dir.exists() {
        return Ok(());
    }

    let cutoff_time = std::time::SystemTime::now()
        - std::time::Duration::from_secs(days_to_keep * 24 * 60 * 60);

    let entries = fs::read_dir(log_dir)?;
    let mut deleted_count = 0;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        
        // 只处理.log文件
        if path.extension().and_then(|s| s.to_str()) == Some("log") {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if modified < cutoff_time {
                        if let Err(e) = fs::remove_file(&path) {
                            tracing::warn!("删除旧日志文件失败: {} - {}", path.display(), e);
                        } else {
                            deleted_count += 1;
                            tracing::info!("删除旧日志文件: {}", path.display());
                        }
                    }
                }
            }
        }
    }

    if deleted_count > 0 {
        tracing::info!("清理完成，删除了 {} 个旧日志文件", deleted_count);
    }

    Ok(())
}

/// 启动日志清理定时任务
pub async fn start_log_cleanup_task() -> anyhow::Result<()> {
    use tokio_cron_scheduler::{Job, JobScheduler};
    
    let sched = JobScheduler::new().await?;
    
    // 每天凌晨2点执行日志清理
    sched.add(
        Job::new_async("0 0 2 * * *", |_uuid, _l| {
            Box::pin(async {
                if let Err(e) = cleanup_old_logs(7) {
                    tracing::error!("日志清理任务执行失败: {}", e);
                }
            })
        })?
    ).await?;

    sched.start().await?;
    tracing::info!("日志清理定时任务已启动，每天凌晨2点执行");
    
    Ok(())
}
