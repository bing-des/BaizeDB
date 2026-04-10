//! MySQL 数据源实现

use async_trait::async_trait;
use serde::de::value;
use sqlx::{MySqlPool, Row, ValueRef};
use crate::state::DbPool;

use super::types::*;

/// MySQL 数据源
pub struct MySQLDataSource {
    pool: MySqlPool,
}

impl MySQLDataSource {
    /// 从 DbPool 创建 MySQL 数据源（必须是 MySQL 类型）
    pub fn from_pool(pool: &DbPool) -> Result<Self, String> {
        match pool {
            DbPool::MySQL(p) => Ok(Self { pool: p.clone() }),
            _ => Err("不是 MySQL 连接池".to_string()),
        }
    }

    /// 从 MySqlPool 直接创建
    #[allow(dead_code)]
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    /// 将 MySQL 类型字符串转换为中间层 DataType
    fn mysql_type_to_intermediate(mysql_type: &str, type_info: Option<&str>) -> DataType {
        let lower = mysql_type.to_lowercase();
        match lower.as_str() {
            "tinyint" => {
                // 检查是否为 TINYINT(1)，如果是则视为布尔类型
                if let Some(info) = type_info {
                    if let Some(len_str) = info.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
                        if let Ok(len) = len_str.parse::<u32>() {
                            if len == 1 {
                                return DataType::Boolean;
                            }
                        }
                    }
                }
                DataType::TinyInt
            }
            "smallint" => DataType::SmallInt,
            "int" | "integer" | "mediumint" => DataType::Integer,
            "bigint" => DataType::BigInt,
            "decimal" => {
                let (precision, scale) = parse_precision_scale(type_info);
                DataType::Decimal { precision, scale }
            }
            "numeric" => {
                let (precision, scale) = parse_precision_scale(type_info);
                DataType::Numeric { precision, scale }
            }
            "float" => DataType::Float,
            "double" => DataType::Double,
            "real" => DataType::Real,
            "char" => {
                let length = parse_length(type_info);
                DataType::Char { length }
            }
            "varchar" => {
                let length = parse_length(type_info);
                DataType::VarChar { length }
            }
            "text" => DataType::Text,
            "mediumtext" => DataType::MediumText,
            "longtext" => DataType::LongText,
            "binary" => {
                let length = parse_length(type_info);
                DataType::Binary { length }
            }
            "varbinary" => {
                let length = parse_length(type_info);
                DataType::VarBinary { length }
            }
            "blob" => DataType::Blob,
            "mediumblob" => DataType::MediumBlob,
            "longblob" => DataType::LongBlob,
            "date" => DataType::Date,
            "time" => DataType::Time,
            "datetime" => {
                let precision = parse_precision(type_info);
                DataType::DateTime { precision }
            }
            "timestamp" => {
                let precision = parse_precision(type_info);
                DataType::Timestamp { precision, with_tz: false }
            }
            "year" => DataType::Integer, // YEAR 类型映射为整数，因为 PostgreSQL 没有 YEAR 类型
            "json" => DataType::Json,
            "boolean" => DataType::Boolean,
            _ => DataType::Unknown(mysql_type.to_string()),
        }
    }


}

/// 解析精度和小数位数（例如 decimal(10,2)）
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

/// 解析精度（例如 datetime(6)）
fn parse_precision(type_info: Option<&str>) -> Option<u32> {
    parse_length(type_info)
}

