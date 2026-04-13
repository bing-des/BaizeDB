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

impl DbPool {
    /// 获取统一的数据库操作句柄，屏蔽 MySQL/PG 差异。
    ///
    /// PG 多库连接池逻辑收敛在此处：
    /// - MySQL: 直接使用主连接池
    /// - PostgreSQL: 根据 database 参数从 db_pools 获取/创建目标库连接池
    pub async fn as_db_ops(
        &self,
        state: &AppState,
        connection_id: &str,
        database: &str,
    ) -> Result<crate::database::wrapper::AnyDbPool, String> {
        use std::sync::Arc;
        use crate::database::wrapper::AnyDbPool;

        match self {
            DbPool::MySQL(pool) => Ok(AnyDbPool::MySQL(Arc::new(pool.clone()))),
            DbPool::PostgreSQL(_) => {
                let pool = ensure_pg_db_pool(state, connection_id, database).await?;
                Ok(AnyDbPool::PG(Arc::new(pool)))
            }
            DbPool::Redis(_) => Err("Redis 不支持数据库操作".into()),
        }
    }
}

/// 确保 PG 指定数据库的连接池存在，返回克隆的 PgPool
pub async fn ensure_pg_db_pool(
    state: &AppState,
    connection_id: &str,
    database: &str,
) -> Result<sqlx::PgPool, String> {
    let db_key = format!("{}:{}", connection_id, database);

    // 快速路径：已存在
    {
        let db_pools = state.db_pools.read().await;
        if let Some(DbPool::PostgreSQL(p)) = db_pools.get(&db_key) {
            return Ok(p.clone());
        }
    }

    // 慢速路径：创建新连接
    let cfg = {
        let conns = state.connections.read().await;
        conns.get(connection_id).cloned().ok_or("连接配置不存在")?
    };

    let url = format!(
        "postgres://{}:{}@{}:{}/{}",
        cfg.username, cfg.password, cfg.host, cfg.port, database
    );

    let new_pool = sqlx::PgPool::connect(&url)
        .await
        .map_err(|e| format!("连接数据库 {} 失败: {}", database, e))?;

    let mut db_pools = state.db_pools.write().await;
    // 再次检查，避免并发创建
    if let Some(DbPool::PostgreSQL(p)) = db_pools.get(&db_key) {
        Ok(p.clone())
    } else {
        db_pools.insert(db_key.clone(), DbPool::PostgreSQL(new_pool.clone()));
        Ok(new_pool)
    }
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
