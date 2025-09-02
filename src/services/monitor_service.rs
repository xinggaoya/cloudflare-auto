use tokio_cron_scheduler::{JobScheduler, Job};
use crate::{
    services::{
        config_service::ConfigService,
        cloudflare::{CloudflareClient, CloudflareConfig},
    },
    utils::network::get_preferred_ipv6,
    config::database::Database,
};
use anyhow::{Result, anyhow};
use std::time::Duration;
use tracing::{info, error, warn, debug};

pub struct MonitorService {
    config_service: ConfigService,
    scheduler: JobScheduler,
}

impl MonitorService {
    pub async fn new(config_service: ConfigService) -> Result<Self> {
        let scheduler = JobScheduler::new().await?;
        Ok(Self {
            config_service,
            scheduler,
        })
    }

    /// 启动监控服务
    pub async fn start(&mut self) -> Result<()> {
        if !self.config_service.has_configuration() {
            warn!("⚠️ 没有找到配置，监控服务未启动");
            return Ok(());
        }

        let config = self.config_service.load_configuration()?;
        let config_service_clone = self.config_service.clone();
        
        info!("🔍 配置监控任务，检查间隔: {}秒", config.check_interval);
        info!("📋 监控域名数量: {}", config.selected_subdomains.len());
        
        // 创建定时任务
        let job = Job::new_repeated_async(
            Duration::from_secs(config.check_interval), 
            move |_uuid, _l| {
                let config_service = config_service_clone.clone();
                Box::pin(async move {
                    debug!("🔄 开始执行监控任务");
                    if let Err(e) = Self::check_and_update(&config_service).await {
                        error!("❌ 监控任务执行失败: {}", e);
                    } else {
                        debug!("✅ 监控任务执行完成");
                    }
                })
            }
        )?;

        self.scheduler.add(job).await?;
        self.scheduler.start().await?;
        
        info!("✅ 监控服务已启动，检查间隔: {}秒", config.check_interval);
        
        Ok(())
    }

    /// 停止监控服务
    pub async fn stop(&mut self) -> Result<()> {
        self.scheduler.shutdown().await?;
        info!("🛑 监控服务已停止");
        Ok(())
    }

    /// 立即执行一次检查更新
    pub async fn check_and_update_now(&self) -> Result<bool> {
        Self::check_and_update(&self.config_service).await
    }

    /// 检查IP变化并更新
    async fn check_and_update(config_service: &ConfigService) -> Result<bool> {
        let config = config_service.load_configuration()?;
        
        // 获取当前IP
        let current_ip = match get_preferred_ipv6() {
            Ok(ip) => ip.to_string(),
            Err(e) => {
                error!("❌ 获取当前IP失败: {}", e);
                return Ok(false);
            }
        };
        
        debug!("🌐 当前检测到的IPv6地址: {}", current_ip);
        
        // 检查IP是否变化
        let last_ip = config_service.get_last_ip()?;
        if let Some(ref last_ip) = last_ip {
            if *last_ip == current_ip {
                debug!("✅ IP地址未变化: {}", current_ip);
                return Ok(false);
            }
        }
        
        info!("🔄 检测到IP地址变化: {} -> {}", last_ip.as_ref().unwrap_or(&"无".to_string()), current_ip);
        
        // 创建Cloudflare客户端
        let cf_config = CloudflareConfig {
            api_key: config.cloudflare_api_key,
            zone_id: config.cloudflare_zone_id,
            root_domain: config.root_domain.clone(),
        };
        
        let client = CloudflareClient::new(cf_config);
        
        // 更新选中的子域名
        let mut success_count = 0;
        let mut total_count = 0;
        let mut error_message = None;
        
        info!("📝 开始更新 {} 个域名记录", config.selected_subdomains.len());
        
        for subdomain in &config.selected_subdomains {
            total_count += 1;
            
            let full_domain = if subdomain.is_empty() {
                config.root_domain.clone()
            } else {
                format!("{}.{}", subdomain, config.root_domain)
            };
            
            debug!("🔍 处理域名: {}", full_domain);
            
            match client.get_aaaa_records(&full_domain).await {
                Ok(records) => {
                    if let Some(record) = records.first() {
                        // 检查IP是否真的发生了变化
                        if record.content == current_ip {
                            debug!("✅ IP地址未变化，跳过更新: {} -> {}", full_domain, current_ip);
                            success_count += 1; // 这种情况也算成功
                            continue;
                        }
                        
                        // 更新现有记录
                        debug!("📝 更新现有DNS记录: {} -> {}", full_domain, current_ip);
                        if let Ok(true) = client.update_dns_record(&record.id, current_ip.parse()?).await {
                            success_count += 1;
                            info!("✅ 成功更新域名: {} -> {}", full_domain, current_ip);
                        } else {
                            error!("❌ 更新域名失败: {}", full_domain);
                            error_message = Some(format!("更新域名失败: {}", full_domain));
                        }
                    } else {
                        // 创建新记录
                        debug!("➕ 创建新DNS记录: {} -> {}", full_domain, current_ip);
                        if let Ok(true) = client.create_aaaa_record(subdomain, current_ip.parse()?).await {
                            success_count += 1;
                            info!("✅ 成功创建域名: {} -> {}", full_domain, current_ip);
                        } else {
                            error!("❌ 创建域名失败: {}", full_domain);
                            error_message = Some(format!("创建域名失败: {}", full_domain));
                        }
                    }
                }
                Err(e) => {
                    error!("❌ 获取域名记录失败 {}: {}", full_domain, e);
                    error_message = Some(format!("获取域名记录失败 {}: {}", full_domain, e));
                }
            }
        }
        
        // 记录DNS更新记录
        let db = Database::new()?;
        if let Err(e) = db.add_dns_update_record(
            last_ip.clone(),
            &current_ip,
            total_count as i32,
            success_count as i32,
            error_message.clone(),
        ) {
            error!("❌ 记录DNS更新记录失败: {}", e);
        }
        
        // 更新最后记录的IP
        if success_count > 0 {
            config_service.update_last_ip(&current_ip)?;
            info!("🎉 IP更新完成: 成功 {}/{} 个域名", success_count, total_count);
            Ok(true)
        } else {
            error!("❌ 所有域名更新都失败了");
            Err(anyhow!("所有域名更新都失败了"))
        }
    }
}