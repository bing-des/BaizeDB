use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::State;
use crate::state::AppState;
use crate::store::LlmConfig;
use crate::store::harness_analyzer::{
    HarnessAnalyzer, HarnessSession,
    AnalysisStage, RelationCandidate,
};
use crate::store::TableRelationAnalysis;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// 会话信息（返回给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub connection_id: String,
    pub database: String,
    pub schema: Option<String>,
    pub current_stage: AnalysisStage,
    pub tables_total: usize,
    pub tables_analyzed: usize,
    pub current_table: Option<String>,
    pub candidates_count: usize,
    pub progress: f32,
    pub is_complete: bool,
}

/// 分析步骤（显示分析过程）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisStep {
    pub step_type: String,           // "tool_call", "tool_result", "message"
    pub content: String,
    pub tool_name: Option<String>,
    pub table_name: Option<String>,
}

/// 开始分析请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartAnalysisRequest {
    pub connection_id: String,
    pub database: String,
    pub schema: Option<String>,
}

/// 轮次分析响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnAnalysisResponse {
    pub session_id: String,
    pub is_complete: bool,
    pub current_stage: AnalysisStage,
    pub current_table: Option<String>,
    pub progress: f32,
    pub new_step: Option<AnalysisStep>,
    pub candidates_count: usize,
    pub relations: Vec<TableRelationAnalysis>,
    pub message: Option<String>,
    pub error: Option<String>,
}

