use reqwest::{Client, header::{HeaderMap, HeaderValue, AUTHORIZATION}};
use serde::{Deserialize, Serialize};
use anyhow::{Result, anyhow};
use std::net::IpAddr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{warn, debug};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CloudflareConfig {
    pub api_key: String,
    pub zone_id: String,
    pub root_domain: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub content: String,
    pub proxied: bool,
    pub ttl: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DnsRecordResponse {
    pub result: Vec<DnsRecord>,
    pub success: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct UpdateDnsRecordRequest {
    #[serde(rename = "type")]
    pub record_type: String,
    pub name: String,
    pub content: String,
    pub ttl: u32,
    pub proxied: bool,
}

pub struct CloudflareClient {
    client: Client,
    config: CloudflareConfig,
}

impl CloudflareClient {
    pub fn new(config: CloudflareConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// 带重试的HTTP请求执行
    async fn execute_with_retry<F, T>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>> + Send + Sync,
    {
        const MAX_RETRIES: u32 = 3;
        const RETRY_DELAY: Duration = Duration::from_secs(2);
        
        let mut last_error = None;
        
        for attempt in 1..=MAX_RETRIES {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < MAX_RETRIES {
                        warn!("⚠️ Cloudflare API请求失败 (尝试 {}/{}), {}秒后重试: {}", 
                            attempt, MAX_RETRIES, RETRY_DELAY.as_secs(), last_error.as_ref().unwrap());
                        sleep(RETRY_DELAY * attempt).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap())
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION, 
            HeaderValue::from_str(&format!("Bearer {}", self.config.api_key)).unwrap()
        );
        headers.insert(
            "Content-Type", 
            HeaderValue::from_static("application/json")
        );
        headers
    }

    /// 测试Cloudflare API连接
    pub async fn test_connection(&self) -> Result<bool> {
        let url = format!("https://api.cloudflare.com/client/v4/zones/{}", self.config.zone_id);
        
        let response = self.execute_with_retry(|| {
            let client = self.client.clone();
            let url = url.clone();
            let headers = self.build_headers();
            
            Box::pin(async move {
                let response = client
                    .get(&url)
                    .headers(headers)
                    .send()
                    .await?;
                
                if response.status().is_success() {
                    Ok(true)
                } else {
                    Err(anyhow!("Cloudflare API测试失败: {}", response.status()))
                }
            })
        }).await?;
        
        Ok(response)
    }

    /// 获取所有DNS记录
    pub async fn get_dns_records(&self) -> Result<Vec<DnsRecord>> {
        let mut all_records = Vec::new();
        let mut page = 1;
        const PER_PAGE: u32 = 100; // Cloudflare API每页最大记录数
        
        loop {
            let url = format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records?page={}&per_page={}",
                self.config.zone_id, page, PER_PAGE
            );
            
            let dns_response = self.execute_with_retry(|| {
                let client = self.client.clone();
                let url = url.clone();
                let headers = self.build_headers();
                
                Box::pin(async move {
                    let response = client
                        .get(&url)
                        .headers(headers)
                        .send()
                        .await?;
                    
                    if response.status().is_success() {
                        let dns_response: DnsRecordResponse = response.json().await?;
                        if dns_response.success {
                            Ok(dns_response.result)
                        } else {
                            Err(anyhow!("获取DNS记录失败"))
                        }
                    } else {
                        Err(anyhow!("HTTP请求失败: {}", response.status()))
                    }
                })
            }).await?;
            
            let response_len = dns_response.len();
            if response_len == 0 {
                break;
            }
            
            all_records.extend(dns_response);
            page += 1;
            
            // 如果返回的记录数少于每页数量，说明已经是最后一页
            if response_len < PER_PAGE as usize {
                break;
            }
        }
        
        Ok(all_records)
    }

    /// 获取指定域名的AAAA记录
    pub async fn get_aaaa_records(&self, domain: &str) -> Result<Vec<DnsRecord>> {
        let records = self.get_dns_records().await?;
        
        // 调试：打印所有记录以帮助诊断
        debug!("🔍 获取到 {} 条DNS记录，正在查找域名: {}", records.len(), domain);
        for record in &records {
            if record.record_type == "AAAA" {
                debug!("📋 AAAA记录: {} -> {}", record.name, record.content);
            }
        }
        
        let aaaa_records: Vec<DnsRecord> = records
            .into_iter()
            .filter(|record| 
                record.record_type == "AAAA" && 
                record.name == domain
            )
            .collect();
        
        debug!("✅ 找到 {} 条匹配的AAAA记录 for {}", aaaa_records.len(), domain);
        
        Ok(aaaa_records)
    }

    /// 更新DNS记录
    pub async fn update_dns_record(&self, record_id: &str, ip: IpAddr) -> Result<bool> {
        debug!("🔄 开始更新DNS记录: ID={}, IP={}", record_id, ip);
        
        // 首先获取记录的详细信息，以获取正确的域名
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
            self.config.zone_id, record_id
        );
        
        // 获取记录信息
        let record_info = self.execute_with_retry(|| {
            let client = self.client.clone();
            let url = url.clone();
            let headers = self.build_headers();
            
            Box::pin(async move {
                let response = client
                    .get(&url)
                    .headers(headers)
                    .send()
                    .await?;
                
                if response.status().is_success() {
                    let record_response: serde_json::Value = response.json().await?;
                    if let Some(result) = record_response.get("result") {
                        if let Some(name) = result.get("name") {
                            if let Some(domain_name) = name.as_str() {
                                debug!("📋 获取到记录域名: {}", domain_name);
                                Ok(domain_name.to_string())
                            } else {
                                Err(anyhow!("无法获取域名名称"))
                            }
                        } else {
                            Err(anyhow!("记录中缺少name字段"))
                        }
                    } else {
                        Err(anyhow!("API响应中缺少result字段"))
                    }
                } else {
                    Err(anyhow!("获取记录信息失败: {}", response.status()))
                }
            })
        }).await?;
        
        debug!("📝 准备更新域名: {} -> {}", record_info, ip);
        
        // 使用获取到的域名进行更新
        let update_request = UpdateDnsRecordRequest {
            record_type: "AAAA".to_string(),
            name: record_info,
            content: ip.to_string(),
            ttl: 1, // 自动TTL
            proxied: false, // 不通过Cloudflare代理
        };
        
        let result = self.execute_with_retry(|| {
            let client = self.client.clone();
            let url = url.clone();
            let headers = self.build_headers();
            let update_request = update_request.clone();
            
            Box::pin(async move {
                let response = client
                    .put(&url)
                    .headers(headers)
                    .json(&update_request)
                    .send()
                    .await?;
                
                if response.status().is_success() {
                    debug!("✅ DNS记录更新成功");
                    Ok(true)
                } else {
                    let error_text = response.text().await?;
                    debug!("❌ DNS记录更新失败: {}", error_text);
                    Err(anyhow!("更新DNS记录失败: {}", error_text))
                }
            })
        }).await?;
        
        Ok(result)
    }

    /// 创建新的AAAA记录
    pub async fn create_aaaa_record(&self, subdomain: &str, ip: IpAddr) -> Result<bool> {
        let full_domain = if subdomain.is_empty() {
            self.config.root_domain.clone()
        } else {
            format!("{}.{}", subdomain, self.config.root_domain)
        };
        
        debug!("➕ 开始创建AAAA记录: {} -> {}", full_domain, ip);
        
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
            self.config.zone_id
        );
        
        let create_request = UpdateDnsRecordRequest {
            record_type: "AAAA".to_string(),
            name: full_domain.clone(),
            content: ip.to_string(),
            ttl: 1,
            proxied: false,
        };
        
        let result = self.execute_with_retry(|| {
            let client = self.client.clone();
            let url = url.clone();
            let headers = self.build_headers();
            let create_request = create_request.clone();
            let full_domain_clone = full_domain.clone();
            
            Box::pin(async move {
                let response = client
                    .post(&url)
                    .headers(headers)
                    .json(&create_request)
                    .send()
                    .await?;
                
                if response.status().is_success() {
                    debug!("✅ AAAA记录创建成功: {}", full_domain_clone);
                    Ok(true)
                } else {
                    let error_text = response.text().await?;
                    debug!("❌ AAAA记录创建失败: {} - {}", full_domain_clone, error_text);
                    Err(anyhow!("创建DNS记录失败: {}", error_text))
                }
            })
        }).await?;
        
        Ok(result)
    }
}