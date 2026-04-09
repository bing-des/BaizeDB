use tauri::State;
use serde::{Deserialize, Serialize};
use sqlx::{Column, Row, TypeInfo, ValueRef, Executor};

use crate::state::{AppState, DbPool};

#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub affected_rows: Option<u64>,
    pub execution_time_ms: u64,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn execute_query(
    connection_id: String,
    sql: String,
    database: Option<String>,
    state: State<'_, AppState>,
) -> std::result::Result<QueryResult, String> {
    let start = std::time::Instant::now();
    let upper = sql.trim().to_uppercase();
    let is_read = upper.starts_with("SELECT") || upper.starts_with("SHOW")
        || upper.starts_with("DESCRIBE") || upper.starts_with("DESC")
        || upper.starts_with("EXPLAIN") || upper.starts_with("WITH");

    println!("Executing query on connection {}: {} isRead: {}", connection_id, sql, is_read);

    // 判断连接类型
    let db_type = {
        let conns = state.connections.read().await;
        let cfg = conns.get(&connection_id).ok_or("连接配置不存在")?;
        cfg.db_type.clone()
    };

    match db_type {
        crate::state::DbType::MySQL => {
            let pools = state.pools.read().await;
            let pool = pools.get(&connection_id).ok_or("连接未激活，请先连接数据库")?;
            let p = match pool { DbPool::MySQL(p) => p, _ => unreachable!() };

            // 如果指定了 database，先 USE database（使用原始查询避免预处理协议错误）
            if let Some(ref db) = database {
                let use_sql = format!("USE `{}`", db);
                // 从池中获取一个连接，执行 USE，然后使用该连接执行后续查询
                let mut conn = p.acquire().await.map_err(|e| e.to_string())?;
                conn.execute(&*use_sql).await.map_err(|e| e.to_string())?;
                // 使用同一个连接执行后续查询
                let executor = &mut *conn;
                if is_read {
                    match sqlx::query(&sql).fetch_all(executor).await {
                        Ok(rows) => {
                            let ms = start.elapsed().as_millis() as u64;
                            if rows.is_empty() {
                                return Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: None, execution_time_ms: ms, error: None });
                            }
                            let columns: Vec<String> = rows[0].columns().iter().map(|c| c.name().to_string()).collect();
                            let data = mysql_to_json(&rows);
                            Ok(QueryResult { columns, rows: data, affected_rows: None, execution_time_ms: ms, error: None })
                        }
                        Err(e) => Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: None, execution_time_ms: start.elapsed().as_millis() as u64, error: Some(e.to_string()) }),
                    }
                } else {
                    match sqlx::query(&sql).execute(executor).await {
                        Ok(r) => Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: Some(r.rows_affected()), execution_time_ms: start.elapsed().as_millis() as u64, error: None }),
                        Err(e) => Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: None, execution_time_ms: start.elapsed().as_millis() as u64, error: Some(e.to_string()) }),
                    }
                }
            } else {
                // 没有指定 database，直接使用池
                if is_read {
                    match sqlx::query(&sql).fetch_all(p).await {
                        Ok(rows) => {
                            let ms = start.elapsed().as_millis() as u64;
                            if rows.is_empty() {
                                return Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: None, execution_time_ms: ms, error: None });
                            }
                            let columns: Vec<String> = rows[0].columns().iter().map(|c| c.name().to_string()).collect();
                            let data = mysql_to_json(&rows);
                            Ok(QueryResult { columns, rows: data, affected_rows: None, execution_time_ms: ms, error: None })
                        }
                        Err(e) => Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: None, execution_time_ms: start.elapsed().as_millis() as u64, error: Some(e.to_string()) }),
                    }
                } else {
                    match sqlx::query(&sql).execute(p).await {
                        Ok(r) => Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: Some(r.rows_affected()), execution_time_ms: start.elapsed().as_millis() as u64, error: None }),
                        Err(e) => Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: None, execution_time_ms: start.elapsed().as_millis() as u64, error: Some(e.to_string()) }),
                    }
                }
            }
        }
        crate::state::DbType::PostgreSQL => {
            // PG: 如果指定了 database，用 db_pools；否则用主 pools
            let db_key = if let Some(ref db) = database {
                Some(format!("{}:{}", connection_id, db))
            } else {
                None
            };

            // 确保 db_pool 存在
            if let Some(ref key) = db_key {
                let db_pools = state.db_pools.read().await;
                if !db_pools.contains_key(key) {
                    drop(db_pools);
                    let db = database.as_ref().unwrap();
                    crate::commands::database::ensure_pg_db_pool(&connection_id, db, &state).await?;
                }
            }

            let p = if let Some(ref key) = db_key {
                let db_pools = state.db_pools.read().await;
                match db_pools.get(key) {
                    Some(DbPool::PostgreSQL(p)) => p.clone(),
                    _ => return Err("数据库连接池未找到".to_string()),
                }
            } else {
                let pools = state.pools.read().await;
                match pools.get(&connection_id) {
                    Some(DbPool::PostgreSQL(p)) => p.clone(),
                    _ => return Err("连接未激活，请先连接数据库".to_string()),
                }
            };

            if is_read {
                    match sqlx::query(&sql).fetch_all(&p).await {
                        Ok(rows) => {
                            let ms = start.elapsed().as_millis() as u64;
                            if rows.is_empty() {
                                return Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: None, execution_time_ms: ms, error: None });
                        }
                        let columns: Vec<String> = rows[0].columns().iter().map(|c| c.name().to_string()).collect();
                        let data = pg_to_json(&rows);
                        Ok(QueryResult { columns, rows: data, affected_rows: None, execution_time_ms: ms, error: None })
                    }
                    Err(e) => Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: None, execution_time_ms: start.elapsed().as_millis() as u64, error: Some(e.to_string()) }),
                }
            } else {
                match sqlx::query(&sql).execute(&p).await {
                    Ok(r) => Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: Some(r.rows_affected()), execution_time_ms: start.elapsed().as_millis() as u64, error: None }),
                    Err(e) => Ok(QueryResult { columns: vec![], rows: vec![], affected_rows: None, execution_time_ms: start.elapsed().as_millis() as u64, error: Some(e.to_string()) }),
                }
            }
        }
        crate::state::DbType::Redis => Err("Redis 不支持 SQL 查询".to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct PagedQueryInput {
    pub connection_id: String,
    pub sql: String,
    pub page: i64,
    pub page_size: i64,
    pub database: Option<String>,
}

#[tauri::command]
pub async fn execute_query_paged(
    input: PagedQueryInput,
    state: State<'_, AppState>,
) -> std::result::Result<QueryResult, String> {
    let offset = (input.page - 1) * input.page_size;
    let paged = format!("{} LIMIT {} OFFSET {}", input.sql.trim_end_matches(';'), input.page_size, offset);
    execute_query(input.connection_id, paged, input.database, state).await
}

fn mysql_to_json(rows: &[sqlx::mysql::MySqlRow]) -> Vec<Vec<serde_json::Value>> {
    rows.iter().map(|row| {
        row.columns().iter().map(|col| {
            let val = row.try_get_raw(col.ordinal()).unwrap();
            if val.is_null() { return serde_json::Value::Null; }
            match val.type_info().name() {
                "INT" | "BIGINT" | "SMALLINT" | "TINYINT" | "MEDIUMINT"
                | "INT UNSIGNED" | "BIGINT UNSIGNED" =>
                    row.try_get::<i64, _>(col.ordinal()).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null),
                "FLOAT" | "DOUBLE" | "DECIMAL" =>
                    row.try_get::<f64, _>(col.ordinal()).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null),
                "BOOLEAN" =>
                    row.try_get::<bool, _>(col.ordinal()).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null),
                _ =>
                    row.try_get::<String, _>(col.ordinal()).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null),
            }
        }).collect()
    }).collect()
}

