use tauri::State;
use serde::Serialize;
use redis::AsyncCommands;

use crate::state::{AppState, DbPool};

/// 从 DbPool 获取 Redis 连接的克隆（MultiplexedConnection 实现了 Clone）
fn get_redis_conn(pool: &DbPool) -> std::result::Result<redis::aio::MultiplexedConnection, String> {
    match pool {
        DbPool::Redis(conn) => Ok(conn.clone()),
        _ => Err("不是 Redis 连接".to_string()),
    }
}

#[derive(Debug, Serialize)]
pub struct RedisDbInfo {
    pub index: u64,
    pub key_count: i64,
}

#[derive(Debug, Serialize)]
pub struct RedisKeyInfo {
    pub name: String,
    pub key_type: String,
    pub ttl: i64,
}

#[derive(Debug, Serialize)]
pub struct RedisKeyValue {
    pub key: String,
    pub key_type: String,
    pub value: serde_json::Value,
    pub ttl: i64,
}

#[derive(Debug, Serialize)]
pub struct RedisScanResult {
    pub cursor: u64,
    pub keys: Vec<RedisKeyInfo>,
}

#[tauri::command]
pub async fn redis_list_dbs(
    connection_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<RedisDbInfo>, String> {
    let pools = state.pools.read().await;
    let pool = pools.get(&connection_id).ok_or("连接未激活")?;

    let mut conn = get_redis_conn(pool)?;
    let mut result = Vec::new();

    for i in 0..16u64 {
        let _ = redis::cmd("SELECT")
            .arg(i)
            .query_async::<()>(&mut conn)
            .await;
        let count: i64 = redis::cmd("DBSIZE")
            .query_async(&mut conn)
            .await
            .unwrap_or(0);
        if i == 0 || count > 0 {
            result.push(RedisDbInfo { index: i, key_count: count });
        }
    }

    // 切回 db0
    let _ = redis::cmd("SELECT")
        .arg(0)
        .query_async::<()>(&mut conn)
        .await;

    if result.is_empty() {
        result.push(RedisDbInfo { index: 0, key_count: 0 });
    }
    Ok(result)
}

#[tauri::command]
pub async fn redis_list_keys(
    connection_id: String,
    db_index: u64,
    pattern: Option<String>,
    cursor: Option<u64>,
    count: Option<u64>,
    state: State<'_, AppState>,
) -> std::result::Result<RedisScanResult, String> {
    let pools = state.pools.read().await;
    let pool = pools.get(&connection_id).ok_or("连接未激活")?;

    let mut conn = get_redis_conn(pool)?;

    // SELECT 到目标 db
    if db_index > 0 {
        redis::cmd("SELECT")
            .arg(db_index)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| format!("SELECT 失败: {}", e))?;
    }

    let pat = pattern.unwrap_or_else(|| "*".to_string());
    let cur = cursor.unwrap_or(0);
    let cnt = count.unwrap_or(200);

    let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
        .arg(cur)
        .arg("MATCH")
        .arg(&pat)
        .arg("COUNT")
        .arg(cnt)
        .query_async(&mut conn)
        .await
        .map_err(|e| format!("SCAN 失败: {}", e))?;

    // 批量获取 key 的类型和 TTL
    let mut key_infos = Vec::new();
    for key in &keys {
        let key_type: String = redis::cmd("TYPE")
            .arg(key)
            .query_async(&mut conn)
            .await
            .unwrap_or_else(|_| "none".to_string());

        let ttl: i64 = redis::cmd("TTL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .unwrap_or(-1);

        key_infos.push(RedisKeyInfo {
            name: key.clone(),
            key_type,
            ttl,
        });
    }

    Ok(RedisScanResult {
        cursor: next_cursor,
        keys: key_infos,
    })
}

#[tauri::command]
pub async fn redis_get_key(
    connection_id: String,
    db_index: u64,
    key: String,
    state: State<'_, AppState>,
) -> std::result::Result<RedisKeyValue, String> {
    let pools = state.pools.read().await;
    let pool = pools.get(&connection_id).ok_or("连接未激活")?;

    let mut conn = get_redis_conn(pool)?;

    if db_index > 0 {
        redis::cmd("SELECT")
            .arg(db_index)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| format!("SELECT 失败: {}", e))?;
    }

    let key_type: String = redis::cmd("TYPE")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .map_err(|e| format!("TYPE 失败: {}", e))?;

    let ttl: i64 = redis::cmd("TTL")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .unwrap_or(-1);

    let value = match key_type.as_str() {
        "string" => {
            let val: String = conn.get(&key).await.map_err(|e| e.to_string())?;
            serde_json::json!(val)
        }
        "list" => {
            let vals: Vec<String> = conn.lrange(&key, 0, -1).await.map_err(|e| e.to_string())?;
            serde_json::json!(vals)
        }
        "set" => {
            let vals: Vec<String> = conn.smembers(&key).await.map_err(|e| e.to_string())?;
            serde_json::json!(vals)
        }
        "zset" => {
            let vals: Vec<(f64, String)> = redis::cmd("ZRANGE")
                .arg(&key)
                .arg(0)
                .arg(-1)
                .arg("WITHSCORES")
                .query_async(&mut conn)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::json!(vals.iter().map(|(score, member)| {
                serde_json::json!({"member": member, "score": score})
            }).collect::<Vec<_>>())
        }
        "hash" => {
            let vals: std::collections::HashMap<String, String> = conn.hgetall(&key).await.map_err(|e| e.to_string())?;
            serde_json::json!(vals)
        }
        _ => serde_json::json!(format!("[{}]", key_type)),
    };

    Ok(RedisKeyValue { key, key_type, value, ttl })
}

#[tauri::command]
pub async fn redis_set_key(
    connection_id: String,
    db_index: u64,
    key: String,
    value: String,
    key_type: String,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let pools = state.pools.read().await;
    let pool = pools.get(&connection_id).ok_or("连接未激活")?;

    let mut conn = get_redis_conn(pool)?;

    if db_index > 0 {
        redis::cmd("SELECT")
            .arg(db_index)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| format!("SELECT 失败: {}", e))?;
    }

    match key_type.as_str() {
        "string" => {
            conn.set::<_, _, ()>(&key, &value).await.map_err(|e| e.to_string())?;
        }
        _ => return Err(format!("暂不支持设置 {} 类型的 key", key_type)),
    }

    Ok(())
}

#[tauri::command]
pub async fn redis_del_key(
    connection_id: String,
    db_index: u64,
    key: String,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let pools = state.pools.read().await;
    let pool = pools.get(&connection_id).ok_or("连接未激活")?;

    let mut conn = get_redis_conn(pool)?;

    if db_index > 0 {
        redis::cmd("SELECT")
            .arg(db_index)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| format!("SELECT 失败: {}", e))?;
    }

    conn.del::<_, ()>(&key).await.map_err(|e| e.to_string())?;
    Ok(())
}
