use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use crate::store::TableRelationAnalysis;
use crate::store::connection_store::TableSchema;

// LLM 分析请求
#[derive(Debug, Clone, Serialize)]
struct LlmRequest {
    model: String,
    messages: Vec<LlmMessage>,
    temperature: f32,
}

#[derive(Debug, Clone, Serialize)]
struct LlmMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmResponse {
    choices: Vec<LlmChoice>,
    // DeepSeek 可能返回 error 字段
    error: Option<LlmError>,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmError {
    message: String,
    #[serde(rename = "type")]
    error_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmChoice {
    message: LlmResponseMessage,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmResponseMessage {
    content: String,
}

/// LLM 分析器
pub struct LlmAnalyzer {
    api_key: String,
    api_url: String,
    model: String,
}

impl LlmAnalyzer {
    pub fn new(api_key: String, api_url: String, model: String) -> Self {
        Self {
            api_key,
            api_url,
            model,
        }
    }
    
    /// 规范化 API URL：如果 URL 以 /v1 结尾但未包含 chat/completions，则自动补全
    fn normalize_api_url(&self) -> String {
        let api_url = &self.api_url;
        
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

    /// 分析表关系
    pub async fn analyze_relations(&self, tables: &[TableSchema]) -> Result<Vec<TableRelationAnalysis>> {
        let prompt = self.build_prompt(tables);
        
        let client = reqwest::Client::new();
        let request = LlmRequest {
            model: self.model.clone(),
            messages: vec![
                LlmMessage {
                    role: "system".to_string(),
                    content: "You are a database expert. Analyze table structures and identify potential relationships between tables.".to_string(),
                },
                LlmMessage {
                    role: "user".to_string(),
                    content: prompt,
                },
            ],
            temperature: 0.3,
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
        
        // 检查 API 返回的错误
        if let Some(error) = llm_response.error {
            return Err(anyhow::anyhow!("LLM API 错误: {}", error.message));
        }
        
        let content = llm_response.choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();
        
        self.parse_response(&content)
    }
    
    fn build_prompt(&self, tables: &[TableSchema]) -> String {
        let mut prompt = String::from("Analyze the following database tables and identify potential relationships between them.\n\n");
        prompt.push_str("Tables:\n\n");
        
        for table in tables {
            prompt.push_str(&format!("Table: {}\n", table.name));
            prompt.push_str("Columns:\n");
            for col in &table.columns {
                let key_info = match &col.key {
                    Some(k) if k == "PRI" => " (PRIMARY KEY)",
                    _ => ""
                };
                prompt.push_str(&format!("  - {}: {}{}\n", col.name, col.data_type, key_info));
            }
            prompt.push_str("\n");
        }
        
        prompt.push_str("\nBased on column names and types, identify relationships between tables. ");
        prompt.push_str("Look for:\n");
        prompt.push_str("1. Columns ending with '_id' that might reference other tables\n");
        prompt.push_str("2. Columns with the same name and type across tables\n");
        prompt.push_str("3. Primary key to foreign key relationships\n\n");
        
        prompt.push_str("Return ONLY a JSON array in this format:\n");
        prompt.push_str(r#"[
  {
    "source_table": "table_name",
    "source_column": "column_name",
    "target_table": "referenced_table",
    "target_column": "referenced_column",
    "relation_type": "one_to_many",
    "confidence": 0.95,
    "reason": "column_name follows naming convention for foreign keys"
  }
]"#);
        
        prompt
    }
    
    fn parse_response(&self, content: &str) -> Result<Vec<TableRelationAnalysis>> {
        log::info!("LLM 原始响应内容: {}", content);
        // 提取 JSON 部分
        let json_str = if content.contains("```json") {
            content.split("```json").nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(content)
        } else if content.contains("```") {
            content.split("```").nth(1)
                .unwrap_or(content)
        } else {
            content
        };
        
        let json_str = json_str.trim();
        
        let relations: Vec<TableRelationAnalysis> = serde_json::from_str(json_str)
            .context("解析 LLM 返回的 JSON 失败")?;
        
        Ok(relations)
    }
}
