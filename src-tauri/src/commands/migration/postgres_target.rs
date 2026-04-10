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
        let column_defs: Vec<String> = schema.columns.iter().map(|col| {
            let pg_type = Self::intermediate_to_postgres_type(&col.data_type);
            let null_clause = if col.nullable { "" } else { " NOT NULL" };
            let default_clause = col.default_value.as_ref().map(|d| format!(" DEFAULT {}", d)).unwrap_or_default();
            format!("{} {}{}{}", col.name, pg_type, null_clause, default_clause)
        }).collect();

        let primary_keys: Vec<String> = schema.columns.iter()
            .filter(|c| c.is_primary_key)
            .map(|c| c.name.clone())
            .collect();

        let pk_clause = if !primary_keys.is_empty() {
            format!(", PRIMARY KEY ({})", primary_keys.join(", "))
        } else {
            String::new()
        };

        // 使用 public.table 全限定名（连接池已连到目标数据库）
        let create_table_sql = format!("CREATE TABLE IF NOT EXISTS public.{} ({}{});", 
            schema.name, column_defs.join(", "), pk_clause);
        
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
        let comment_sql = format!("COMMENT ON COLUMN public.{}.{} IS '{}';", table, column, escaped_comment);
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

        // 构建 INSERT 语句模板（使用源表列名，因为 INSERT 语句必须匹配表定义）
        let column_names: Vec<String> = schema.columns.iter().map(|c| c.name.clone()).collect();
        let placeholders: Vec<String> = (1..=column_names.len()).map(|i| format!("${}", i)).collect();
        // 使用 public.table 全限定名
        let insert_sql = format!(
            "INSERT INTO public.{} ({}) VALUES ({}) ON CONFLICT DO NOTHING",
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
                
                log::debug!(
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
                            DataType::Decimal { .. } | DataType::Numeric { .. } => args.add::<Option<String>>(None),
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
                    Value::Bool(b) => args.add(*b),
                    Value::TinyInt(i) => {
                        // 根据目标列类型提升，避免二进制格式不匹配
                        match bind_data_type {
                            DataType::Boolean => args.add(*i != 0),
                            DataType::SmallInt => args.add(*i as i16),
                            DataType::Integer => args.add(*i as i32),
                            DataType::BigInt | DataType::Serial | DataType::BigSerial => args.add(*i as i64),
                            _ => args.add(*i as i16), // PostgreSQL 没有 tinyint，默认用 smallint
                        }
                    }
                    Value::SmallInt(i) => {
                        match bind_data_type {
                            DataType::Integer => args.add(*i as i32),
                            DataType::BigInt | DataType::Serial | DataType::BigSerial => args.add(*i as i64),
                            _ => args.add(*i),
                        }
                    }
                    Value::Integer(i) => {
                        match bind_data_type {
                            DataType::BigInt | DataType::Serial | DataType::BigSerial => args.add(*i as i64),
                            DataType::SmallInt => args.add(*i as i16),
                            _ => args.add(*i),
                        }
                    }
                    Value::BigInt(i) => args.add(*i),
                    Value::Float(f) => {
                        match bind_data_type {
                            DataType::Double => args.add(*f as f64),
                            _ => args.add(*f),
                        }
                    }
                    Value::Double(d) => args.add(*d),
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
                                } else {
                                    // 如果包含小数点，尝试解析为浮点数然后截断（警告）
                                    if let Ok(f) = s.parse::<f64>() {
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
                                        return Err(format!("无法将字符串 '{}' 解析为目标整数类型 {:?}", s, bind_data_type));
                                    }
                                }
                            }
                            DataType::Float | DataType::Real => {
                                if let Ok(f) = s.parse::<f32>() {
                                    args.add(f)
                                } else {
                                    return Err(format!("无法将字符串 '{}' 解析为浮点数", s));
                                }
                            }
                            DataType::Double => {
                                if let Ok(f) = s.parse::<f64>() {
                                    args.add(f)
                                } else {
                                    return Err(format!("无法将字符串 '{}' 解析为双精度浮点数", s));
                                }
                            }
                            DataType::Decimal { .. } | DataType::Numeric { .. } => {
                                // 对于 PostgreSQL 的 decimal/numeric，直接传递字符串，PostgreSQL 会处理转换
                                args.add(s.clone())
                            }
                            _ => args.add(s.clone()), // 其他类型直接作为字符串传递
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
                                    None => return Err(format!("无法将字符串 '{}' 解析为时间戳 (目标列类型: {:?})", s, bind_data_type)),
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
                                } else {
                                    return Err(format!("无法将字符串 '{}' 解析为带时区的时间戳", s));
                                }
                            }
                            DataType::Date => {
                                // 尝试解析为日期
                                if let Ok(d) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
                                    args.add::<chrono::NaiveDate>(d)
                                } else {
                                    return Err(format!("无法将字符串 '{}' 解析为日期", s));
                                }
                            }
                            DataType::Time => {
                                // 尝试解析为时间（支持带毫秒和不带毫秒）
                                if let Ok(t) = chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S%.f") {
                                    args.add::<chrono::NaiveTime>(t)
                                } else {
                                    return Err(format!("无法将字符串 '{}' 解析为时间", s));
                                }
                            }
                            DataType::Boolean => {
                                // 尝试将字符串转为布尔值
                                match s.to_lowercase().as_str() {
                                    "true" | "1" | "yes" | "on" => args.add(true),
                                    "false" | "0" | "no" | "off" | "" => args.add(false),
                                    _ => return Err(format!("无法将字符串 '{}' 解析为布尔值", s)),
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
                                } else {
                                    return Err(format!("无法将字符串 '{}' 解析为整数", s));
                                }
                            }
                            DataType::Float | DataType::Real => {
                                if let Ok(f) = s.parse::<f32>() {
                                    args.add(f)
                                } else {
                                    return Err(format!("无法将字符串 '{}' 解析为浮点数", s));
                                }
                            }
                            DataType::Double => {
                                if let Ok(f) = s.parse::<f64>() {
                                    args.add(f)
                                } else {
                                    return Err(format!("无法将字符串 '{}' 解析为双精度浮点数", s));
                                }
                            }
                            DataType::Decimal { .. } | DataType::Numeric { .. } => {
                                // 对于 PostgreSQL 的 decimal/numeric，直接传递字符串
                                args.add(s.clone())
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
                    Value::Bytes(b) => args.add(b.clone()),
                    Value::Date(d) => args.add::<chrono::NaiveDate>(*d),
                    Value::Time(t) => args.add::<chrono::NaiveTime>(*t),
                    Value::DateTime(dt) => {
                        // 根据目标列类型决定绑定类型
                        match bind_data_type {
                            DataType::Timestamp { with_tz: true, .. } => {
                                // 目标列是带时区的时间戳，将 NaiveDateTime 转换为 DateTime<Utc>（假设为 UTC）
                                let utc = chrono::Utc.from_utc_datetime(dt);
                                args.add::<chrono::DateTime<chrono::Utc>>(utc)
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
                            _ => {
                                // 其他类型，默认按 DateTime<Utc> 处理
                                args.add::<chrono::DateTime<chrono::Utc>>(*ts)
                            }
                        }
                    }
                    Value::Json(s) => {
                        // PostgreSQL json 列期望 serde_json::Value，不能绑字符串
                        match serde_json::from_str::<serde_json::Value>(s) {
                            Ok(v) => args.add(v),
                            Err(_) => {
                                // 无效 JSON，包装为 JSON 字符串值
                                log::warn!("[PG目标] JSON 值无效，包装为字符串: {}", s);
                                args.add(serde_json::Value::String(s.clone()))
                            }
                        }
                    }
                    Value::Jsonb(s) => {
                        // PostgreSQL jsonb 列同样期望 serde_json::Value
                        match serde_json::from_str::<serde_json::Value>(s) {
                            Ok(v) => args.add(v),
                            Err(_) => {
                                log::warn!("[PG目标] JSONB 值无效，包装为字符串: {}", s);
                                args.add(serde_json::Value::String(s.clone()))
                            }
                        }
                    }
                    Value::Uuid(s) => {
                        // PostgreSQL uuid 列期望 uuid::Uuid 类型，不能绑字符串
                        match uuid::Uuid::parse_str(s) {
                            Ok(u) => args.add(u),
                            Err(_) => {
                                log::warn!("[PG目标] UUID 值无效，作为字符串传递: {}", s);
                                args.add(s.clone())
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
        let sql = format!("TRUNCATE TABLE public.{} CASCADE", table);
        log::info!("[PG目标] 清空表数据: {} （数据库: {}）", sql, database);
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("清空表 {} 数据失败: {}", table, e))?;
        Ok(())
    }
}