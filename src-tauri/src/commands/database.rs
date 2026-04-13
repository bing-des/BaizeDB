use tauri::State;
use serde::Serialize;

use crate::state::AppState;

// ====== 前端兼容的旧结构体（保持 API 不变）======

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
    #[serde(rename = "tableType")]
    pub table_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "dataType")]
    pub data_type: String,
    pub nullable: bool,
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "defaultValue")]
    pub default_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TableDataResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub total: i64,
}

// ====== Tauri 命令（通过 AnyDbPool 统一调用，零 match）======

#[tauri::command]
pub async fn list_databases(
    connection_id: String,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<DatabaseInfo>, String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    let db_ops = pool.as_db_ops(&state, &connection_id, "").await?;
    let metas = db_ops.list_databases().await?;

    Ok(metas.into_iter().map(|m| DatabaseInfo { name: m.name }).collect())
}

#[tauri::command]
pub async fn list_tables(
    connection_id: String,
    database: String,
    schema: Option<String>,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<TableInfo>, String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    let metas = db_ops.list_tables(&database, schema.as_deref()).await?;

    Ok(metas.into_iter().map(|m| TableInfo {
        name: m.name,
        table_type: m.table_type.unwrap_or_default(),
        row_count: m.row_count,
    }).collect())
}

#[tauri::command]
pub async fn list_schemas(
    connection_id: String,
    database: String,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<SchemaInfo>, String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    let schemas = db_ops.list_schemas(&database).await?;

    Ok(schemas.into_iter().map(|s| SchemaInfo { name: s.name }).collect())
}

#[tauri::command]
pub async fn list_columns(
    connection_id: String,
    database: String,
    table: String,
    state: State<'_, AppState>,
) -> std::result::Result<Vec<ColumnInfo>, String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    let cols = db_ops.list_columns(&database, &table).await?;

    Ok(cols.into_iter().map(|c| ColumnInfo {
        name: c.name,
        data_type: c.data_type,
        nullable: c.nullable,
        key: c.key,
        default_value: c.default_value,
        comment: c.comment,
    }).collect())
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
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    let r = db_ops.get_table_data(&database, &table, page, page_size).await?;

    Ok(TableDataResult {
        columns: r.columns,
        rows: r.rows,
        total: r.total.unwrap_or(0),
    })
}

#[tauri::command]
pub async fn get_table_row_count(
    connection_id: String,
    database: String,
    table: String,
    state: State<'_, AppState>,
) -> std::result::Result<i64, String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    db_ops.get_row_count(&database, &table).await
}
