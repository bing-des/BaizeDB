use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tauri::Manager;

use crate::state::{ConnectionConfig, DbType};

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