/// 候选关系列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidatesResponse {
    pub candidates: Vec<RelationCandidateInfo>,
    pub summary: CandidatesSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationCandidateInfo {
    pub source_table: String,
    pub source_column: String,
    pub target_table: String,
    pub target_column: String,
    pub confidence: f32,
    pub reason: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidatesSummary {
    pub total: usize,
    pub avg_confidence: f32,
}

// ─────────────────────────────────────────────────────────────────────────────
// 会话存储管理
// ─────────────────────────────────────────────────────────────────────────────

/// 全局分析会话存储
#[derive(Clone)]
pub struct HarnessSessionStore {
    sessions: Arc<RwLock<std::collections::HashMap<String, HarnessSession>>>,
    analyzer_cache: Arc<RwLock<std::collections::HashMap<String, crate::store::harness_analyzer::HarnessAnalyzer>>>,
}

impl HarnessSessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            analyzer_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// 获取会话
    pub async fn get_session(&self, session_id: &str) -> Option<HarnessSession> {
        let sessions = self.sessions.read().await;
        sessions.values().find(|s| s.id == session_id).cloned()
    }

    /// 获取或创建会话
    pub async fn get_or_create_session(
        &self,
        request: &StartAnalysisRequest,
        config: &LlmConfig,
    ) -> Result<String, String> {
        let key = format!(
            "{}:{}{}",
            request.connection_id,
            request.database,
            request.schema.as_ref().map(|s| format!(":{}", s)).unwrap_or_default()
        );

        // 检查是否已存在会话
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(&key) {
                if !matches!(session.current_stage, AnalysisStage::Completed) {
                    return Ok(session.id.clone());
                }
            }
        }

        // 创建新会话
        let analyzer = HarnessAnalyzer::new(
            config.api_key.clone(),
            config.api_url.clone(),
            config.model.clone(),
        );

        let session = HarnessAnalyzer::create_session(
            request.connection_id.clone(),
            request.database.clone(),
            request.schema.clone(),
        );

        let session_id = session.id.clone();

        // 存储会话和分析器
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(key, session);
        }
        {
            let mut analyzers = self.analyzer_cache.write().await;
            analyzers.insert(session_id.clone(), analyzer);
        }

        Ok(session_id)
    }

    /// 获取分析器
    pub async fn get_analyzer(&self, session_id: &str) -> Option<HarnessAnalyzer> {
        let analyzers = self.analyzer_cache.read().await;
        analyzers.get(session_id).cloned()
    }

    /// 获取会话信息
    pub async fn get_session_info(&self, session_id: &str) -> Option<SessionInfo> {
        let sessions = self.sessions.read().await;
        let session = sessions.values().find(|s| s.id == session_id)?;

        let candidates_count = session.results.iter()
            .map(|r| r.outgoing_relations.len())
            .sum();

        Some(SessionInfo {
            id: session.id.clone(),
            connection_id: session.connection_id.clone(),
            database: session.database.clone(),
            schema: session.schema.clone(),
            current_stage: session.current_stage.clone(),
            tables_total: session.tables.len(),
            tables_analyzed: session.results.len(),
            current_table: session.current_table().cloned(),
            candidates_count,
            progress: session.progress(),
            is_complete: matches!(session.current_stage, AnalysisStage::Completed),
        })
    }

    /// 更新会话
    pub async fn update_session(&self, session: &HarnessSession) {
        let key = format!(
            "{}:{}{}",
            session.connection_id,
            session.database,
            session.schema.as_ref().map(|s| format!(":{}", s)).unwrap_or_default()
        );
        let mut sessions = self.sessions.write().await;
        sessions.insert(key, session.clone());
    }

    /// 删除会话
    pub async fn remove_session(&self, session_id: &str) {
        // 先查找 key
        let key = {
            let sessions = self.sessions.read().await;
            sessions.iter()
                .find(|(_, s)| s.id == session_id)
                .map(|(k, _)| k.clone())
        };

        if let Some(key) = key {
            let mut sessions = self.sessions.write().await;
            sessions.remove(&key);
        }

        let mut analyzers = self.analyzer_cache.write().await;
        analyzers.remove(session_id);
    }

    /// 获取所有会话
    pub async fn list_sessions(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.values()
            .filter(|s| !matches!(s.current_stage, AnalysisStage::Completed))
            .map(|s| {
                let candidates_count = s.results.iter()
                    .map(|r| r.outgoing_relations.len())
                    .sum();
                SessionInfo {
                    id: s.id.clone(),
                    connection_id: s.connection_id.clone(),
                    database: s.database.clone(),
                    schema: s.schema.clone(),
                    current_stage: s.current_stage.clone(),
                    tables_total: s.tables.len(),
                    tables_analyzed: s.results.len(),
                    current_table: s.current_table().cloned(),
                    candidates_count,
                    progress: s.progress(),
                    is_complete: matches!(s.current_stage, AnalysisStage::Completed),
                }
            })
            .collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tauri Commands
// ─────────────────────────────────────────────────────────────────────────────

/// 开始新的分析会话
#[tauri::command]
pub async fn harness_start_analysis(
    state: State<'_, AppState>,
    request: StartAnalysisRequest,
) -> Result<SessionInfo, String> {
    // 获取 LLM 配置
    let config = state.store.get_llm_config().await?;

    if !config.enabled || config.api_key.is_empty() {
        return Err("LLM 未配置或未启用，请先在设置中配置 LLM".to_string());
    }

    // 创建或获取会话
    let session_id = state.harness_store.get_or_create_session(&request, &config).await?;

    // 获取会话信息
    state.harness_store.get_session_info(&session_id)
        .await
        .ok_or_else(|| "创建会话失败".to_string())
}

/// 执行一轮分析
#[tauri::command]
pub async fn harness_run_turn(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<TurnAnalysisResponse, String> {
    // 获取分析器
    let analyzer = state.harness_store.get_analyzer(&session_id)
        .await
        .ok_or_else(|| "会话不存在".to_string())?;

    // 获取会话
    let mut session = {
        let sessions = state.harness_store.get_session(&session_id).await;
        sessions.ok_or_else(|| "会话不存在".to_string())?
    };

    // 执行一轮分析
    let result = analyzer.run_analysis_turn(&mut session, &state).await
        .map_err(|e| format!("分析失败: {}", e))?;

    // 构建步骤信息
    let new_step = result.message.as_ref().map(|msg| {
        let step_type = if msg.contains("开始分析") || msg.contains("分析完成") {
            "stage".to_string()
        } else if msg.contains("工具:") {
            "tool_result".to_string()
        } else {
            "message".to_string()
        };

        AnalysisStep {
            step_type,
            content: msg.clone(),
            tool_name: if msg.contains("工具:") {
                msg.split("工具:").nth(1).map(|s| s.split('-').next().unwrap_or(s).trim().to_string())
            } else {
                None
            },
            table_name: result.current_table.clone(),
        }
    });

    // 保存所有未保存的表关系
    let mut saved_count = 0;
    let mut saved_tables = Vec::new();
    
    // 注意：需要先收集要保存的数据，然后再修改 session
    let results_to_save: Vec<(usize, String, Vec<TableRelationAnalysis>)> = session.results
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.saved_to_sqlite)
        .map(|(idx, r)| {
            let relations: Vec<TableRelationAnalysis> = r.outgoing_relations.iter()
                .map(|c| TableRelationAnalysis {
                    source_table: c.source_table.clone(),
                    source_column: c.source_column.clone(),
                    target_table: c.target_table.clone(),
                    target_column: c.target_column.clone(),
                    relation_type: if c.confidence >= 0.7 { "strong".to_string() } else { "weak".to_string() },
                    confidence: c.confidence,
                    reason: c.reason.clone(),
                })
                .collect();
            (idx, r.table_name.clone(), relations)
        })
        .collect();
    
    for (idx, table_name, relations) in results_to_save {
        if !relations.is_empty() {
            if let Err(e) = state.store.save_relations(
                &session.connection_id,
                &session.database,
                &relations
            ).await {
                log::error!("[Harness] 保存表 {} 关系到 SQLite 失败: {}", table_name, e);
            } else {
                log::info!("[Harness] 表 {} 已保存 {} 个关系到 SQLite", 
                    table_name, relations.len());
                saved_count += 1;
                saved_tables.push(table_name);
            }
        }
        // 标记为已保存
        if let Some(r) = session.results.get_mut(idx) {
            r.saved_to_sqlite = true;
        }
    }
    
    if saved_count > 0 {
        log::info!("[Harness] 本次共保存 {} 张表的关系到 SQLite", saved_count);
    }
    
    // 如果完成，返回所有关系
    let relations: Vec<TableRelationAnalysis> = if result.is_complete {
        session.results.iter()
            .flat_map(|r| r.outgoing_relations.iter().map(|c| TableRelationAnalysis {
                source_table: c.source_table.clone(),
                source_column: c.source_column.clone(),
                target_table: c.target_table.clone(),
                target_column: c.target_column.clone(),
                relation_type: if c.confidence >= 0.7 { "strong".to_string() } else { "weak".to_string() },
                confidence: c.confidence,
                reason: c.reason.clone(),
            }))
            .collect()
    } else {
        vec![]
    };

    // 统计候选数
    let candidates_count = session.results.iter()
        .map(|r| r.outgoing_relations.len())
        .sum();

    // 更新会话
    state.harness_store.update_session(&session).await;

    Ok(TurnAnalysisResponse {
        session_id: session_id.clone(),
        is_complete: result.is_complete,
        current_stage: session.current_stage.clone(),
        current_table: result.current_table,
        progress: result.progress,
        new_step,
        candidates_count,
        relations,
        message: result.message,
        error: result.error,
    })
}

/// 获取会话信息
#[tauri::command]
pub async fn harness_get_session_info(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<SessionInfo, String> {
    state.harness_store.get_session_info(&session_id)
        .await
        .ok_or_else(|| "会话不存在".to_string())
}

/// 获取候选关系列表
#[tauri::command]
pub async fn harness_get_candidates(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<CandidatesResponse, String> {
    let session = state.harness_store.get_session(&session_id)
        .await
        .ok_or_else(|| "会话不存在".to_string())?;

    let mut all_candidates: Vec<RelationCandidateInfo> = Vec::new();

    for result in &session.results {
        for c in &result.outgoing_relations {
            all_candidates.push(RelationCandidateInfo {
                source_table: c.source_table.clone(),
                source_column: c.source_column.clone(),
                target_table: c.target_table.clone(),
                target_column: c.target_column.clone(),
                confidence: c.confidence,
                reason: c.reason.clone(),
                verified: c.verified,
            });
        }
    }

    let total = all_candidates.len();
    let avg_confidence = if !all_candidates.is_empty() {
        all_candidates.iter().map(|c| c.confidence).sum::<f32>() / all_candidates.len() as f32
    } else {
        0.0
    };

    Ok(CandidatesResponse {
        candidates: all_candidates,
        summary: CandidatesSummary {
            total,
            avg_confidence,
        },
    })
}

/// 获取分析步骤历史（简化为表分析记录）
#[tauri::command]
pub async fn harness_get_steps(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<AnalysisStep>, String> {
    let session = state.harness_store.get_session(&session_id)
        .await
        .ok_or_else(|| "会话不存在".to_string())?;

    let mut steps = Vec::new();

    // 添加每个表的分析结果
    for result in &session.results {
        steps.push(AnalysisStep {
            step_type: "table_complete".to_string(),
            content: format!(
                "表 [{}] 分析完成，发现 {} 个候选关系",
                result.table_name,
                result.outgoing_relations.len()
            ),
            tool_name: None,
            table_name: Some(result.table_name.clone()),
        });
    }

    Ok(steps)
}

/// 删除分析会话
#[tauri::command]
pub async fn harness_delete_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    state.harness_store.remove_session(&session_id).await;
    Ok(())
}

/// 获取所有活跃会话
#[tauri::command]
pub async fn harness_list_sessions(
    state: State<'_, AppState>,
) -> Result<Vec<SessionInfo>, String> {
    Ok(state.harness_store.list_sessions().await)
}
