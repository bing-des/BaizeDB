use serde::Serialize;

/// 数据库元信息
#[derive(Debug, Serialize)]
pub struct DatabaseMeta {
    pub name: String,
}

/// Schema 元信息（PG 专有）
#[derive(Debug, Serialize)]
pub struct SchemaMeta {
    pub name: String,
}

/// 表元信息
#[derive(Debug, Serialize)]
pub struct TableMeta {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_type: Option<String>,
    /// MySQL: 行数估算; PG: None (需额外查询)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<i64>,
}

/// 列元信息
#[derive(Debug, Serialize)]
pub struct ColumnMeta {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// 查询结果集
#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    /// SELECT 查询时为 None，DML 时为受影响行数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_rows: Option<u64>,
    /// 执行耗时（毫秒）
    pub execution_time_ms: u64,
    /// 错误信息（SQL 语法错误等，非连接错误——连接错误用 Err 返回）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 总行数（仅 get_table_data 分页查询时使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
}

/// 数据库操作 trait — 屏蔽 MySQL/PG 差异，统一接口
///
/// # Pool 参数
/// - MySQL: 直接使用主连接池（一个连接对应一个数据库）
/// - PostgreSQL: 需要传入**目标数据库的连接池**
///   （PG 主连接池连到默认库，切换 database 需要独立连接）
#[allow(unused_variables)]
pub trait DbOps: Send + Sync {
    /// 获取所有数据库/Schema 列表
    async fn list_databases(&self) -> Result<Vec<DatabaseMeta>, String>;

    /// 获取指定数据库下的 Schema 列表（MySQL 返回空数组）
    async fn list_schemas(&self, database: &str) -> Result<Vec<SchemaMeta>, String>;

    /// 获取表列表
    async fn list_tables(&self, database: &str, schema: Option<&str>) -> Result<Vec<TableMeta>, String>;

    /// 获取列信息
    async fn list_columns(&self, database: &str, table: &str) -> Result<Vec<ColumnMeta>, String>;

    /// 获取表数据（分页）
    async fn get_table_data(
        &self,
        database: &str,
        table: &str,
        page: i64,
        page_size: i64,
    ) -> Result<QueryResult, String>;

    /// 获取表行数
    async fn get_row_count(&self, database: &str, table: &str) -> Result<i64, String>;

    /// 执行 SQL 查询并返回结果集
    async fn query_sql(&self, sql: &str) -> Result<QueryResult, String>;

    /// 执行 SQL（无返回值）
    async fn execute_sql(&self, sql: &str) -> Result<u64, String>;
}
