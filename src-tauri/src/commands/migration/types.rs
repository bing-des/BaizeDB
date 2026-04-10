//! 迁移模块类型定义
//! 
//! 定义中间层数据类型和转换 trait，支持任意数据库之间的迁移

use serde::{Deserialize, Serialize};

/// 迁移输入参数
#[derive(Debug, Deserialize)]
pub struct MigrationInput {
    pub source_connection_id: String,
    pub target_connection_id: String,
    pub source_database: String,
    pub target_database: Option<String>,
    pub tables: Option<Vec<String>>,
    pub migrate_structure: Option<bool>,
    pub migrate_data: Option<bool>,
    pub truncate_target: Option<bool>,
    pub batch_size: Option<usize>,
}

/// 迁移进度
#[derive(Debug, Clone, Serialize)]
pub struct MigrationProgress {
    /// 迁移任务唯一 ID
    pub migration_id: String,
    pub current_table: String,
    pub total_tables: usize,
    pub tables_completed: usize,
    pub rows_migrated: usize,
    /// 当前表已迁移行数（用于单表进度）
    pub current_table_rows: usize,
    pub status: MigrationStatus,
    /// 错误信息（仅 Failed 状态时有值）
    pub error: Option<String>,
}

/// 迁移状态（简单枚举，前端可直接用字符串匹配）
#[derive(Debug, Clone, Serialize, PartialEq)]
#[allow(dead_code)]
pub enum MigrationStatus {
    NotStarted,
    Preparing,
    MigratingStructure,
    MigratingData,
    Completed,
    Failed,
}

/// 中间层列信息（数据库无关）
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

/// 中间层数据类型枚举
/// 
/// 这是迁移的中间表示，所有数据库类型都先映射到这里，
/// 然后再从这里映射到目标数据库类型
#[derive(Debug, Clone)]
pub enum DataType {
    // 整数类型
    TinyInt,
    SmallInt,
    Integer,
    BigInt,
    
    // 精确小数
    Decimal { precision: Option<u32>, scale: Option<u32> },
    Numeric { precision: Option<u32>, scale: Option<u32> },
    
    // 浮点数
    Float,
    Double,
    Real,
    
    // 字符串类型
    Char { length: Option<u32> },
    VarChar { length: Option<u32> },
    Text,
    MediumText,
    LongText,
    
    // 二进制类型
    Binary { length: Option<u32> },
    VarBinary { length: Option<u32> },
    Blob,
    MediumBlob,
    LongBlob,
    Bytea,
    
    // 日期时间类型
    Date,
    Time,
    DateTime { precision: Option<u32> },
    Timestamp { precision: Option<u32>, with_tz: bool },
    Year,
    
    // JSON 类型
    Json,
    Jsonb,
    
    // 布尔类型
    Boolean,
    
    // 特殊类型
    Uuid,
    Serial,
    BigSerial,
    
    // 未知/自定义类型
    Unknown(String),
}

impl DataType {
    /// 获取类型的通用分类（用于数据提取时决定如何解析）
    pub fn category(&self) -> DataTypeCategory {
        match self {
            DataType::TinyInt | DataType::SmallInt | DataType::Integer | 
            DataType::BigInt | DataType::Serial | DataType::BigSerial => {
                DataTypeCategory::Integer
            }
            DataType::Decimal { .. } | DataType::Numeric { .. } |
            DataType::Float | DataType::Double | DataType::Real => {
                DataTypeCategory::Float
            }
            DataType::Boolean => DataTypeCategory::Boolean,
            DataType::Date => DataTypeCategory::Date,
            DataType::Time | DataType::DateTime { .. } | DataType::Timestamp { .. } | DataType::Year => {
                DataTypeCategory::DateTime
            }
            DataType::Binary { .. } | DataType::VarBinary { .. } | 
            DataType::Blob | DataType::MediumBlob | DataType::LongBlob | DataType::Bytea => {
                DataTypeCategory::Binary
            }
            _ => DataTypeCategory::Text,
        }
    }
}

/// 数据类型分类（用于数据提取）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataTypeCategory {
    Integer,
    Float,
    Boolean,
    Date,
    DateTime,
    Binary,
    Text,
}

/// 中间层数据值（数据库无关）
/// 
/// 这是迁移的数据中间表示，所有数据库的数据都先转换到这里，
/// 然后再从这里转换到目标数据库
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Value {
    Null,
    Bool(bool),
    TinyInt(i8),
    SmallInt(i16),
    Integer(i32),
    BigInt(i64),
    Float(f32),
    Double(f64),
    Decimal(String), // 高精度小数用字符串表示避免精度丢失
    String(String),
    Bytes(Vec<u8>),
    Date(chrono::NaiveDate),
    Time(chrono::NaiveTime),
    DateTime(chrono::NaiveDateTime),
    Timestamp(chrono::DateTime<chrono::Utc>),
    Json(String),
    Jsonb(String),
    Uuid(String),
}

/// 表结构定义
#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnDef>,
}

/// 数据行
#[derive(Debug, Clone)]
pub struct DataRow {
    pub values: Vec<Value>,
}

/// 数据源 trait
/// 
/// 实现此 trait 来支持从特定数据库读取数据
#[async_trait::async_trait]
pub trait DataSource: Send + Sync {
    /// 获取数据源类型名称
    fn source_type(&self) -> &'static str;
    
    /// 列出所有表
    async fn list_tables(&self, database: &str) -> Result<Vec<String>, String>;
    
    /// 获取表结构
    async fn get_table_schema(&self, database: &str, table: &str) -> Result<TableSchema, String>;
    
    /// 读取表数据（分页）
    async fn read_table_data(
        &self,
        database: &str,
        table: &str,
        schema: &TableSchema,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<DataRow>, String>;
}

/// 数据目标 trait
/// 
/// 实现此 trait 来支持写入数据到特定数据库
#[async_trait::async_trait]
pub trait DataTarget: Send + Sync {
    /// 获取目标类型名称
    fn target_type(&self) -> &'static str;
    
    /// 创建表（如果不存在）
    async fn create_table(&self, database: &str, schema: &TableSchema) -> Result<(), String>;
    
    /// 添加列注释
    async fn add_column_comment(
        &self,
        database: &str,
        table: &str,
        column: &str,
        comment: &str,
    ) -> Result<(), String>;
    
    /// 插入数据行
    async fn insert_rows(
        &self,
        database: &str,
        table: &str,
        schema: &TableSchema,
        rows: &[DataRow],
    ) -> Result<usize, String>;

    /// 清空目标表数据（TRUNCATE）
    async fn truncate_table(
        &self,
        database: &str,
        table: &str,
    ) -> Result<(), String>;
}

/// 类型转换器 trait
/// 
/// 实现此 trait 来支持特定数据库类型到中间层的双向转换
#[allow(dead_code)]
pub trait TypeConverter {
    /// 将数据库特定类型转换为中间层类型
    fn to_intermediate(&self, db_type: &str, type_info: Option<&str>) -> DataType;
    
    /// 将中间层类型转换为目标数据库类型
    fn from_intermediate(&self, data_type: &DataType) -> String;
}

/// 值转换器 trait
/// 
/// 实现此 trait 来支持特定数据库值到中间层的双向转换
#[allow(dead_code)]
pub trait ValueConverter {
    /// 将数据库特定值转换为中间层值
    fn to_intermediate(&self, value: &dyn std::any::Any, target_type: &DataType) -> Value;
    
    /// 将中间层值转换为目标数据库值
    fn from_intermediate(&self, value: &Value) -> Box<dyn std::any::Any>;
}
