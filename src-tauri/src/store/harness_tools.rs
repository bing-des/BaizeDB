use serde_json::{Value as JsonValue, Map};
use crate::store::harness_analyzer::{SubAnalysisSession, RelationCandidate};
use crate::state::AppState;

/// 工具执行结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub result: Option<JsonValue>,
    pub error: Option<String>,
}

/// 获取所有工具定义（用于 LLM 函数调用）
pub fn get_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_database_schema",
                "description": "获取数据库中所有表的列表和基本信息",
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
                "description": "获取指定表的完整结构信息，包括所有字段的名称、类型、备注等",
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
                "description": "获取表的数据样本，用于了解数据分布和格式",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "table_name": {
                            "type": "string",
                            "description": "表名"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "返回的行数，默认 10",
                            "default": 10
                        }
                    },
                    "required": ["table_name"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_column_unique_values",
                "description": "获取某列的唯一值分布，用于验证外键关系",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "table_name": {
                            "type": "string",
                            "description": "表名"
                        },
                        "column_name": {
                            "type": "string",
                            "description": "列名"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "返回的唯一值数量，默认 50",
                            "default": 50
                        }
                    },
                    "required": ["table_name", "column_name"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "verify_foreign_key",
                "description": "验证两个表之间是否存在外键关系（通过值重叠率判断）",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "source_table": {
                            "type": "string",
                            "description": "源表名（通常包含外键的表）"
                        },
                        "source_column": {
                            "type": "string",
                            "description": "源列名（外键列）"
                        },
                        "target_table": {
                            "type": "string",
                            "description": "目标表名（通常包含主键的表）"
                        },
                        "target_column": {
                            "type": "string",
                            "description": "目标列名（主键列）"
                        },
                        "sample_size": {
                            "type": "integer",
                            "description": "采样大小，用于验证",
                            "default": 100
                        }
                    },
                    "required": ["source_table", "source_column", "target_table", "target_column"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "add_candidate",
                "description": "添加一个候选关系到分析列表",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "source_table": {
                            "type": "string",
                            "description": "源表名"
                        },
                        "source_column": {
                            "type": "string",
                            "description": "源列名"
                        },
                        "target_table": {
                            "type": "string",
                            "description": "目标表名"
                        },
                        "target_column": {
                            "type": "string",
                            "description": "目标列名"
                        },
                        "confidence": {
                            "type": "number",
                            "description": "初始置信度 0.0-1.0"
                        },
                        "reason": {
                            "type": "string",
                            "description": "识别理由"
                        }
                    },
                    "required": ["source_table", "source_column", "target_table", "target_column", "confidence", "reason"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "update_candidate",
                "description": "更新候选关系的验证状态",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "source_table": {
                            "type": "string",
                            "description": "源表名"
                        },
                        "source_column": {
                            "type": "string",
                            "description": "源列名"
                        },
                        "target_table": {
                            "type": "string",
                            "description": "目标表名"
                        },
                        "target_column": {
                            "type": "string",
                            "description": "目标列名"
                        },
                        "verified": {
                            "type": "boolean",
                            "description": "是否验证通过"
                        },
                        "confidence": {
                            "type": "number",
                            "description": "更新后的置信度 0.0-1.0"
                        },
                        "verification_method": {
                            "type": "string",
                            "description": "验证方法"
                        }
                    },
                    "required": ["source_table", "source_column", "target_table", "target_column"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_candidates",
                "description": "获取当前所有候选关系及其状态",
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
                "name": "list_tables",
                "description": "列出数据库中所有表名",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        }),
    ]
}

