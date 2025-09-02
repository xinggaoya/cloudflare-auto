use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use anyhow::Result;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub cloudflare_api_key: String,
    pub cloudflare_zone_id: String,
    pub root_domain: String,
    pub selected_subdomains: Vec<String>,
    pub check_interval: u64, // 检查间隔（秒）
    pub last_ip: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DnsUpdateRecord {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub old_ip: Option<String>,
    pub new_ip: String,
    pub domain_count: i32,
    pub success_count: i32,
    pub error_message: Option<String>,
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_path = "config.db";
        let conn = Connection::open(db_path)?;
        
        // 创建配置表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS config (
                id INTEGER PRIMARY KEY,
                cloudflare_api_key TEXT NOT NULL,
                cloudflare_zone_id TEXT NOT NULL,
                root_domain TEXT NOT NULL,
                selected_subdomains TEXT NOT NULL,
                check_interval INTEGER DEFAULT 300,
                last_ip TEXT
            )",
            [],
        )?;

        // 创建DNS更新记录表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS dns_update_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                old_ip TEXT,
                new_ip TEXT,
                domain_count INTEGER,
                success_count INTEGER,
                error_message TEXT
            )",
            [],
        )?;
        
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// 保存配置
    pub fn save_config(&self, config: &AppConfig) -> Result<()> {
        let subdomains_json = serde_json::to_string(&config.selected_subdomains)
            .unwrap_or_else(|_| "[]".to_string());
        
        let conn = self.conn.lock().unwrap();
        
        // 先删除旧配置
        conn.execute("DELETE FROM config", [])?;
        
        // 插入新配置
        conn.execute(
            "INSERT INTO config (
                cloudflare_api_key, 
                cloudflare_zone_id, 
                root_domain, 
                selected_subdomains, 
                check_interval, 
                last_ip
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                config.cloudflare_api_key,
                config.cloudflare_zone_id,
                config.root_domain,
                subdomains_json,
                config.check_interval,
                config.last_ip
            ],
        )?;
        
        Ok(())
    }

    /// 加载配置
    pub fn load_config(&self) -> Result<AppConfig> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT 
                cloudflare_api_key, 
                cloudflare_zone_id, 
                root_domain, 
                selected_subdomains, 
                check_interval, 
                last_ip 
             FROM config LIMIT 1"
        )?;
        
        let config = stmt.query_row([], |row| {
            let subdomains_json: String = row.get(3)?;
            let selected_subdomains: Vec<String> = serde_json::from_str(&subdomains_json)
                .unwrap_or_else(|_| Vec::new());
            
            Ok(AppConfig {
                cloudflare_api_key: row.get(0)?,
                cloudflare_zone_id: row.get(1)?,
                root_domain: row.get(2)?,
                selected_subdomains,
                check_interval: row.get(4)?,
                last_ip: row.get(5)?,
            })
        })?;
        
        Ok(config)
    }

    /// 检查是否有配置
    pub fn has_config(&self) -> bool {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM config", [], |row| row.get(0))
            .unwrap_or(0);
        
        count > 0
    }

    /// 更新最后记录的IP地址
    pub fn update_last_ip(&self, ip: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE config SET last_ip = ?1",
            params![ip],
        )?;
        
        Ok(())
    }

    /// 获取最后记录的IP地址
    pub fn get_last_ip(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT last_ip FROM config LIMIT 1")?;
        
        let last_ip: Option<String> = stmt.query_row([], |row| row.get(0))?;
        
        Ok(last_ip)
    }

    /// 添加DNS更新记录
    pub fn add_dns_update_record(
        &self,
        old_ip: Option<String>,
        new_ip: &str,
        domain_count: i32,
        success_count: i32,
        error_message: Option<String>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO dns_update_records (old_ip, new_ip, domain_count, success_count, error_message) 
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![old_ip, new_ip, domain_count, success_count, error_message],
        )?;
        
        Ok(())
    }

    /// 获取所有DNS更新记录，按时间倒序排列
    pub fn get_dns_update_records(&self, limit: Option<i32>) -> Result<Vec<DnsUpdateRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut query = "
            SELECT id, timestamp, old_ip, new_ip, domain_count, success_count, error_message 
            FROM dns_update_records 
            ORDER BY timestamp DESC
        ".to_string();
        
        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }
        
        let mut stmt = conn.prepare(&query)?;
        let records = stmt.query_map([], |row| {
            Ok(DnsUpdateRecord {
                id: row.get(0)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                old_ip: row.get(2)?,
                new_ip: row.get(3)?,
                domain_count: row.get(4)?,
                success_count: row.get(5)?,
                error_message: row.get(6)?,
            })
        })?;
        
        let mut result = Vec::new();
        for record in records {
            result.push(record?);
        }
        
        Ok(result)
    }

    /// 获取最近的DNS更新记录
    pub fn get_recent_dns_update_records(&self, count: i32) -> Result<Vec<DnsUpdateRecord>> {
        self.get_dns_update_records(Some(count))
    }
}