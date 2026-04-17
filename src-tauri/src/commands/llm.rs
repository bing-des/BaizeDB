use serde::{Deserialize, Serialize};
use tauri::State;
use crate::state::AppState;
use crate::store::{LlmAnalyzer, TableRelationAnalysis, LlmConfig};
use crate::store::connection_store::{ColumnMeta, TableSchema};

/// 规范化 API URL：如果 URL 以 /v1 结尾但未包含 chat/completions，则自动补全
fn normalize_api_url(api_url: &str) -> String {
    // 如果 URL 已经是完整的聊天完成端点，直接返回
    if api_url.contains("/chat/completions") {
        return api_url.to_string();
    }
    
    // 如果 URL 以 /v1 结尾，补全 /chat/completions
    if api_url.ends_with("/v1") {
        return format!("{}/chat/completions", api_url);
    }
    
    // 如果 URL 以 /v1/ 结尾，补全 chat/completions
    if api_url.ends_with("/v1/") {
        return format!("{}chat/completions", api_url);
    }
    
    // 其他情况，假设是基础 URL，尝试添加 /v1/chat/completions
    let base = api_url.trim_end_matches('/');
    format!("{}/v1/chat/completions", base)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeRelationsRequest {
    pub connection_id: String,
    pub database: String,
    pub schema: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeRelationsResponse {
    pub relations: Vec<TableRelationAnalysis>,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfigRequest {
    pub api_key: String,
    pub api_url: String,
    pub model: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfigResponse {
    pub config: LlmConfig,
}

/// 获取表关系分析结果（优先从 SQLite 读取，不存在则调用 LLM）
#[tauri::command]
pub async fn get_table_relations(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    schema: Option<String>,
) -> Result<AnalyzeRelationsResponse, String> {
    let store = &state.store;
    
    // 先检查 SQLite 中是否有缓存
    let has_cache = store.has_relations(&connection_id, &database).await?;
    
    if has_cache {
        let relations = store.get_relations(&connection_id, &database).await?;
        
        return Ok(AnalyzeRelationsResponse {
            relations,
            from_cache: true,
        });
    }
    
    // 没有缓存，调用 LLM 分析
    let req = AnalyzeRelationsRequest {
        connection_id,
        database,
        schema,
    };
    analyze_with_llm(state, req).await
}

/// 强制刷新 - 重新调用 LLM 分析
#[tauri::command]
pub async fn refresh_table_relations(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    schema: Option<String>,
) -> Result<AnalyzeRelationsResponse, String> {
    let store = &state.store;
    
    // 删除旧数据
    store.delete_relations(&connection_id, &database).await?;
    
    // 重新分析
    let req = AnalyzeRelationsRequest {
        connection_id,
        database,
        schema,
    };
    analyze_with_llm(state, req).await
}

/// 使用 LLM 分析表关系
async fn analyze_with_llm(
    state: State<'_, AppState>,
    req: AnalyzeRelationsRequest,
) -> Result<AnalyzeRelationsResponse, String> {
    let store = &state.store;
    
    // 获取所有表结构
    let tables = fetch_table_schemas(&state, &req.connection_id, &req.database, req.schema.as_deref()).await?;
    
    if tables.is_empty() {
        return Ok(AnalyzeRelationsResponse {
            relations: vec![],
            from_cache: false,
        });
    }
    
    // 从 SQLite 读取 LLM 配置
    let config = store.get_llm_config().await?;
    
    if !config.enabled || config.api_key.is_empty() {
        return Err("LLM 未配置或未启用，请先在设置中配置 LLM".to_string());
    }
    
    let analyzer = LlmAnalyzer::new(config.api_key, config.api_url, config.model);
    let relations = analyzer.analyze_relations(&tables).await
        .map_err(|e| format!("LLM 分析失败: {}", e))?;
    
    // 保存到 SQLite
    store.save_relations(&req.connection_id, &req.database, &relations).await?;
    
    Ok(AnalyzeRelationsResponse {
        relations,
        from_cache: false,
    })
}

/// 获取所有表结构
async fn fetch_table_schemas(
    state: &AppState,
    connection_id: &str,
    database: &str,
    schema: Option<&str>,
) -> Result<Vec<TableSchema>, String> {
    // 获取 DbPool - 先从 pools 获取主连接池
    let db_pool = {
        let pools = state.pools.read().await;
        match pools.get(connection_id) {
            Some(p) => p.clone(),
            None => return Err("数据库未连接".to_string()),
        }
    };
    
    // 转换为 AnyDbPool（PG 会按需创建 db_pools 中的连接）
    let pool = db_pool.as_db_ops(state, connection_id, database).await?;
    
    let table_metas = pool.list_tables(database, schema).await?;
    
    let mut schemas = Vec::new();
    for table_meta in table_metas {
        let column_metas = pool.list_columns(database, &table_meta.name).await?;
        
        let columns: Vec<ColumnMeta> = column_metas.into_iter().map(|c| ColumnMeta {
            name: c.name,
            data_type: c.data_type,
            nullable: c.nullable,
            key: c.key,
        }).collect();
        
        schemas.push(TableSchema {
            name: table_meta.name,
            columns,
        });
    }
    
    Ok(schemas)
}

/// 检查是否有缓存的分析结果
#[tauri::command]
pub async fn has_relation_analysis(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<bool, String> {
    state.store.has_relations(&connection_id, &database).await
}

/// 删除分析结果
#[tauri::command]
pub async fn clear_relation_analysis(
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<(), String> {
    state.store.delete_relations(&connection_id, &database).await
}

/// 获取 LLM 配置
#[tauri::command]
pub async fn get_llm_config(
    state: State<'_, AppState>,
) -> Result<LlmConfigResponse, String> {
    let config = state.store.get_llm_config().await?;
    Ok(LlmConfigResponse { config })
}

/// 保存 LLM 配置
#[tauri::command]
pub async fn save_llm_config(
    state: State<'_, AppState>,
    req: LlmConfigRequest,
) -> Result<(), String> {
    let config = LlmConfig {
        api_key: req.api_key,
        api_url: req.api_url,
        model: req.model,
        enabled: req.enabled,
    };
    
    state.store.save_llm_config(&config).await
}

/// 测试 LLM 配置
#[tauri::command]
pub async fn test_llm_config(
    _state: State<'_, AppState>,
    api_key: String,
    api_url: String,
    model: String,
) -> Result<String, String> {
    use reqwest::Client;
    
    let client = Client::new();
    
    // 规范化 API URL
    let normalized_url = normalize_api_url(&api_url);
    
    // 构建一个简单的测试请求（OpenAI 格式）
    let request = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "Hello, please respond with 'OK'."}
        ],
        "max_tokens": 10,
        "temperature": 0.0
    });
    
    let response = client
        .post(&normalized_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {}", e))?;
    
    let status = response.status();
    let response_text = response.text().await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    
    if !status.is_success() {
        // 尝试解析错误信息
        let error_msg = if let Ok(err_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if let Some(error_obj) = err_json.get("error") {
                if let Some(message) = error_obj.get("message") {
                    format!("API 错误: {}", message)
                } else {
                    format!("API 错误: {}", error_obj)
                }
            } else if let Some(message) = err_json.get("message") {
                format!("API 错误: {}", message)
            } else {
                format!("HTTP {}: {}", status, response_text)
            }
        } else {
            format!("HTTP {}: {}", status, response_text)
        };
        return Err(format!("连接测试失败: {}", error_msg));
    }
    
    // 尝试解析响应
    let response_json: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("解析响应 JSON 失败: {}", e))?;
    
    // 检查是否有 choices
    if let Some(choices) = response_json.get("choices") {
        if choices.as_array().map(|a| a.is_empty()).unwrap_or(true) {
            return Err("响应中没有 choices 数组".to_string());
        }
    } else {
        return Err("响应中没有 choices 字段".to_string());
    }
    
    Ok("连接测试成功".to_string())
}
