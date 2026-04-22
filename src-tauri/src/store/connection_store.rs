//! 连接配置存储 + 关系存储（委托给 RelationStore）
//!
//! 保持原有 API 兼容，内部委托给 relation_store 模块。

use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tauri::Manager;


use crate::state::{ConnectionConfig, DbType};
use crate::store::harness_types::{TableRelationAnalysis, LlmConfig};
use crate::store::relation_store::RelationStore;

/// 列元信息（用于 LLM 分析）
#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub key: Option<String>,
}

/// 表结构（用于 LLM 分析）
#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnMeta>,
}

// ---------------------------------------------------------------------------
// SQLite 实现
// ---------------------------------------------------------------------------

pub struct SqliteConnectionStore {
    pool: Arc<RwLock<Option<SqlitePool>>>,
}

impl SqliteConnectionStore {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(RwLock::new(None)),
        }
    }

    async fn get_pool(&self) -> Result<SqlitePool, String> {
        self.pool.read().await
            .clone()
            .ok_or_else(|| "SQLite 未初始化".to_string())
    }

    /// 建表并创建连接池
    pub async fn init_pool(&self, db_path: &std::path::Path) -> Result<(), String> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
            .await
            .map_err(|e| format!("创建 SQLite 连接池失败: {}", e))?;

        // 创建连接配置表
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS connections (
                id   TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                db_type TEXT NOT NULL,
                host TEXT NOT NULL,
                port  INTEGER NOT NULL,
                username TEXT NOT NULL,
                password TEXT NOT NULL,
                database TEXT,
                ssl   INTEGER NOT NULL DEFAULT 0
            )"#,
        )
        .execute(&pool)
        .await
        .map_err(|e| format!("建表失败: {}", e))?;

        // 创建表关系分析表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS table_relations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                connection_id TEXT NOT NULL,
                database TEXT NOT NULL,
                source_table TEXT NOT NULL,
                source_column TEXT NOT NULL,
                target_table TEXT NOT NULL,
                target_column TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                confidence REAL NOT NULL,
                reason TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(&pool)
        .await
        .map_err(|e| format!("创建表关系表失败: {}", e))?;

        // 创建唯一索引
        sqlx::query(
            r#"
            CREATE UNIQUE INDEX IF NOT EXISTS idx_relations_unique 
            ON table_relations(connection_id, database, source_table, source_column)
            "#
        )
        .execute(&pool)
        .await
        .map_err(|e| format!("创建索引失败: {}", e))?;

        // 创建查询索引
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_relations_conn_db 
            ON table_relations(connection_id, database)
            "#
        )
        .execute(&pool)
        .await
        .map_err(|e| format!("创建索引失败: {}", e))?;

        // 创建设置表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(&pool)
        .await
        .map_err(|e| format!("创建设置表失败: {}", e))?;

        // 创建 LLM 配置表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS llm_config (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                api_key TEXT NOT NULL DEFAULT '',
                api_url TEXT NOT NULL DEFAULT 'https://api.openai.com/v1/chat/completions',
                model TEXT NOT NULL DEFAULT 'gpt-3.5-turbo',
                enabled INTEGER NOT NULL DEFAULT 0,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(&pool)
        .await
        .map_err(|e| format!("创建 LLM 配置表失败: {}", e))?;

        // 插入默认配置（如果不存在）
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO llm_config (id, api_key, api_url, model, enabled)
            VALUES (1, '', 'https://api.openai.com/v1/chat/completions', 'gpt-3.5-turbo', 0)
            "#
        )
        .execute(&pool)
        .await
        .map_err(|e| format!("插入默认配置失败: {}", e))?;

        let mut guard = self.pool.write().await;
        *guard = Some(pool);
        Ok(())
    }
}

/// 连接配置存储抽象 trait
#[async_trait::async_trait]
#[allow(dead_code)]
pub trait ConnectionStore: Send + Sync {
    async fn init(&self) -> Result<(), String>;
    async fn insert(&self, config: &ConnectionConfig) -> Result<(), String>;
    async fn delete(&self, id: &str) -> Result<(), String>;
    async fn list_all(&self) -> Result<Vec<ConnectionConfig>, String>;
}

#[async_trait::async_trait]
impl ConnectionStore for SqliteConnectionStore {
    async fn init(&self) -> Result<(), String> {
        Ok(())
    }

