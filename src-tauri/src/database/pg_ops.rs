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

                    // 精确匹配整数类型（避免 TIMESTAMP 包含 INT 子串被误匹配）
                    if tname == "INT2" || tname == "INT4" || tname == "INT8"
                        || tname == "SERIAL" || tname == "BIGSERIAL" || tname == "SMALLSERIAL"
                    {
                        row.try_get::<i64, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(serde_json::Value::Null)
                    } else if tname.contains("FLOAT") || tname.contains("NUMERIC")
                    {
                        row.try_get::<f64, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(serde_json::Value::Null)
                    } else if tname == "BOOL" {
                        row.try_get::<bool, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(serde_json::Value::Null)
                    } else if tname == "TIMESTAMP" {
                        // timestamp without time zone → NaiveDateTime
                        row.try_get::<chrono::NaiveDateTime, _>(i)
                            .map(|v| serde_json::json!(v.to_string()))
                            .unwrap_or_else(|_| {
                                serde_json::json!(format!("[{}]", val.type_info().name()))
                            })
                    } else if tname == "TIMESTAMPTZ" {
                        // timestamp with time zone → DateTime<Utc>
                        row.try_get::<chrono::DateTime<chrono::Utc>, _>(i)
                            .map(|v| serde_json::json!(v.to_string()))
                            .unwrap_or_else(|_| {
                                serde_json::json!(format!("[{}]", val.type_info().name()))
                            })
                    } else if tname == "DATE" {
                        row.try_get::<chrono::NaiveDate, _>(i)
                            .map(|v| serde_json::json!(v.to_string()))
                            .unwrap_or_else(|_| {
                                serde_json::json!(format!("[{}]", val.type_info().name()))
                            })
                    } else if tname == "TIME" {
                        row.try_get::<chrono::NaiveTime, _>(i)
                            .map(|v| serde_json::json!(v.to_string()))
                            .unwrap_or_else(|_| {
                                serde_json::json!(format!("[{}]", val.type_info().name()))
                            })
                    } else if tname == "TIMETZ" {
                        row.try_get::<String, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or_else(|_| {
                                serde_json::json!(format!("[{}]", val.type_info().name()))
                            })
                    } else if tname == "UUID" {
                        row.try_get::<uuid::Uuid, _>(i)
                            .map(|v| serde_json::json!(v.to_string()))
                            .unwrap_or_else(|_| {
                                serde_json::json!(format!("[{}]", val.type_info().name()))
                            })
                    } else if tname == "JSON" || tname == "JSONB" {
                        row.try_get::<serde_json::Value, _>(i)
                            .map(|v| serde_json::json!(v.to_string()))
                            .unwrap_or_else(|_| {
                                serde_json::json!(format!("[{}]", val.type_info().name()))
                            })
                    } else {
                        row.try_get::<String, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or_else(|_| {
                                serde_json::json!(format!("[{}]", val.type_info().name()))
                            })
                    }
                })
                .collect()
        })
        .collect()
}
