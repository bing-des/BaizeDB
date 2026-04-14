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
    pub data_type: String,
    pub nullable: bool,
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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

/// 更新表格数据（支持多行批量更新）
#[derive(serde::Deserialize)]
pub struct RowUpdate {
    /// 行索引（前端传来的，用于定位原始行）
    #[allow(dead_code)]
    pub row_index: i64,
    /// 主键值（用于 WHERE 定位）
    pub primary_key_value: serde_json::Value,
    /// 要更新的列名和值
    pub column_values: std::collections::HashMap<String, serde_json::Value>,
    /// 列名→PG类型名的映射（PG 二进制协议需要此信息来决定参数绑定方式）
    pub column_types: std::collections::HashMap<String, String>,
}

/// 批量更新表格数据
#[tauri::command]
pub async fn update_table_data(
    connection_id: String,
    database: String,
    table: String,
    primary_key: String,
    updates: Vec<RowUpdate>,
    state: State<'_, AppState>,
) -> std::result::Result<u64, String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    let mut total_affected = 0u64;

    for update in updates {
        let affected = db_ops
            .update_row(&database, &table, &primary_key, update.primary_key_value, update.column_values, update.column_types)
            .await?;
        total_affected += affected;
    }

    Ok(total_affected)
}

/// 删除表格数据（根据主键删除多行）
#[tauri::command]
pub async fn delete_table_data(
    connection_id: String,
    database: String,
    table: String,
    primary_key: String,
    primary_key_values: Vec<serde_json::Value>,
    state: State<'_, AppState>,
) -> std::result::Result<u64, String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    let mut total_affected = 0u64;

    for pk_value in primary_key_values {
        let affected = db_ops
            .delete_row(&database, &table, &primary_key, pk_value)
            .await?;
        total_affected += affected;
    }

    Ok(total_affected)
}

/// 插入一行新数据到表格
#[tauri::command]
pub async fn insert_table_data(
    connection_id: String,
    database: String,
    table: String,
    column_values: std::collections::HashMap<String, serde_json::Value>,
    column_types: Option<std::collections::HashMap<String, String>>,
    state: State<'_, AppState>,
) -> std::result::Result<u64, String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    db_ops.insert_row(&database, &table, column_values, column_types.unwrap_or_default()).await
}