#[async_trait]
impl DataSource for MySQLDataSource {
    fn source_type(&self) -> &'static str {
        "mysql"
    }

    async fn list_tables(&self, database: &str) -> Result<Vec<String>, String> {
        let sql = "SELECT CAST(TABLE_NAME AS CHAR) FROM information_schema.TABLES WHERE TABLE_SCHEMA = ?";
        let rows = sqlx::query_as::<_, (String,)>(sql)
            .bind(database)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn get_table_schema(&self, database: &str, table: &str) -> Result<TableSchema, String> {
        let sql = format!(
            "SELECT CAST(COLUMN_NAME AS CHAR), CAST(DATA_TYPE AS CHAR), \
            CAST(IS_NULLABLE AS CHAR), CAST(COALESCE(COLUMN_KEY, '') AS CHAR), \
            CAST(COALESCE(COLUMN_DEFAULT, '') AS CHAR), \
            CAST(COALESCE(COLUMN_COMMENT, '') AS CHAR), \
            CAST(COALESCE(COLUMN_TYPE, '') AS CHAR) \
            FROM information_schema.COLUMNS \
            WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' ORDER BY ORDINAL_POSITION",
            database, table
        );
        
        let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        
        let columns: Vec<ColumnDef> = rows.into_iter().map(|r| {
            let (name, data_type, nullable, key, default_value, comment, column_type) = r;
            
            // 使用 COLUMN_TYPE（例如 int(11), varchar(255)）来获取完整类型信息
            let data_type = Self::mysql_type_to_intermediate(&data_type, Some(&column_type));
            
            ColumnDef {
                name,
                data_type,
                nullable: nullable == "YES",
                is_primary_key: key.contains("PRI"),
                default_value: if default_value.is_empty() { None } else { Some(default_value) },
                comment: if comment.is_empty() { None } else { Some(comment) },
            }
        }).collect();
        
        Ok(TableSchema {
            name: table.to_string(),
            columns,
        })
    }

    async fn read_table_data(
        &self,
        database: &str,
        table: &str,
        schema: &TableSchema,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<DataRow>, String> {
        // 读取数据行
        let sql = format!("SELECT * FROM `{}`.`{}` LIMIT {} OFFSET {}", database, table, limit, offset);
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
                log::debug!("[MySQL源] 读取列 '{}', 分类 '{:?}'", col.name, category);
                let value: Option<Value> = match category {
                    DataTypeCategory::Integer => {
                        if let Ok(v) = row.try_get::<Option<i8>, _>(i) {
                            v.map(|x: i8| Value::TinyInt(x))
                        } else if let Ok(v) = row.try_get::<Option<i16>, _>(i) {
                            v.map(|x: i16| Value::SmallInt(x))
                        } else if let Ok(v) = row.try_get::<Option<i32>, _>(i) {
                            v.map(|x: i32| Value::Integer(x))
                        } else if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
                            v.map(|x: i64| Value::BigInt(x))
                        } else if let Ok(v) = row.try_get::<Option<u64>, _>(i) {
                            v.map(|x: u64| Value::BigInt(x as i64))
                        }else {
                            None
                        }
                    }
                    DataTypeCategory::Float => {
                        if let Ok(v) = row.try_get::<Option<f32>, _>(i) {
                            v.map(|x: f32| Value::Float(x))
                        } else if let Ok(v) = row.try_get::<Option<f64>, _>(i) {
                            v.map(|x: f64| Value::Double(x))
                        } else if let Ok(v) = row.try_get::<Option<String>, _>(i) {
                            // decimal 类型可能以字符串形式返回
                            v.map(|x: String| Value::Decimal(x))
                        } else {
                            None
                        }
                    }
                    DataTypeCategory::Boolean => {
                        if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
                            v.map(|x: bool| Value::Bool(x))
                        } else if let Ok(v) = row.try_get::<Option<i8>, _>(i) {
                            v.map(|n: i8| Value::Bool(n != 0))
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
                        if let Ok(v) = row.try_get::<Option<chrono::NaiveDateTime>, _>(i) {
                            v.map(|x: chrono::NaiveDateTime| Value::DateTime(x))
                        } else if let Ok(v) = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(i) {
                            v.map(|x: chrono::DateTime<chrono::Utc>| Value::Timestamp(x))
                        } else if let Ok(v) = row.try_get::<Option<chrono::NaiveTime>, _>(i) {
                            v.map(|x: chrono::NaiveTime| Value::Time(x))
                        } else if let Ok(v) = row.try_get::<Option<String>, _>(i) {
                            // MySQL datetime 有时以字符串形式返回
                            log::warn!("[MySQL源] 列 '{}' datetime 类型值以字符串形式返回: {:?}", col.name, v);
                            v.map(|x: String| Value::String(x))
                        } else {
                            log::warn!("[MySQL源] 列 '{}' datetime 类型值无法提取", col.name);
                            None
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
                        } else if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(i) {
                            v.map(|x: Vec<u8>| Value::Bytes(x))
                        } else {
                            None
                        }
                    }
                    DataTypeCategory::Json => {
                        // MySQL JSON 类型以字符串形式返回，需要验证 JSON 格式是否正确
                        if let Ok(v) = row.try_get::<Option<serde_json::Value>, _>(i) {
                            v.map(|x| Value::Json(x.to_string()))
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