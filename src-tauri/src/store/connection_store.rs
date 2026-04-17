use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions, Row};
use tauri::Manager;
use serde::{Deserialize, Serialize};

use crate::state::{ConnectionConfig, DbType};

/// 表关系分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRelationAnalysis {
    pub source_table: String,
    pub source_column: String,
    pub target_table: String,
    pub target_column: String,
    pub relation_type: String,
    pub confidence: f32,
    pub reason: String,
}

/// LLM 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmConfig {
    pub api_key: String,
    pub api_url: String,
    pub model: String,
    pub enabled: bool,
}

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

/// 连接配置存储抽象 trait
///
/// 屏蔽底层实现细节，支持未来切换存储后端（如文件、远程服务等）。
#[async_trait::async_trait]
#[allow(dead_code)]
pub trait ConnectionStore: Send + Sync {
    /// 初始化存储（建表等）
    async fn init(&self) -> Result<(), String>;

    /// 插入连接配置
    async fn insert(&self, config: &ConnectionConfig) -> Result<(), String>;

    /// 根据 ID 删除
    async fn delete(&self, id: &str) -> Result<(), String>;

    /// 查询所有连接配置（按 name 排序）
    async fn list_all(&self) -> Result<Vec<ConnectionConfig>, String>;
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

#[async_trait::async_trait]
impl ConnectionStore for SqliteConnectionStore {
    async fn init(&self) -> Result<(), String> {
        // 建表逻辑已在 init_pool 中完成，此处为兼容 trait 的空操作
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
// SqliteConnectionStore 额外方法（非 trait 方法）
// ---------------------------------------------------------------------------

impl SqliteConnectionStore {
    // ---------------------------------------------------------------------------
    // LLM 分析结果存储方法
    // ---------------------------------------------------------------------------

    /// 保存表关系分析结果
    pub async fn save_relations(
        &self,
        connection_id: &str,
        database: &str,
        relations: &[TableRelationAnalysis],
    ) -> Result<(), String> {
        let pool = self.get_pool().await?;
        let mut tx = pool.begin().await.map_err(|e| format!("开始事务失败: {}", e))?;

        for relation in relations {
            // 先删除已存在的相同关系（基于复合唯一键）
            sqlx::query(
                r#"
                DELETE FROM table_relations 
                WHERE connection_id = ?1 
                  AND database = ?2 
                  AND source_table = ?3 
                  AND source_column = ?4
                "#
            )
            .bind(connection_id)
            .bind(database)
            .bind(&relation.source_table)
            .bind(&relation.source_column)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("删除旧关系失败: {}", e))?;

            // 插入新关系
            sqlx::query(
                r#"
                INSERT INTO table_relations 
                (connection_id, database, source_table, source_column, target_table, target_column, relation_type, confidence, reason, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, CURRENT_TIMESTAMP)
                "#
            )
            .bind(connection_id)
            .bind(database)
            .bind(&relation.source_table)
            .bind(&relation.source_column)
            .bind(&relation.target_table)
            .bind(&relation.target_column)
            .bind(&relation.relation_type)
            .bind(relation.confidence)
            .bind(&relation.reason)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("保存关系失败: {}", e))?;
        }

        tx.commit().await.map_err(|e| format!("提交事务失败: {}", e))?;
        Ok(())
    }

    /// 读取表关系分析结果
    pub async fn get_relations(
        &self,
        connection_id: &str,
        database: &str,
    ) -> Result<Vec<TableRelationAnalysis>, String> {
        let pool = self.get_pool().await?;

        let rows = sqlx::query(
            r#"
            SELECT source_table, source_column, target_table, target_column, 
                   relation_type, confidence, reason
            FROM table_relations
            WHERE connection_id = ?1 AND database = ?2
            ORDER BY confidence DESC
            "#
        )
        .bind(connection_id)
        .bind(database)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("查询关系失败: {}", e))?;

        let mut relations = Vec::new();
        for row in rows {
            relations.push(TableRelationAnalysis {
                source_table: row.try_get("source_table").map_err(|e| e.to_string())?,
                source_column: row.try_get("source_column").map_err(|e| e.to_string())?,
                target_table: row.try_get("target_table").map_err(|e| e.to_string())?,
                target_column: row.try_get("target_column").map_err(|e| e.to_string())?,
                relation_type: row.try_get("relation_type").map_err(|e| e.to_string())?,
                confidence: row.try_get("confidence").map_err(|e| e.to_string())?,
                reason: row.try_get("reason").map_err(|e| e.to_string())?,
            });
        }

