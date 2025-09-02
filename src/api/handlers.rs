use axum::{extract::State, Json, response::IntoResponse};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use crate::services::config_service::ConfigService;
use crate::config::database::{Database, DnsUpdateRecord};

#[derive(Debug, Deserialize)]
pub struct TestConfigRequest {
    pub api_key: String,
    pub zone_id: String,
    pub root_domain: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveConfigRequest {
    pub api_key: String,
    pub zone_id: String,
    pub root_domain: String,
    pub selected_subdomains: Vec<String>,
    pub check_interval: u64,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DomainListResponse {
    pub domains: Vec<String>,
    pub current_ip: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConfigStatus {
    pub configured: bool,
    pub current_config: Option<serde_json::Value>,
}

pub async fn test_config(
    State(service): State<ConfigService>,
    Json(payload): Json<TestConfigRequest>,
) -> impl IntoResponse {
    info!("🧪 收到配置测试请求，域名: {}", payload.root_domain);
    
    match service.test_config(&payload.api_key, &payload.zone_id, &payload.root_domain).await {
        Ok(true) => {
            info!("✅ 配置测试成功，域名: {}", payload.root_domain);
            Json(ApiResponse::<()> {
                success: true,
                data: None,
                message: Some("配置测试成功".to_string()),
            })
        },
        Ok(false) => {
            warn!("⚠️ 配置测试失败，域名: {}", payload.root_domain);
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some("配置测试失败".to_string()),
            })
        },
        Err(e) => {
            error!("❌ 配置测试错误，域名: {} - {}", payload.root_domain, e);
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some(format!("配置测试错误: {}", e)),
            })
        },
    }
}

pub async fn get_domain_list(
    State(service): State<ConfigService>,
    Json(payload): Json<TestConfigRequest>,
) -> impl IntoResponse {
    match service.get_domain_list(&payload.api_key, &payload.zone_id, &payload.root_domain).await {
        Ok(domains) => {
            let current_ip = service.get_current_ipv6().ok();
            Json(ApiResponse {
                success: true,
                data: Some(DomainListResponse { domains, current_ip }),
                message: None,
            })
        }
        Err(e) => Json(ApiResponse::<DomainListResponse> {
            success: false,
            data: None,
            message: Some(format!("获取域名列表失败: {}", e)),
        }),
    }
}

pub async fn save_config(
    State(service): State<ConfigService>,
    Json(payload): Json<SaveConfigRequest>,
) -> impl IntoResponse {
    info!("💾 收到配置保存请求，域名: {}，子域名数量: {}", 
          payload.root_domain, payload.selected_subdomains.len());
    
    match service.save_configuration_and_update(
        payload.api_key,
        payload.zone_id,
        payload.root_domain.clone(),
        payload.selected_subdomains.clone(),
        payload.check_interval,
    ).await {
        Ok(()) => {
            info!("✅ 配置保存并更新成功，域名: {}，检查间隔: {}秒", 
                  payload.root_domain, payload.check_interval);
            Json(ApiResponse::<()> {
                success: true,
                data: None,
                message: Some("配置保存并更新成功".to_string()),
            })
        },
        Err(e) => {
            error!("❌ 配置保存失败，域名: {} - {}", payload.root_domain, e);
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                message: Some(format!("配置保存失败: {}", e)),
            })
        },
    }
}

pub async fn get_config_status(
    State(service): State<ConfigService>,
) -> impl IntoResponse {
    let configured = service.has_configuration();
    let current_config = if configured {
        match service.load_configuration() {
            Ok(config) => Some(serde_json::to_value(config).unwrap()),
            Err(_) => None,
        }
    } else {
        None
    };
    
    Json(ApiResponse {
        success: true,
        data: Some(ConfigStatus {
            configured,
            current_config,
        }),
        message: None,
    })
}

pub async fn get_current_ip(
    State(service): State<ConfigService>,
) -> impl IntoResponse {
    match service.get_current_ipv6() {
        Ok(ip) => Json(ApiResponse {
            success: true,
            data: Some(ip),
            message: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            message: Some(format!("获取当前IP失败: {}", e)),
        }),
    }
}

#[derive(Debug, Serialize)]
pub struct DnsUpdateRecordsResponse {
    pub records: Vec<DnsUpdateRecord>,
}

/// 获取DNS更新记录
pub async fn get_dns_update_records() -> impl IntoResponse {
    let db = match Database::new() {
        Ok(db) => db,
        Err(e) => {
            error!("❌ 数据库连接失败: {}", e);
            return Json(ApiResponse::<DnsUpdateRecordsResponse> {
                success: false,
                data: None,
                message: Some(format!("数据库连接失败: {}", e)),
            });
        }
    };
    
    match db.get_recent_dns_update_records(50) {
        Ok(records) => {
            info!("📊 获取到 {} 条DNS更新记录", records.len());
            Json(ApiResponse {
                success: true,
                data: Some(DnsUpdateRecordsResponse { records }),
                message: None,
            })
        }
        Err(e) => {
            error!("❌ 获取DNS更新记录失败: {}", e);
            Json(ApiResponse::<DnsUpdateRecordsResponse> {
                success: false,
                data: None,
                message: Some(format!("获取DNS更新记录失败: {}", e)),
            })
        }
    }
}