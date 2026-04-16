use crate::state::AppState;
use serde::Serialize;
use tauri::State;

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
    pub column_types: Option<Vec<String>>,
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

    Ok(metas
        .into_iter()
        .map(|m| DatabaseInfo { name: m.name })
        .collect())
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

    Ok(metas
        .into_iter()
        .map(|m| TableInfo {
            name: m.name,
            table_type: m.table_type.unwrap_or_default(),
            row_count: m.row_count,
        })
        .collect())
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

    Ok(schemas
        .into_iter()
        .map(|s| SchemaInfo { name: s.name })
        .collect())
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

    Ok(cols
        .into_iter()
        .map(|c| ColumnInfo {
            name: c.name,
            data_type: c.data_type,
            nullable: c.nullable,
            key: c.key,
            default_value: c.default_value,
            comment: c.comment,
        })
        .collect())
}

#[tauri::command]
pub async fn get_table_data(
    connection_id: String,
    database: String,
    table: String,
    page: i64,
    page_size: i64,
    sort_by: Option<String>,
    sort_order: Option<String>,
    filters: Option<std::collections::HashMap<String, String>>,
    state: State<'_, AppState>,
) -> std::result::Result<TableDataResult, String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    let r = db_ops
        .get_table_data(
            &database, &table, page, page_size, sort_by, sort_order, filters,
        )
        .await?;

    Ok(TableDataResult {
        columns: r.columns,
        column_types: r.column_types,
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
    primary_key_type: String,
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
            .update_row(
                &database,
                &table,
                &primary_key,
                &primary_key_type,
                update.primary_key_value,
                update.column_values,
                update.column_types,
            )
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
    primary_key_type: String,
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
            .delete_row(&database, &table, &primary_key, &primary_key_type, pk_value)
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
    db_ops
        .insert_row(
            &database,
            &table,
            column_values,
            column_types.unwrap_or_default(),
        )
        .await
}

/// 删除数据库（DROP DATABASE）
#[tauri::command]
pub async fn drop_database(
    connection_id: String,
    database_name: String,
    state: State<'_, AppState>,
) -> std::result::Result<u64, String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    // drop 需要在目标库之外执行，用主连接池即可
    let db_ops = pool.as_db_ops(&state, &connection_id, "").await?;
    // 安全检查：不允许删除系统库
    let forbidden = ["mysql", "information_schema", "performance_schema", "postgres", "template0", "template1"];
    if forbidden.contains(&database_name.as_str()) {
        return Err(format!("不允许删除系统数据库: {}", database_name));
    }
    let sql = format!(r#"DROP DATABASE "{}""#, database_name.replace('"', ""));
    db_ops.execute_sql(&sql).await
}

/// 删除表（DROP TABLE）
#[tauri::command]
pub async fn drop_table(
    connection_id: String,
    database: String,
    table: String,
    schema: Option<String>,
    state: State<'_, AppState>,
) -> std::result::Result<u64, String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };

    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    db_ops.drop_table(&database, &table, schema.as_deref()).await
}

/// 创建表（CREATE TABLE）
#[derive(serde::Deserialize)]
pub struct CreateTableInput {
    pub table_name: String,
    pub columns: Vec<CreateTableColumn>,
    pub comment: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct CreateTableColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
    pub is_primary_key: bool,
}

