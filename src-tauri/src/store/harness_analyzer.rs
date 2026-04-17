use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use anyhow::{Result, Context};
use crate::store::TableRelationAnalysis;
use crate::store::ToolResult;
use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// 分析阶段
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "stage", content = "data")]
pub enum AnalysisStage {
    /// 准备阶段：收集所有表名
    Preparing,
    /// 分析阶段：正在分析某张表
    Analyzing(String),
    /// 完成阶段
    Completed,
    /// 失败
    Failed(String),
}

impl Default for AnalysisStage {
    fn default() -> Self {
        AnalysisStage::Preparing
    }
}

/// 单表分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableAnalysisResult {
    /// 表名
    pub table_name: String,
    /// 该表作为源的关系
    pub outgoing_relations: Vec<RelationCandidate>,
    /// 该表作为目标的关系
    pub incoming_relations: Vec<RelationCandidate>,
    /// 分析耗时（轮次）
    pub turns_used: usize,
    /// 是否已保存到 SQLite
    #[serde(default)]
    pub saved_to_sqlite: bool,
}

/// 主会话状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessSession {
    pub id: String,
    pub connection_id: String,
    pub database: String,
    pub schema: Option<String>,
    pub tables: Vec<String>,
    pub current_table_index: usize,
    pub results: Vec<TableAnalysisResult>,
    pub current_stage: AnalysisStage,
    pub current_sub_session: Option<SubAnalysisSession>,
}

impl HarnessSession {
    pub fn new(id: String, connection_id: String, database: String, schema: Option<String>) -> Self {
        Self {
            id,
            connection_id,
            database,
            schema,
            tables: Vec::new(),
            current_table_index: 0,
            results: Vec::new(),
            current_stage: AnalysisStage::Preparing,
            current_sub_session: None,
        }
    }

    /// 获取当前正在分析的表名
    pub fn current_table(&self) -> Option<&String> {
        self.tables.get(self.current_table_index)
    }

    /// 是否所有表都已分析完成
    pub fn is_complete(&self) -> bool {
        self.current_table_index >= self.tables.len() && self.current_sub_session.is_none()
    }

    /// 获取进度百分比
    pub fn progress(&self) -> f32 {
        if self.tables.is_empty() {
            return 0.0;
        }
        let completed = self.results.len();
        let in_progress = if self.current_sub_session.is_some() { 0.5 } else { 0.0 };
        (completed as f32 + in_progress) / self.tables.len() as f32
    }

    /// 获取汇总关系
    pub fn get_all_relations(&self) -> Vec<TableRelationAnalysis> {
        let mut relations = Vec::new();
        for result in &self.results {
            for candidate in &result.outgoing_relations {
                if candidate.confidence >= 0.7 {
                    relations.push(TableRelationAnalysis {
                        source_table: candidate.source_table.clone(),
                        source_column: candidate.source_column.clone(),
                        target_table: candidate.target_table.clone(),
                        target_column: candidate.target_column.clone(),
                        relation_type: determine_relation_type(candidate),
                        confidence: candidate.confidence,
                        reason: candidate.reason.clone(),
                    });
                }
            }
        }
        relations
    }
}

/// 单表分析子会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAnalysisSession {
    pub connection_id: String,
    pub database: String,
    pub schema: Option<String>,
    pub table_name: String,
    pub table_schema: String,        // 该表的结构信息（最小上下文）
    pub other_tables_preview: String, // 其他表的预览（表名和主键，仅用于匹配）
    pub candidates: Vec<RelationCandidate>,
    pub turns_used: usize,
    pub max_turns: usize,
    pub messages: Vec<ChatMessage>,
    pub completed: bool,
}

impl SubAnalysisSession {
    pub fn new(
        connection_id: String,
        database: String,
        schema: Option<String>,
        table_name: String,
        table_schema: String,
        other_tables_preview: String,
    ) -> Self {
        Self {
            connection_id,
            database,
            schema,
            table_name,
            table_schema,
            other_tables_preview,
            candidates: Vec::new(),
            turns_used: 0,
            max_turns: 50,
            messages: Vec::new(),
            completed: false,
        }
    }

