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

    async fn update_row(
        &self,
        _database: &str,
        table: &str,
        primary_key: &str,
        primary_key_value: serde_json::Value,
        column_values: std::collections::HashMap<String, serde_json::Value>,
        column_types: std::collections::HashMap<String, String>,
    ) -> Result<u64, String> {
        if column_values.is_empty() {
            return Ok(0);
        }

        // PG 的 table 参数格式为 "schema.table"，需要拆分引用
        let safe_table = pg_safe_table_ref(table);

        // PG 标识符用双引号包裹（防止保留字冲突）
        // 非字符串列的占位符加 ::type 显式转换（PG 二进制协议不接受 text→bigint 等隐式转换）
        let mut set_parts = Vec::new();
        let col_names: Vec<String> = column_values.keys().cloned().collect();
        for (i, col) in col_names.iter().enumerate() {
            let placeholder = match column_types.get(col).map(|s| s.as_str()) {
                Some(t) if !matches!(t.to_uppercase().as_str(), "TEXT" | "VARCHAR" | "CHAR" | "BPCHAR" | "NAME" | "CHARACTER"
                    | "JSON" | "JSONB") => format!("${}::{}", i + 1, t),
                _ => format!("${}", i + 1),
            };
            set_parts.push(format!("\"{}\" = {}", col, placeholder));
        }
        let where_clause = format!("\"{}\" = ${}", primary_key, col_names.len() + 1);

        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            safe_table, set_parts.join(", "), where_clause
        );

        // 使用 PgArguments 手动构建参数，避免 Query::bind 所有权问题
        let mut args = sqlx::postgres::PgArguments::default();

        // 绑定 SET 值（使用列类型信息做类型感知绑定）
        for col in &col_names {
            let ct = column_types.get(col).map(|s| s.as_str());
            bind_json_value_to_pg_args(&mut args, column_values.get(col).unwrap(), ct);
        }

        // 绑定 WHERE 主键值
        bind_json_value_to_pg_args(&mut args, &primary_key_value, None);

        let query = sqlx::query_with(&sql, args);

        let result = query
            .execute(self)
            .await
            .map_err(|e| format!("UPDATE 失败: {}", e))?;

        Ok(result.rows_affected())
    }

    async fn delete_row(&self, _database: &str, table: &str, primary_key: &str, primary_key_value: serde_json::Value) -> Result<u64, String> {
        let safe_table = pg_safe_table_ref(table);
        let sql = format!("DELETE FROM {} WHERE \"{}\" = $1", safe_table, primary_key);
        let mut args = sqlx::postgres::PgArguments::default();
        bind_json_value_to_pg_args(&mut args, &primary_key_value, None);
        let result = sqlx::query_with(&sql, args)
            .execute(self)
            .await
            .map_err(|e| format!("DELETE 失败: {}", e))?;
        Ok(result.rows_affected())
    }

    async fn insert_row(&self, _database: &str, table: &str, column_values: std::collections::HashMap<String, serde_json::Value>, column_types: std::collections::HashMap<String, String>) -> Result<u64, String> {
        if column_values.is_empty() {
            return Err("插入数据不能为空".into());
        }
        let col_names: Vec<String> = column_values.keys().cloned().collect();
        // 根据列类型决定占位符格式：
        // - 非字符串类型需要 $N::type 显式转换（PG 二进制协议不接受 text→bigint 等隐式转换）
        let placeholders: Vec<String> = (1..=col_names.len()).map(|n| {
            match col_names.get(n-1).and_then(|c| column_types.get(c)).map(|s| s.as_str()) {
                Some(t) if !matches!(t.to_uppercase().as_str(), "TEXT" | "VARCHAR" | "CHAR" | "BPCHAR" | "NAME" | "CHARACTER"
                    | "JSON" | "JSONB") => format!("${}::{}", n, t),
                _ => format!("${}", n),
            }
        }).collect();
        let safe_table = pg_safe_table_ref(table);
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            safe_table,
            col_names.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(", "),
            placeholders.join(", ")
        );
        let mut args = sqlx::postgres::PgArguments::default();
        for col in &col_names {
            let col_type = column_types.get(col).map(|s| s.as_str());
            bind_json_value_to_pg_args(&mut args, column_values.get(col).unwrap(), col_type);
        }
        let result = sqlx::query_with(&sql, args)
            .execute(self)
            .await
            .map_err(|e| format!("INSERT 失败: {}", e))?;
        Ok(result.rows_affected())
    }
}

/// 将 "schema.table" 格式的表名拆分为 PG 安全引用: "schema"."table"
/// 如果不含点号，则直接包裹为 "table"
fn pg_safe_table_ref(table: &str) -> String {
    if let Some(dot_pos) = table.find('.') {
        let schema = &table[..dot_pos];
        let tbl = &table[dot_pos + 1..];
        format!("\"{}\".\"{}\"", schema, tbl)
    } else {
        format!("\"{}\"", table)
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

/// 将 JSON 值绑定到 PgArguments（避免 Query::bind 消耗所有权的问题）
/// col_type: 目标列的 PG 类型名（如 "int8", "bigint", "timestamp"），用于决定绑定方式
fn bind_json_value_to_pg_args(args: &mut sqlx::postgres::PgArguments, val: &serde_json::Value, col_type: Option<&str>) {
    use sqlx::Arguments;
    match val {
        serde_json::Value::Null => { let _ = args.add(None::<String>); }
        serde_json::Value::Number(n) => {
            // 根据目标列类型选择正确的数值绑定
            if n.is_f64() {
                let _ = args.add(n.as_f64().unwrap_or(0.0));
            } else if n.is_i64() {
                let _ = args.add(n.as_i64().unwrap_or(0i64));
            } else {
                let _ = args.add(n.as_u64().unwrap_or(0) as i64);
            }
        }
        serde_json::Value::String(s) => {
            // 非字符串类型的目标列：尝试解析为对应 Rust 类型再绑定
            let upper = col_type.map(|t| t.to_ascii_uppercase());
            match upper.as_deref() {
                Some("BIGINT") | Some("INT8") | Some("INT") if s.parse::<i64>().is_ok() => {
                    let _ = args.add(s.parse::<i64>().unwrap());
                }
                Some("INTEGER") | Some("INT4") if s.parse::<i32>().is_ok() => {
                    let _ = args.add(s.parse::<i32>().unwrap());
                }
                Some("SMALLINT") | Some("INT2") if s.parse::<i16>().is_ok() => {
                    let _ = args.add(s.parse::<i16>().unwrap());
                }
                Some("DOUBLE PRECISION") | Some("FLOAT8") | Some("FLOAT") if s.parse::<f64>().is_ok() => {
                    let _ = args.add(s.parse::<f64>().unwrap());
                }
                Some("REAL") | Some("FLOAT4") if s.parse::<f32>().is_ok() => {
                    let _ = args.add(s.parse::<f32>().unwrap());
                }
                Some("BOOLEAN") | Some("BOOL") if s.eq_ignore_ascii_case("true")
                    || s.eq_ignore_ascii_case("false") || s == "1" || s == "0" => {
                    let _ = args.add(s.eq_ignore_ascii_case("true") || s == "1");
                }
                // 日期/时间/JSON 等复杂类型：绑 String（由 ::type 强转兜底）
                _ => { let _ = args.add(s.clone()); }
            }
        }
        serde_json::Value::Bool(b) => { let _ = args.add(*b); }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            let _ = args.add(val.to_string());
        }
    }
}