#[tauri::command]
pub async fn create_table(
    connection_id: String,
    database: String,
    schema: Option<String>,
    input: CreateTableInput,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };
    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    let is_pg = db_ops.is_postgres();
    
    // 构建完整表名
    let full_table_name = if let Some(s) = &schema {
        if is_pg {
            format!(r#""{}"."{}""#, s.replace('"', ""), input.table_name.replace('"', ""))
        } else {
            format!(r#"`{}`.`{}`"#, s.replace('`', ""), input.table_name.replace('`', ""))
        }
    } else {
        if is_pg {
            format!(r#""{}""#, input.table_name.replace('"', ""))
        } else {
            format!(r#"`{}`"#, input.table_name.replace('`', ""))
        }
    };
    
    // 构建列定义
    let mut column_defs: Vec<String> = Vec::new();
    let mut pk_columns: Vec<String> = Vec::new();
    
    for col in &input.columns {
        let col_name = if is_pg {
            format!(r#""{}""#, col.name.replace('"', ""))
        } else {
            format!(r#"`{}`"#, col.name.replace('`', ""))
        };
        
        let mut def = format!("{} {}", col_name, col.data_type);
        
        if !col.nullable {
            def.push_str(" NOT NULL");
        }
        
        if let Some(default) = &col.default_value {
            def.push_str(&format!(" DEFAULT {}", default));
        }
        
        if is_pg {
            // PG 的列注释单独处理
        } else if let Some(comment) = &col.comment {
            def.push_str(&format!(" COMMENT '{}'", comment.replace('\\', "\\\\").replace('\'', "\\'")));
        }
        
        column_defs.push(def);
        
        if col.is_primary_key {
            pk_columns.push(col_name);
        }
    }
    
    // 添加主键约束
    if !pk_columns.is_empty() {
        let pk_constraint = if is_pg {
            format!("PRIMARY KEY ({})", pk_columns.join(", "))
        } else {
            format!("PRIMARY KEY ({})", pk_columns.join(", "))
        };
        column_defs.push(pk_constraint);
    }
    
    // 构建 CREATE TABLE 语句
    let sql = format!(
        "CREATE TABLE {} (\n  {}\n)",
        full_table_name,
        column_defs.join(",\n  ")
    );
    
    db_ops.execute_sql(&sql).await.map(|_| ())?;
    
    // PG: 单独添加表注释
    if is_pg {
        if let Some(comment) = &input.comment {
            let comment_sql = format!(
                "COMMENT ON TABLE {} IS '{}'",
                full_table_name,
                comment.replace('\\', "\\\\").replace('\'', "\\'")
            );
            db_ops.execute_sql(&comment_sql).await?;
        }
        // PG: 单独添加列注释
        for col in &input.columns {
            if let Some(comment) = &col.comment {
                let col_name = format!(r#""{}""#, col.name.replace('"', ""));
                let comment_sql = format!(
                    "COMMENT ON COLUMN {}.{} IS '{}'",
                    full_table_name,
                    col_name,
                    comment.replace('\\', "\\\\").replace('\'', "\\'")
                );
                db_ops.execute_sql(&comment_sql).await?;
            }
        }
    } else {
        // MySQL: 表注释
        if let Some(comment) = &input.comment {
            let comment_sql = format!(
                "ALTER TABLE {} COMMENT = '{}'",
                full_table_name,
                comment.replace('\\', "\\\\").replace('\'', "\\'")
            );
            db_ops.execute_sql(&comment_sql).await?;
        }
    }
    
    Ok(())
}

/// 新增列（ALTER TABLE ... ADD COLUMN）
#[derive(serde::Deserialize)]
pub struct AddColumnInput {
    pub column_name: String,
    pub column_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

#[tauri::command]
pub async fn add_column(
    connection_id: String,
    database: String,
    table: String,
    input: AddColumnInput,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };
    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    db_ops
        .add_column(
            &database,
            &table,
            &input.column_name,
            &input.column_type,
            input.nullable,
            input.default_value.as_deref(),
            input.comment.as_deref(),
        )
        .await
}

/// 删除列（ALTER TABLE ... DROP COLUMN）
#[tauri::command]
pub async fn drop_column(
    connection_id: String,
    database: String,
    table: String,
    column_name: String,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };
    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    db_ops.drop_column(&database, &table, &column_name).await
}

/// 修改列定义
#[derive(serde::Deserialize)]
pub struct ModifyColumnInput {
    pub old_name: String,
    pub new_name: String,
    pub column_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

#[tauri::command]
pub async fn modify_column(
    connection_id: String,
    database: String,
    table: String,
    input: ModifyColumnInput,
    state: State<'_, AppState>,
) -> std::result::Result<(), String> {
    let pool = {
        let pools = state.pools.read().await;
        pools.get(&connection_id).cloned().ok_or("连接未激活")?
    };
    let db_ops = pool.as_db_ops(&state, &connection_id, &database).await?;
    db_ops
        .modify_column(
            &database,
            &table,
            &input.old_name,
            &input.new_name,
            &input.column_type,
            input.nullable,
            input.default_value.as_deref(),
            input.comment.as_deref(),
        )
        .await
}
