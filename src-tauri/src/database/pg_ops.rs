use sqlx::{PgPool, Row, Column, ValueRef, TypeInfo};
use crate::database::db_ops::{
    ColumnMeta, DatabaseMeta, DbOps, QueryResult, SchemaMeta, TableMeta,
};

impl DbOps for PgPool {
    async fn list_databases(&self) -> Result<Vec<DatabaseMeta>, String> {
        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname",
        )
        .fetch_all(self)
        .await
        .map_err(|e| e.to_string())?;

        Ok(rows.into_iter().map(|r| DatabaseMeta { name: r.0 }).collect())
    }

    async fn list_schemas(&self, _database: &str) -> Result<Vec<SchemaMeta>, String> {
        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT nspname FROM pg_namespace \
             WHERE nspname NOT IN ('pg_catalog', 'information_schema') \
             ORDER BY nspname",
        )
        .fetch_all(self)
        .await
        .map_err(|e| e.to_string())?;

        Ok(rows
            .into_iter()
            .map(|r| SchemaMeta { name: r.0 })
            .collect())
    }

    async fn list_tables(&self, _database: &str, schema: Option<&str>) -> Result<Vec<TableMeta>, String> {
        let schema_name = schema.unwrap_or("public");

        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT tablename, 'BASE TABLE' \
             FROM pg_catalog.pg_tables \
             WHERE schemaname = $1 ORDER BY tablename",
        )
        .bind(schema_name)
        .fetch_all(self)
        .await
        .map_err(|e| e.to_string())?;

        Ok(rows
            .into_iter()
            .map(|r| TableMeta {
                name: r.0,
                table_type: Some(r.1),
                row_count: None,
            })
            .collect())
    }

    async fn list_columns(&self, database: &str, table: &str) -> Result<Vec<ColumnMeta>, String> {
        let sql =
            "SELECT c.column_name, c.data_type, c.is_nullable, \
                    CASE WHEN kcu.column_name IS NOT NULL THEN 'PRI' ELSE '' END, \
                    COALESCE(c.column_default, ''), '', '' \
             FROM information_schema.columns c \
             LEFT JOIN information_schema.key_column_usage kcu \
               ON c.table_name = kcu.table_name AND c.column_name = kcu.column_name \
               AND kcu.constraint_name IN ( \
                 SELECT constraint_name FROM information_schema.table_constraints \
                 WHERE constraint_type = 'PRIMARY KEY' AND table_name = $1) \
             WHERE c.table_catalog = $2 AND c.table_name = $3 ORDER BY c.ordinal_position";

        let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(sql)
            .bind(table)
            .bind(database)
            .bind(table)
            .fetch_all(self)
            .await
            .map_err(|e| e.to_string())?;

        Ok(rows
            .into_iter()
            .map(|r| ColumnMeta {
                name: r.0,
                data_type: r.1,
                nullable: r.2 == "YES",
                key: if r.3.is_empty() { None } else { Some(r.3) },
                default_value: if r.4.is_empty() { None } else { Some(r.4) },
                extra: None,
                comment: None,
            })
            .collect())
    }

    async fn get_table_data(
        &self,
        _database: &str,
        table: &str,
        page: i64,
        page_size: i64,
    ) -> Result<QueryResult, String> {
        let start = std::time::Instant::now();
        let offset = (page - 1) * page_size;
        let safe_table = format!("\"{}\"", table);

        let count: i64 = sqlx::query(&format!("SELECT COUNT(*) FROM {}", safe_table))
            .fetch_one(self)
            .await
            .map_err(|e| e.to_string())?
            .get(0);

        let rows = sqlx::query(&format!(
            "SELECT * FROM {} LIMIT {} OFFSET {}",
            safe_table, page_size, offset
        ))
        .fetch_all(self)
        .await
        .map_err(|e| e.to_string())?;

        if rows.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                affected_rows: None,
                execution_time_ms: start.elapsed().as_millis() as u64,
                error: None,
                total: Some(count),
            });
        }

        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        let data = pg_rows_to_json(&rows);

        Ok(QueryResult {
            columns,
            rows: data,
            affected_rows: None,
            execution_time_ms: start.elapsed().as_millis() as u64,
            error: None,
            total: Some(count),
        })
    }

    async fn get_row_count(&self, _database: &str, table: &str) -> Result<i64, String> {
        let row = sqlx::query(&format!("SELECT COUNT(*) FROM \"{}\"", table))
            .fetch_one(self)
            .await
            .map_err(|e| e.to_string())?;
        Ok(row.get(0))
    }

    async fn query_sql(&self, sql: &str) -> Result<QueryResult, String> {
        let start = std::time::Instant::now();
        let rows = sqlx::query(sql)
            .fetch_all(self)
            .await
            .map_err(|e| e.to_string())?;

        if rows.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                affected_rows: None,
                execution_time_ms: start.elapsed().as_millis() as u64,
                error: None,
                total: None,
            });
        }

        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        let data = pg_rows_to_json(&rows);

        Ok(QueryResult {
            columns,
            rows: data,
            affected_rows: None,
            execution_time_ms: start.elapsed().as_millis() as u64,
            error: None,
            total: None,
        })
    }

    async fn execute_sql(&self, sql: &str) -> Result<u64, String> {
        let result = sqlx::query(sql)
            .execute(self)
            .await
            .map_err(|e| e.to_string())?;
        Ok(result.rows_affected())
    }
}

