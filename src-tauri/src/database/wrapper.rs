use std::sync::Arc;
use sqlx::{MySqlPool, PgPool};
use crate::database::db_ops::{DbOps, QueryResult};

// ─── Arc<MySqlPool> 委托给 MySqlPool 的 DbOps impl ───
impl DbOps for Arc<MySqlPool> {
    async fn list_databases(&self) -> Result<Vec<crate::database::db_ops::DatabaseMeta>, String> {
        DbOps::list_databases(self.as_ref()).await
    }
    async fn list_schemas(&self, database: &str) -> Result<Vec<crate::database::db_ops::SchemaMeta>, String> {
        DbOps::list_schemas(self.as_ref(), database).await
    }
    async fn list_tables(&self, database: &str, schema: Option<&str>) -> Result<Vec<crate::database::db_ops::TableMeta>, String> {
        DbOps::list_tables(self.as_ref(), database, schema).await
    }
    async fn list_columns(&self, database: &str, table: &str) -> Result<Vec<crate::database::db_ops::ColumnMeta>, String> {
        DbOps::list_columns(self.as_ref(), database, table).await
    }
    async fn get_table_data(&self, database: &str, table: &str, page: i64, page_size: i64) -> Result<QueryResult, String> {
        DbOps::get_table_data(self.as_ref(), database, table, page, page_size).await
    }
    async fn get_row_count(&self, database: &str, table: &str) -> Result<i64, String> {
        DbOps::get_row_count(self.as_ref(), database, table).await
    }
    async fn query_sql(&self, sql: &str) -> Result<QueryResult, String> {
        DbOps::query_sql(self.as_ref(), sql).await
    }
    async fn execute_sql(&self, sql: &str) -> Result<u64, String> {
        DbOps::execute_sql(self.as_ref(), sql).await
    }
}

// ─── Arc<PgPool> 委托给 PgPool 的 DbOps impl ───
impl DbOps for Arc<PgPool> {
    async fn list_databases(&self) -> Result<Vec<crate::database::db_ops::DatabaseMeta>, String> {
        DbOps::list_databases(self.as_ref()).await
    }
    async fn list_schemas(&self, database: &str) -> Result<Vec<crate::database::db_ops::SchemaMeta>, String> {
        DbOps::list_schemas(self.as_ref(), database).await
    }
    async fn list_tables(&self, database: &str, schema: Option<&str>) -> Result<Vec<crate::database::db_ops::TableMeta>, String> {
        DbOps::list_tables(self.as_ref(), database, schema).await
    }
    async fn list_columns(&self, database: &str, table: &str) -> Result<Vec<crate::database::db_ops::ColumnMeta>, String> {
        DbOps::list_columns(self.as_ref(), database, table).await
    }
    async fn get_table_data(&self, database: &str, table: &str, page: i64, page_size: i64) -> Result<QueryResult, String> {
        DbOps::get_table_data(self.as_ref(), database, table, page, page_size).await
    }
    async fn get_row_count(&self, database: &str, table: &str) -> Result<i64, String> {
        DbOps::get_row_count(self.as_ref(), database, table).await
    }
    async fn query_sql(&self, sql: &str) -> Result<QueryResult, String> {
        DbOps::query_sql(self.as_ref(), sql).await
    }
    async fn execute_sql(&self, sql: &str) -> Result<u64, String> {
        DbOps::execute_sql(self.as_ref(), sql).await
    }
}

/// 统一的数据库操作句柄（enum + Arc，避免 dyn Trait 生命周期问题）
pub enum AnyDbPool {
    MySQL(Arc<MySqlPool>),
    PG(Arc<PgPool>),
}

impl AnyDbPool {
    pub async fn list_databases(&self) -> Result<Vec<crate::database::db_ops::DatabaseMeta>, String> {
        match self {
            AnyDbPool::MySQL(p) => p.list_databases().await,
            AnyDbPool::PG(p) => p.list_databases().await,
        }
    }

    pub async fn list_schemas(&self, database: &str) -> Result<Vec<crate::database::db_ops::SchemaMeta>, String> {
        match self {
            AnyDbPool::MySQL(p) => p.list_schemas(database).await,
            AnyDbPool::PG(p) => p.list_schemas(database).await,
        }
    }

    pub async fn list_tables(&self, database: &str, schema: Option<&str>) -> Result<Vec<crate::database::db_ops::TableMeta>, String> {
        match self {
            AnyDbPool::MySQL(p) => p.list_tables(database, schema).await,
            AnyDbPool::PG(p) => p.list_tables(database, schema).await,
        }
    }

    pub async fn list_columns(&self, database: &str, table: &str) -> Result<Vec<crate::database::db_ops::ColumnMeta>, String> {
        match self {
            AnyDbPool::MySQL(p) => p.list_columns(database, table).await,
            AnyDbPool::PG(p) => p.list_columns(database, table).await,
        }
    }

    pub async fn get_table_data(&self, database: &str, table: &str, page: i64, page_size: i64) -> Result<QueryResult, String> {
        match self {
            AnyDbPool::MySQL(p) => p.get_table_data(database, table, page, page_size).await,
            AnyDbPool::PG(p) => p.get_table_data(database, table, page, page_size).await,
        }
    }

    pub async fn get_row_count(&self, database: &str, table: &str) -> Result<i64, String> {
        match self {
            AnyDbPool::MySQL(p) => p.get_row_count(database, table).await,
            AnyDbPool::PG(p) => p.get_row_count(database, table).await,
        }
    }

    pub async fn query_sql(&self, sql: &str) -> Result<QueryResult, String> {
        match self {
            AnyDbPool::MySQL(p) => p.query_sql(sql).await,
            AnyDbPool::PG(p) => p.query_sql(sql).await,
        }
    }

    pub async fn execute_sql(&self, sql: &str) -> Result<u64, String> {
        match self {
            AnyDbPool::MySQL(p) => p.execute_sql(sql).await,
            AnyDbPool::PG(p) => p.execute_sql(sql).await,
        }
    }
}