        Ok(relations)
    }

    /// 检查是否存在分析结果
    pub async fn has_relations(&self, connection_id: &str, database: &str) -> Result<bool, String> {
        let pool = self.get_pool().await?;

        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM table_relations
            WHERE connection_id = ?1 AND database = ?2
            "#
        )
        .bind(connection_id)
        .bind(database)
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("查询失败: {}", e))?;

        let count: i64 = row.try_get("count").map_err(|e| e.to_string())?;
        Ok(count > 0)
    }

    /// 删除分析结果
    pub async fn delete_relations(&self, connection_id: &str, database: &str) -> Result<(), String> {
        let pool = self.get_pool().await?;

        sqlx::query(
            r#"
            DELETE FROM table_relations
            WHERE connection_id = ?1 AND database = ?2
            "#
        )
        .bind(connection_id)
        .bind(database)
        .execute(&pool)
        .await
        .map_err(|e| format!("删除关系失败: {}", e))?;

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // LLM 配置存储方法
    // ---------------------------------------------------------------------------

    /// 获取 LLM 配置
    pub async fn get_llm_config(&self) -> Result<LlmConfig, String> {
        let pool = self.get_pool().await?;

        let row = sqlx::query(
            r#"
            SELECT api_key, api_url, model, enabled
            FROM llm_config
            WHERE id = 1
            "#
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("查询配置失败: {}", e))?;

        Ok(LlmConfig {
            api_key: row.try_get("api_key").map_err(|e| e.to_string())?,
            api_url: row.try_get("api_url").map_err(|e| e.to_string())?,
            model: row.try_get("model").map_err(|e| e.to_string())?,
            enabled: row.try_get::<i64, _>("enabled").map(|v| v != 0).unwrap_or(false),
        })
    }

    /// 保存 LLM 配置
    pub async fn save_llm_config(&self, config: &LlmConfig) -> Result<(), String> {
        let pool = self.get_pool().await?;

        sqlx::query(
            r#"
            INSERT INTO llm_config (id, api_key, api_url, model, enabled, updated_at)
            VALUES (1, ?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
            ON CONFLICT(id) 
            DO UPDATE SET
                api_key = excluded.api_key,
                api_url = excluded.api_url,
                model = excluded.model,
                enabled = excluded.enabled,
                updated_at = CURRENT_TIMESTAMP
            "#
        )
        .bind(&config.api_key)
        .bind(&config.api_url)
        .bind(&config.model)
        .bind(if config.enabled { 1 } else { 0 })
        .execute(&pool)
        .await
        .map_err(|e| format!("保存配置失败: {}", e))?;

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // 通用设置存储方法
    // ---------------------------------------------------------------------------

    /// 获取通用设置
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let pool = self.get_pool().await?;

        let row = sqlx::query(
            r#"
            SELECT value FROM settings WHERE key = ?1
            "#
        )
        .bind(key)
        .fetch_optional(&pool)
        .await
        .map_err(|e| format!("查询设置失败: {}", e))?;

        match row {
            Some(r) => Ok(Some(r.try_get("value").map_err(|e| e.to_string())?)),
            None => Ok(None),
        }
    }

    /// 保存通用设置
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let pool = self.get_pool().await?;

        sqlx::query(
            r#"
            INSERT INTO settings (key, value, updated_at)
            VALUES (?1, ?2, CURRENT_TIMESTAMP)
            ON CONFLICT(key) 
            DO UPDATE SET
                value = excluded.value,
                updated_at = CURRENT_TIMESTAMP
            "#
        )
        .bind(key)
        .bind(value)
        .execute(&pool)
        .await
        .map_err(|e| format!("保存设置失败: {}", e))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 便捷初始化函数：建表 + 加载到内存 HashMap
// ---------------------------------------------------------------------------

/// 初始化已有 store 实例（建表 + 创建连接池）+ 加载已有连接
///
/// 直接操作传入的 store 实例，不创建新对象。
/// 返回已加载的连接配置列表，由调用方写入 AppState.connections。
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
