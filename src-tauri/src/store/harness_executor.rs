//! Harness SQL 执行器 — 简化版
//!
//! 只提供执行SQL的工具，大模型控制分析流程。

use serde_json::{Value as JsonValue, Map};
use crate::store::harness_types::ToolResult;
use crate::state::AppState;


// ─────────────────────────────────────────────────────────────────────────────
// 辅助宏
// ─────────────────────────────────────────────────────────────────────────────

macro_rules! tool_err {
    ($msg:expr) => {
        ToolResult {
            success: false,
            result: None,
            error: Some($msg.to_string()),
        }
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具定义（用于 LLM function calling）
// ─────────────────────────────────────────────────────────────────────────────

/// 获取所有工具定义
pub fn get_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "execute_sql",
                "description": "执行SQL语句并返回结果。支持SELECT/INSERT/UPDATE/DELETE等所有SQL操作。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "sql": {
                            "type": "string",
                            "description": "要执行的SQL语句"
                        }
                    },
                    "required": ["sql"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_tables",
                "description": "列出数据库中的所有表",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_table_schema",
                "description": "获取指定表的完整结构信息",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "table_name": {
                            "type": "string",
                            "description": "表名"
                        }
                    },
                    "required": ["table_name"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_table_sample",
                "description": "获取表的数据样本",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "table_name": {
                            "type": "string",
                            "description": "表名"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "返回行数，默认10",
                            "default": 10
                        }
                    },
                    "required": ["table_name"]
                }
            }
        }),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具执行入口
// ─────────────────────────────────────────────────────────────────────────────

