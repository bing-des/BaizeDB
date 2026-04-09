use tauri::State;
use serde::Deserialize;
use uuid::Uuid;

use crate::state::{AppState, ConnectionConfig, DbPool, DbType};

#[derive(Debug, Deserialize)]
pub struct NewConnectionInput {
    pub name: String,
    pub db_type: DbType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: Option<String>,
    pub ssl: bool,
}

/// connect_db 的可选参数：支持重连时传入完整配置
#[derive(Debug, Deserialize)]
pub struct ConnectOptions {
    /// 重连场景：批量传入连接配置列表
    pub configs: Option<Vec<ConnectionConfig>>,
}

#[tauri::command]
pub async fn add_connection(
    input: NewConnectionInput,
    state: State<'_, AppState>,
) -> std::result::Result<ConnectionConfig, String> {
    let config = ConnectionConfig {
        id: Uuid::new_v4().to_string(),
        name: input.name,
        db_type: input.db_type,
        host: input.host,
        port: input.port,
        username: input.username,
        password: input.password,
        database: input.database,
        ssl: input.ssl,
    };

    let mut conns = state.connections.write().await;
    conns.insert(config.id.clone(), config.clone());
    Ok(config)
}

#[tauri::command]
pub async fn remove_connection(
    id: String,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let mut conns = state.connections.write().await;
    conns.remove(&id);
    let mut pools = state.pools.write().await;
    pools.remove(&id);
    Ok(())
}

#[tauri::command]
pub async fn list_connections(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<ConnectionConfig>, String> {
    let conns = state.connections.read().await;
    let mut list: Vec<ConnectionConfig> = conns.values().cloned().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(list)
}

#[tauri::command]
pub async fn test_connection(
    input: NewConnectionInput,
    _state: State<'_, AppState>,
) -> std::result::Result<String, String> {
    let url = build_url(&input.db_type, &input.host, input.port, &input.username, &input.password, input.database.as_deref());

    match input.db_type {
        DbType::MySQL => {
            match sqlx::MySqlPool::connect(&url).await {
                Ok(pool) => {
                    pool.close().await;
                    Ok("连接成功".to_string())
                }
                Err(e) => Err(format!("连接失败: {}", e)),
            }
        }
        DbType::PostgreSQL => {
            match sqlx::PgPool::connect(&url).await {
                Ok(pool) => {
                    pool.close().await;
                    Ok("连接成功".to_string())
                }
                Err(e) => Err(format!("连接失败: {}", e)),
            }
        }
        DbType::Redis => {
            let redis_url = build_redis_url(&input.host, input.port, &input.password, input.database.as_deref());
            match redis::Client::open(redis_url.as_str()) {
                Ok(client) => {
                    match client.get_multiplexed_async_connection().await {
                        Ok(_conn) => Ok("连接成功".to_string()),
                        Err(e) => Err(format!("连接失败: {}", e)),
                    }
                }
                Err(e) => Err(format!("连接失败: {}", e)),
            }
        }
    }
}

#[tauri::command]
pub async fn connect_db(
    id: String,
    state: State<'_, AppState>,
    options: Option<ConnectOptions>,
) -> std::result::Result<(), String> {
    // 优先从内存中取（正常连接流程）；内存中没有则用传入的配置（重启后重连）
    // 优先从内存中取；内存中没有则从传入的配置列表中查找（重启后重连场景）
    let config = {
        let conns = state.connections.read().await;
        match conns.get(&id).cloned() {
            Some(cfg) => (cfg, false),
            None => {
                let cfg = options
                    .and_then(|o| o.configs)
                    .and_then(|configs| configs.into_iter().find(|c| c.id == id))
                    .ok_or_else(|| format!("连接不存在: {}", id))?;
                (cfg, true) // 需要写回 connections map
            }
        }
    };
    let (cfg, needs_restore) = config;

    // 重连时把配置写回 connections map，方便后续命令使用
    if needs_restore {
        let mut conns = state.connections.write().await;
        conns.insert(id.clone(), cfg.clone());
    }

    let url = build_url(
        &cfg.db_type,
        &cfg.host,
        cfg.port,
        &cfg.username,
        &cfg.password,
        cfg.database.as_deref(),
    );

    let pool = match cfg.db_type {
        DbType::MySQL => {
            let p = sqlx::MySqlPool::connect(&url).await.map_err(|e| e.to_string())?;
            DbPool::MySQL(p)
        }
        DbType::PostgreSQL => {
            let p = sqlx::PgPool::connect(&url).await.map_err(|e| e.to_string())?;
            DbPool::PostgreSQL(p)
        }
        DbType::Redis => {
            let redis_url = build_redis_url(&cfg.host, cfg.port, &cfg.password, cfg.database.as_deref());
            let client = redis::Client::open(redis_url.as_str()).map_err(|e| e.to_string())?;
            let conn = client.get_multiplexed_async_connection().await.map_err(|e| e.to_string())?;
            DbPool::Redis(conn)
        }
    };

    let mut pools = state.pools.write().await;
    pools.insert(id, pool);
    Ok(())
}

#[tauri::command]
pub async fn disconnect_db(
    id: String,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    // 关闭主连接池
    let mut pools = state.pools.write().await;
    if let Some(pool) = pools.remove(&id) {
        match pool {
            DbPool::MySQL(p) => p.close().await,
            DbPool::PostgreSQL(p) => p.close().await,
            DbPool::Redis(_) => { /* Redis MultiplexedConnection 自动关闭 */ }
        }
    }
    drop(pools);

    // 关闭该连接下所有数据库级别的连接池
    let mut db_pools = state.db_pools.write().await;
    let prefix = format!("{}:", id);
    let keys_to_remove: Vec<String> = db_pools.keys()
        .filter(|k| k.starts_with(&prefix))
        .cloned()
        .collect();
    for key in keys_to_remove {
        match db_pools.remove(&key) {
            Some(DbPool::PostgreSQL(p)) => p.close().await,
            Some(DbPool::Redis(_)) => {}
            _ => {}
        }
    }

    Ok(())
}

fn build_url(db_type: &DbType, host: &str, port: u16, user: &str, pass: &str, db: Option<&str>) -> String {
    match db_type {
        DbType::MySQL => {
            let db_part = db.map(|d| format!("/{}", d)).unwrap_or_default();
            format!("mysql://{}:{}@{}:{}{}", user, pass, host, port, db_part)
        }
        DbType::PostgreSQL => {
            let db_part = db.unwrap_or("postgres");
            format!("postgres://{}:{}@{}:{}/{}", user, pass, host, port, db_part)
        }
        DbType::Redis => {
            // Redis URL 不从这里构建，用 build_redis_url
            build_redis_url(host, port, pass, db)
        }
    }
}

fn build_redis_url(host: &str, port: u16, password: &str, db: Option<&str>) -> String {
    let db_num = db.and_then(|d| d.parse::<u64>().ok()).unwrap_or(0);
    if password.is_empty() {
        format!("redis://{}:{}/{}", host, port, db_num)
    } else {
        format!("redis://:{}@{}:{}/{}", password, host, port, db_num)
    }
}
