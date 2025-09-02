use axum::{
    routing::{get, post},
    Router, response::Html,
};
use tower_http::services::ServeDir;
use crate::services::config_service::ConfigService;
use super::handlers::*;

pub fn configure_routes() -> Router<ConfigService> {
    Router::new()
        // 根路径返回主页面
        .route("/", get(index_handler))
        // API路由
        .route("/api/test-config", post(test_config))
        .route("/api/domain-list", post(get_domain_list))
        .route("/api/save-config", post(save_config))
        .route("/api/config-status", get(get_config_status))
        .route("/api/current-ip", get(get_current_ip))
        .route("/api/dns-update-records", get(get_dns_update_records))
        // 静态文件服务
        .nest_service("/static", ServeDir::new("static"))
        // 为了兼容性，也提供直接的静态文件访问
        .nest_service("/css", ServeDir::new("static/css"))
        .nest_service("/js", ServeDir::new("static/js"))
}

// 根路径处理器，返回主页面
async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../../static/index.html"))
}