    async fn insert(&self, config: &ConnectionConfig) -> Result<(), String> {
        let pool = self.get_pool().await?;
        let db_type = match config.db_type {
            DbType::MySQL => "mysql",
            DbType::PostgreSQL => "postgresql",
            DbType::Redis => "redis",
        };

        sqlx::query(
            "INSERT INTO connections (id, name, db_type, host, port, username, password, database, ssl)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&config.id)
        .bind(&config.name)
        .bind(db_type)
        .bind(&config.host)
        .bind(config.port as i64)
        .bind(&config.username)
        .bind(&config.password)
        .bind(&config.database)
        .bind(config.ssl)
        .execute(&pool)
        .await
        .map_err(|e| format!("插入连接失败: {}", e))?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), String> {
        let pool = self.get_pool().await?;
        sqlx::query("DELETE FROM connections WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| format!("删除连接失败: {}", e))?;
        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<ConnectionConfig>, String> {
        let pool = self.get_pool().await?;

        let rows = sqlx::query_as::<_, (String, String, String, String, i64, String, String, Option<String>, bool)>(
            "SELECT id, name, db_type, host, port, username, password, database, ssl FROM connections ORDER BY name"
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("查询连接失败: {}", e))?;

        let configs = rows.into_iter().map(|row| {
            let db_type = match row.2.as_str() {
                "postgresql" => DbType::PostgreSQL,
                "redis" => DbType::Redis,
                _ => DbType::MySQL,
            };
            ConnectionConfig {
                id: row.0,
                name: row.1,
                db_type,
                host: row.3,
                port: row.4 as u16,
                username: row.5,
                password: row.6,
                database: row.7,
                ssl: row.8,
            }
        }).collect();

        Ok(configs)
    }
}

// ---------------------------------------------------------------------------
// SqliteConnectionStore 额外方法（委托给 RelationStore）
// ---------------------------------------------------------------------------

impl SqliteConnectionStore {
    // ---------------------------------------------------------------------------
    // LLM 分析结果存储方法（委托给 RelationStore）
    // ---------------------------------------------------------------------------

    /// 保存表关系分析结果
    pub async fn save_relations(
        &self,
        connection_id: &str,
        database: &str,
        relations: &[TableRelationAnalysis],
    ) -> Result<(), String> {
        let pool = self.get_pool().await?;
        RelationStore::save_relations(&pool, connection_id, database, relations).await
    }

    /// 读取表关系分析结果
    pub async fn get_relations(
        &self,
        connection_id: &str,
        database: &str,
    ) -> Result<Vec<TableRelationAnalysis>, String> {
        let pool = self.get_pool().await?;
        RelationStore::get_relations(&pool, connection_id, database).await
    }

    /// 检查是否存在分析结果
    pub async fn has_relations(&self, connection_id: &str, database: &str) -> Result<bool, String> {
        let pool = self.get_pool().await?;
        RelationStore::has_relations(&pool, connection_id, database).await
    }

    /// 删除分析结果
    pub async fn delete_relations(&self, connection_id: &str, database: &str) -> Result<(), String> {
        let pool = self.get_pool().await?;
        RelationStore::delete_relations(&pool, connection_id, database).await
    }

    // ---------------------------------------------------------------------------
    // LLM 配置存储方法（委托给 RelationStore）
    // ---------------------------------------------------------------------------

    /// 获取 LLM 配置
    pub async fn get_llm_config(&self) -> Result<LlmConfig, String> {
        let pool = self.get_pool().await?;
        RelationStore::get_llm_config(&pool).await
    }

    /// 保存 LLM 配置
    pub async fn save_llm_config(&self, config: &LlmConfig) -> Result<(), String> {
        let pool = self.get_pool().await?;
        RelationStore::save_llm_config(&pool, config).await
    }

    // ---------------------------------------------------------------------------
    // 通用设置存储方法（委托给 RelationStore）
    // ---------------------------------------------------------------------------

    /// 获取通用设置
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let pool = self.get_pool().await?;
        RelationStore::get_setting(&pool, key).await
    }

    /// 保存通用设置
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let pool = self.get_pool().await?;
        RelationStore::set_setting(&pool, key, value).await
    }
}

// ---------------------------------------------------------------------------
// 便捷初始化函数：建表 + 加载到内存 HashMap
// ---------------------------------------------------------------------------

pub async fn init_store(
    store: &SqliteConnectionStore,
    app: &tauri::App,
) -> Result<Vec<ConnectionConfig>, Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    let db_path = app_data_dir.join("baizedb.db");
    log::info!("连接配置数据库路径: {:?}", db_path);

    store.init_pool(&db_path).await?;

    let conns = store.list_all().await?;
    log::info!("从存储层加载了 {} 个连接配置", conns.len());

    Ok(conns)
}
