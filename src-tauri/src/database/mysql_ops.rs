use crate::database::db_ops::{
    ColumnMeta, DatabaseMeta, DbOps, QueryResult, SchemaMeta, TableMeta,
};
use sqlx::{Column, MySqlPool, Row, TypeInfo, ValueRef};

impl DbOps for MySqlPool {
    async fn list_databases(&self) -> Result<Vec<DatabaseMeta>, String> {
        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT CAST(schema_name AS CHAR) FROM information_schema.schemata \
             WHERE schema_name NOT IN ('information_schema','performance_schema','mysql','sys') \
             ORDER BY schema_name",
        )
        .fetch_all(self)
        .await
        .map_err(|e| e.to_string())?;

        Ok(rows
            .into_iter()
            .map(|r| DatabaseMeta { name: r.0 })
            .collect())
    }

    async fn list_schemas(&self, _database: &str) -> Result<Vec<SchemaMeta>, String> {
        // MySQL 没有 schema 概念（database = schema）
        Ok(vec![])
    }

    async fn list_tables(
        &self,
        database: &str,
        _schema: Option<&str>,
    ) -> Result<Vec<TableMeta>, String> {
        let rows = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT CAST(TABLE_NAME AS CHAR), CAST(TABLE_TYPE AS CHAR), \
                    CAST(COALESCE(TABLE_ROWS, 0) AS SIGNED) \
             FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA = ? ORDER BY TABLE_NAME",
        )
        .bind(database)
        .fetch_all(self)
        .await
        .map_err(|e| e.to_string())?;

        Ok(rows
            .into_iter()
            .map(|r| TableMeta {
                name: r.0,
                table_type: Some(r.1),
                row_count: Some(r.2),
            })
            .collect())
    }

    async fn list_columns(&self, database: &str, table: &str) -> Result<Vec<ColumnMeta>, String> {
        let sql = format!(
            "SELECT CAST(COLUMN_NAME AS CHAR), CAST(DATA_TYPE AS CHAR), \
                    CAST(IS_NULLABLE AS CHAR), \
                    CAST(COALESCE(COLUMN_KEY, '') AS CHAR), \
                    CAST(COALESCE(COLUMN_DEFAULT, '') AS CHAR), \
                    CAST(COALESCE(EXTRA, '') AS CHAR), \
                    CAST(COALESCE(COLUMN_COMMENT, '') AS CHAR) \
             FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' ORDER BY ORDINAL_POSITION",
            database, table
        );

        let rows =
            sqlx::query_as::<_, (String, String, String, String, String, String, String)>(&sql)
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
                extra: if r.5.is_empty() { None } else { Some(r.5) },
                comment: if r.6.is_empty() { None } else { Some(r.6) },
            })
            .collect())
    }

    async fn get_table_data(
        &self,
        database: &str,
        table: &str,
        page: i64,
        page_size: i64,
        sort_by: Option<String>,
        sort_order: Option<String>,
        filters: Option<std::collections::HashMap<String, String>>,
    ) -> Result<QueryResult, String> {
        let start = std::time::Instant::now();
        let offset = (page - 1) * page_size;

        // 构建 WHERE 子句
        let mut where_clauses = Vec::new();
        if let Some(filters) = filters {
            for (col, filter_value) in filters {
                if !filter_value.is_empty() {
                    // 解析 "操作符|值" 格式
                    if let Some((op, val)) = filter_value.split_once('|') {
                        match op.to_uppercase().as_str() {
                            "IS NULL" => where_clauses.push(format!("`{}` IS NULL", col)),
                            "IS NOT NULL" => where_clauses.push(format!("`{}` IS NOT NULL", col)),
                            _ => {
                                let safe_val = val.replace("'", "''");
                                where_clauses.push(format!("`{}` {} '{}'", col, op, safe_val));
                            }
                        }
                    } else {
                        // 兼容旧格式：纯文本作为值，使用 LIKE
                        where_clauses.push(format!(
                            "`{}` LIKE '%{}%'",
                            col,
                            filter_value.replace("'", "''")
                        ));
                    }
                }
            }
        }
        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_clauses.join(" AND "))
        };

        // 构建 ORDER BY 子句
        let order_sql = if let Some(sort_col) = sort_by {
            let order = sort_order.unwrap_or_else(|| "asc".to_string());
            let order_dir = if order.to_lowercase() == "desc" {
                "DESC"
            } else {
                "ASC"
            };
            format!(" ORDER BY `{}` {}", sort_col, order_dir)
        } else {
            String::new()
        };

        // 带过滤条件的总数查询
        let count_sql = format!(
            "SELECT COUNT(*) FROM `{}`.`{}`{}",
            database, table, where_sql
        );
        let count: i64 = sqlx::query(&count_sql)
            .fetch_one(self)
            .await
            .map_err(|e| e.to_string())?
            .get(0);

        // 主查询（带过滤、排序、分页）
        let query_sql = format!(
            "SELECT * FROM `{}`.`{}`{}{} LIMIT {} OFFSET {}",
            database, table, where_sql, order_sql, page_size, offset
        );
        let rows = sqlx::query(&query_sql)
            .fetch_all(self)
            .await
            .map_err(|e| e.to_string())?;

        if rows.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                column_types: None,
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
        let column_types: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.type_info().name().to_string())
            .collect();
        let data = mysql_rows_to_json(&rows);

        Ok(QueryResult {
            columns,
            rows: data,
            column_types: Some(column_types),
            affected_rows: None,
            execution_time_ms: start.elapsed().as_millis() as u64,
            error: None,
            total: Some(count),
        })
    }

    async fn get_row_count(&self, database: &str, table: &str) -> Result<i64, String> {
        let row = sqlx::query(&format!("SELECT COUNT(*) FROM `{}`.`{}`", database, table))
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
                column_types: None,
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
        let column_types: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.type_info().name().to_string())
            .collect();
        let data = mysql_rows_to_json(&rows);

        Ok(QueryResult {
            columns,
            rows: data,
            column_types: Some(column_types),
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

    async fn update_row(
        &self,
        database: &str,
        table: &str,
        primary_key: &str,
        primary_key_type: &str,
        primary_key_value: serde_json::Value,
        column_values: std::collections::HashMap<String, serde_json::Value>,
        _column_types: std::collections::HashMap<String, String>,
    ) -> Result<u64, String> {
        update_row_impl(
            self,
            database,
            table,
            primary_key,
            primary_key_type,
            primary_key_value,
            column_values,
        )
        .await
    }

    async fn delete_row(
        &self,
        database: &str,
        table: &str,
        primary_key: &str,
        _primary_key_type: &str,
        primary_key_value: serde_json::Value,
    ) -> Result<u64, String> {
        let sql = format!(
            "DELETE FROM `{}`.`{}` WHERE `{}` = ?",
            database, table, primary_key
        );
        let mut args = sqlx::mysql::MySqlArguments::default();
        bind_json_value_to_mysql_args(&mut args, &primary_key_value);
        let result = sqlx::query_with(&sql, args)
            .execute(self)
            .await
            .map_err(|e| format!("DELETE 失败: {}", e))?;
        Ok(result.rows_affected())
    }

    async fn insert_row(
        &self,
        database: &str,
        table: &str,
        column_values: std::collections::HashMap<String, serde_json::Value>,
        _column_types: std::collections::HashMap<String, String>,
    ) -> Result<u64, String> {
        if column_values.is_empty() {
            return Err("插入数据不能为空".into());
        }
        let col_names: Vec<String> = column_values.keys().cloned().collect();
        let placeholders: Vec<String> = (0..col_names.len()).map(|_| "?".to_string()).collect();
        let sql = format!(
            "INSERT INTO `{}`.{} ({}) VALUES ({})",
            database,
            table,
            col_names
                .iter()
                .map(|c| format!("`{}`", c))
                .collect::<Vec<_>>()
                .join(", "),
            placeholders.join(", ")
        );
        let mut args = sqlx::mysql::MySqlArguments::default();
        for col in &col_names {
            bind_json_value_to_mysql_args(&mut args, column_values.get(col).unwrap());
        }
        let result = sqlx::query_with(&sql, args)
            .execute(self)
            .await
            .map_err(|e| format!("INSERT 失败: {}", e))?;
        Ok(result.rows_affected())
    }
}

fn mysql_rows_to_json(rows: &[sqlx::mysql::MySqlRow]) -> Vec<Vec<serde_json::Value>> {
    rows.iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|i| {
                    let val = row.try_get_raw(i).unwrap();
                    if val.is_null() {
                        return serde_json::Value::Null;
                    }
                    let tname = val.type_info().name().to_uppercase();
                    if tname.contains("INT") || tname.contains("SERIAL") {
                        row.try_get::<i64, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(serde_json::Value::Null)
                    } else if tname.contains("FLOAT")
                        || tname.contains("DOUBLE")
                        || tname.contains("DECIMAL")
                    {
                        row.try_get::<f64, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(serde_json::Value::Null)
                    } else if tname == "BOOL" || tname == "BOOLEAN" {
                        row.try_get::<bool, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or(serde_json::Value::Null)
                    } else if tname == "DATE"
                        || tname == "DATETIME"
                        || tname == "TIMESTAMP"
                        || tname == "TIME"
                    {
                        row.try_get::<chrono::NaiveDateTime, _>(i)
                            .map(|v| serde_json::json!(v.to_string()))
                            .or_else(|_| {
                                row.try_get::<chrono::NaiveDate, _>(i)
                                    .map(|v| serde_json::json!(v.to_string()))
                            })
                            .or_else(|_| {
                                row.try_get::<chrono::NaiveTime, _>(i)
                                    .map(|v| serde_json::json!(v.to_string()))
                            })
                            .unwrap_or_else(|_| serde_json::json!(format!("[{}]", tname)))
                    } else if tname == "JSON" {
                        row.try_get::<serde_json::Value, _>(i)
                            .map(|v| v)
                            .unwrap_or_else(|_| serde_json::json!(format!("[{}]", tname)))
                    } else if tname == "BINARY" || tname == "VARBINARY" {
                        row.try_get::<Vec<u8>, _>(i)
                            .map(|v| serde_json::json!(String::from_utf8_lossy(&v).to_string()))
                            .unwrap_or_else(|_| serde_json::json!(format!("[{}]", tname)))
                    } else {
                        row.try_get::<String, _>(i)
                            .map(|v| serde_json::json!(v))
                            .unwrap_or_else(|_| serde_json::json!(format!("[{}]", tname)))
                    }
                })
                .collect()
        })
        .collect()
}

