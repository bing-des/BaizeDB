use tauri::State;
use serde::{Deserialize, Serialize};

use crate::state::{AppState};
use crate::database::db_ops::QueryResult as DbQueryResult;

/// 前端兼容的查询结果（Tauri 命令直接返回此结构）
#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub affected_rows: Option<u64>,
    pub execution_time_ms: u64,
    pub error: Option<String>,
}

impl From<DbQueryResult> for QueryResult {
    fn from(r: DbQueryResult) -> Self {
        Self {
            columns: r.columns,
            rows: r.rows,
            affected_rows: r.affected_rows,
            execution_time_ms: r.execution_time_ms,
            error: r.error,
        }
    }
}

/// 判断 SQL 是否为读操作
fn is_read_query(sql: &str) -> bool {
    let upper = sql.trim().to_uppercase();
    upper.starts_with("SELECT") || upper.starts_with("SHOW")
        || upper.starts_with("DESCRIBE") || upper.starts_with("DESC")
        || upper.starts_with("EXPLAIN") || upper.starts_with("WITH")
}

#[tauri::command]
pub async fn execute_query(
    connection_id: String,
    sql: String,
    database: Option<String>,
    state: State<'_, AppState>,
) -> std::result::Result<QueryResult, String> {
    let start = std::time::Instant::now();
    let is_read = is_read_query(&sql);

    println!("Executing query on connection {}: {} isRead: {}", connection_id, sql, is_read);

    // 获取连接池
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活，请先连接数据库")?
    };

    // PG 需要指定 database 来获取正确的连接池；MySQL 传空字符串即可
    // 对于 query 场景，如果前端没传 database，尝试用连接的默认数据库
    let db_name = match database {
        Some(db) => db,
        None => {
            // 尝试从连接配置获取默认数据库
            let conns = state.connections.read().await;
            conns.get(&connection_id)
                .and_then(|c| c.database.clone())
                .unwrap_or_default()
        }
    };

    // 如果 db_name 仍然为空且是 MySQL，允许不指定（MySQL 可以在无默认库时执行某些查询）
    // 但 PG 必须要有 database
    if db_name.is_empty() {
        let conns = state.connections.read().await;
        let cfg = conns.get(&connection_id).ok_or("连接配置不存在")?;
        if cfg.db_type == crate::state::DbType::PostgreSQL {
            drop(conns);
            return Err("PostgreSQL 查询需要指定数据库".into());
        }
        drop(conns);
    }

    let db_ops = pool.as_db_ops(&state, &connection_id, &db_name).await?;

    let result: DbQueryResult = if is_read {
        db_ops.query_sql(&sql).await?
    } else {
        let affected = db_ops.execute_sql(&sql).await?;
        let ms = start.elapsed().as_millis() as u64;
        DbQueryResult {
            columns: vec![],
            rows: vec![],
            column_types: None,
            affected_rows: Some(affected),
            execution_time_ms: ms,
            error: None,
            total: None,
        }
    };

    Ok(result.into())
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