/// 执行工具
pub async fn execute_tool(
    tool_name: &str,
    arguments: JsonValue,
    connection_id: &str,
    database: &str,
    state: &AppState,
) -> ToolResult {
    log::info!("[Harness] 执行工具: {} | 参数: {:?}", tool_name, arguments);

    match tool_name {
        "execute_sql" => exec_execute_sql(arguments, connection_id, database, state).await,
        "list_tables" => exec_list_tables(connection_id, database, state).await,
        "get_table_schema" => exec_get_table_schema(arguments, connection_id, database, state).await,
        "get_table_sample" => exec_get_table_sample(arguments, connection_id, database, state).await,
        _ => tool_err!(format!("未知工具: {}", tool_name)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具实现
// ─────────────────────────────────────────────────────────────────────────────

/// 工具一：执行SQL
async fn exec_execute_sql(
    args: JsonValue,
    connection_id: &str,
    database: &str,
    state: &AppState,
) -> ToolResult {
    let sql = match args.get("sql").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return tool_err!("缺少 sql 参数"),
    };

    // 获取数据库连接
    let pools = state.pools.read().await;
    let pool = match pools.get(connection_id) {
        Some(p) => p,
        None => return tool_err!("数据库未连接"),
    };

    let db_ops = match pool.as_db_ops(state, connection_id, database).await {
        Ok(ops) => ops,
        Err(e) => return tool_err!(e),
    };

    // 执行SQL
    let result = match db_ops.query_sql(sql).await {
        Ok(r) => {
            let rows: Vec<Map<String, JsonValue>> = r.rows.iter().map(|row| {
                let mut m = Map::new();
                for (i, col) in r.columns.iter().enumerate() {
                    if let Some(val) = row.get(i) {
                        m.insert(col.clone(), val.clone());
                    }
                }
                m
            }).collect();

            let mut response = Map::new();
            response.insert("columns".to_string(), JsonValue::Array(
                r.columns.iter().map(|c| JsonValue::String(c.clone())).collect()
            ));
            response.insert("rows".to_string(), JsonValue::Array(
                rows.into_iter().map(JsonValue::Object).collect()
            ));
            response.insert("row_count".to_string(), JsonValue::Number(r.rows.len().into()));

            JsonValue::Object(response)
        }
        Err(e) => return tool_err!(format!("SQL执行失败: {}", e)),
    };

    ToolResult {
        success: true,
        result: Some(result),
        error: None,
    }
}

/// 列出所有表
async fn exec_list_tables(
    connection_id: &str,
    database: &str,
    state: &AppState,
) -> ToolResult {
    let pools = state.pools.read().await;
    let pool = match pools.get(connection_id) {
        Some(p) => p,
        None => return tool_err!("数据库未连接"),
    };

    let db_ops = match pool.as_db_ops(state, connection_id, database).await {
        Ok(ops) => ops,
        Err(e) => return tool_err!(e),
    };

    let tables = match db_ops.list_tables(database, None).await {
        Ok(t) => t,
        Err(e) => return tool_err!(e),
    };

    let result: Vec<JsonValue> = tables.iter().map(|t| {
        let mut m = Map::new();
        m.insert("name".to_string(), JsonValue::String(t.name.clone()));
        if let Some(ref tt) = t.table_type {
            m.insert("table_type".to_string(), JsonValue::String(tt.clone()));
        }
        if let Some(rc) = t.row_count {
            m.insert("row_count".to_string(), JsonValue::Number(rc.into()));
        }
        JsonValue::Object(m)
    }).collect();

    ToolResult {
        success: true,
        result: Some(JsonValue::Array(result)),
        error: None,
    }
}

/// 获取表结构
async fn exec_get_table_schema(
    args: JsonValue,
    connection_id: &str,
    database: &str,
    state: &AppState,
) -> ToolResult {
    let table_name = match args.get("table_name").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return tool_err!("缺少 table_name 参数"),
    };

    let pools = state.pools.read().await;
    let pool = match pools.get(connection_id) {
        Some(p) => p,
        None => return tool_err!("数据库未连接"),
    };

    let db_ops = match pool.as_db_ops(state, connection_id, database).await {
        Ok(ops) => ops,
        Err(e) => return tool_err!(e),
    };

    let columns = match db_ops.list_columns(database, table_name).await {
        Ok(c) => c,
        Err(e) => return tool_err!(e),
    };

    let result: Vec<JsonValue> = columns.iter().map(|c| {
        let mut m = Map::new();
        m.insert("name".to_string(), JsonValue::String(c.name.clone()));
        m.insert("data_type".to_string(), JsonValue::String(c.data_type.clone()));
        m.insert("nullable".to_string(), JsonValue::Bool(c.nullable));
        if let Some(ref k) = c.key {
            m.insert("key".to_string(), JsonValue::String(k.clone()));
        }
        if let Some(ref dv) = c.default_value {
            m.insert("default_value".to_string(), JsonValue::String(dv.clone()));
        }
        if let Some(ref cm) = c.comment {
            m.insert("comment".to_string(), JsonValue::String(cm.clone()));
        }
        JsonValue::Object(m)
    }).collect();

    ToolResult {
        success: true,
        result: Some(JsonValue::Array(result)),
        error: None,
    }
}

/// 获取表数据样本
async fn exec_get_table_sample(
    args: JsonValue,
    connection_id: &str,
    database: &str,
    state: &AppState,
) -> ToolResult {
    let table_name = match args.get("table_name").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return tool_err!("缺少 table_name 参数"),
    };

    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(10) as i64;

    let pools = state.pools.read().await;
    let pool = match pools.get(connection_id) {
        Some(p) => p,
        None => return tool_err!("数据库未连接"),
    };

    let db_ops = match pool.as_db_ops(state, connection_id, database).await {
        Ok(ops) => ops,
        Err(e) => return tool_err!(e),
    };

    let result = match db_ops.get_table_data(database, table_name, 1, limit, None, None, None).await {
        Ok(r) => {
            let rows: Vec<Map<String, JsonValue>> = r.rows.iter().map(|row| {
                let mut m = Map::new();
                for (i, col) in r.columns.iter().enumerate() {
                    if let Some(val) = row.get(i) {
                        m.insert(col.clone(), val.clone());
                    }
                }
                m
            }).collect();

            let mut response = Map::new();
            response.insert("columns".to_string(), JsonValue::Array(
                r.columns.iter().map(|c| JsonValue::String(c.clone())).collect()
            ));
            response.insert("rows".to_string(), JsonValue::Array(
                rows.into_iter().map(JsonValue::Object).collect()
            ));

            JsonValue::Object(response)
        }
        Err(e) => return tool_err!(e),
    };

    ToolResult {
        success: true,
        result: Some(result),
        error: None,
    }
}