    /// 提取分析结果
    pub fn extract_result(self) -> TableAnalysisResult {
        let (outgoing, incoming): (Vec<_>, Vec<_>) = self.candidates
            .into_iter()
            .partition(|c| c.source_table == self.table_name);

        TableAnalysisResult {
            table_name: self.table_name,
            outgoing_relations: outgoing,
            incoming_relations: incoming,
            turns_used: self.turns_used,
            saved_to_sqlite: false,
        }
    }
}

/// 候选关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationCandidate {
    pub source_table: String,
    pub source_column: String,
    pub target_table: String,
    pub target_column: String,
    pub confidence: f32,
    pub reason: String,
    pub verified: bool,
    pub verification_method: Option<String>,
}

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// LLM 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmRequest {
    model: String,
    messages: Vec<LlmMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<LlmRequestToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmRequestToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: LlmFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmFunction {
    name: String,
    arguments: String,
}

/// LLM 响应
#[derive(Debug, Clone, Deserialize)]
struct LlmResponse {
    choices: Vec<LlmChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<LlmApiError>,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmChoice {
    message: LlmResponseMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmResponseMessage {
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<LlmResponseToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmResponseToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: LlmFunction,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmApiError {
    message: String,
    #[serde(rename = "type")]
    error_type: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// 分析阶段（用于提示词分阶段）
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum AnalysisPhase {
    Discovery,      // 发现阶段：探索表结构，找出候选外键
    Validation,     // 验证阶段：验证候选关系
    Finalization,   // 结束阶段：总结并结束
}

// ─────────────────────────────────────────────────────────────────────────────
// Harness Analyzer
// ─────────────────────────────────────────────────────────────────────────────

/// Harness 多阶段分析器（按表分析）
#[derive(Clone)]
pub struct HarnessAnalyzer {
    api_key: String,
    api_url: String,
    model: String,
}

impl HarnessAnalyzer {
    pub fn new(api_key: String, api_url: String, model: String) -> Self {
        Self {
            api_key,
            api_url,
            model,
        }
    }

    /// 创建新会话
    pub fn create_session(
        connection_id: String,
        database: String,
        schema: Option<String>,
    ) -> HarnessSession {
        HarnessSession::new(
            uuid::Uuid::new_v4().to_string(),
            connection_id,
            database,
            schema,
        )
    }

    /// 执行一轮分析 - 主会话协调器
    pub async fn run_analysis_turn(
        &self,
        session: &mut HarnessSession,
        state: &AppState,
    ) -> Result<AnalysisTurnResult> {
        // 阶段1: 准备 - 获取所有表名
        if let AnalysisStage::Preparing = session.current_stage {
            return self.prepare_tables(session, state).await;
        }

        // 阶段2: 分析单张表 - 需要取出 sub_session 来避免双重可变借用
        if session.current_sub_session.is_some() {
            // 取出 sub_session
            let mut sub = session.current_sub_session.take().unwrap();
            let result = self.run_sub_session_turn(&mut sub, session, state).await;
            // 只有当子会话未完成时才放回去
            if !sub.completed {
                session.current_sub_session = Some(sub);
            }
            // 如果子会话已完成，finish_sub_session 已经处理了状态转换
            return result;
        }

        // 阶段3: 检查是否还有未分析的表
        if session.current_table_index < session.tables.len() {
            // 启动下一个表的分析
            let table_name = session.tables[session.current_table_index].clone();
            session.current_stage = AnalysisStage::Analyzing(table_name.clone());
            self.start_table_analysis(session, &table_name, state).await
        } else {
            // 所有表分析完成
            session.current_stage = AnalysisStage::Completed;
            Ok(AnalysisTurnResult {
                is_complete: true,
                current_table: None,
                progress: 1.0,
                message: Some(format!(
                    "分析完成！共分析 {} 张表，发现 {} 个候选关系",
                    session.tables.len(),
                    session.results.iter().map(|r| r.outgoing_relations.len()).sum::<usize>()
                )),
                error: None,
            })
        }
    }

    /// 准备阶段：获取所有表名
    async fn prepare_tables(
        &self,
        session: &mut HarnessSession,
        state: &AppState,
    ) -> Result<AnalysisTurnResult> {
        use crate::database::db_ops::DbOps;

        let pools = state.pools.read().await;
        let pool = pools.get(&session.connection_id)
            .ok_or_else(|| anyhow::anyhow!("连接池不存在"))?;
        let db_ops = pool.as_db_ops(state, &session.connection_id, &session.database).await
            .map_err(|e| anyhow::anyhow!("获取数据库连接失败: {}", e))?;

        let tables = db_ops.list_tables(&session.database, session.schema.as_deref())
            .await
            .map_err(|e| anyhow::anyhow!("获取表列表失败: {}", e))?;

        session.tables = tables.into_iter().map(|t| t.name).collect();
        session.current_stage = AnalysisStage::Analyzing(
            session.tables.first().cloned().unwrap_or_default()
        );

        if session.tables.is_empty() {
            session.current_stage = AnalysisStage::Failed("数据库中没有找到表".to_string());
            return Ok(AnalysisTurnResult {
                is_complete: true,
                current_table: None,
                progress: 0.0,
                message: Some("未找到任何表".to_string()),
                error: Some("数据库中没有表".to_string()),
            });
        }

        // 启动第一张表的分析
        let first_table = session.tables.first().cloned().unwrap();
        self.start_table_analysis(
            session,
            &first_table,
            state
        ).await
    }

    /// 开始分析某张表
    async fn start_table_analysis(
        &self,
        session: &mut HarnessSession,
        table_name: &str,
        state: &AppState,
    ) -> Result<AnalysisTurnResult> {
        use crate::database::db_ops::DbOps;

        let pools = state.pools.read().await;
        let pool = pools.get(&session.connection_id)
            .ok_or_else(|| anyhow::anyhow!("连接池不存在"))?;
        let db_ops = pool.as_db_ops(state, &session.connection_id, &session.database).await
            .map_err(|e| anyhow::anyhow!("获取数据库连接失败: {}", e))?;

        // 获取目标表的完整结构
        let columns = db_ops.list_columns(&session.database, table_name)
            .await
            .map_err(|e| anyhow::anyhow!("获取表结构失败: {}", e))?;

        let table_schema = self.format_table_schema(table_name, &columns);

        // 获取其他表的预览（表名 + 主键，仅用于匹配）
        let other_tables_preview = self.get_other_tables_preview(session, table_name, state).await?;

        // 创建子会话
        let sub_session = SubAnalysisSession::new(
            session.connection_id.clone(),
            session.database.clone(),
            session.schema.clone(),
            table_name.to_string(),
            table_schema,
            other_tables_preview,
        );

        session.current_sub_session = Some(sub_session);

        Ok(AnalysisTurnResult {
            is_complete: false,
            current_table: Some(table_name.to_string()),
            progress: session.progress(),
            message: Some(format!("开始分析表: {}", table_name)),
            error: None,
        })
    }

    /// 格式化表结构（最小上下文）
    fn format_table_schema(&self, table_name: &str, columns: &[crate::database::db_ops::ColumnMeta]) -> String {
        let mut lines = vec![format!("表: {}\n字段:", table_name)];
        lines.push("─".repeat(50));

        for col in columns {
            // key 字段通常是 "PRI" 表示主键
            let pk_marker = if col.key.as_ref().map(|k| k == "PRI").unwrap_or(false) { " [PK]" } else { "" };
            let null_marker = if col.nullable { " (NULL)" } else { " (NOT NULL)" };
            let default = col.default_value.as_ref()
                .map(|d| format!(" DEFAULT {}", d))
                .unwrap_or_default();
            let comment = col.comment.as_ref()
                .map(|c| format!(" -- {}", c))
                .unwrap_or_default();

            lines.push(format!(
                "  {}: {}{}{}{}",
                col.name, col.data_type, null_marker, default, pk_marker
            ));
            if !comment.trim().is_empty() && !comment.contains("NOT NULL") && !comment.contains("NULL)") {
                lines.push(format!("    备注: {}", comment.replace(" -- ", "")));
            }
        }

        lines.join("\n")
    }

    /// 获取其他表的预览（表名 + 主键信息）
    async fn get_other_tables_preview(
        &self,
        session: &HarnessSession,
        exclude_table: &str,
        state: &AppState,
    ) -> Result<String> {
        use crate::database::db_ops::DbOps;

        let pools = state.pools.read().await;
        let pool = pools.get(&session.connection_id)
            .ok_or_else(|| anyhow::anyhow!("连接池不存在"))?;
        let db_ops = pool.as_db_ops(state, &session.connection_id, &session.database).await
            .map_err(|e| anyhow::anyhow!("获取数据库连接失败: {}", e))?;

        let mut lines = vec!["\n\n数据库中的其他表（用于匹配外键关系）:".to_string()];

        for table_name in &session.tables {
            if table_name == exclude_table {
                continue;
            }

            let columns = db_ops.list_columns(&session.database, table_name)
                .await
                .map_err(|e| anyhow::anyhow!("获取表结构失败: {}", e))?;

            // 只列出主键和可能有外键的字段 (key == "PRI" 表示主键)
            let relevant: Vec<_> = columns.iter()
                .filter(|c| c.key.as_ref().map(|k| k == "PRI").unwrap_or(false) || c.name.ends_with("_id") || c.name == "id")
                .collect();

            if !relevant.is_empty() {
                lines.push(format!("\n表: {}", table_name));
                for col in relevant {
                    let pk = if col.key.as_ref().map(|k| k == "PRI").unwrap_or(false) { " [主键]" } else { "" };
                    lines.push(format!("  - {}: {}{}", col.name, col.data_type, pk));
                }
            }
        }

        if lines.len() == 1 {
            lines.push("无".to_string());
        }

        Ok(lines.join("\n"))
    }

    /// 运行单表分析的一轮
    async fn run_sub_session_turn(
        &self,
        sub_session: &mut SubAnalysisSession,
        parent_session: &mut HarnessSession,
        state: &AppState,
    ) -> Result<AnalysisTurnResult> {
        sub_session.turns_used += 1;

        // 超时检查
        if sub_session.turns_used > sub_session.max_turns {
            sub_session.completed = true;
            return self.finish_sub_session(sub_session, parent_session);
        }

        // 构建系统提示（最小上下文）
        let system_prompt = self.build_sub_session_prompt(sub_session);

        // 构建消息
        let messages = self.build_sub_messages(sub_session, &system_prompt);

        // 获取工具定义
        let tools = crate::store::harness_tools::get_tool_definitions();

        // 调用 LLM
        let response = self.call_llm(&messages, &tools).await?;
        log::info!("[LLM 响应] 表: {} | 轮次: {} | 内容: {}", 
            sub_session.table_name, sub_session.turns_used, response.choices.first().map(|c| c.message.content.clone()).unwrap_or_default());
        // 检查是否有 tool_calls
        if let Some(tool_calls) = &response.choices.first().and_then(|c| c.message.tool_calls.clone()) {
            if !tool_calls.is_empty() {
                let tool_call = &tool_calls[0];
                let arguments: JsonValue = serde_json::from_str(&tool_call.function.arguments)
                    .unwrap_or(JsonValue::Null);

                log::info!("[LLM ToolCall] 表: {} | 工具: {} | 参数: {}", 
                    sub_session.table_name, 
                    tool_call.function.name, 
                    tool_call.function.arguments);

                // 执行工具
                let tool_result = crate::store::harness_tools::execute_tool(
                    &tool_call.function.name,
                    arguments,
                    sub_session,
                    state,
                ).await;

                // 添加工具调用到消息历史
                sub_session.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: String::new(),
                    tool_calls: Some(vec![ToolCall {
                        id: tool_call.id.clone(),
                        name: tool_call.function.name.clone(),
                        arguments: tool_call.function.arguments.clone(),
                    }]),
                    tool_call_id: None,
                });

                let error_msg = tool_result.error.clone().unwrap_or_default();
                let result_json = if tool_result.success {
                    tool_result.result.clone().unwrap_or(JsonValue::Null)
                } else {
                    JsonValue::String(error_msg)
                };

                sub_session.messages.push(ChatMessage {
                    role: "tool".to_string(),
                    content: serde_json::to_string(&result_json).unwrap_or_default(),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                });

                // 检查工具是否返回了关系候选
                self.parse_candidates_from_result(sub_session, &tool_result);

                // 注意：verify_foreign_key 执行成功后，让LLM根据提示词决定是否继续验证其他候选
                // 不再自动结束，确保一张表的多个候选外键都能被验证
                
        // 强制结束条件：达到最大轮次（增加轮次以支持多外键验证）
        if sub_session.turns_used >= 20 {
            log::info!("[LLM 强制结束] 表: {} 达到最大轮次 ({}轮)，结束分析", 
                sub_session.table_name, sub_session.turns_used);
            sub_session.completed = true;
            return self.finish_sub_session(sub_session, parent_session);
        }

                return Ok(AnalysisTurnResult {
                    is_complete: false,
                    current_table: Some(sub_session.table_name.clone()),
                    progress: parent_session.progress(),
                    message: Some(format!(
                        "[{}] 工具: {} - {}",
                        sub_session.table_name,
                        tool_call.function.name,
                        if tool_result.success { "成功" } else { "失败" }
                    )),
                    error: if tool_result.success { None } else { tool_result.error },
                });
            }
        }

        // LLM 返回文本内容
        let content = response.choices.first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        log::info!("[LLM 输出] 表: {} | 内容: {}", sub_session.table_name, content);

        sub_session.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: content.clone(),
            tool_calls: None,
            tool_call_id: None,
        });

        // 检查是否完成 - 只有当LLM明确表示完成且没有未验证候选时才结束
        let unverified_count = sub_session.candidates.iter().filter(|c| !c.verified).count();
        if content.contains("ANALYSIS_COMPLETE") || content.contains("分析完成") {
            if unverified_count == 0 {
                // 所有候选都已验证，可以结束
                log::info!("[LLM 完成确认] 表: {} 所有候选已验证，结束分析", sub_session.table_name);
                sub_session.completed = true;
                return self.finish_sub_session(sub_session, parent_session);
            } else {
                // 还有未验证候选，不能结束，继续验证
                log::info!("[LLM 完成拒绝] 表: {} 还有{}个未验证候选，继续验证", 
                    sub_session.table_name, unverified_count);
                // 添加系统提示让LLM继续验证
                sub_session.messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: format!("注意：还有 {} 个候选关系未验证，请继续调用 verify_foreign_key 验证所有候选后再结束。", unverified_count),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }

        // 解析文本中的候选关系
        self.parse_candidates_from_text(sub_session, &content);

        Ok(AnalysisTurnResult {
            is_complete: false,
            current_table: Some(sub_session.table_name.clone()),
            progress: parent_session.progress(),
            message: Some(content),
            error: None,
        })
    }

    /// 解析工具结果中的候选关系
    fn parse_candidates_from_result(&self, sub_session: &mut SubAnalysisSession, result: &ToolResult) {
        if let Some(json) = &result.result {
            // 尝试解析为 { "candidates": [...] } 格式（verify_foreign_key 返回的格式）
            if let Ok(wrapper) = serde_json::from_value::<serde_json::Map<String, JsonValue>>(json.clone()) {
                if let Some(candidates_json) = wrapper.get("candidates") {
                    if let Ok(candidates) = serde_json::from_value::<Vec<RelationCandidate>>(candidates_json.clone()) {
                        for candidate in candidates {
                            // 只保存验证通过的关系，或记录未验证的候选
                            self.add_candidate_if_not_exists(sub_session, candidate);
                        }
                        return;
                    }
                }
            }
            
            // 尝试直接解析为 Vec<RelationCandidate>（add_candidate 等工具返回的格式）
            if let Ok(candidates) = serde_json::from_value::<Vec<RelationCandidate>>(json.clone()) {
                for candidate in candidates {
                    self.add_candidate_if_not_exists(sub_session, candidate);
                }
            }
        }
    }
    
    /// 添加候选关系到子会话（如果不存在）
    fn add_candidate_if_not_exists(&self, sub_session: &mut SubAnalysisSession, candidate: RelationCandidate) {
        // 去重检查
        if !sub_session.candidates.iter().any(|c| 
            c.source_table == candidate.source_table &&
            c.source_column == candidate.source_column &&
            c.target_table == candidate.target_table &&
            c.target_column == candidate.target_column
        ) {
            log::info!("[候选关系] 表: {} | {}.{} → {}.{} | confidence: {:.2}% | verified: {}",
                sub_session.table_name,
                candidate.source_table, candidate.source_column,
                candidate.target_table, candidate.target_column,
                candidate.confidence * 100.0,
                candidate.verified
            );
            sub_session.candidates.push(candidate);
        }
    }

    /// 从文本中解析候选关系
    fn parse_candidates_from_text(&self, sub_session: &mut SubAnalysisSession, content: &str) {
        // 尝试解析 JSON 数组
        if let Ok(candidates) = serde_json::from_str::<Vec<RelationCandidate>>(content) {
            for candidate in candidates {
                if !sub_session.candidates.iter().any(|c| c.source_table == candidate.source_table
                    && c.source_column == candidate.source_column
                    && c.target_table == candidate.target_table
                    && c.target_column == candidate.target_column)
                {
                    sub_session.candidates.push(candidate);
                }
            }
        }
    }

    /// 完成子会话
    fn finish_sub_session(
        &self,
        sub_session: &mut SubAnalysisSession,
        parent_session: &mut HarnessSession,
    ) -> Result<AnalysisTurnResult> {
        // 提取结果并保存（clone 因为 extract_result 需要 ownership）
        let result = sub_session.clone().extract_result();
        let table_name = result.table_name.clone();
        let outgoing_count = result.outgoing_relations.len();
        let incoming_count = result.incoming_relations.len();
        parent_session.results.push(result);

        // 清除子会话，移到下一张表
        parent_session.current_sub_session = None;
        parent_session.current_table_index += 1;

        // 检查是否还有未分析的表
        if parent_session.current_table_index >= parent_session.tables.len() {
            // 所有表分析完成
            parent_session.current_stage = AnalysisStage::Completed;
        } else {
            // 启动下一张表的分析
            parent_session.current_stage = AnalysisStage::Analyzing(
                parent_session.tables.get(parent_session.current_table_index)
                    .cloned()
                    .unwrap_or_default()
            );
        }

        Ok(AnalysisTurnResult {
            is_complete: false,
            current_table: None,
            progress: parent_session.progress(),
            message: Some(format!(
                "表 [{}] 分析完成，发现 {} 个外键关系，{} 个被引用关系",
                table_name, outgoing_count, incoming_count
            )),
            error: None,
        })
    }

    /// 构建子会话的系统提示（分阶段）
    fn build_sub_session_prompt(&self, sub_session: &SubAnalysisSession) -> String {
        let phase = Self::detect_phase(sub_session);
        
        match phase {
            AnalysisPhase::Discovery => self.build_discovery_prompt(sub_session),
            AnalysisPhase::Validation => self.build_validation_prompt(sub_session),
            AnalysisPhase::Finalization => self.build_finalization_prompt(sub_session),
        }
    }

    /// 检测当前分析阶段
    fn detect_phase(sub_session: &SubAnalysisSession) -> AnalysisPhase {
        let verified_count = sub_session.candidates.iter().filter(|c| c.verified).count();
        let unverified_count = sub_session.candidates.iter().filter(|c| !c.verified).count();
        
        // 如果达到最大轮次的80%，进入结束阶段
        if sub_session.turns_used >= (sub_session.max_turns * 8 / 10) {
            log::info!("[阶段检测] 表: {} 达到80%最大轮次，进入Finalization", sub_session.table_name);
            return AnalysisPhase::Finalization;
        }
        
        // Discovery阶段：没有候选且轮次较少时探索
        // 一旦有了候选，或者超过8轮，就进入Validation
        if sub_session.candidates.is_empty() && sub_session.turns_used < 8 {
            log::info!("[阶段检测] 表: {} 无候选，第{}轮，继续Discovery", 
                sub_session.table_name, sub_session.turns_used);
            return AnalysisPhase::Discovery;
        }
        
        // 有未验证候选时，保持在Validation阶段继续验证
        if unverified_count > 0 {
            log::info!("[阶段检测] 表: {} 还有{}个未验证候选，继续Validation", 
                sub_session.table_name, unverified_count);
            return AnalysisPhase::Validation;
        }
        
        // 有候选但都已验证，进入结束阶段总结
        if verified_count > 0 {
            log::info!("[阶段检测] 表: {} {}个候选都已验证，进入Finalization", 
                sub_session.table_name, verified_count);
        }
        AnalysisPhase::Finalization
    }

    /// 阶段1: 发现阶段 - 探索表结构，找出候选外键
    fn build_discovery_prompt(&self, sub_session: &SubAnalysisSession) -> String {
        format!(
            r#"你是数据库外键分析专家。当前是**发现阶段**。

## 任务
分析表 "{}" 的字段，找出所有可能引用其他表的候选外键。

## 表结构
{}

## 数据库中的其他表（用于匹配）
{}

## 发现规则（按优先级）
1. **字段名匹配**: 字段名以 "_id" / "_code" / "_no" 结尾，或等于 "id"
2. **类型匹配**: 字段类型与目标表主键类型一致
3. **命名规律**: 如 user_id → users 表, order_no → orders 表

## 输出要求
列出你发现的所有候选关系，格式：
```json
{{
  "source_table": "{}",
  "source_column": "字段名",
  "target_table": "目标表名", 
  "target_column": "目标字段名",
  "confidence": 0.0-1.0,
  "reason": "为什么认为可能是外键"
}}
```

## 下一步
- 如果有候选关系：**立即调用 verify_foreign_key 验证最可能的一个**
- 如果没有候选：**直接回复 "ANALYSIS_COMPLETE"**
"#,
            sub_session.table_name,
            sub_session.table_schema,
            sub_session.other_tables_preview,
            sub_session.table_name
        )
    }

    /// 阶段2: 验证阶段 - 验证候选关系
    fn build_validation_prompt(&self, sub_session: &SubAnalysisSession) -> String {
        let verified_candidates: Vec<_> = sub_session.candidates.iter().filter(|c| c.verified).collect();
        let unverified_candidates: Vec<_> = sub_session.candidates.iter().filter(|c| !c.verified).collect();
        
        let verified_info = verified_candidates.iter()
            .map(|c| format!(
                "- {}.{} → {}.{} (置信度: {:.0}%) ✓",
                c.source_table, c.source_column, c.target_table, c.target_column, c.confidence * 100.0
            ))
            .collect::<Vec<_>>()
            .join("\n");
        
        let unverified_info = unverified_candidates.iter()
            .map(|c| format!(
                "- {}.{} → {}.{} (置信度: {:.0}%)",
                c.source_table, c.source_column, c.target_table, c.target_column, c.confidence * 100.0
            ))
            .collect::<Vec<_>>()
            .join("\n");
        
        // 找出表中还未探索的疑似外键字段
        let all_columns = sub_session.table_schema.lines()
            .filter(|l| l.starts_with("  ") && l.contains(":"))
            .map(|l| l.trim().split(':').next().unwrap_or("").trim().to_string())
            .filter(|n| !n.is_empty())
            .collect::<Vec<_>>();
        
        let explored_columns: std::collections::HashSet<_> = sub_session.candidates.iter()
            .map(|c| c.source_column.clone())
            .collect();
        
        let unexplored_candidates: Vec<_> = all_columns.iter()
            .filter(|col| {
                (col.ends_with("_id") || col.ends_with("_code") || col.ends_with("_no")) 
                && !explored_columns.contains(*col)
            })
            .map(|c| c.as_str())
            .collect();
        
        let unexplored_info = if unexplored_candidates.is_empty() {
            "(无)".to_string()
        } else {
            unexplored_candidates.join(", ")
        };
        
        format!(
            r#"你是数据库外键分析专家。当前是**验证阶段**。

## 表
{}

## 已验证的关系
{}

## 待验证的候选关系（请逐个验证）
{}

## 表中还有其他可能的候选字段（待探索）
{}

## 验证规则
使用 verify_foreign_key 工具逐个验证数据一致性：
- **source_table**: 当前表
- **source_column**: 疑似外键字段  
- **target_table**: 被引用的表
- **target_column**: 被引用的字段（通常是主键）
- **sample_size**: 建议 100-200

## 重要说明
- **一张表可以有多个外键指向不同表**
- **验证完一个候选后，必须继续验证下一个，不要结束！**
- **只有当所有待验证候选都验证完成后，才能回复 "ANALYSIS_COMPLETE"**

## 下一步（按优先级）
1. **如果还有待验证候选**: **立即调用 verify_foreign_key 验证下一个**
2. **如果还有其他疑似外键字段**: 先调用 verify_foreign_key 验证已发现的候选
3. **所有候选都验证完成后**: 回复 "ANALYSIS_COMPLETE" 结束

## 验证结果解读
- **overlap_rate >= 90%**: 强外键关系
- **overlap_rate 50-90%**: 中等关联，可能是部分匹配
- **overlap_rate < 50%**: 弱关联或无关
"#,
            sub_session.table_name,
            if verified_info.is_empty() { "(无)" } else { &verified_info },
            if unverified_info.is_empty() { "(无 - 继续探索其他字段)" } else { &unverified_info },
            unexplored_info
        )
    }

    /// 阶段3: 结束阶段 - 总结并结束
    fn build_finalization_prompt(&self, sub_session: &SubAnalysisSession) -> String {
        let verified_count = sub_session.candidates.iter().filter(|c| c.verified).count();
        let unverified_count = sub_session.candidates.iter().filter(|c| !c.verified).count();
        
        // 如果有未验证候选，提示要继续验证
        let task_instruction = if unverified_count > 0 {
            format!(
                r#"**警告：还有 {} 个候选关系未验证！**

由于接近最大轮次限制，请立即完成以下任务：
1. **立即调用 verify_foreign_key 验证所有未验证候选**（一张表多个外键很常见）
2. 快速验证完所有候选后，回复 "ANALYSIS_COMPLETE" 结束"#,
                unverified_count
            )
        } else {
            r#"所有候选关系都已验证完成。

请确认是否还有其他疑似外键字段未探索（以 "_id"、"_code"、"_no" 结尾的字段）。
如果没有，回复 "ANALYSIS_COMPLETE" 结束当前表的分析。"#.to_string()
        };
        
        format!(
            r#"你是数据库外键分析专家。当前是**结束阶段**。

## 表: {}

## 状态
- 已验证关系: {} 个
- 待验证候选: {} 个
- 已用轮次: {}/{}

## 任务
{}

## 重要提醒
- **一张表可以有多个外键指向不同表，很常见！**
- **请务必确保所有候选关系都已验证**
- **验证完成后回复 "ANALYSIS_COMPLETE"**
"#,
            sub_session.table_name,
            verified_count,
            unverified_count,
            sub_session.turns_used,
            sub_session.max_turns,
            task_instruction
        )
    }

    /// 构建子会话消息（只保留最近几轮对话以节省 token）
    fn build_sub_messages(&self, sub_session: &SubAnalysisSession, system_prompt: &str) -> Vec<LlmMessage> {
        let mut messages = vec![LlmMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];

        // 只保留最近 10 条消息，避免上下文膨胀
        let recent_messages = sub_session.messages.iter().rev().take(10).cloned().collect::<Vec<_>>();
        for msg in recent_messages.iter().rev() {
            messages.push(LlmMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
                tool_calls: msg.tool_calls.as_ref().map(|calls| {
                    calls.iter().map(|c| LlmRequestToolCall {
                        id: c.id.clone(),
                        call_type: "function".to_string(),
                        function: LlmFunction {
                            name: c.name.clone(),
                            arguments: c.arguments.clone(),
                        },
                    }).collect()
                }),
                tool_call_id: msg.tool_call_id.clone(),
                name: None,
            });
        }

        messages
    }