fn pg_to_json(rows: &[sqlx::postgres::PgRow]) -> Vec<Vec<serde_json::Value>> {
    rows.iter().map(|row| {
        row.columns().iter().map(|col| {
            let val = row.try_get_raw(col.ordinal()).unwrap();
            if val.is_null() { return serde_json::Value::Null; }
            match val.type_info().name() {
                "INT2" | "INT4" | "INT8" =>
                    row.try_get::<i64, _>(col.ordinal()).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null),
                "FLOAT4" | "FLOAT8" | "NUMERIC" =>
                    row.try_get::<f64, _>(col.ordinal()).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null),
                "BOOL" =>
                    row.try_get::<bool, _>(col.ordinal()).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null),
                "TIMESTAMP" =>
                    row.try_get::<chrono::NaiveDateTime, _>(col.ordinal())
                        .map(|v| serde_json::json!(v.to_string()))
                        .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name()))),
                "TIMESTAMPTZ" =>
                    row.try_get::<chrono::DateTime<chrono::Utc>, _>(col.ordinal())
                        .map(|v| serde_json::json!(v.to_string()))
                        .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name()))),
                "DATE" =>
                    row.try_get::<chrono::NaiveDate, _>(col.ordinal())
                        .map(|v| serde_json::json!(v.to_string()))
                        .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name()))),
                "TIME" =>
                    row.try_get::<chrono::NaiveTime, _>(col.ordinal())
                        .map(|v| serde_json::json!(v.to_string()))
                        .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name()))),
                "UUID" =>
                    row.try_get::<uuid::Uuid, _>(col.ordinal())
                        .map(|v| serde_json::json!(v.to_string()))
                        .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name()))),
                "JSON" | "JSONB" =>
                    row.try_get::<serde_json::Value, _>(col.ordinal())
                        .map(|v| serde_json::json!(v.to_string()))
                        .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name()))),
                _ =>
                    row.try_get::<String, _>(col.ordinal()).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null),
            }
        }).collect()
    }).collect()
}
