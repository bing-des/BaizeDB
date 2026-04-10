//! MySQL 数据目标实现

use async_trait::async_trait;
use sqlx::{MySqlPool, Arguments};
use sqlx::mysql::MySqlArguments;
use crate::state::DbPool;

use super::types::*;

/// MySQL 数据目标
pub struct MySQLTarget {
    pool: MySqlPool,
}

impl MySQLTarget {
    /// 从 DbPool 创建 MySQL 目标（必须是 MySQL 类型）
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

    /// 将中间层 DataType 转换为 MySQL 类型字符串
    fn intermediate_to_mysql_type(data_type: &DataType) -> String {
        match data_type {
            DataType::TinyInt => "TINYINT".to_string(),
            DataType::SmallInt => "SMALLINT".to_string(),
            DataType::Integer => "INT".to_string(),
            DataType::BigInt => "BIGINT".to_string(),
            DataType::Decimal { precision, scale } => {
                match (precision, scale) {
                    (Some(p), Some(s)) => format!("DECIMAL({},{})", p, s),
                    (Some(p), None) => format!("DECIMAL({})", p),
                    _ => "DECIMAL".to_string(),
                }
            }
            DataType::Numeric { precision, scale } => {
                match (precision, scale) {
                    (Some(p), Some(s)) => format!("NUMERIC({},{})", p, s),
                    (Some(p), None) => format!("NUMERIC({})", p),
                    _ => "NUMERIC".to_string(),
                }
            }
            DataType::Float => "FLOAT".to_string(),
            DataType::Double => "DOUBLE".to_string(),
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
            DataType::MediumText => "MEDIUMTEXT".to_string(),
            DataType::LongText => "LONGTEXT".to_string(),
            DataType::Binary { length } => {
                match length {
                    Some(l) => format!("BINARY({})", l),
                    None => "BINARY".to_string(),
                }
            }
            DataType::VarBinary { length } => {
                match length {
                    Some(l) => format!("VARBINARY({})", l),
                    None => "VARBINARY".to_string(),
                }
            }
            DataType::Blob => "BLOB".to_string(),
            DataType::MediumBlob => "MEDIUMBLOB".to_string(),
            DataType::LongBlob => "LONGBLOB".to_string(),
            DataType::Bytea => "BLOB".to_string(), // MySQL 没有 BYTEA，用 BLOB 替代
            DataType::Date => "DATE".to_string(),
            DataType::Time => "TIME".to_string(),
            DataType::DateTime { precision } => {
                match precision {
                    Some(p) => format!("DATETIME({})", p),
                    None => "DATETIME".to_string(),
                }
            }
            DataType::Timestamp { precision, with_tz: _ } => {
                match precision {
                    Some(p) => format!("TIMESTAMP({})", p),
                    None => "TIMESTAMP".to_string(),
                }
            }
            DataType::Year => "YEAR".to_string(),
            DataType::Json => "JSON".to_string(),
            DataType::Jsonb => "JSON".to_string(), // MySQL 没有 JSONB，用 JSON 替代
            DataType::Boolean => "BOOLEAN".to_string(),
            DataType::Uuid => "CHAR(36)".to_string(), // UUID 存储为字符串
            DataType::Serial => "INT AUTO_INCREMENT".to_string(),
            DataType::BigSerial => "BIGINT AUTO_INCREMENT".to_string(),
            DataType::Unknown(typ) => typ.clone(),
        }
    }
}

#[async_trait]
impl DataTarget for MySQLTarget {
    fn target_type(&self) -> &'static str {
        "mysql"
    }

    async fn create_table(&self, database: &str, schema: &TableSchema) -> Result<(), String> {
        let column_defs: Vec<String> = schema.columns.iter().map(|col| {
            let mysql_type = Self::intermediate_to_mysql_type(&col.data_type);
            let null_clause = if col.nullable { "" } else { " NOT NULL" };
            let default_clause = col.default_value.as_ref().map(|d| format!(" DEFAULT {}", d)).unwrap_or_default();
            format!("`{}` {}{}{}", col.name, mysql_type, null_clause, default_clause)
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

        let create_table_sql = format!("CREATE TABLE IF NOT EXISTS `{}`.`{}` ({}{});", 
            database, schema.name, column_defs.join(", "), pk_clause);
        
        sqlx::query(&create_table_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        
        Ok(())
    }

    #[allow(unused_variables)]
    async fn add_column_comment(
        &self,
        database: &str,
        table: &str,
        column: &str,
        comment: &str,
    ) -> Result<(), String> {
        // MySQL 列注释在创建表时通过 COMMENT 子句添加，或者通过 ALTER TABLE 修改
        // 这里使用 ALTER TABLE 添加注释
        // 暂时跳过列注释实现，因为需要查询现有列类型
        // 为了简化，先记录日志并返回成功
        log::warn!("MySQL 列注释功能暂未实现（表 {}.{} 列 {}）", database, table, column);
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

        // 构建 INSERT 语句模板
        let column_names: Vec<String> = schema.columns.iter().map(|c| format!("`{}`", c.name)).collect();
        let placeholders: Vec<String> = vec!["?".to_string(); column_names.len()];
        let insert_sql = format!(
            "INSERT IGNORE INTO `{}`.`{}` ({}) VALUES ({})",
            database,
            table,
            column_names.join(", "),
            placeholders.join(", ")
        );

        let mut inserted = 0;
        for row in rows {
            let mut args = MySqlArguments::default();
            
            for (i, (col, value)) in schema.columns.iter().zip(row.values.iter()).enumerate() {
                log::debug!(
                    "[MySQL绑定] 表={} 列[{}]='{}' 源类型={:?} 值={:?}",
                    table, i, col.name, col.data_type, value
                );
                match value {
                    Value::Null => args.add::<Option<String>>(None),
                    Value::Bool(b) => args.add(*b),
                    Value::TinyInt(i) => args.add(*i),
                    Value::SmallInt(i) => args.add(*i),
                    Value::Integer(i) => args.add(*i),
                    Value::BigInt(i) => args.add(*i),
                    Value::Float(f) => args.add(*f),
                    Value::Double(d) => args.add(*d),
                    Value::Decimal(s) => args.add(s.clone()), // 作为字符串传递，MySQL 会转换
                    Value::String(s) => args.add(s.clone()),
                    Value::Bytes(b) => args.add(b.as_slice()),
                    Value::Date(d) => args.add(*d),
                    Value::Time(t) => args.add(*t),
                    Value::DateTime(dt) => args.add(*dt),
                    Value::Timestamp(ts) => args.add(ts.naive_utc()), // 转换为 naive datetime
                    Value::Json(s) => args.add(s.clone()),
                    Value::Jsonb(s) => args.add(s.clone()),
                    Value::Uuid(s) => args.add(s.clone()),
                }.map_err(|e| format!("绑定参数失败: {}", e))?;
            }
            
            let result = sqlx::query_with(&insert_sql, args)
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
            let affected = result.rows_affected();
            if affected == 0 {
                log::warn!("[MySQL执行] 表 {} 行未插入（INSERT IGNORE 跳过）", table);
            }
            inserted += affected as usize;
        }

        Ok(inserted)
    }

    async fn truncate_table(
        &self,
        database: &str,
        table: &str,
    ) -> Result<(), String> {
        let sql = format!("TRUNCATE TABLE `{}`.`{}`", database, table);
        log::info!("[MySQL目标] 清空表数据: {}", sql);
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("清空表 {} 数据失败: {}", table, e))?;
        Ok(())
    }
}