/// 执行工具
pub async fn execute_tool(
    tool_name: &str,
    arguments: JsonValue,
    session: &mut SubAnalysisSession,
    state: &AppState,
) -> ToolResult {
    log::info!("执行工具: {} with args: {:?}", tool_name, arguments);

    match tool_name {
        "get_database_schema" => get_database_schema(session, state).await,
        "get_table_schema" => get_table_schema(session, state, arguments).await,
        "get_table_sample" => get_table_sample(session, state, arguments).await,
        "get_column_unique_values" => get_column_unique_values(session, state, arguments).await,
        "verify_foreign_key" => verify_foreign_key(session, state, arguments).await,
        "add_candidate" => add_candidate(session, arguments),
        "update_candidate" => update_candidate(session, arguments),
        "get_candidates" => get_candidates(session),
        "list_tables" => list_tables(session, state).await,
        _ => ToolResult {
            success: false,
            result: None,
            error: Some(format!("未知工具: {}", tool_name)),
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具实现
// ─────────────────────────────────────────────────────────────────────────────

async fn get_database_schema(session: &SubAnalysisSession, state: &AppState) -> ToolResult {
    let pools = state.pools.read().await;
    let pool = match pools.get(&session.connection_id) {
        Some(p) => p,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("数据库未连接".to_string()),
        },
    };

    let db_pool = match pool.as_db_ops(state, &session.connection_id, &session.database).await {
        Ok(p) => p,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(e),
        },
    };

    let tables = match db_pool.list_tables(&session.database, session.schema.as_deref()).await {
        Ok(t) => t,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(e),
        },
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

async fn get_table_schema(session: &SubAnalysisSession, state: &AppState, args: JsonValue) -> ToolResult {
    let table_name = match args.get("table_name").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 table_name 参数".to_string()),
        },
    };

    let pools = state.pools.read().await;
    let pool = match pools.get(&session.connection_id) {
        Some(p) => p,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("数据库未连接".to_string()),
        },
    };

    let db_pool = match pool.as_db_ops(state, &session.connection_id, &session.database).await {
        Ok(p) => p,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(e),
        },
    };

    let columns = match db_pool.list_columns(&session.database, table_name).await {
        Ok(c) => c,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(e),
        },
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

async fn get_table_sample(session: &SubAnalysisSession, state: &AppState, args: JsonValue) -> ToolResult {
    let table_name = match args.get("table_name").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 table_name 参数".to_string()),
        },
    };

    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(10) as i64;

    let pools = state.pools.read().await;
    let pool = match pools.get(&session.connection_id) {
        Some(p) => p,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("数据库未连接".to_string()),
        },
    };

    let db_pool = match pool.as_db_ops(state, &session.connection_id, &session.database).await {
        Ok(p) => p,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(e),
        },
    };

    let result = match db_pool.get_table_data(&session.database, table_name, 1, limit, None, None, None).await {
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
        },
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(e),
        },
    };

    ToolResult {
        success: true,
        result: Some(result),
        error: None,
    }
}

async fn get_column_unique_values(session: &SubAnalysisSession, state: &AppState, args: JsonValue) -> ToolResult {
    let table_name = match args.get("table_name").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 table_name 参数".to_string()),
        },
    };

    let column_name = match args.get("column_name").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 column_name 参数".to_string()),
        },
    };

    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(50) as i64;

    let pools = state.pools.read().await;
    let pool = match pools.get(&session.connection_id) {
        Some(p) => p,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("数据库未连接".to_string()),
        },
    };

    let db_pool = match pool.as_db_ops(state, &session.connection_id, &session.database).await {
        Ok(p) => p,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(e),
        },
    };

    // 使用 SQL 查询获取唯一值
    let quoted_col = if db_pool.is_postgres() {
        format!("\"{}\"", column_name)
    } else {
        column_name.to_string()
    };

    let sql = format!("SELECT DISTINCT {} FROM {} ORDER BY {} LIMIT {}", 
        quoted_col, table_name, quoted_col, limit);

    let result = match db_pool.query_sql(&sql).await {
        Ok(r) => {
            let values: Vec<JsonValue> = r.rows.iter()
                .filter_map(|row| row.first().cloned())
                .collect();

            let mut response = Map::new();
            response.insert("column".to_string(), JsonValue::String(column_name.to_string()));
            response.insert("unique_values".to_string(), JsonValue::Array(values));
            JsonValue::Object(response)
        },
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(e),
        },
    };

    ToolResult {
        success: true,
        result: Some(result),
        error: None,
    }
}