fn pg_rows_to_json(rows: &[sqlx::postgres::PgRow]) -> Vec<Vec<serde_json::Value>> {
    rows.iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|i| {
                    let val = row.try_get_raw(i).unwrap();
                    if val.is_null() {
                        return serde_json::Value::Null;
                    }
                    let tname = val.type_info().name().to_uppercase();

                    // ── 整数类型（PG 二进制协议精确匹配）─────────────
                    if tname == "INT2" || tname == "SMALLSERIAL" {
                        row.try_get::<i16, _>(i)
                            .map(|v| serde_json::json!(v as i64))
                            .unwrap_or(serde_json::Value::Null)
                    } else if tname == "INT4" || tname == "SERIAL" || tname == "INTEGER" {
                        row.try_get::<i32, _>(i)
                            .map(|v| serde_json::json!(v as i64))
                            .unwrap_or(serde_json::Value::Null)
                    } else if tname == "INT8" || tname == "BIGSERIAL" || tname == "BIGINT" {
                        row.try_get::<i64, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(serde_json::Value::Null)

                    // ── 浮点类型（FLOAT4 用 f32，FLOAT8/NUMERIC 用 f64）──
                    } else if tname == "FLOAT4" || tname == "REAL" {
                        row.try_get::<f32, _>(i)
                            .map(|v| serde_json::json!(v as f64))
                            .unwrap_or(serde_json::Value::Null)
                    } else if tname == "FLOAT8" || tname == "DOUBLE PRECISION" {
                        row.try_get::<f64, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(serde_json::Value::Null)
                    } else if tname == "NUMERIC" || tname == "DECIMAL" {
                        // NUMERIC 精度可能超过 f64，用 String 读取再尝试解析
                        row.try_get::<String, _>(i)
                            .ok()
                            .and_then(|s| s.parse::<f64>().ok().map(|n| serde_json::json!(n)))
                            .unwrap_or_else(|| row.try_get::<String, _>(i)
                                .map(|v| serde_json::json!(v))
                                .unwrap_or(serde_json::Value::Null))

                    // ── 布尔 ─────────────────────────────────────
                    } else if tname == "BOOL" || tname == "BOOLEAN" {
                        row.try_get::<bool, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(serde_json::Value::Null)

                    // ── 字符串（各种 char/varchar/text 变体）─────────
                    } else if tname == "TEXT" || tname == "VARCHAR"
                        || tname == "CHAR" || tname == "BPCHAR"
                        || tname == "NAME" || tname == "CHARACTER"
                    {
                        row.try_get::<String, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(serde_json::Value::Null)

                    // ── 日期时间 ─────────────────────────────────
                    } else if tname == "DATE" {
                        row.try_get::<chrono::NaiveDate, _>(i)
                            .map(|v| serde_json::json!(v.format("%Y-%m-%d").to_string()))
                            .unwrap_or(fallback_type(&tname))
                    } else if tname == "TIME" {
                        row.try_get::<chrono::NaiveTime, _>(i)
                            .map(|v| serde_json::json!(v.format("%H:%M:%S%.f").to_string()))
                            .unwrap_or(fallback_type(&tname))
                    } else if tname == "TIMETZ" {
                        // time with time zone → String (sqlx 不直接支持 TimeTz 二进制读取)
                        row.try_get::<String, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(fallback_type(&tname))
                    } else if tname == "TIMESTAMP" || tname == "TIMESTAMP WITHOUT TIME ZONE" {
                        row.try_get::<chrono::NaiveDateTime, _>(i)
                            .map(|v| serde_json::json!(v.format("%Y-%m-%d %H:%M:%S").to_string()))
                            .unwrap_or(fallback_type(&tname))
                    } else if tname == "TIMESTAMPTZ" || tname == "TIMESTAMP WITH TIME ZONE" {
                        row.try_get::<chrono::DateTime<chrono::Utc>, _>(i)
                            .map(|v| serde_json::json!(v.to_rfc3339_opts(
                                chrono::SecondsFormat::Secs,
                                false,
                            )))
                            .unwrap_or(fallback_type(&tname))

                    // ── 二进制 ─────────────────────────────────────
                    } else if tname == "BYTEA" {
                        row.try_get::<Vec<u8>, _>(i)
                            .map(|bytes| {
                                let engine = base64::engine::general_purpose::STANDARD;
                                serde_json::json!(base64::engine::Engine::encode(&engine, &bytes))
                            })
                            .unwrap_or(serde_json::Value::Null)

                    // ── UUID ──────────────────────────────────────
                    } else if tname == "UUID" {
                        row.try_get::<uuid::Uuid, _>(i)
                            .map(|v| serde_json::json!(v.to_string()))
                            .unwrap_or(fallback_type(&tname))

                    // ── JSON ──────────────────────────────────────
                    } else if tname == "JSON" || tname == "JSONB" {
                        row.try_get::<serde_json::Value, _>(i)
                            .map(|v| v.clone())
                            .unwrap_or(fallback_type(&tname))

                    // ── 网络地址 ──────────────────────────────────
                    } else if tname == "INET" || tname == "CIDR" {
                        // sqlx 0.8 对 IpNet 的 PG 支持有限，降级为 String 读取
                        row.try_get::<String, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(fallback_type(&tname))
                    } else if tname == "MACADDR" || tname == "MACADDR8" {
                        // sqlx 不直接支持 macaddr Decode，降级为 String
                        row.try_get::<String, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(fallback_type(&tname))

                    // ── 兜底：未知类型按字符串处理 ─────────────────
                    } else {
                        row.try_get::<String, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(fallback_type(&tname))
                    }
                })
                .collect()
        })
        .collect()
}

/// 兜底处理：返回 [TypeName] 格式的调试信息，而非 panic 或 Null
fn fallback_type(type_name: &str) -> serde_json::Value {
    format!("[{}]", type_name).into()
}

