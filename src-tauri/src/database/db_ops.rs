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
    pub column_types: Option<Vec<String>>,
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
#[allow(async_fn_in_trait)]
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
        sort_by: Option<String>,
        sort_order: Option<String>,
        filters: Option<std::collections::HashMap<String, String>>,
    ) -> Result<QueryResult, String>;

    /// 获取表行数
    async fn get_row_count(&self, database: &str, table: &str) -> Result<i64, String>;

    /// 执行 SQL 查询并返回结果集
    async fn query_sql(&self, sql: &str) -> Result<QueryResult, String>;

    /// 执行 SQL（无返回值）
    async fn execute_sql(&self, sql: &str) -> Result<u64, String>;

    /// 更新单行数据（根据主键定位行，更新指定列的值）
    ///
    /// # 参数
    /// - `database`: 数据库名
    /// - `table`: 表名
    /// - `primary_key`: 主键列名（用于 WHERE 定位）
    /// - `primary_key_value`: 主键值
    /// - `column_values`: 要更新的列名和值
    async fn update_row(
        &self,
        database: &str,
        table: &str,
        primary_key: &str,
        primary_key_type: &str,
        primary_key_value: serde_json::Value,
        column_values: std::collections::HashMap<String, serde_json::Value>,
        column_types: std::collections::HashMap<String, String>,
    ) -> Result<u64, String>;

    /// 删除单行数据（根据主键定位）
    async fn delete_row(
        &self,
        database: &str,
        table: &str,
        primary_key: &str,
        primary_key_type: &str,
        primary_key_value: serde_json::Value,
    ) -> Result<u64, String>;

    /// 插入一行新数据
    ///
    /// # 参数
    /// - `column_values`: 要插入的列名和值
    /// - `column_types`: 列名→PG类型名的映射（如 {"role_id":"bigint","name":"varchar"}），
    ///   PG 二进制协议需要此信息来决定参数绑定方式
    async fn insert_row(
        &self,
        database: &str,
        table: &str,
        column_values: std::collections::HashMap<String, serde_json::Value>,
        column_types: std::collections::HashMap<String, String>,
    ) -> Result<u64, String>;

    /// 是否为 PostgreSQL（用于区分 SQL 语法差异）
    fn is_postgres(&self) -> bool;

    /// 删除数据库（DROP DATABASE）
    async fn drop_database(&self, database_name: &str) -> Result<u64, String>;

    /// 删除表（DROP TABLE）
    async fn drop_table(&self, database: &str, table: &str, schema: Option<&str>) -> Result<u64, String>;

    // ─────────── 表结构管理 ───────────

    /// 新增列（ALTER TABLE ... ADD COLUMN）
    ///
    /// # 参数
    /// - `database`: 数据库名
    /// - `table`: 表名（PG 格式：schema.table 或 table）
    /// - `column_name`: 新列名
    /// - `column_type`: 列类型字符串（如 `VARCHAR(255)`, `INT`, `TEXT`）
    /// - `nullable`: 是否允许 NULL
    /// - `default_value`: 可选的默认值表达式字符串
    /// - `comment`: 可选注释（MySQL 专有；PG 通过 COMMENT ON 实现）
    async fn add_column(
        &self,
        database: &str,
        table: &str,
        column_name: &str,
        column_type: &str,
        nullable: bool,
        default_value: Option<&str>,
        comment: Option<&str>,
    ) -> Result<(), String>;

    /// 删除列（ALTER TABLE ... DROP COLUMN）
    async fn drop_column(
        &self,
        database: &str,
        table: &str,
        column_name: &str,
    ) -> Result<(), String>;

    /// 修改列定义（ALTER TABLE ... MODIFY COLUMN / ALTER COLUMN）
    ///
    /// # 参数
    /// - `old_name`: 原列名
    /// - `new_name`: 新列名（若不改名，传与 old_name 相同的值）
    /// - `column_type`: 新类型字符串
    /// - `nullable`: 是否允许 NULL
    /// - `default_value`: 新默认值（None 表示 DROP DEFAULT）
    /// - `comment`: 新注释
    async fn modify_column(
        &self,
        database: &str,
        table: &str,
        old_name: &str,
        new_name: &str,
        column_type: &str,
        nullable: bool,
        default_value: Option<&str>,
        comment: Option<&str>,
    ) -> Result<(), String>;
}
