use tauri::State;
use serde::Serialize;
use sqlx::{Column, Row, TypeInfo, ValueRef};

use crate::state::{AppState, DbPool};

#[derive(Debug, Serialize)]
pub struct DatabaseInfo {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct SchemaInfo {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct TableInfo {
    pub name: String,
    pub table_type: String,
    pub row_count: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub key: Option<String>,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TableDataResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub total: i64,
}

/// 确保 PG 指定数据库的连接池存在。如果不存在则创建。
/// 返回 db_pools 中的 key。
pub async fn ensure_pg_db_pool(
    connection_id: &str,
    database: &str,
    state: &AppState,
) -> std::result::Result<String, String> {
    let db_key = format!("{}:{}", connection_id, database);

    // 先检查是否已存在
    {
        let db_pools = state.db_pools.read().await;
        if db_pools.contains_key(&db_key) {
            return Ok(db_key);
        }
    }

    // 不存在，创建新连接池
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
    db_pools.insert(db_key.clone(), DbPool::PostgreSQL(new_pool));

    Ok(db_key)
}

#[tauri::command]
pub async fn list_databases(
    connection_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<DatabaseInfo>, String> {
    let pools = state.pools.read().await;
    let pool = pools.get(&connection_id).ok_or("连接未激活")?;

    match pool {
        DbPool::MySQL(p) => {
            // CAST AS CHAR 避免某些 MySQL 版本返回 VARBINARY
            let rows = sqlx::query_as::<_, (String,)>(
                "SELECT CAST(schema_name AS CHAR) FROM information_schema.schemata \
                 WHERE schema_name NOT IN ('information_schema','performance_schema','mysql','sys') \
                 ORDER BY schema_name"
            ).fetch_all(p).await.map_err(|e| e.to_string())?;
            Ok(rows.into_iter().map(|r| DatabaseInfo { name: r.0 }).collect())
        }
        DbPool::PostgreSQL(p) => {
            let rows = sqlx::query_as::<_, (String,)>(
                "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname"
            ).fetch_all(p).await.map_err(|e| e.to_string())?;
            Ok(rows.into_iter().map(|r| DatabaseInfo { name: r.0 }).collect())
        }
        DbPool::Redis(_) => Err("Redis 不支持 list_databases".to_string()),
    }
}

#[tauri::command]
pub async fn list_tables(
    connection_id: String,
    database: String,
    schema: Option<String>,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<TableInfo>, String> {
    let pools = state.pools.read().await;
    let pool = pools.get(&connection_id).ok_or("连接未激活")?;

    match pool {
        DbPool::MySQL(p) => {
            let rows = sqlx::query_as::<_, (String, String, i64)>(
                "SELECT CAST(TABLE_NAME AS CHAR), CAST(TABLE_TYPE AS CHAR), \
                        CAST(COALESCE(TABLE_ROWS, 0) AS SIGNED) \
                 FROM information_schema.TABLES \
                 WHERE TABLE_SCHEMA = ? ORDER BY TABLE_NAME"
            ).bind(&database)
                .fetch_all(p).await.map_err(|e| e.to_string())?;
            Ok(rows.into_iter().map(|r| TableInfo {
                name: r.0,
                table_type: r.1,
                row_count: Some(r.2),
            }).collect())
        }
        DbPool::PostgreSQL(_) => {
            drop(pools);
            let db_key = ensure_pg_db_pool(&connection_id, &database, &state).await?;
            let db_pools = state.db_pools.read().await;
            let p = match db_pools.get(&db_key) {
                Some(DbPool::PostgreSQL(p)) => p,
                _ => return Err("数据库连接池未找到".to_string()),
            };
            let schema_name = schema.as_deref().unwrap_or("public");
            let rows = sqlx::query_as::<_, (String, String)>(
                "SELECT tablename, 'BASE TABLE' \
                 FROM pg_catalog.pg_tables \
                 WHERE schemaname = $1 ORDER BY tablename"
            ).bind(schema_name)
                .fetch_all(p).await.map_err(|e| e.to_string())?;
            Ok(rows.into_iter().map(|r| TableInfo {
                name: r.0,
                table_type: r.1,
                row_count: None,
            }).collect())
        }
        DbPool::Redis(_) => Err("Redis 不支持 list_tables".to_string()),
    }
}

#[tauri::command]
pub async fn list_schemas(
    connection_id: String,
    database: String,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<SchemaInfo>, String> {
    let pools = state.pools.read().await;
    let pool = pools.get(&connection_id).ok_or("连接未激活")?;

    match pool {
        DbPool::MySQL(_) => {
            // MySQL 没有 schema 概念
            Ok(vec![])
        }
        DbPool::PostgreSQL(_) => {
            drop(pools);
            let db_key = ensure_pg_db_pool(&connection_id, &database, &state).await?;
            let db_pools = state.db_pools.read().await;
            let p = match db_pools.get(&db_key) {
                Some(DbPool::PostgreSQL(p)) => p,
                _ => return Err("数据库连接池未找到".to_string()),
            };
            let rows = sqlx::query_as::<_, (String,)>(
                "SELECT nspname FROM pg_namespace \
                 WHERE nspname NOT IN ('pg_catalog', 'information_schema') \
                 ORDER BY nspname"
            ).fetch_all(p).await.map_err(|e| e.to_string())?;
            Ok(rows.into_iter().map(|r| SchemaInfo { name: r.0 }).collect())
        }
        DbPool::Redis(_) => Err("Redis 不支持 list_schemas".to_string()),
    }
}

#[tauri::command]
pub async fn list_columns(
    connection_id: String,
    database: String,
    table: String,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<ColumnInfo>, String> {
    let pools = state.pools.read().await;
    let pool = pools.get(&connection_id).ok_or("连接未激活")?;

    match pool {
        DbPool::MySQL(p) => {
            let sql = format!(
                "SELECT CAST(COLUMN_NAME AS CHAR), CAST(DATA_TYPE AS CHAR), \
                        CAST(IS_NULLABLE AS CHAR), \
                        CAST(COALESCE(COLUMN_KEY, '') AS CHAR), \
                        CAST(COALESCE(COLUMN_DEFAULT, '') AS CHAR), \
                        CAST(COALESCE(COLUMN_COMMENT, '') AS CHAR) \
                 FROM information_schema.COLUMNS \
                 WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' ORDER BY ORDINAL_POSITION",
                database, table
            );
            let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(&sql)
                .fetch_all(p).await.map_err(|e| e.to_string())?;
            Ok(rows.into_iter().map(|r| ColumnInfo {
                name: r.0,
                data_type: r.1,
                nullable: r.2 == "YES",
                key: if r.3.is_empty() { None } else { Some(r.3) },
                default_value: if r.4.is_empty() { None } else { Some(r.4) },
                comment: if r.5.is_empty() { None } else { Some(r.5) },
            }).collect())
        }
        DbPool::PostgreSQL(_) => {
            drop(pools);
            let db_key = ensure_pg_db_pool(&connection_id, &database, &state).await?;
            let db_pools = state.db_pools.read().await;
            let p = match db_pools.get(&db_key) {
                Some(DbPool::PostgreSQL(p)) => p,
                _ => return Err("数据库连接池未找到".to_string()),
            };
            let sql = "SELECT c.column_name, c.data_type, c.is_nullable, \
                        CASE WHEN kcu.column_name IS NOT NULL THEN 'PRI' ELSE '' END, \
                        COALESCE(c.column_default, ''), '' \
                 FROM information_schema.columns c \
                 LEFT JOIN information_schema.key_column_usage kcu \
                   ON c.table_name = kcu.table_name AND c.column_name = kcu.column_name \
                   AND kcu.constraint_name IN ( \
                     SELECT constraint_name FROM information_schema.table_constraints \
                     WHERE constraint_type = 'PRIMARY KEY' AND table_name = $1) \
                 WHERE c.table_catalog = $2 AND c.table_name = $3 ORDER BY c.ordinal_position";
            let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(sql)
                .bind(&table)
                .bind(&database)
                .bind(&table)
                .fetch_all(p).await.map_err(|e| e.to_string())?;
            Ok(rows.into_iter().map(|r| ColumnInfo {
                name: r.0,
                data_type: r.1,
                nullable: r.2 == "YES",
                key: if r.3.is_empty() { None } else { Some(r.3) },
                default_value: if r.4.is_empty() { None } else { Some(r.4) },
                comment: None,
            }).collect())
        }
        DbPool::Redis(_) => Err("Redis 不支持 list_columns".to_string()),
    }
}

#[tauri::command]
pub async fn get_table_data(
    connection_id: String,
    database: String,
    table: String,
    page: i64,
    page_size: i64,
    state: State<'_, AppState>,
) -> std::result::Result<TableDataResult, String> {
    let offset = (page - 1) * page_size;
    let pools = state.pools.read().await;
    let pool = pools.get(&connection_id).ok_or("连接未激活")?;

    match pool {
        DbPool::MySQL(p) => {
            let count: i64 = sqlx::query(&format!("SELECT COUNT(*) FROM `{database}`.`{table}`"))
                .fetch_one(p).await.map_err(|e| e.to_string())?
                .get::<i64, _>(0);
            let rows = sqlx::query(&format!("SELECT * FROM `{database}`.`{table}` LIMIT {page_size} OFFSET {offset}"))
                .fetch_all(p).await.map_err(|e| e.to_string())?;
            if rows.is_empty() {
                return Ok(TableDataResult { columns: vec![], rows: vec![], total: count });
            }
            let columns: Vec<String> = rows[0].columns().iter().map(|c| c.name().to_string()).collect();
            let data = mysql_to_json(&rows);
            Ok(TableDataResult { columns, rows: data, total: count })
        }
        DbPool::PostgreSQL(_) => {
            drop(pools);
            let db_key = ensure_pg_db_pool(&connection_id, &database, &state).await?;
            let db_pools = state.db_pools.read().await;
            let p = match db_pools.get(&db_key) {
                Some(DbPool::PostgreSQL(p)) => p,
                _ => return Err("数据库连接池未找到".to_string()),
            };
            let schema_table = format!("\"{}\"", table);
            let count: i64 = sqlx::query(&format!("SELECT COUNT(*) FROM {}", schema_table))
                .fetch_one(p).await.map_err(|e| e.to_string())?
                .get::<i64, _>(0);
            let rows = sqlx::query(&format!("SELECT * FROM {} LIMIT {} OFFSET {}", schema_table, page_size, offset))
                .fetch_all(p).await.map_err(|e| e.to_string())?;
            if rows.is_empty() {
                return Ok(TableDataResult { columns: vec![], rows: vec![], total: count });
            }
            let columns: Vec<String> = rows[0].columns().iter().map(|c| c.name().to_string()).collect();
            let data = pg_to_json(&rows);
            Ok(TableDataResult { columns, rows: data, total: count })
        }
        DbPool::Redis(_) => Err("Redis 不支持 get_table_data".to_string()),
    }
}

#[tauri::command]
pub async fn get_table_row_count(
    connection_id: String,
    database: String,
    table: String,
    state: State<'_, AppState>,
) -> std::result::Result<i64, String> {
    let pools = state.pools.read().await;
    let pool = pools.get(&connection_id).ok_or("连接未激活")?;

    match pool {
        DbPool::MySQL(p) => {
            let row = sqlx::query(&format!("SELECT COUNT(*) FROM `{database}`.`{table}`"))
                .fetch_one(p).await.map_err(|e| e.to_string())?;
            Ok(row.get::<i64, _>(0))
        }
        DbPool::PostgreSQL(_) => {
            drop(pools);
            let db_key = ensure_pg_db_pool(&connection_id, &database, &state).await?;
            let db_pools = state.db_pools.read().await;
            let p = match db_pools.get(&db_key) {
                Some(DbPool::PostgreSQL(p)) => p,
                _ => return Err("数据库连接池未找到".to_string()),
            };
            let row = sqlx::query(&format!("SELECT COUNT(*) FROM \"{}\"", table))
                .fetch_one(p).await.map_err(|e| e.to_string())?;
            Ok(row.get::<i64, _>(0))
        }
        DbPool::Redis(_) => Err("Redis 不支持 get_table_row_count".to_string()),
    }
}

fn mysql_to_json(rows: &[sqlx::mysql::MySqlRow]) -> Vec<Vec<serde_json::Value>> {
    rows.iter().map(|row| {
        (0..row.columns().len()).map(|i| {
            let val = row.try_get_raw(i).unwrap();
            if val.is_null() { return serde_json::Value::Null; }
            let tname = val.type_info().name().to_uppercase();
            if tname.contains("INT") || tname.contains("SERIAL") {
                row.try_get::<i64, _>(i).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null)
            } else if tname.contains("FLOAT") || tname.contains("DOUBLE") || tname.contains("DECIMAL") {
                row.try_get::<f64, _>(i).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null)
            } else if tname == "BOOL" || tname == "BOOLEAN" {
                row.try_get::<bool, _>(i).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null)
            } else if tname == "DATE" || tname == "DATETIME" || tname == "TIMESTAMP" || tname == "TIME" {
                row.try_get::<chrono::NaiveDateTime, _>(i)
                    .map(|v| serde_json::json!(v.to_string()))
                    .or_else(|_| row.try_get::<chrono::NaiveDate, _>(i).map(|v| serde_json::json!(v.to_string())))
                    .or_else(|_| row.try_get::<chrono::NaiveTime, _>(i).map(|v| serde_json::json!(v.to_string())))
                    .unwrap_or_else(|_| serde_json::json!(format!("[{}]", tname)))
            } else if tname == "JSON"{
                row.try_get::<serde_json::Value, _>(i)
                    .map(|v| v)
                    .unwrap_or_else(|_| serde_json::json!(format!("[{}]", tname)))
            } else if tname == "BINARY" || tname == "VARBINARY" {
                row.try_get::<Vec<u8>, _>(i)
                    .map(|v| serde_json::json!(String::from_utf8_lossy(&v).to_string()))
                    .unwrap_or_else(|_| serde_json::json!(format!("[{}]", tname)))
            } else {
                row.try_get::<String, _>(i)
                    .map(|v| serde_json::json!(v))
                    .unwrap_or_else(|_| serde_json::json!(format!("[{}]", tname)))
            }
        }).collect()
    }).collect()
}

fn pg_to_json(rows: &[sqlx::postgres::PgRow]) -> Vec<Vec<serde_json::Value>> {
    rows.iter().map(|row| {
        (0..row.columns().len()).map(|i| {
            let val = row.try_get_raw(i).unwrap();
            if val.is_null() { return serde_json::Value::Null; }
            let tname = val.type_info().name().to_uppercase();
            // 精确匹配整数类型（避免 TIMESTAMP 包含 INT 子串被误匹配）
            if tname == "INT2" || tname == "INT4" || tname == "INT8"
                || tname == "SERIAL" || tname == "BIGSERIAL" || tname == "SMALLSERIAL" {
                row.try_get::<i64, _>(i).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null)
            } else if tname.contains("FLOAT") || tname.contains("NUMERIC") {
                row.try_get::<f64, _>(i).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null)
            } else if tname == "BOOL" {
                row.try_get::<bool, _>(i).map(|v| serde_json::json!(v)).unwrap_or(serde_json::Value::Null)
            } else if tname == "TIMESTAMP" {
                // timestamp without time zone → NaiveDateTime
                row.try_get::<chrono::NaiveDateTime, _>(i)
                    .map(|v| serde_json::json!(v.to_string()))
                    .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name())))
            } else if tname == "TIMESTAMPTZ" {
                // timestamp with time zone → DateTime<Utc>
                row.try_get::<chrono::DateTime<chrono::Utc>, _>(i)
                    .map(|v| serde_json::json!(v.to_string()))
                    .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name())))
            } else if tname == "DATE" {
                row.try_get::<chrono::NaiveDate, _>(i)
                    .map(|v| serde_json::json!(v.to_string()))
                    .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name())))
            } else if tname == "TIME" {
                row.try_get::<chrono::NaiveTime, _>(i)
                    .map(|v| serde_json::json!(v.to_string()))
                    .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name())))
            } else if tname == "TIMETZ" {
                // timetz → 用 String 尝试，失败显示类型名
                row.try_get::<String, _>(i)
                    .map(|v| serde_json::json!(v))
                    .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name())))
            } else if tname == "UUID" {
                row.try_get::<uuid::Uuid, _>(i)
                    .map(|v| serde_json::json!(v.to_string()))
                    .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name())))
            } else if tname == "JSON" || tname == "JSONB" {
                row.try_get::<serde_json::Value, _>(i)
                    .map(|v| serde_json::json!(v.to_string()))
                    .unwrap_or_else(|_| serde_json::json!(format!("[{}]", val.type_info().name())))
            } else {
                row.try_get::<String, _>(i)
                    .map(|v| serde_json::json!(v))
                    .unwrap_or_else(|_| {
                    serde_json::json!(format!("[{}]", val.type_info().name()))
                })
            }
        }).collect()
    }).collect()
}
