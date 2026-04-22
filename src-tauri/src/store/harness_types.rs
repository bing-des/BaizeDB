//! Harness 类型定义 — 简化版
//!
//! 只保留核心数据结构，所有执行流程由大模型控制。

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

// ─────────────────────────────────────────────────────────────────────────────
// 表关系
// ─────────────────────────────────────────────────────────────────────────────

/// 表关系分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRelationAnalysis {
    pub source_table: String,
    pub source_column: String,
    pub target_table: String,
    pub target_column: String,
    pub relation_type: String,
    pub confidence: f32,
    pub reason: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// LLM 配置
// ─────────────────────────────────────────────────────────────────────────────

/// LLM 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmConfig {
    pub api_key: String,
    pub api_url: String,
    pub model: String,
    pub enabled: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具相关
// ─────────────────────────────────────────────────────────────────────────────

/// 工具执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub result: Option<JsonValue>,
    pub error: Option<String>,
}

/// 执行 SQL 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteSqlRequest {
    pub connection_id: String,
    pub database: String,
    pub sql: String,
}

/// 保存关系请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveRelationsRequest {
    pub connection_id: String,
    pub database: String,
    pub relations: Vec<TableRelationAnalysis>,
}

/// 查询关系请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRelationsRequest {
    pub connection_id: String,
    pub database: String,
}

/// 关系列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationsResponse {
    pub relations: Vec<TableRelationAnalysis>,
    pub count: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// 辅助函数
// ─────────────────────────────────────────────────────────────────────────────

/// 被忽略的外键候选字段（审计、时间、软删除等基础字段）
pub const IGNORED_FK_FIELDS: &[&str] = &[
    "create_by", "created_by", "update_by", "updated_by",
    "create_user", "update_user", "create_time", "created_time",
    "update_time", "updated_time", "create_at", "update_at",
    "delete_flag", "is_deleted", "deleted", "del_flag",
    "tenant_id",
    "remark", "remarks", "sort_order", "version",
    "is_enable", "is_active", "status",
];

/// 判断字段是否应该被忽略（不作为外键候选）
pub fn is_ignored_fk_field(column_name: &str) -> bool {
    let col_lower = column_name.to_lowercase();
    IGNORED_FK_FIELDS.contains(&col_lower.as_str())
}

/// 判断关系类型
pub fn determine_relation_type(source_column: &str, target_column: &str) -> String {
    if target_column == "id" || target_column.ends_with("_id") {
        "many_to_one".to_string()  // 多对一：多条源记录指向一条目标记录
    } else {
        "one_to_one".to_string()
    }
}
