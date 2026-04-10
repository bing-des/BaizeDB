//! PostgreSQL 数据源实现

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use crate::state::DbPool;

use super::types::*;
/// PostgreSQL 数据源
pub struct PostgreSQLDataSource {
    pool: PgPool,
}

impl PostgreSQLDataSource {
    /// 从 DbPool 创建 PostgreSQL 数据源（必须是 PostgreSQL 类型）
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
                let (precision, scale) = parse_precision_scale(type_info);
                DataType::Decimal { precision, scale }
            }
            "real" | "float4" => DataType::Float,
            "double precision" | "float8" => DataType::Double,
            "character" | "char" => {
                let length = parse_length(type_info);
                DataType::Char { length }
            }
            "character varying" | "varchar" => {
                let length = parse_length(type_info);
                DataType::VarChar { length }
            }
            "text" => DataType::Text,
            "bytea" => DataType::Bytea,
            "date" => DataType::Date,
            "time" | "time without time zone" => DataType::Time,
            "timestamp" | "timestamp without time zone" => {
                let precision = parse_precision(type_info);
                DataType::DateTime { precision }
            }
            "timestamp with time zone" | "timestamptz" => {
                let precision = parse_precision(type_info);
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
    parse_length(type_info)
}

#[async_trait]
impl DataSource for PostgreSQLDataSource {
    fn source_type(&self) -> &'static str {
        "postgresql"
    }

    async fn list_tables(&self, database: &str) -> Result<Vec<String>, String> {
        let sql = "SELECT table_name FROM information_schema.tables WHERE table_catalog = $1 AND table_schema NOT IN ('pg_catalog', 'information_schema') ORDER BY table_name";
        let rows = sqlx::query_as::<_, (String,)>(sql)
            .bind(database)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn get_table_schema(&self, database: &str, table: &str) -> Result<TableSchema, String> {
        // 直接查询 information_schema.columns 获取列信息
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
        
        let rows = sqlx::query_as::<_, (String, String, String, String, String, String, Option<i32>, Option<i32>, Option<i32>, Option<i32>)>(sql)
            .bind(database)
            .bind(table)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        
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
        
        Ok(TableSchema {
            name: table.to_string(),
            columns,
        })
    }

    async fn read_table_data(
        &self,
        _database: &str,
        table: &str,
        schema: &TableSchema,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<DataRow>, String> {
        // 连接池已直接连到源数据库，无需 SET search_path
        // 使用 public.table 全限定名读取数据
        let sql = format!("SELECT * FROM public.{} LIMIT {} OFFSET {}", table, limit, offset);
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        // 转换为中间层 DataRow
        let mut data_rows = Vec::with_capacity(rows.len());
        for row in rows {
            let mut values = Vec::with_capacity(schema.columns.len());
            for (i, col) in schema.columns.iter().enumerate() {
                let category = col.data_type.category();
                let value = match category {
                    DataTypeCategory::Integer => {
                        if let Ok(v) = row.try_get::<Option<i16>, _>(i) {
                            v.map(|x: i16| Value::SmallInt(x))
                        } else if let Ok(v) = row.try_get::<Option<i32>, _>(i) {
                            v.map(|x: i32| Value::Integer(x))
                        } else if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
                            v.map(|x: i64| Value::BigInt(x))
                        } else {
                            None
                        }
                    }
                    DataTypeCategory::Float => {
                        if let Ok(v) = row.try_get::<Option<f32>, _>(i) {
                            v.map(|x: f32| Value::Float(x))
                        } else if let Ok(v) = row.try_get::<Option<f64>, _>(i) {
                            v.map(|x: f64| Value::Double(x))
                        } else if let Ok(v) = row.try_get::<Option<String>, _>(i) {
                            // numeric/decimal 可能以字符串形式返回
                            v.map(|x: String| Value::Decimal(x))
                        } else {
                            None
                        }
                    }
                    DataTypeCategory::Boolean => {
                        if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
                            v.map(|x: bool| Value::Bool(x))
                        } else {
                            None
                        }
                    }
                    DataTypeCategory::Date => {
                        if let Ok(v) = row.try_get::<Option<chrono::NaiveDate>, _>(i) {
                            v.map(|x: chrono::NaiveDate| Value::Date(x))
                        } else {
                            None
                        }
                    }
                    DataTypeCategory::DateTime => {
                        // 检查是否是带时区的时间戳
                        match &col.data_type {
                            DataType::Timestamp { with_tz: true, .. } => {
                                // 带时区的时间戳
                                if let Ok(v) = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(i) {
                                    v.map(|x: chrono::DateTime<chrono::Utc>| Value::Timestamp(x))
                                } else if let Ok(v) = row.try_get::<Option<chrono::NaiveDateTime>, _>(i) {
                                    v.map(|x: chrono::NaiveDateTime| Value::DateTime(x))
                                } else if let Ok(v) = row.try_get::<Option<String>, _>(i) {
                                    log::warn!("[PG源] 列 '{}' timestamptz 值以字符串形式返回: {:?}", col.name, v);
                                    v.map(|x: String| Value::String(x))
                                } else {
                                    None
                                }
                            }
                            _ => {
                                // 不带时区的时间戳
                                if let Ok(v) = row.try_get::<Option<chrono::NaiveDateTime>, _>(i) {
                                    v.map(|x: chrono::NaiveDateTime| Value::DateTime(x))
                                } else if let Ok(v) = row.try_get::<Option<chrono::NaiveTime>, _>(i) {
                                    v.map(|x: chrono::NaiveTime| Value::Time(x))
                                } else if let Ok(v) = row.try_get::<Option<String>, _>(i) {
                                    log::warn!("[PG源] 列 '{}' timestamp 值以字符串形式返回: {:?}", col.name, v);
                                    v.map(|x: String| Value::String(x))
                                } else {
                                    None
                                }
                            }
                        }
                    }
                    DataTypeCategory::Binary => {
                        if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(i) {
                            v.map(|x: Vec<u8>| Value::Bytes(x))
                        } else {
                            None
                        }
                    }
                    DataTypeCategory::Text => {
                        if let Ok(v) = row.try_get::<Option<String>, _>(i) {
                            v.map(|x: String| Value::String(x))
                        } else {
                            None
                        }
                    },
                    DataTypeCategory::Json => {
                        // MySQL JSON 类型以字符串形式返回，需要验证 JSON 格式是否正确
                        if let Ok(v) = row.try_get::<Option<serde_json::Value>, _>(i) {
                            v.map(|x: serde_json::Value| Value::Json(x.to_string()))
                        } else {
                            None
                        }
                    }
                };
                values.push(value.unwrap_or(Value::Null));
            }
            data_rows.push(DataRow { values });
        }

        Ok(data_rows)
    }
}