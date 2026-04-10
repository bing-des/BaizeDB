//! PostgreSQL 数据目标实现
//!
//! 重要：PostgreSQL 连接池在创建时已直接连接到目标数据库，
//! 所以不需要 SET search_path 切换数据库（search_path 只能切换 schema，不能切换 database）。
//! 所有操作直接在 public schema 上执行，使用 public.table 全限定名。

use async_trait::async_trait;
use chrono::TimeZone;
use sqlx::{PgPool, Arguments};
use sqlx::postgres::PgArguments;
use crate::state::DbPool;

use super::types::*;

/// PostgreSQL 数据目标
pub struct PostgreSQLTarget {
    pool: PgPool,
}

impl PostgreSQLTarget {
    /// 从 DbPool 创建 PostgreSQL 目标（必须是 PostgreSQL 类型）
    pub fn from_pool(pool: &DbPool) -> Result<Self, String> {
        match pool {
            DbPool::PostgreSQL(p) => Ok(Self { pool: p.clone() }),
            _ => Err("不是 PostgreSQL 连接池".to_string()),
        }
    }

    /// 从 PgPool 直接创建
    #[allow(dead_code)]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 将 PostgreSQL 类型字符串转换为中间层 DataType
    fn postgres_type_to_intermediate(pg_type: &str, type_info: Option<&str>) -> DataType {
        let lower = pg_type.to_lowercase();
        match lower.as_str() {
            "smallint" | "int2" => DataType::SmallInt,
            "integer" | "int" | "int4" => DataType::Integer,
            "bigint" | "int8" => DataType::BigInt,
            "decimal" | "numeric" => {
                let (precision, scale) = Self::parse_precision_scale(type_info);
                DataType::Decimal { precision, scale }
            }
            "real" | "float4" => DataType::Float,
            "double precision" | "float8" => DataType::Double,
            "character" | "char" => {
                let length = Self::parse_length(type_info);
                DataType::Char { length }
            }
            "character varying" | "varchar" => {
                let length = Self::parse_length(type_info);
                DataType::VarChar { length }
            }
            "text" => DataType::Text,
            "bytea" => DataType::Bytea,
            "date" => DataType::Date,
            "time" | "time without time zone" => DataType::Time,
            "timestamp" | "timestamp without time zone" => {
                let precision = Self::parse_precision(type_info);
                DataType::DateTime { precision }
            }
            "timestamp with time zone" | "timestamptz" => {
                let precision = Self::parse_precision(type_info);
                DataType::Timestamp { precision, with_tz: true }
            }
            "boolean" | "bool" => DataType::Boolean,
            "json" => DataType::Json,
            "jsonb" => DataType::Jsonb,
            "uuid" => DataType::Uuid,
            "serial" => DataType::Serial,
            "bigserial" => DataType::BigSerial,
            _ => DataType::Unknown(pg_type.to_string()),
        }
    }

