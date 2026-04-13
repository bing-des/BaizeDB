use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    pub db_type: DbType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: Option<String>,
    pub ssl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    MySQL,
    PostgreSQL,
    Redis,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::new(),
            db_type: DbType::MySQL,
            host: "localhost".to_string(),
            port: 3306,
            username: "root".to_string(),
            password: String::new(),
            database: None,
            ssl: false,
        }
    }
}

#[derive(Clone)]
pub enum DbPool {
    MySQL(sqlx::MySqlPool),
    PostgreSQL(sqlx::PgPool),
    Redis(redis::aio::MultiplexedConnection),
}

pub struct AppState {
    /// 运行时连接配置缓存（供快速查询）
    pub connections: Arc<RwLock<HashMap<String, ConnectionConfig>>>,
    /// 主连接池（connection_id → pool）
    pub pools: Arc<RwLock<HashMap<String, DbPool>>>,

    /// 数据库级别连接池（"connection_id:database" → pool），PG 展开不同库时按需创建
    pub db_pools: Arc<RwLock<HashMap<String, DbPool>>>,
    /// 连接配置存储（trait 对象，屏蔽底层实现）
    pub store: Arc<crate::store::connection_store::SqliteConnectionStore>,
}

impl AppState {
    pub fn new(
        store: Arc<crate::store::connection_store::SqliteConnectionStore>,
    ) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            pools: Arc::new(RwLock::new(HashMap::new())),
            db_pools: Arc::new(RwLock::new(HashMap::new())),
            store,
        }
    }
}
