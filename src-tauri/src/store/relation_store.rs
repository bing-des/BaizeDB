//! 表关系存储器
//!
//! 独立于连接配置存储，专门管理分析结果的持久化。
//! 复用 SqliteConnectionStore 的连接池。

use sqlx::{SqlitePool, Row};
use crate::store::harness_types::{TableRelationAnalysis, LlmConfig};

/// 表关系存储器
pub struct RelationStore;

impl RelationStore {
    /// 保存表关系分析结果
    pub async fn save_relations(
        pool: &SqlitePool,
        connection_id: &str,
        database: &str,
        relations: &[TableRelationAnalysis],
    ) -> Result<(), String> {
        let mut tx = pool.begin().await.map_err(|e| format!("开始事务失败: {}", e))?;

        for relation in relations {
            // 先删除已存在的相同关系
            sqlx::query(
                r#"
                DELETE FROM table_relations 
                WHERE connection_id = ?1 
                  AND database = ?2 
                  AND source_table = ?3 
                  AND source_column = ?4
                "#
            )
            .bind(connection_id)
            .bind(database)
            .bind(&relation.source_table)
            .bind(&relation.source_column)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("删除旧关系失败: {}", e))?;

            // 插入新关系
            sqlx::query(
                r#"
                INSERT INTO table_relations 
                (connection_id, database, source_table, source_column, target_table, target_column, relation_type, confidence, reason, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, CURRENT_TIMESTAMP)
                "#
            )
            .bind(connection_id)
            .bind(database)
            .bind(&relation.source_table)
            .bind(&relation.source_column)
            .bind(&relation.target_table)
            .bind(&relation.target_column)
            .bind(&relation.relation_type)
            .bind(relation.confidence)
            .bind(&relation.reason)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("保存关系失败: {}", e))?;
        }

        tx.commit().await.map_err(|e| format!("提交事务失败: {}", e))?;
        Ok(())
    }

    /// 读取表关系分析结果
    pub async fn get_relations(
        pool: &SqlitePool,
        connection_id: &str,
        database: &str,
    ) -> Result<Vec<TableRelationAnalysis>, String> {
        let rows = sqlx::query(
            r#"
            SELECT source_table, source_column, target_table, target_column, 
                   relation_type, confidence, reason
            FROM table_relations
            WHERE connection_id = ?1 AND database = ?2
            ORDER BY confidence DESC
            "#
        )
        .bind(connection_id)
        .bind(database)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("查询关系失败: {}", e))?;

        let mut relations = Vec::new();
        for row in rows {
            relations.push(TableRelationAnalysis {
                source_table: row.try_get("source_table").map_err(|e| e.to_string())?,
                source_column: row.try_get("source_column").map_err(|e| e.to_string())?,
                target_table: row.try_get("target_table").map_err(|e| e.to_string())?,
                target_column: row.try_get("target_column").map_err(|e| e.to_string())?,
                relation_type: row.try_get("relation_type").map_err(|e| e.to_string())?,
                confidence: row.try_get("confidence").map_err(|e| e.to_string())?,
                reason: row.try_get("reason").map_err(|e| e.to_string())?,
            });
        }

        Ok(relations)
    }

    /// 检查是否存在分析结果
    pub async fn has_relations(
        pool: &SqlitePool,
        connection_id: &str,
        database: &str,
    ) -> Result<bool, String> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM table_relations
            WHERE connection_id = ?1 AND database = ?2
            "#
        )
        .bind(connection_id)
        .bind(database)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("查询失败: {}", e))?;

        let count: i64 = row.try_get("count").map_err(|e| e.to_string())?;
        Ok(count > 0)
    }

    /// 删除分析结果
    pub async fn delete_relations(
        pool: &SqlitePool,
        connection_id: &str,
        database: &str,
    ) -> Result<(), String> {
        sqlx::query(
            r#"
            DELETE FROM table_relations
            WHERE connection_id = ?1 AND database = ?2
            "#
        )
        .bind(connection_id)
        .bind(database)
        .execute(pool)
        .await
        .map_err(|e| format!("删除关系失败: {}", e))?;

        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // LLM 配置存储
    // ─────────────────────────────────────────────────────────────────────────

    /// 获取 LLM 配置
    pub async fn get_llm_config(pool: &SqlitePool) -> Result<LlmConfig, String> {
        let row = sqlx::query(
            r#"
            SELECT api_key, api_url, model, enabled
            FROM llm_config
            WHERE id = 1
            "#
        )
        .fetch_one(pool)
        .await
        .map_err(|e| format!("查询配置失败: {}", e))?;

        Ok(LlmConfig {
            api_key: row.try_get("api_key").map_err(|e| e.to_string())?,
            api_url: row.try_get("api_url").map_err(|e| e.to_string())?,
            model: row.try_get("model").map_err(|e| e.to_string())?,
            enabled: row.try_get::<i64, _>("enabled").map(|v| v != 0).unwrap_or(false),
        })
    }

    /// 保存 LLM 配置
    pub async fn save_llm_config(pool: &SqlitePool, config: &LlmConfig) -> Result<(), String> {
        sqlx::query(
            r#"
            INSERT INTO llm_config (id, api_key, api_url, model, enabled, updated_at)
            VALUES (1, ?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
            ON CONFLICT(id) 
            DO UPDATE SET
                api_key = excluded.api_key,
                api_url = excluded.api_url,
                model = excluded.model,
                enabled = excluded.enabled,
                updated_at = CURRENT_TIMESTAMP
            "#
        )
        .bind(&config.api_key)
        .bind(&config.api_url)
        .bind(&config.model)
        .bind(if config.enabled { 1 } else { 0 })
        .execute(pool)
        .await
        .map_err(|e| format!("保存配置失败: {}", e))?;

        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 通用设置存储
    // ─────────────────────────────────────────────────────────────────────────

    /// 获取通用设置
    pub async fn get_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>, String> {
        let row = sqlx::query(
            r#"
            SELECT value FROM settings WHERE key = ?1
            "#
        )
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("查询设置失败: {}", e))?;

        match row {
            Some(r) => Ok(Some(r.try_get("value").map_err(|e| e.to_string())?)),
            None => Ok(None),
        }
    }

    /// 保存通用设置
    pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<(), String> {
        sqlx::query(
            r#"
            INSERT INTO settings (key, value, updated_at)
            VALUES (?1, ?2, CURRENT_TIMESTAMP)
            ON CONFLICT(key) 
            DO UPDATE SET
                value = excluded.value,
                updated_at = CURRENT_TIMESTAMP
            "#
        )
        .bind(key)
        .bind(value)
        .execute(pool)
        .await
        .map_err(|e| format!("保存设置失败: {}", e))?;

        Ok(())
    }
}
