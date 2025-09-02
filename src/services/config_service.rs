use crate::config::database::{Database, AppConfig};
use crate::services::cloudflare::{CloudflareClient, CloudflareConfig};
use crate::utils::network::get_preferred_ipv6;
use anyhow::Result;
use tracing::{info, error};

#[derive(Clone)]
pub struct ConfigService {
    db: Database,
}

impl ConfigService {
    pub fn new() -> Result<Self> {
        let db = Database::new()?;
        Ok(Self { db })
    }

    /// 测试Cloudflare配置
    pub async fn test_config(
        &self, 
        api_key: &str, 
        zone_id: &str, 
        root_domain: &str
    ) -> Result<bool> {
        let config = CloudflareConfig {
            api_key: api_key.to_string(),
            zone_id: zone_id.to_string(),
            root_domain: root_domain.to_string(),
        };
        
        let client = CloudflareClient::new(config);
        client.test_connection().await
    }

    /// 保存配置
    pub fn save_configuration(
        &self,
        api_key: String,
        zone_id: String,
        root_domain: String,
        selected_subdomains: Vec<String>,
        check_interval: u64,
    ) -> Result<()> {
        // 先获取当前IP，用于初始化配置
        let current_ip = match get_preferred_ipv6() {
            Ok(ip) => Some(ip.to_string()),
            Err(_) => None,
        };
        
        let config = AppConfig {
            cloudflare_api_key: api_key,
            cloudflare_zone_id: zone_id,
            root_domain,
            selected_subdomains,
            check_interval,
            last_ip: current_ip,
        };
        
        self.db.save_config(&config)
    }

    /// 保存配置并立即更新
    pub async fn save_configuration_and_update(
        &self,
        api_key: String,
        zone_id: String,
        root_domain: String,
        selected_subdomains: Vec<String>,
        check_interval: u64,
    ) -> Result<()> {
        // 先获取当前IP，用于初始化配置
        let current_ip = match get_preferred_ipv6() {
            Ok(ip) => Some(ip.to_string()),
            Err(_) => None,
        };
        
        let config = AppConfig {
            cloudflare_api_key: api_key,
            cloudflare_zone_id: zone_id,
            root_domain: root_domain.clone(),
            selected_subdomains: selected_subdomains.clone(),
            check_interval,
            last_ip: current_ip,
        };
        
        self.db.save_config(&config)?;
        
        // 保存配置后立即执行更新
        info!("💾 配置保存完成，开始立即更新...");
        if let Err(e) = self.check_and_update_now().await {
            error!("❌ 立即更新失败: {}", e);
            // 不返回错误，因为配置保存成功了
        }
        
        Ok(())
    }

    /// 加载配置
    pub fn load_configuration(&self) -> Result<AppConfig> {
        self.db.load_config()
    }

    /// 检查是否有配置
    pub fn has_configuration(&self) -> bool {
        self.db.has_config()
    }

    /// 获取域名列表
    pub async fn get_domain_list(
        &self,
        api_key: &str,
        zone_id: &str,
        root_domain: &str
    ) -> Result<Vec<String>> {
        let config = CloudflareConfig {
            api_key: api_key.to_string(),
            zone_id: zone_id.to_string(),
            root_domain: root_domain.to_string(),
        };
        
        let client = CloudflareClient::new(config);
        let records = client.get_dns_records().await?;
        
        // 提取所有子域名
        let mut subdomains = Vec::new();
        for record in records {
            if record.name != root_domain && record.name.ends_with(&format!(".{}", root_domain)) {
                let subdomain = record.name
                    .trim_end_matches(&format!(".{}", root_domain))
                    .to_string();
                if !subdomain.is_empty() {
                    subdomains.push(subdomain);
                }
            }
        }
        
        subdomains.sort();
        subdomains.dedup();
        
        Ok(subdomains)
    }

    /// 更新最后记录的IP
    pub fn update_last_ip(&self, ip: &str) -> Result<()> {
        self.db.update_last_ip(ip)
    }

    /// 获取最后记录的IP
    pub fn get_last_ip(&self) -> Result<Option<String>> {
        self.db.get_last_ip()
    }

    /// 获取当前IPv6地址
    pub fn get_current_ipv6(&self) -> Result<String> {
        let ip = get_preferred_ipv6()?;
        Ok(ip.to_string())
    }

    /// 立即执行IP检查和更新
    pub async fn check_and_update_now(&self) -> Result<bool> {
        if !self.has_configuration() {
            info!("⚠️ 没有配置，跳过立即更新");
            return Ok(false);
        }

        let config = self.load_configuration()?;
        
        // 获取当前IP
        let current_ip = match get_preferred_ipv6() {
            Ok(ip) => ip.to_string(),
            Err(e) => {
                error!("❌ 获取当前IP失败: {}", e);
                return Ok(false);
            }
        };

        info!("🌐 立即更新 - 当前检测到的IPv6地址: {}", current_ip);
        
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
        
        info!("📝 立即更新 - 开始更新 {} 个域名记录", config.selected_subdomains.len());
        
        for subdomain in &config.selected_subdomains {
            total_count += 1;
            
            let full_domain = if subdomain.is_empty() {
                config.root_domain.clone()
            } else {
                format!("{}.{}", subdomain, config.root_domain)
            };
            
            info!("🔍 立即更新 - 处理域名: {}", full_domain);
            
            match client.get_aaaa_records(&full_domain).await {
                Ok(records) => {
                    if let Some(record) = records.first() {
                        // 检查IP是否真的发生了变化
                        if record.content == current_ip {
                            info!("✅ 立即更新 - IP地址未变化，跳过更新: {} -> {}", full_domain, current_ip);
                            success_count += 1; // 这种情况也算成功
                            continue;
                        }
                        
                        // 更新现有记录
                        if let Ok(true) = client.update_dns_record(&record.id, current_ip.parse()?).await {
                            success_count += 1;
                            info!("✅ 立即更新 - 成功更新域名: {} -> {}", full_domain, current_ip);
                        } else {
                            error!("❌ 立即更新 - 更新域名失败: {}", full_domain);
                            error_message = Some(format!("更新域名失败: {}", full_domain));
                        }
                    } else {
                        // 创建新记录
                        if let Ok(true) = client.create_aaaa_record(subdomain, current_ip.parse()?).await {
                            success_count += 1;
                            info!("✅ 立即更新 - 成功创建域名: {} -> {}", full_domain, current_ip);
                        } else {
                            error!("❌ 立即更新 - 创建域名失败: {}", full_domain);
                            error_message = Some(format!("创建域名失败: {}", full_domain));
                        }
                    }
                }
                Err(e) => {
                    error!("❌ 立即更新 - 获取域名记录失败 {}: {}", full_domain, e);
                    error_message = Some(format!("获取域名记录失败 {}: {}", full_domain, e));
                }
            }
        }
        
        // 记录DNS更新记录
        let last_ip = self.get_last_ip()?;
        if let Err(e) = self.db.add_dns_update_record(
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
            self.update_last_ip(&current_ip)?;
            info!("🎉 立即更新完成: 成功 {}/{} 个域名", success_count, total_count);
            Ok(true)
        } else {
            error!("❌ 立即更新 - 所有域名更新都失败了");
            Ok(false)
        }
    }
}