async fn verify_foreign_key(session: &SubAnalysisSession, state: &AppState, args: JsonValue) -> ToolResult {
    let source_table = match args.get("source_table").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 source_table 参数".to_string()),
        },
    };

    let source_column = match args.get("source_column").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 source_column 参数".to_string()),
        },
    };

    let target_table = match args.get("target_table").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 target_table 参数".to_string()),
        },
    };

    let target_column = match args.get("target_column").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 target_column 参数".to_string()),
        },
    };

    let sample_size = args.get("sample_size").and_then(|v| v.as_i64()).unwrap_or(100) as i64;

    let pools = state.pools.read().await;
    let pool = match pools.get(&session.connection_id) {
        Some(p) => p,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("数据库未连接".to_string()),
        },
    };

    let db_pool = match pool.as_db_ops(state, &session.connection_id, &session.database).await {
        Ok(p) => p,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(e),
        },
    };

    let is_pg = db_pool.is_postgres();

    let quote = |s: &str| -> String {
        if is_pg { format!("\"{}\"", s) } else { s.to_string() }
    };

    // 检查目标表是否存在该列
    let target_columns = match db_pool.list_columns(&session.database, target_table).await {
        Ok(cols) => cols,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(format!("获取目标表结构失败: {}", e)),
        },
    };

    if !target_columns.iter().any(|c| c.name == target_column) {
        // 目标表字段不存在 - 这是唯一失败的情况
        return ToolResult {
            success: true,
            result: Some(serde_json::json!({ 
                "candidates": [{
                    "source_table": source_table,
                    "source_column": source_column,
                    "target_table": target_table,
                    "target_column": target_column,
                    "confidence": 0.0,
                    "reason": format!("目标表 {} 不存在列 {}", target_table, target_column),
                    "verified": false,
                    "verification_method": null
                }]
            })),
            error: None,
        };
    }

    // 计算源表中非空外键值
    let source_sql = format!(
        "SELECT COUNT(DISTINCT {}) as total, COUNT({}) as non_null FROM {} WHERE {} IS NOT NULL",
        quote(source_column), quote(source_column), source_table, quote(source_column)
    );

    let source_result = match db_pool.query_sql(&source_sql).await {
        Ok(r) => r,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(format!("查询源表失败: {}", e)),
        },
    };

    let source_stats = if let Some(row) = source_result.rows.first() {
        let total: i64 = row.get(0).and_then(|v| v.as_i64()).unwrap_or(0);
        let non_null: i64 = row.get(1).and_then(|v| v.as_i64()).unwrap_or(0);
        (total, non_null)
    } else {
        (0, 0)
    };

    // 计算目标表中唯一值数量
    let target_sql = format!(
        "SELECT COUNT(DISTINCT {}) as unique_count FROM {}",
        quote(target_column), target_table
    );

    let target_result = match db_pool.query_sql(&target_sql).await {
        Ok(r) => r,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(format!("查询目标表失败: {}", e)),
        },
    };

    let target_unique: i64 = target_result.rows.first()
        .and_then(|row| row.get(0))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    // 计算重叠率（源表外键值在目标表中存在的比例）
    let overlap_sql = format!(
        "SELECT COUNT(DISTINCT s.{}) as overlap_count 
         FROM {} s 
         WHERE s.{} IS NOT NULL 
         AND EXISTS (SELECT 1 FROM {} t WHERE t.{} = s.{})",
        quote(source_column), source_table, quote(source_column),
        target_table, quote(target_column), quote(source_column)
    );

    let overlap_result = match db_pool.query_sql(&overlap_sql).await {
        Ok(r) => r,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(format!("计算重叠率失败: {}", e)),
        },
    };

    let overlap_count: i64 = overlap_result.rows.first()
        .and_then(|row| row.get(0))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    // 计算重叠率
    let overlap_rate = if source_stats.1 > 0 {
        overlap_count as f64 / source_stats.1 as f64
    } else {
        0.0
    };

    // 判断是否为有效外键关系
    // 只要目标表字段存在，就认为验证通过（verified=true）
    // confidence 根据 overlap_rate 计算
    let has_data = source_stats.1 > 0;
    let confidence = if has_data { overlap_rate as f32 } else { 0.0 };
    let is_valid = has_data; // 只要有数据就认为有效，confidence 反映匹配程度

    // 返回 RelationCandidate 格式
    let candidates = vec![serde_json::json!({
        "source_table": source_table,
        "source_column": source_column,
        "target_table": target_table,
        "target_column": target_column,
        "confidence": confidence,
        "reason": format!(
            "重叠率: {:.1}% ({}/{} 个值匹配), 源表 {} 条非空值, 目标表 {} 条唯一值, {}",
            overlap_rate * 100.0, overlap_count, source_stats.1, source_stats.1, target_unique,
            if confidence >= 0.9 { "强关联" } else if confidence >= 0.5 { "中等关联" } else if confidence > 0.0 { "弱关联" } else { "无数据" }
        ),
        "verified": is_valid,
        "verification_method": if is_valid { Some("overlap_check".to_string()) } else { None }
    })];

    ToolResult {
        success: true,
        result: Some(serde_json::json!({ "candidates": candidates })),
        error: None,
    }
}

