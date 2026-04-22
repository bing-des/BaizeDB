//! Harness 工具层 — 简化版
//!
//! 只暴露3个核心工具给前端，执行流程由大模型控制：
//! 1. execute_sql - 执行SQL并返回结果
//! 2. get_relations - 查询已保存的表关系
//! 3. save_relations - 保存表关系

use tauri::State;
use crate::state::AppState;
use crate::store::harness_types::{
    LlmConfig, ToolResult,
    ExecuteSqlRequest, SaveRelationsRequest, QueryRelationsRequest, RelationsResponse,
};
use crate::store::harness_executor;

// ─────────────────────────────────────────────────────────────────────────────
// 工具定义
// ─────────────────────────────────────────────────────────────────────────────

/// 获取工具定义（供前端展示给LLM）
#[tauri::command]
pub async fn harness_get_tool_definitions() -> Result<Vec<serde_json::Value>, String> {
    Ok(harness_executor::get_tool_definitions())
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具一：执行SQL
// ─────────────────────────────────────────────────────────────────────────────

/// 执行SQL语句
#[tauri::command]
pub async fn harness_execute_sql(
    state: State<'_, AppState>,
    request: ExecuteSqlRequest,
) -> Result<ToolResult, String> {
    let args = serde_json::json!({
        "sql": request.sql
    });
    
    let result = harness_executor::execute_tool(
        "execute_sql",
        args,
        &request.connection_id,
        &request.database,
        &state,
    ).await;
    
    Ok(result)
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具二：查询已保存的关系
// ─────────────────────────────────────────────────────────────────────────────

/// 获取已保存的表关系
#[tauri::command]
pub async fn harness_get_relations(
    state: State<'_, AppState>,
    request: QueryRelationsRequest,
) -> Result<RelationsResponse, String> {
    let relations = state.store.get_relations(
        &request.connection_id,
        &request.database,
    ).await?;
    
    let count = relations.len();
    
    Ok(RelationsResponse {
        relations,
        count,
    })
}

/// 检查是否存在已保存的关系
#[tauri::command]
pub async fn harness_has_relations(
    state: State<'_, AppState>,
    request: QueryRelationsRequest,
) -> Result<bool, String> {
    state.store.has_relations(
        &request.connection_id,
        &request.database,
    ).await
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具三：保存关系
// ─────────────────────────────────────────────────────────────────────────────

/// 保存表关系分析结果
#[tauri::command]
pub async fn harness_save_relations(
    state: State<'_, AppState>,
    request: SaveRelationsRequest,
) -> Result<(), String> {
    state.store.save_relations(
        &request.connection_id,
        &request.database,
        &request.relations,
    ).await
}

/// 删除表关系分析结果
#[tauri::command]
pub async fn harness_delete_relations(
    state: State<'_, AppState>,
    request: QueryRelationsRequest,
) -> Result<(), String> {
    state.store.delete_relations(
        &request.connection_id,
        &request.database,
    ).await
}

// ─────────────────────────────────────────────────────────────────────────────
// 导入导出
// ─────────────────────────────────────────────────────────────────────────────

/// 导出表关系为 JSON
#[tauri::command]
pub async fn harness_export_relations(
    state: State<'_, AppState>,
    request: QueryRelationsRequest,
) -> Result<String, String> {
    let relations = state.store.get_relations(
        &request.connection_id,
        &request.database,
    ).await?;
    
    serde_json::to_string_pretty(&relations)
        .map_err(|e| format!("JSON 序列化失败: {}", e))
}

/// 从 JSON 导入表关系
#[tauri::command]
pub async fn harness_import_relations(
    state: State<'_, AppState>,
    request: SaveRelationsRequest,
) -> Result<usize, String> {
    state.store.save_relations(
        &request.connection_id,
        &request.database,
        &request.relations,
    ).await?;
    
    Ok(request.relations.len())
}

// ─────────────────────────────────────────────────────────────────────────────
// 辅助工具（供LLM使用）
// ─────────────────────────────────────────────────────────────────────────────

/// 列出所有表
#[tauri::command]
pub async fn harness_list_tables(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<ToolResult, String> {
    let result = harness_executor::execute_tool(
        "list_tables",
        serde_json::json!({}),
        &connection_id,
        &database,
        &state,
    ).await;
    
    Ok(result)
}

/// 获取表结构
#[tauri::command]
pub async fn harness_get_table_schema(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    table_name: String,
) -> Result<ToolResult, String> {
    let args = serde_json::json!({
        "table_name": table_name
    });
    
    let result = harness_executor::execute_tool(
        "get_table_schema",
        args,
        &connection_id,
        &database,
        &state,
    ).await;
    
    Ok(result)
}

/// 获取表数据样本
#[tauri::command]
pub async fn harness_get_table_sample(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    table_name: String,
    limit: Option<i64>,
) -> Result<ToolResult, String> {
    let args = serde_json::json!({
        "table_name": table_name,
        "limit": limit.unwrap_or(10)
    });
    
    let result = harness_executor::execute_tool(
        "get_table_sample",
        args,
        &connection_id,
        &database,
        &state,
    ).await;
    
    Ok(result)
}

// ─────────────────────────────────────────────────────────────────────────────
// LLM 配置
// ─────────────────────────────────────────────────────────────────────────────

/// 获取 LLM 配置
#[tauri::command]
pub async fn harness_get_llm_config(
    state: State<'_, AppState>,
) -> Result<LlmConfig, String> {
    state.store.get_llm_config().await
}

/// 保存 LLM 配置
#[tauri::command]
pub async fn harness_save_llm_config(
    state: State<'_, AppState>,
    config: LlmConfig,
) -> Result<(), String> {
    state.store.save_llm_config(&config).await
}