async fn update_row_impl(
    pool: &sqlx::MySqlPool,
    _database: &str,
    table: &str,
    primary_key: &str,
    _primary_key_type: &str,
    primary_key_value: serde_json::Value,
    column_values: std::collections::HashMap<String, serde_json::Value>,
) -> Result<u64, String> {
    if column_values.is_empty() {
        return Ok(0);
    }

    // 构建 SET 子句和参数
    let mut set_parts = Vec::new();
    let col_names: Vec<String> = column_values.keys().cloned().collect();
    for col in &col_names {
        set_parts.push(format!("`{}` = ?", col));
    }

    let sql = format!(
        "UPDATE `{}`.{} SET {} WHERE `{}` = ?",
        _database,
        table,
        set_parts.join(", "),
        primary_key
    );

    // 使用 MySqlArguments 手动构建参数，避免 Query::bind 所有权问题
    let mut args = sqlx::mysql::MySqlArguments::default();

    // 绑定 SET 值
    for col in &col_names {
        let val = column_values.get(col).unwrap();
        bind_json_value_to_mysql_args(&mut args, val);
    }

    // 绑定 WHERE 主键值
    bind_json_value_to_mysql_args(&mut args, &primary_key_value);

    let result = sqlx::query_with(&sql, args)
        .execute(pool)
        .await
        .map_err(|e| format!("UPDATE 失败: {}", e))?;

    Ok(result.rows_affected())
}

/// 将 JSON 值绑定到 MySqlArguments（避免 Query::bind 消耗所有权的问题）
fn bind_json_value_to_mysql_args(args: &mut sqlx::mysql::MySqlArguments, val: &serde_json::Value) {
    use sqlx::Arguments;
    match val {
        serde_json::Value::Null => {
            let _ = args.add(None::<String>);
        }
        serde_json::Value::Number(n) => {
            if n.is_f64() {
                let _ = args.add(n.as_f64().unwrap_or(0.0));
            } else if n.is_i64() {
                let _ = args.add(n.as_i64().unwrap_or(0i64));
            } else {
                let _ = args.add(n.as_u64().unwrap_or(0) as i64);
            }
        }
        serde_json::Value::String(s) => {
            let _ = args.add(s.clone());
        }
        serde_json::Value::Bool(b) => {
            let _ = args.add(*b);
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            let _ = args.add(val.to_string());
        }
    }
}