    async fn call_llm(&self, messages: &[LlmMessage], tools: &[serde_json::Value]) -> Result<LlmResponse> {
        let client = reqwest::Client::new();
        let request = LlmRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            temperature: 0.3,
            tools: if tools.is_empty() { None } else { Some(tools.to_vec()) },
        };

        let normalized_url = self.normalize_api_url();

        let response = client
            .post(&normalized_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("调用 LLM API 失败")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("LLM API 错误: {}", error_text));
        }

        let llm_response: LlmResponse = response.json().await
            .context("解析 LLM 响应失败")?;
        if let Some(error) = &llm_response.error {
            return Err(anyhow::anyhow!("LLM API 错误: {}", error.message));
        }

        Ok(llm_response)
    }

    fn normalize_api_url(&self) -> String {
        if self.api_url.contains("/chat/completions") {
            return self.api_url.clone();
        }
        if self.api_url.ends_with("/v1") {
            return format!("{}/chat/completions", self.api_url);
        }
        if self.api_url.ends_with("/v1/") {
            return format!("{}chat/completions", self.api_url);
        }
        let base = self.api_url.trim_end_matches('/');
        format!("{}/v1/chat/completions", base)
    }
}

/// 判断关系类型
fn determine_relation_type(candidate: &RelationCandidate) -> String {
    if candidate.target_column.contains("_id") || candidate.target_column == "id" {
        "one_to_many".to_string()
    } else {
        "one_to_one".to_string()
    }
}

/// 分析轮次结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisTurnResult {
    pub is_complete: bool,
    pub current_table: Option<String>,
    pub progress: f32,
    pub message: Option<String>,
    pub error: Option<String>,
}