    /// 解析精度和小数位数（例如 numeric(10,2)）
    fn parse_precision_scale(type_info: Option<&str>) -> (Option<u32>, Option<u32>) {
        if let Some(info) = type_info {
            if let Some(inner) = info.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
                let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
                if parts.len() == 2 {
                    let precision = parts[0].parse::<u32>().ok();
                    let scale = parts[1].parse::<u32>().ok();
                    return (precision, scale);
                } else if parts.len() == 1 {
                    let precision = parts[0].parse::<u32>().ok();
                    return (precision, None);
                }
            }
        }
        (None, None)
    }

    /// 解析长度（例如 varchar(255)）
    fn parse_length(type_info: Option<&str>) -> Option<u32> {
        if let Some(info) = type_info {
            if let Some(inner) = info.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
                return inner.parse::<u32>().ok();
            }
        }
        None
    }

    /// 解析精度（例如 timestamp(6)）
    fn parse_precision(type_info: Option<&str>) -> Option<u32> {
        Self::parse_length(type_info)
    }

    /// 判断默认值是否是 PG 兼容的
    /// MySQL 的默认值可能包含 PG 不兼容的语法，需要过滤掉
    fn is_pg_compatible_default(default: &str) -> bool {
        let d = default.trim();
        // 空值跳过
        if d.is_empty() { return false; }
        // 包含 ON UPDATE 的跳过（MySQL 特有）
        if d.contains("ON UPDATE") { return false; }
        // PG 已知的函数/关键字
        let upper = d.to_uppercase();
        if upper == "NULL" { return true; }
        if upper == "CURRENT_TIMESTAMP" { return true; }
        if upper == "CURRENT_DATE" { return true; }
        if upper == "CURRENT_TIME" { return true; }
        if upper == "TRUE" || upper == "FALSE" { return true; }
        // 纯数字（整数或小数，含负号）
        if d.starts_with('-') || d.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            // 确保是合法数字
            return d.parse::<f64>().is_ok() || d.parse::<i64>().is_ok();
        }
        // 单引号包裹的字符串值
        if d.starts_with('\'') && d.ends_with('\'') { return true; }
        // 其他情况（如 uuid()、0x...、表达式等）一律跳过
        // 包含特殊字符的（如十六进制 0x、函数调用含括号等）
        if d.starts_with("0x") || d.starts_with("0X") { return false; }
        if upper.starts_with("UUID()") { return false; }
        if upper.starts_with("GEN_RANDOM_UUID()") { return true; }
        if upper.starts_with("NOW()") { return true; }
        // 包含括号的函数调用（可能是 PG 不认识的函数）
        // 只允许已知的 PG 函数
        if d.contains('(') && d.contains(')') {
            // 已知安全的 PG 函数
            return upper.starts_with("NOW(") 
                || upper.starts_with("CURRENT_TIMESTAMP(")
                || upper.starts_with("GEN_RANDOM_UUID(")
                || upper.starts_with("CURRENT_DATE(")
                || upper.starts_with("CURRENT_TIME(");
        }
        false
    }

    /// 尝试多种格式将字符串解析为 NaiveDateTime
    fn parse_naive_datetime(s: &str) -> Option<chrono::NaiveDateTime> {
        // 常见日期时间格式列表
        let formats = [
            "%Y-%m-%d %H:%M:%S%.f",   // 2024-01-01 12:00:00 或 2024-01-01 12:00:00.123
            "%Y-%m-%dT%H:%M:%S%.f",   // ISO 8601
            "%Y/%m/%d %H:%M:%S%.f",   // 斜杠分隔
            "%Y-%m-%d %H:%M:%S",      // 不带毫秒
            "%Y-%m-%dT%H:%M:%S",      // ISO 8601 不带毫秒
            "%Y/%m/%d %H:%M:%S",      // 斜杠分隔不带毫秒
        ];
        for fmt in &formats {
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
                return Some(dt);
            }
        }
        // 如果所有格式都失败，尝试截断毫秒部分后重试
        if let Some(dot_pos) = s.rfind('.') {
            let truncated = &s[..dot_pos];
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(truncated, "%Y-%m-%d %H:%M:%S") {
                return Some(dt);
            }
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(truncated, "%Y-%m-%dT%H:%M:%S") {
                return Some(dt);
            }
        }
        None
    }

    /// 获取目标表的实际架构（如果表存在）
    /// 连接池已直接连到目标数据库，无需 SET search_path
    async fn get_target_table_schema(&self, database: &str, table: &str) -> Result<Option<TableSchema>, String> {
        // 查询 information_schema.columns 获取列信息
        // 连接池已连到目标数据库，直接查询即可
        let sql = "
            SELECT 
                c.column_name,
                c.data_type,
                c.is_nullable,
                CASE WHEN kcu.column_name IS NOT NULL THEN 'PRI' ELSE '' END AS column_key,
                COALESCE(c.column_default, '') AS column_default,
                '' AS column_comment,
                c.character_maximum_length,
                c.numeric_precision,
                c.numeric_scale,
                c.datetime_precision
            FROM information_schema.columns c
            LEFT JOIN information_schema.key_column_usage kcu
                ON c.table_catalog = kcu.table_catalog
                AND c.table_schema = kcu.table_schema
                AND c.table_name = kcu.table_name
                AND c.column_name = kcu.column_name
                AND kcu.constraint_name IN (
                    SELECT constraint_name 
                    FROM information_schema.table_constraints
                    WHERE constraint_type = 'PRIMARY KEY' 
                    AND table_catalog = $1
                    AND table_schema = c.table_schema
                    AND table_name = $2
                )
            WHERE c.table_catalog = $1
                AND c.table_schema = 'public'
                AND c.table_name = $2
            ORDER BY c.ordinal_position
        ";
        
        let rows_result = sqlx::query_as::<_, (String, String, String, String, String, String, Option<i32>, Option<i32>, Option<i32>, Option<i32>)>(sql)
            .bind(database)
            .bind(table)
            .fetch_all(&self.pool)
            .await;
        
        match rows_result {
            Ok(rows) => {
                if rows.is_empty() {
                    // 表不存在或没有列
                    return Ok(None);
                }
                
                let columns: Vec<ColumnDef> = rows.into_iter().map(|r| {
                    let (name, data_type, nullable, key, default_value, _comment, char_len, num_precision, num_scale, dt_precision) = r;
                    
                    // 构建类型信息字符串（用于解析精度/长度）
                    let type_info = if let Some(len) = char_len {
                        Some(format!("({})", len))
                    } else if num_precision.is_some() || num_scale.is_some() {
                        let precision = num_precision.map(|p| p.to_string()).unwrap_or_default();
                        let scale = num_scale.map(|s| s.to_string()).unwrap_or_default();
                        if scale.is_empty() {
                            Some(format!("({})", precision))
                        } else {
                            Some(format!("({},{})", precision, scale))
                        }
                    } else if dt_precision.is_some() {
                        dt_precision.map(|p| format!("({})", p))
                    } else {
                        None
                    };
                    
                    let data_type = Self::postgres_type_to_intermediate(&data_type, type_info.as_deref());
                    
                    ColumnDef {
                        name,
                        data_type,
                        nullable: nullable == "YES",
                        is_primary_key: key == "PRI",
                        default_value: if default_value.is_empty() { None } else { Some(default_value) },
                        comment: None, // PostgreSQL 信息模式中没有列注释
                    }
                }).collect();
                
                Ok(Some(TableSchema {
                    name: table.to_string(),
                    columns,
                }))
            }
            Err(e) => {
                // 查询失败，可能表不存在
                log::debug!("获取目标表架构失败: {}", e);
                Ok(None)
            }
        }
    }

    /// 将中间层 DataType 转换为 PostgreSQL 类型字符串
    fn intermediate_to_postgres_type(data_type: &DataType) -> String {
        match data_type {
            DataType::TinyInt => "SMALLINT".to_string(),
            DataType::SmallInt => "SMALLINT".to_string(),
            DataType::Integer => "INTEGER".to_string(),
            DataType::BigInt => "BIGINT".to_string(),
            DataType::Decimal { precision, scale } => {
                match (precision, scale) {
                    (Some(p), Some(s)) => format!("NUMERIC({},{})", p, s),
                    (Some(p), None) => format!("NUMERIC({})", p),
                    _ => "NUMERIC".to_string(),
                }
            }
            DataType::Numeric { precision, scale } => {
                match (precision, scale) {
                    (Some(p), Some(s)) => format!("NUMERIC({},{})", p, s),
                    (Some(p), None) => format!("NUMERIC({})", p),
                    _ => "NUMERIC".to_string(),
                }
            }
            DataType::Float => "REAL".to_string(),
            DataType::Double => "DOUBLE PRECISION".to_string(),
            DataType::Real => "REAL".to_string(),
            DataType::Char { length } => {
                match length {
                    Some(l) => format!("CHAR({})", l),
                    None => "CHAR".to_string(),
                }
            }
            DataType::VarChar { length } => {
                match length {
                    Some(l) => format!("VARCHAR({})", l),
                    None => "VARCHAR".to_string(),
                }
            }
            DataType::Text => "TEXT".to_string(),
            DataType::MediumText => "TEXT".to_string(),
            DataType::LongText => "TEXT".to_string(),
            DataType::Binary { length } => {
                match length {
                    Some(_l) => "BYTEA".to_string(), // PostgreSQL 没有长度限制的 binary
                    None => "BYTEA".to_string(),
                }
            }
            DataType::VarBinary { length: _ } => "BYTEA".to_string(),
            DataType::Blob => "BYTEA".to_string(),
            DataType::MediumBlob => "BYTEA".to_string(),
            DataType::LongBlob => "BYTEA".to_string(),
            DataType::Bytea => "BYTEA".to_string(),
            DataType::Date => "DATE".to_string(),
            DataType::Time => "TIME".to_string(),
            DataType::DateTime { precision } => {
                match precision {
                    Some(p) => format!("TIMESTAMP({})", p),
                    None => "TIMESTAMP".to_string(),
                }
            }
            DataType::Timestamp { precision, with_tz } => {
                let tz = if *with_tz { " WITH TIME ZONE" } else { "" };
                match precision {
                    Some(p) => format!("TIMESTAMP({}){}", p, tz),
                    None => format!("TIMESTAMP{}", tz),
                }
            }
            DataType::Year => "INTEGER".to_string(),
            DataType::Json => "JSON".to_string(),
            DataType::Jsonb => "JSONB".to_string(),
            DataType::Boolean => "BOOLEAN".to_string(),
            DataType::Uuid => "UUID".to_string(),
            DataType::Serial => "SERIAL".to_string(),
            DataType::BigSerial => "BIGSERIAL".to_string(),
            DataType::Unknown(typ) => typ.clone(),
        }
    }
}