fn add_candidate(session: &mut SubAnalysisSession, args: JsonValue) -> ToolResult {

    let source_table = match args.get("source_table").and_then(|v| v.as_str()) {
        Some(v) => v.to_string(),
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 source_table 参数".to_string()),
        },
    };

    let source_column = match args.get("source_column").and_then(|v| v.as_str()) {
        Some(v) => v.to_string(),
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 source_column 参数".to_string()),
        },
    };

    let target_table = match args.get("target_table").and_then(|v| v.as_str()) {
        Some(v) => v.to_string(),
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 target_table 参数".to_string()),
        },
    };

    let target_column = match args.get("target_column").and_then(|v| v.as_str()) {
        Some(v) => v.to_string(),
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 target_column 参数".to_string()),
        },
    };

    let confidence = args.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
    let reason = args.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let candidate = RelationCandidate {
        source_table,
        source_column,
        target_table,
        target_column,
        confidence,
        reason,
        verified: false,
        verification_method: None,
    };

    session.candidates.push(candidate.clone());

    ToolResult {
        success: true,
        result: Some(serde_json::to_value(candidate).unwrap_or(JsonValue::Null)),
        error: None,
    }
}

fn update_candidate(session: &mut SubAnalysisSession, args: JsonValue) -> ToolResult {
    let source_table = match args.get("source_table").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("缺少 source_table 参数".to_string()),
        },
    };

    let source_column = args.get("source_column").and_then(|v| v.as_str());
    let target_table = args.get("target_table").and_then(|v| v.as_str());
    let target_column = args.get("target_column").and_then(|v| v.as_str());

    // 查找并更新候选
    for candidate in &mut session.candidates {
        if candidate.source_table == source_table {
            if source_column.map(|s| candidate.source_column == s).unwrap_or(true) &&
               target_table.map(|t| candidate.target_table == t).unwrap_or(true) &&
               target_column.map(|t| candidate.target_column == t).unwrap_or(true) {
                
                if let Some(v) = args.get("verified").and_then(|v| v.as_bool()) {
                    candidate.verified = v;
                }
                if let Some(c) = args.get("confidence").and_then(|v| v.as_f64()) {
                    candidate.confidence = c as f32;
                }
                if let Some(m) = args.get("verification_method").and_then(|v| v.as_str()) {
                    candidate.verification_method = Some(m.to_string());
                }

                return ToolResult {
                    success: true,
                    result: Some(serde_json::to_value(candidate.clone()).unwrap_or(JsonValue::Null)),
                    error: None,
                };
            }
        }
    }

    ToolResult {
        success: false,
        result: None,
        error: Some("未找到匹配的候选关系".to_string()),
    }
}

fn get_candidates(session: &SubAnalysisSession) -> ToolResult {
    let candidates: Vec<_> = session.candidates.iter().map(|c| {
        let mut map = Map::new();
        map.insert("source_table".to_string(), JsonValue::String(c.source_table.clone()));
        map.insert("source_column".to_string(), JsonValue::String(c.source_column.clone()));
        map.insert("target_table".to_string(), JsonValue::String(c.target_table.clone()));
        map.insert("target_column".to_string(), JsonValue::String(c.target_column.clone()));
        map.insert("confidence".to_string(), JsonValue::Number(serde_json::Number::from_f64(c.confidence as f64).unwrap_or(serde_json::Number::from(0))));
        map.insert("reason".to_string(), JsonValue::String(c.reason.clone()));
        map.insert("verified".to_string(), JsonValue::Bool(c.verified));
        if let Some(ref method) = c.verification_method {
            map.insert("verification_method".to_string(), JsonValue::String(method.clone()));
        }
        JsonValue::Object(map)
    }).collect();

    let mut response = Map::new();
    response.insert("total".to_string(), JsonValue::Number(session.candidates.len().into()));
    response.insert("verified".to_string(), JsonValue::Number(session.candidates.iter().filter(|c| c.verified).count().into()));
    response.insert("candidates".to_string(), JsonValue::Array(candidates));

    ToolResult {
        success: true,
        result: Some(JsonValue::Object(response)),
        error: None,
    }
}

async fn list_tables(session: &SubAnalysisSession, state: &AppState) -> ToolResult {
    let pools = state.pools.read().await;
    let pool = match pools.get(&session.connection_id) {
        Some(p) => p,
        None => return ToolResult {
            success: false,
            result: None,
            error: Some("数据库未连接".to_string()),
        },
    };

    let db_pool = match pool.as_db_ops(state, &session.connection_id, &session.database).await {
        Ok(p) => p,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(e),
        },
    };

    let tables = match db_pool.list_tables(&session.database, session.schema.as_deref()).await {
        Ok(t) => t,
        Err(e) => return ToolResult {
            success: false,
            result: None,
            error: Some(e),
        },
    };

    let table_names: Vec<JsonValue> = tables.iter()
        .map(|t| JsonValue::String(t.name.clone()))
        .collect();

    ToolResult {
        success: true,
        result: Some(JsonValue::Array(table_names)),
        error: None,
    }
}