#[async_trait]
impl DataTarget for PostgreSQLTarget {
    fn target_type(&self) -> &'static str {
        "postgresql"
    }

    async fn create_table(&self, database: &str, schema: &TableSchema) -> Result<(), String> {
        // 过滤掉无法映射的列（Unknown 类型没有 PG 对应）
        let column_defs: Vec<String> = schema.columns.iter().map(|col| {
            let pg_type = Self::intermediate_to_postgres_type(&col.data_type);
            let null_clause = if col.nullable { "" } else { " NOT NULL" };
            // default_value 可能包含 MySQL 特有语法（如 CURRENT_TIMESTAMP ON UPDATE），
            // PG 不兼容，所以跳过包含 ON UPDATE 的默认值
            // default_value 处理：只保留 PG 兼容的默认值
            // MySQL 的默认值可能包含 PG 不兼容的语法（如 uuid()、0x...、0000-00-00 等）
            // 这些会导致 CREATE TABLE 语法错误
            let default_clause = col.default_value.as_ref()
                .filter(|d| Self::is_pg_compatible_default(d))
                .map(|d| format!(" DEFAULT {}", d))
                .unwrap_or_default();
            format!("\"{}\" {}{}{}", col.name, pg_type, null_clause, default_clause)
        }).collect();

        // 如果列定义为空（所有列都被忽略），跳过建表
        if column_defs.is_empty() {
            log::warn!("[PG目标] 表 {} 没有列定义，跳过建表", schema.name);
            return Ok(());
        }

        let primary_keys: Vec<String> = schema.columns.iter()
            .filter(|c| c.is_primary_key)
            .map(|c| format!("\"{}\"", c.name))
            .collect();

        let pk_clause = if !primary_keys.is_empty() {
            format!(", PRIMARY KEY ({})", primary_keys.join(", "))
        } else {
            String::new()
        };

        // 使用 public."table" 全限定名（连接池已连到目标数据库）
        let create_table_sql = format!("CREATE TABLE IF NOT EXISTS public.\"{}\" ({}{});", 
            schema.name, column_defs.join(", "), pk_clause);
        
        log::info!("[PG目标] 建表 SQL: {}", create_table_sql);

        sqlx::query(&create_table_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("创建表失败: {}", e))?;
        
        log::info!("[PG目标] 创建表 public.{} 成功（数据库: {}）", schema.name, database);
        
        Ok(())
    }

    async fn add_column_comment(
        &self,
        database: &str,
        table: &str,
        column: &str,
        comment: &str,
    ) -> Result<(), String> {
        let escaped_comment = comment.replace("'", "''");
        // 使用 public.table 全限定名
        let comment_sql = format!("COMMENT ON COLUMN public.\"{}\".\"{}\" IS '{}';", table, column, escaped_comment);
        sqlx::query(&comment_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("添加注释失败: {}", e))?;
        
        log::debug!("[PG目标] 添加注释: public.{}.{} （数据库: {}）", table, column, database);
        Ok(())
    }

    async fn insert_rows(
        &self,
        database: &str,
        table: &str,
        schema: &TableSchema,
        rows: &[DataRow],
    ) -> Result<usize, String> {
        if rows.is_empty() {
            return Ok(0);
        }

        // 先获取目标表的实际架构（连接池已连到目标数据库）
        let target_schema = self.get_target_table_schema(database, table).await?;
        
        // 决定使用哪个架构进行类型绑定：优先使用目标架构，否则使用源架构
        let use_target_schema = target_schema.is_some();
        let target_columns = if let Some(ref ts) = target_schema {
            &ts.columns
        } else {
            &schema.columns
        };
        
        log::debug!("[迁移] 表 {} - 目标表存在: {}, 目标列数: {}, 源列数: {}", 
            table, use_target_schema, target_columns.len(), schema.columns.len());
        
        // 确保列数量匹配（如果使用目标架构但列数不匹配，可能有问题，但继续尝试）
        if use_target_schema && target_columns.len() != schema.columns.len() {
            log::warn!("目标表列数 ({}) 与源表列数 ({}) 不匹配，使用源表列顺序", 
                target_columns.len(), schema.columns.len());
        }

        // 构建 INSERT 语句模板（列名加引号防止关键字冲突）
        let column_names: Vec<String> = schema.columns.iter().map(|c| format!("\"{}\"", c.name)).collect();
        let placeholders: Vec<String> = (1..=column_names.len()).map(|i| format!("${}", i)).collect();
        // 使用 public.table 全限定名
        let insert_sql = format!(
            "INSERT INTO public.\"{}\" ({}) VALUES ({}) ON CONFLICT DO NOTHING",
            table,
            column_names.join(", "),
            placeholders.join(", ")
        );

        log::debug!("[PG目标] 开始插入 {} 行到 public.{} （数据库: {}）", rows.len(), table, database);

        let mut inserted = 0;
        for row in rows {
            let mut args = PgArguments::default();
            
            // 同时遍历列和值，确保类型正确绑定
            for (i, (col, value)) in schema.columns.iter().zip(row.values.iter()).enumerate() {
                // 获取目标列类型（如果存在且索引有效）
                let target_col = if use_target_schema && i < target_columns.len() {
                    Some(&target_columns[i])
                } else {
                    None
                };
                
                // 决定使用哪个数据类型进行绑定：优先目标列类型，否则使用源列类型
                let bind_data_type = if let Some(tc) = target_col {
                    &tc.data_type
                } else {
                    &col.data_type
                };
                
                log::info!(
                    "[PG绑定] 表={} 列[{}]='{}' 源类型={:?} 目标类型={:?} 绑定类型={:?} 值={:?}",
                    table, i, col.name, col.data_type,
                    target_col.map(|tc| &tc.data_type),
                    bind_data_type,
                    value
                );
                
                match value {
                    Value::Null => {
                        // NULL 值需要根据目标列类型绑定正确的 Option<T>::None
                        // 否则 PostgreSQL 会推断参数类型为 text，导致类型不匹配
                        match bind_data_type {
                            DataType::TinyInt | DataType::SmallInt => args.add::<Option<i16>>(None),
                            DataType::Integer | DataType::Serial => args.add::<Option<i32>>(None),
                            DataType::BigInt | DataType::BigSerial => args.add::<Option<i64>>(None),
                            DataType::Float | DataType::Real => args.add::<Option<f32>>(None),
                            DataType::Double => args.add::<Option<f64>>(None),
                            DataType::Decimal { .. } | DataType::Numeric { .. } => args.add::<Option<f64>>(None),
                            DataType::Char { .. } | DataType::VarChar { .. } | DataType::Text | 
                            DataType::MediumText | DataType::LongText => args.add::<Option<String>>(None),
                            DataType::Boolean => args.add::<Option<bool>>(None),
                            DataType::Date => args.add::<Option<chrono::NaiveDate>>(None),
                            DataType::Time => args.add::<Option<chrono::NaiveTime>>(None),
                            DataType::DateTime { .. } | DataType::Timestamp { with_tz: false, .. } => {
                                args.add::<Option<chrono::NaiveDateTime>>(None)
                            }
                            DataType::Timestamp { with_tz: true, .. } => {
                                args.add::<Option<chrono::DateTime<chrono::Utc>>>(None)
                            }
                            DataType::Binary { .. } | DataType::VarBinary { .. } | 
                            DataType::Blob | DataType::MediumBlob | DataType::LongBlob | DataType::Bytea => {
                                args.add::<Option<Vec<u8>>>(None)
                            }
                            DataType::Json | DataType::Jsonb => args.add::<Option<serde_json::Value>>(None),
                            DataType::Uuid => args.add::<Option<uuid::Uuid>>(None),
                            DataType::Year => args.add::<Option<i32>>(None),
                            DataType::Unknown(_) => args.add::<Option<String>>(None),
                        }
                    }
                    Value::Bool(b) => {
                        match bind_data_type {
                            DataType::Json | DataType::Jsonb => {
                                args.add(serde_json::Value::Bool(*b))
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText => {
                                args.add(b.to_string())
                            }
                            DataType::TinyInt | DataType::SmallInt => args.add(if *b { 1i16 } else { 0 }),
                            DataType::Integer | DataType::Serial => args.add(if *b { 1i32 } else { 0 }),
                            DataType::BigInt | DataType::BigSerial => args.add(if *b { 1i64 } else { 0 }),
                            DataType::VarChar { .. } | DataType::Char { .. } => args.add(b.to_string()),
                            _ => args.add(*b),
                        }
                    }
                    Value::TinyInt(i) => {
                        // 根据目标列类型提升或转换，避免二进制格式不匹配
                        match bind_data_type {
                            DataType::Boolean => args.add(*i != 0),
                            DataType::SmallInt => args.add(*i as i16),
                            DataType::Integer | DataType::Serial => args.add(*i as i32),
                            DataType::BigInt | DataType::BigSerial => args.add(*i as i64),
                            DataType::Float | DataType::Real => args.add(*i as f32),
                            DataType::Double => args.add(*i as f64),
                            DataType::Decimal { .. } | DataType::Numeric { .. } => args.add(*i as f64),
                            DataType::Json | DataType::Jsonb => {
                                args.add(serde_json::Value::Number((*i as i64).into()))
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(i.to_string())
                            }
                            _ => args.add(*i as i16), // PostgreSQL 没有 tinyint，默认用 smallint
                        }
                    }
                    Value::SmallInt(i) => {
                        match bind_data_type {
                            DataType::Integer | DataType::Serial => args.add(*i as i32),
                            DataType::BigInt | DataType::BigSerial => args.add(*i as i64),
                            DataType::Float | DataType::Real => args.add(*i as f32),
                            DataType::Double => args.add(*i as f64),
                            DataType::Decimal { .. } | DataType::Numeric { .. } => args.add(*i as f64),
                            DataType::Json | DataType::Jsonb => {
                                args.add(serde_json::Value::Number((*i as i64).into()))
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(i.to_string())
                            }
                            DataType::Boolean => args.add(*i != 0),
                            _ => args.add(*i),
                        }
                    }
                    Value::Integer(i) => {
                        match bind_data_type {
                            DataType::BigInt | DataType::BigSerial => args.add(*i as i64),
                            DataType::SmallInt => args.add(*i as i16),
                            DataType::Float | DataType::Real => args.add(*i as f32),
                            DataType::Double => args.add(*i as f64),
                            DataType::Decimal { .. } | DataType::Numeric { .. } => args.add(*i as f64),
                            DataType::Json | DataType::Jsonb => {
                                args.add(serde_json::Value::Number((*i as i64).into()))
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(i.to_string())
                            }
                            DataType::Boolean => args.add(*i != 0),
                            _ => args.add(*i),
                        }
                    }
                    Value::BigInt(i) => {
                        match bind_data_type {
                            DataType::SmallInt => args.add(*i as i16),
                            DataType::Integer | DataType::Serial => args.add(*i as i32),
                            DataType::Float | DataType::Real => args.add(*i as f32),
                            DataType::Double => args.add(*i as f64),
                            DataType::Decimal { .. } | DataType::Numeric { .. } => args.add(*i as f64),
                            DataType::Json | DataType::Jsonb => {
                                // 源值是 bigint，但目标列是 json → 转为 JSON number
                                args.add(serde_json::Value::Number((*i).into()))
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(i.to_string())
                            }
                            DataType::Boolean => args.add(*i != 0),
                            DataType::Uuid => {
                                // bigint → uuid 没有合理转换，降级 NULL
                                log::warn!("[PG目标] BigInt 值 {} 无法转换为 UUID，降级为 NULL", i);
                                args.add::<Option<uuid::Uuid>>(None)
                            }
                            DataType::Date | DataType::Time | DataType::DateTime { .. } | 
                            DataType::Timestamp { .. } => {
                                // bigint → 时间类型无法转换，降级 NULL
                                log::warn!("[PG目标] BigInt 值 {} 无法转换为时间类型（目标: {:?}），降级为 NULL", i, bind_data_type);
                                args.add::<Option<String>>(None)
                            }
                            DataType::Binary { .. } | DataType::VarBinary { .. } | 
                            DataType::Blob | DataType::MediumBlob | DataType::LongBlob | DataType::Bytea => {
                                log::warn!("[PG目标] BigInt 值 {} 无法转换为二进制类型，降级为 NULL", i);
                                args.add::<Option<Vec<u8>>>(None)
                            }
                            _ => args.add(*i),
                        }
                    }
                    Value::Float(f) => {
                        match bind_data_type {
                            DataType::Double => args.add(*f as f64),
                            DataType::Decimal { .. } | DataType::Numeric { .. } => args.add(*f as f64),
                            DataType::Json | DataType::Jsonb => {
                                let v = serde_json::Number::from_f64(*f as f64)
                                    .map(serde_json::Value::Number)
                                    .unwrap_or(serde_json::Value::String(f.to_string()));
                                args.add(v)
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(f.to_string())
                            }
                            DataType::Integer | DataType::Serial => args.add(f.trunc() as i32),
                            DataType::BigInt | DataType::BigSerial => args.add(f.trunc() as i64),
                            DataType::SmallInt => args.add(f.trunc() as i16),
                            _ => args.add(*f),
                        }
                    }
                    Value::Double(d) => {
                        match bind_data_type {
                            DataType::Float | DataType::Real => args.add(*d as f32),
                            DataType::Decimal { .. } | DataType::Numeric { .. } => args.add(*d),
                            DataType::Json | DataType::Jsonb => {
                                let v = serde_json::Number::from_f64(*d)
                                    .map(serde_json::Value::Number)
                                    .unwrap_or(serde_json::Value::String(d.to_string()));
                                args.add(v)
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(d.to_string())
                            }
                            DataType::Integer | DataType::Serial => args.add(d.trunc() as i32),
                            DataType::BigInt | DataType::BigSerial => args.add(d.trunc() as i64),
                            DataType::SmallInt => args.add(d.trunc() as i16),
                            DataType::Boolean => args.add(*d != 0.0),
                            _ => args.add(*d),
                        }
                    }
                    Value::Decimal(s) => {
                        // 根据目标列类型决定如何绑定十进制字符串
                        match bind_data_type {
                            DataType::TinyInt | DataType::SmallInt | DataType::Integer | DataType::BigInt => {
                                // 尝试解析为整数
                                if let Ok(i) = s.parse::<i64>() {
                                    match bind_data_type {
                                        DataType::TinyInt => args.add(i as i16),
                                        DataType::SmallInt => args.add(i as i16),
                                        DataType::Integer => args.add(i as i32),
                                        DataType::BigInt => args.add(i),
                                        _ => unreachable!(),
                                    }
                                } else if let Ok(f) = s.parse::<f64>() {
                                    // 包含小数点，截断为整数（警告）
                                    let i = f.trunc() as i64;
                                    log::warn!("十进制值 '{}' 被截断为整数 {}", s, i);
                                    match bind_data_type {
                                        DataType::TinyInt => args.add(i as i16),
                                        DataType::SmallInt => args.add(i as i16),
                                        DataType::Integer => args.add(i as i32),
                                        DataType::BigInt => args.add(i),
                                        _ => unreachable!(),
                                    }
                                } else {
                                    // PG 二进制协议不接受 text → integer 转换，降级为 NULL
                                    log::warn!("[PG目标] Decimal 值 '{}' 无法解析为数字，降级为 NULL", s);
                                    args.add::<Option<i64>>(None)
                                }
                            }
                            DataType::Float | DataType::Real => {
                                if let Ok(f) = s.parse::<f32>() {
                                    args.add(f)
                                } else if let Ok(f) = s.parse::<f64>() {
                                    args.add(f as f32)
                                } else {
                                    log::warn!("[PG目标] Decimal 值 '{}' 无法解析为浮点数，降级为 NULL", s);
                                    args.add::<Option<f32>>(None)
                                }
                            }
                            DataType::Double => {
                                if let Ok(f) = s.parse::<f64>() {
                                    args.add(f)
                                } else {
                                    log::warn!("[PG目标] Decimal 值 '{}' 无法解析为双精度，降级为 NULL", s);
                                    args.add::<Option<f64>>(None)
                                }
                            }
                            DataType::Decimal { .. } | DataType::Numeric { .. } => {
                                // PG 二进制协议不接受 text → numeric 转换
                                // 解析为 f64，PG 会自动做 double precision → numeric 隐式转换
                                if let Ok(f) = s.parse::<f64>() {
                                    args.add(f)
                                } else {
                                    log::warn!("[PG目标] Decimal 值 '{}' 无法解析为数值，降级为 NULL", s);
                                    args.add::<Option<f64>>(None)
                                }
                            }
                            _ => {
                                // 其他未知类型，尝试解析为 f64 或 NULL
                                if let Ok(f) = s.parse::<f64>() {
                                    args.add(f)
                                } else {
                                    log::warn!("[PG目标] Decimal 值 '{}' 无法解析，降级为 NULL", s);
                                    args.add::<Option<f64>>(None)
                                }
                            }
                        }
                    }
                    Value::String(s) => {
                        // 根据目标列类型决定如何绑定字符串
                        match bind_data_type {
                            DataType::DateTime { .. } | DataType::Timestamp { with_tz: false, .. } => {
                                // 尝试解析为 NaiveDateTime (timestamp without time zone)
                                let parsed = Self::parse_naive_datetime(&s);
                                match parsed {
                                    Some(dt) => args.add::<chrono::NaiveDateTime>(dt),
                                    None => {
                                        // 回退：尝试解析为纯日期，补 00:00:00
                                        if let Ok(d) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
                                            let dt = d.and_hms_opt(0, 0, 0).unwrap();
                                            log::warn!("[PG目标] 字符串 '{}' 无时间部分，转为时间戳 {} (目标列类型: {:?})", s, dt, bind_data_type);
                                            args.add::<chrono::NaiveDateTime>(dt)
                                        } else {
                                            // PG 二进制协议不接受 text → timestamp 转换，降级为 NULL
                                            log::warn!("[PG目标] 字符串 '{}' 无法解析为时间戳，降级为 NULL (目标列类型: {:?})", s, bind_data_type);
                                            args.add::<Option<chrono::NaiveDateTime>>(None)
                                        }
                                    }
                                }
                            }
                            DataType::Timestamp { with_tz: true, .. } => {
                                // 尝试解析为带时区的 DateTime (UTC)
                                // 先尝试 RFC3339 格式
                                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&s) {
                                    args.add::<chrono::DateTime<chrono::Utc>>(dt.with_timezone(&chrono::Utc))
                                } else if let Some(naive) = Self::parse_naive_datetime(&s) {
                                    // 从 NaiveDateTime 转为 DateTime<Utc>（假设为 UTC）
                                    let utc = chrono::Utc.from_utc_datetime(&naive);
                                    args.add::<chrono::DateTime<chrono::Utc>>(utc)
                                } else if let Ok(d) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
                                    // 回退：纯日期补 00:00:00 UTC
                                    let naive = d.and_hms_opt(0, 0, 0).unwrap();
                                    let utc = chrono::Utc.from_utc_datetime(&naive);
                                    log::warn!("[PG目标] 字符串 '{}' 无时间部分，转为带时区时间戳 {} (目标列类型: {:?})", s, utc, bind_data_type);
                                    args.add::<chrono::DateTime<chrono::Utc>>(utc)
                                } else {
                                    // PG 二进制协议不接受 text → timestamptz 转换，降级为 NULL
                                    log::warn!("[PG目标] 字符串 '{}' 无法解析为带时区时间戳，降级为 NULL", s);
                                    args.add::<Option<chrono::DateTime<chrono::Utc>>>(None)
                                }
                            }
                            DataType::Date => {
                                // 尝试解析为日期（支持多种格式）
                                if let Ok(d) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
                                    args.add::<chrono::NaiveDate>(d)
                                } else if let Ok(d) = chrono::NaiveDate::parse_from_str(&s, "%Y/%m/%d") {
                                    args.add::<chrono::NaiveDate>(d)
                                } else if let Some(dt) = Self::parse_naive_datetime(&s) {
                                    // 从 datetime 中提取日期
                                    args.add::<chrono::NaiveDate>(dt.date())
                                } else {
                                    // PG 二进制协议不接受 text → date 转换，降级为 NULL
                                    log::warn!("[PG目标] 字符串 '{}' 无法解析为日期，降级为 NULL", s);
                                    args.add::<Option<chrono::NaiveDate>>(None)
                                }
                            }
                            DataType::Time => {
                                // 尝试解析为时间（支持多种格式）
                                if let Ok(t) = chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S%.f") {
                                    args.add::<chrono::NaiveTime>(t)
                                } else if let Ok(t) = chrono::NaiveTime::parse_from_str(&s, "%H:%M") {
                                    args.add::<chrono::NaiveTime>(t)
                                } else if let Some(dt) = Self::parse_naive_datetime(&s) {
                                    // 从 datetime 中提取时间
                                    args.add::<chrono::NaiveTime>(dt.time())
                                } else {
                                    // PG 二进制协议不接受 text → time 转换，降级为 NULL
                                    log::warn!("[PG目标] 字符串 '{}' 无法解析为时间，降级为 NULL", s);
                                    args.add::<Option<chrono::NaiveTime>>(None)
                                }
                            }
                            DataType::Boolean => {
                                // 尝试将字符串转为布尔值
                                match s.to_lowercase().as_str() {
                                    "true" | "1" | "yes" | "on" => args.add(true),
                                    "false" | "0" | "no" | "off" | "" => args.add(false),
                                    _ => {
                                        // PG 二进制协议不接受 text → boolean 转换
                                        log::warn!("[PG目标] 字符串 '{}' 无法解析为布尔值，降级为 NULL", s);
                                        args.add::<Option<bool>>(None)
                                    }
                                }
                            }
                            DataType::TinyInt | DataType::SmallInt | DataType::Integer | DataType::BigInt |
                            DataType::Serial | DataType::BigSerial => {
                                // 尝试解析为整数
                                if let Ok(i) = s.parse::<i64>() {
                                    match bind_data_type {
                                        DataType::TinyInt | DataType::SmallInt => args.add(i as i16),
                                        DataType::Integer | DataType::Serial => args.add(i as i32),
                                        DataType::BigInt | DataType::BigSerial => args.add(i),
                                        _ => unreachable!(),
                                    }
                                } else if let Ok(f) = s.parse::<f64>() {
                                    // 尝试作为浮点数解析后截断
                                    let i = f.trunc() as i64;
                                    log::warn!("[PG目标] 字符串 '{}' 解析为浮点后截断为整数 {}", s, i);
                                    match bind_data_type {
                                        DataType::TinyInt | DataType::SmallInt => args.add(i as i16),
                                        DataType::Integer | DataType::Serial => args.add(i as i32),
                                        DataType::BigInt | DataType::BigSerial => args.add(i),
                                        _ => unreachable!(),
                                    }
                                } else {
                                    // PG 二进制协议不接受 text → integer 转换
                                    log::warn!("[PG目标] 字符串 '{}' 无法解析为整数，降级为 NULL（目标列类型: {:?}）", s, bind_data_type);
                                    args.add::<Option<i64>>(None)
                                }
                            }
                            DataType::Float | DataType::Real => {
                                if let Ok(f) = s.parse::<f32>() {
                                    args.add(f)
                                } else if let Ok(f) = s.parse::<f64>() {
                                    args.add(f as f32)
                                } else {
                                    log::warn!("[PG目标] 字符串 '{}' 无法解析为浮点数，降级为 NULL", s);
                                    args.add::<Option<f32>>(None)
                                }
                            }
                            DataType::Double => {
                                if let Ok(f) = s.parse::<f64>() {
                                    args.add(f)
                                } else {
                                    log::warn!("[PG目标] 字符串 '{}' 无法解析为双精度浮点数，降级为 NULL", s);
                                    args.add::<Option<f64>>(None)
                                }
                            }
                            DataType::Decimal { .. } | DataType::Numeric { .. } => {
                                // PG 二进制协议不接受 text → numeric 转换
                                // 解析为 f64，PG 会自动做 double precision → numeric 隐式转换
                                if let Ok(f) = s.parse::<f64>() {
                                    args.add(f)
                                } else {
                                    log::warn!("[PG目标] 字符串 '{}' 无法解析为数值，降级为 NULL（目标列类型: {:?}）", s, bind_data_type);
                                    args.add::<Option<f64>>(None)
                                }
                            }
                            DataType::Json | DataType::Jsonb => {
                                // 字符串绑定到 json/jsonb 列需要解析为 serde_json::Value
                                match serde_json::from_str::<serde_json::Value>(s) {
                                    Ok(v) => args.add(v),
                                    Err(_) => {
                                        log::warn!("[PG目标] 字符串绑定到 JSON 列但无法解析，包装为字符串: {}", s);
                                        args.add(serde_json::Value::String(s.clone()))
                                    }
                                }
                            }
                            DataType::Uuid => {
                                // 字符串绑定到 uuid 列需要解析为 uuid::Uuid
                                match uuid::Uuid::parse_str(s) {
                                    Ok(u) => args.add(u),
                                    Err(_) => {
                                        log::warn!("[PG目标] 字符串绑定到 UUID 列但无法解析: {}", s);
                                        args.add(s.clone())
                                    }
                                }
                            }
                            _ => args.add(s.clone()), // 其他类型直接作为字符串传递
                        }
                    }
                    Value::Bytes(b) => {
                        match bind_data_type {
                            DataType::Json | DataType::Jsonb => {
                                // 二进制 → JSON：尝试 UTF-8 解码后解析，否则降级 NULL
                                match String::from_utf8(b.clone()) {
                                    Ok(s) => match serde_json::from_str::<serde_json::Value>(&s) {
                                        Ok(v) => args.add(v),
                                        Err(_) => {
                                            log::warn!("[PG目标] 二进制数据转 JSON 失败，包装为字符串");
                                            args.add(serde_json::Value::String(s))
                                        }
                                    },
                                    Err(_) => {
                                        log::warn!("[PG目标] 二进制数据无法解码为 UTF-8，降级为 NULL（目标: JSON）");
                                        args.add::<Option<serde_json::Value>>(None)
                                    }
                                }
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                // 二进制 → 文本：尝试 UTF-8 解码
                                match String::from_utf8(b.clone()) {
                                    Ok(s) => args.add(s),
                                    Err(_) => {
                                        log::warn!("[PG目标] 二进制数据无法解码为文本，降级为 NULL");
                                        args.add::<Option<String>>(None)
                                    }
                                }
                            }
                            _ => args.add(b.clone()),
                        }
                    }
                    Value::Date(d) => {
                        match bind_data_type {
                            DataType::Json | DataType::Jsonb => {
                                args.add(serde_json::Value::String(d.to_string()))
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(d.to_string())
                            }
                            DataType::DateTime { .. } | DataType::Timestamp { with_tz: false, .. } => {
                                let dt = d.and_hms_opt(0, 0, 0).unwrap();
                                args.add::<chrono::NaiveDateTime>(dt)
                            }
                            DataType::Timestamp { with_tz: true, .. } => {
                                let dt = d.and_hms_opt(0, 0, 0).unwrap();
                                let utc = chrono::Utc.from_utc_datetime(&dt);
                                args.add::<chrono::DateTime<chrono::Utc>>(utc)
                            }
                            _ => args.add::<chrono::NaiveDate>(*d),
                        }
                    }
                    Value::Time(t) => {
                        match bind_data_type {
                            DataType::Json | DataType::Jsonb => {
                                args.add(serde_json::Value::String(t.to_string()))
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(t.to_string())
                            }
                            _ => args.add::<chrono::NaiveTime>(*t),
                        }
                    }
                    Value::DateTime(dt) => {
                        // 根据目标列类型决定绑定类型
                        match bind_data_type {
                            DataType::Timestamp { with_tz: true, .. } => {
                                // 目标列是带时区的时间戳，将 NaiveDateTime 转换为 DateTime<Utc>（假设为 UTC）
                                let utc = chrono::Utc.from_utc_datetime(dt);
                                args.add::<chrono::DateTime<chrono::Utc>>(utc)
                            }
                            DataType::Json | DataType::Jsonb => {
                                args.add(serde_json::Value::String(dt.to_string()))
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(dt.to_string())
                            }
                            DataType::Date => {
                                args.add::<chrono::NaiveDate>(dt.date())
                            }
                            _ => {
                                // 其他情况（包括 DateTime 不带时区）直接使用 NaiveDateTime
                                args.add::<chrono::NaiveDateTime>(*dt)
                            }
                        }
                    }
                    Value::Timestamp(ts) => {
                        // 根据目标列类型决定绑定类型
                        match bind_data_type {
                            DataType::DateTime { .. } => {
                                // 目标列是不带时区的时间戳，转换为 NaiveDateTime
                                let naive = ts.naive_utc();
                                args.add::<chrono::NaiveDateTime>(naive)
                            }
                            DataType::Timestamp { with_tz: true, .. } => {
                                // 目标列是带时区的时间戳，直接使用 DateTime<Utc>
                                args.add::<chrono::DateTime<chrono::Utc>>(*ts)
                            }
                            DataType::Timestamp { with_tz: false, .. } => {
                                // 目标列是不带时区的时间戳，转换为 NaiveDateTime
                                let naive = ts.naive_utc();
                                args.add::<chrono::NaiveDateTime>(naive)
                            }
                            DataType::Json | DataType::Jsonb => {
                                args.add(serde_json::Value::String(ts.to_rfc3339()))
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(ts.to_rfc3339())
                            }
                            DataType::Date => {
                                args.add::<chrono::NaiveDate>(ts.naive_utc().date())
                            }
                            _ => {
                                // 其他类型，默认按 DateTime<Utc> 处理
                                args.add::<chrono::DateTime<chrono::Utc>>(*ts)
                            }
                        }
                    }
                    Value::Json(s) => {
                        // PostgreSQL json 列期望 serde_json::Value，不能绑字符串
                        match bind_data_type {
                            DataType::Jsonb => {
                                // json → jsonb：解析后重新绑
                                match serde_json::from_str::<serde_json::Value>(s) {
                                    Ok(v) => args.add(v),
                                    Err(_) => {
                                        log::warn!("[PG目标] JSON 值无效，包装为字符串: {}", s);
                                        args.add(serde_json::Value::String(s.clone()))
                                    }
                                }
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                // json → text：直接传字符串
                                args.add(s.clone())
                            }
                            _ => {
                                // 默认按 json 处理
                                match serde_json::from_str::<serde_json::Value>(s) {
                                    Ok(v) => args.add(v),
                                    Err(_) => {
                                        log::warn!("[PG目标] JSON 值无效，包装为字符串: {}", s);
                                        args.add(serde_json::Value::String(s.clone()))
                                    }
                                }
                            }
                        }
                    }
                    Value::Jsonb(s) => {
                        // PostgreSQL jsonb 列同样期望 serde_json::Value
                        match bind_data_type {
                            DataType::Json => {
                                // jsonb → json：解析后绑
                                match serde_json::from_str::<serde_json::Value>(s) {
                                    Ok(v) => args.add(v),
                                    Err(_) => {
                                        log::warn!("[PG目标] JSONB 值无效，包装为字符串: {}", s);
                                        args.add(serde_json::Value::String(s.clone()))
                                    }
                                }
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(s.clone())
                            }
                            _ => {
                                match serde_json::from_str::<serde_json::Value>(s) {
                                    Ok(v) => args.add(v),
                                    Err(_) => {
                                        log::warn!("[PG目标] JSONB 值无效，包装为字符串: {}", s);
                                        args.add(serde_json::Value::String(s.clone()))
                                    }
                                }
                            }
                        }
                    }
                    Value::Uuid(s) => {
                        // PostgreSQL uuid 列期望 uuid::Uuid 类型，不能绑字符串
                        match bind_data_type {
                            DataType::Json | DataType::Jsonb => {
                                // uuid → json：包装为字符串值
                                args.add(serde_json::Value::String(s.clone()))
                            }
                            DataType::Text | DataType::MediumText | DataType::LongText |
                            DataType::VarChar { .. } | DataType::Char { .. } => {
                                args.add(s.clone())
                            }
                            _ => {
                                // 默认按 UUID 类型处理
                                match uuid::Uuid::parse_str(s) {
                                    Ok(u) => args.add(u),
                                    Err(_) => {
                                        log::warn!("[PG目标] UUID 值无效，作为字符串传递: {}", s);
                                        args.add(s.clone())
                                    }
                                }
                            }
                        }
                    }
                }.map_err(|e| format!("绑定参数失败: {}", e))?;
            }
            
            log::debug!("[PG执行] SQL: {} 参数: {:?}", insert_sql, args);
            let result = sqlx::query_with(&insert_sql, args)
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
            let affected = result.rows_affected();
            if affected == 0 {
                log::warn!("[PG执行] 表 {} 行未插入（ON CONFLICT DO NOTHING 跳过）", table);
            }
            inserted += affected as usize;
        }

        log::info!("[PG目标] 表 public.{} 插入 {} 行 （数据库: {}）", table, inserted, database);
        Ok(inserted)
    }

    async fn truncate_table(
        &self,
        database: &str,
        table: &str,
    ) -> Result<(), String> {
        // 使用 public.table 全限定名
        let sql = format!("TRUNCATE TABLE public.\"{}\" CASCADE", table);
        log::info!("[PG目标] 清空表数据: {} （数据库: {}）", sql, database);
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("清空表 {} 数据失败: {}", table, e))?;
        Ok(())
    }
}