use crate::database::db_ops::{DbOps, QueryResult};
use sqlx::{MySqlPool, PgPool};
use std::sync::Arc;

macro_rules! impl_dbops_for_arc_pool {
    ($pool:ty) => {
        impl DbOps for Arc<$pool> {
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
            async fn get_table_data(
                &self, database: &str, table: &str, page: i64, page_size: i64,
                sort_by: Option<String>, sort_order: Option<String>,
                filters: Option<std::collections::HashMap<String, String>>,
            ) -> Result<QueryResult, String> {
                DbOps::get_table_data(self.as_ref(), database, table, page, page_size, sort_by, sort_order, filters).await
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
            async fn update_row(
                &self, database: &str, table: &str, primary_key: &str, primary_key_type: &str,
                primary_key_value: serde_json::Value,
                column_values: std::collections::HashMap<String, serde_json::Value>,
                column_types: std::collections::HashMap<String, String>,
            ) -> Result<u64, String> {
                DbOps::update_row(self.as_ref(), database, table, primary_key, primary_key_type, primary_key_value, column_values, column_types).await
            }
            async fn delete_row(
                &self, database: &str, table: &str, primary_key: &str, primary_key_type: &str,
                primary_key_value: serde_json::Value,
            ) -> Result<u64, String> {
                DbOps::delete_row(self.as_ref(), database, table, primary_key, primary_key_type, primary_key_value).await
            }
            async fn insert_row(
                &self, database: &str, table: &str,
                column_values: std::collections::HashMap<String, serde_json::Value>,
                column_types: std::collections::HashMap<String, String>,
            ) -> Result<u64, String> {
                DbOps::insert_row(self.as_ref(), database, table, column_values, column_types).await
            }
            fn is_postgres(&self) -> bool {
                DbOps::is_postgres(self.as_ref())
            }
            async fn drop_database(&self, database_name: &str) -> Result<u64, String> {
                DbOps::drop_database(self.as_ref(), database_name).await
            }
            async fn drop_table(&self, database: &str, table: &str, schema: Option<&str>) -> Result<u64, String> {
                DbOps::drop_table(self.as_ref(), database, table, schema).await
            }
            async fn add_column(
                &self, database: &str, table: &str, column_name: &str, column_type: &str,
                nullable: bool, default_value: Option<&str>, comment: Option<&str>,
            ) -> Result<(), String> {
                DbOps::add_column(self.as_ref(), database, table, column_name, column_type, nullable, default_value, comment).await
            }
            async fn drop_column(&self, database: &str, table: &str, column_name: &str) -> Result<(), String> {
                DbOps::drop_column(self.as_ref(), database, table, column_name).await
            }
            async fn modify_column(
                &self, database: &str, table: &str, old_name: &str, new_name: &str, column_type: &str,
                nullable: bool, default_value: Option<&str>, comment: Option<&str>,
            ) -> Result<(), String> {
                DbOps::modify_column(self.as_ref(), database, table, old_name, new_name, column_type, nullable, default_value, comment).await
            }
        }
    };
}

impl_dbops_for_arc_pool!(MySqlPool);
impl_dbops_for_arc_pool!(PgPool);

/// 统一的数据库操作句柄（enum + Arc，避免 dyn Trait 生命周期问题）
pub enum AnyDbPool {
    MySQL(Arc<MySqlPool>),
    PG(Arc<PgPool>),
}

macro_rules! impl_any_db_pool {
    () => {
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

            pub async fn get_table_data(
                &self, database: &str, table: &str, page: i64, page_size: i64,
                sort_by: Option<String>, sort_order: Option<String>,
                filters: Option<std::collections::HashMap<String, String>>,
            ) -> Result<QueryResult, String> {
                match self {
                    AnyDbPool::MySQL(p) => p.get_table_data(database, table, page, page_size, sort_by, sort_order, filters).await,
                    AnyDbPool::PG(p) => p.get_table_data(database, table, page, page_size, sort_by, sort_order, filters).await,
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

            pub async fn update_row(
                &self, database: &str, table: &str, primary_key: &str, primary_key_type: &str,
                primary_key_value: serde_json::Value,
                column_values: std::collections::HashMap<String, serde_json::Value>,
                column_types: std::collections::HashMap<String, String>,
            ) -> Result<u64, String> {
                match self {
                    AnyDbPool::MySQL(p) => p.update_row(database, table, primary_key, primary_key_type, primary_key_value, column_values, column_types).await,
                    AnyDbPool::PG(p) => p.update_row(database, table, primary_key, primary_key_type, primary_key_value, column_values, column_types).await,
                }
            }

            pub async fn delete_row(
                &self, database: &str, table: &str, primary_key: &str, primary_key_type: &str,
                primary_key_value: serde_json::Value,
            ) -> Result<u64, String> {
                match self {
                    AnyDbPool::MySQL(p) => p.delete_row(database, table, primary_key, primary_key_type, primary_key_value).await,
                    AnyDbPool::PG(p) => p.delete_row(database, table, primary_key, primary_key_type, primary_key_value).await,
                }
            }

            pub async fn insert_row(
                &self, database: &str, table: &str,
                column_values: std::collections::HashMap<String, serde_json::Value>,
                column_types: std::collections::HashMap<String, String>,
            ) -> Result<u64, String> {
                match self {
                    AnyDbPool::MySQL(p) => p.insert_row(database, table, column_values, column_types).await,
                    AnyDbPool::PG(p) => p.insert_row(database, table, column_values, column_types).await,
                }
            }

            pub fn is_postgres(&self) -> bool {
                matches!(self, AnyDbPool::PG(_))
            }

            pub async fn drop_database(&self, database_name: &str) -> Result<u64, String> {
                match self {
                    AnyDbPool::MySQL(p) => p.drop_database(database_name).await,
                    AnyDbPool::PG(p) => p.drop_database(database_name).await,
                }
            }

            pub async fn drop_table(&self, database: &str, table: &str, schema: Option<&str>) -> Result<u64, String> {
                match self {
                    AnyDbPool::MySQL(p) => p.drop_table(database, table, schema).await,
                    AnyDbPool::PG(p) => p.drop_table(database, table, schema).await,
                }
            }

            pub async fn add_column(
                &self, database: &str, table: &str, column_name: &str, column_type: &str,
                nullable: bool, default_value: Option<&str>, comment: Option<&str>,
            ) -> Result<(), String> {
                match self {
                    AnyDbPool::MySQL(p) => p.add_column(database, table, column_name, column_type, nullable, default_value, comment).await,
                    AnyDbPool::PG(p) => p.add_column(database, table, column_name, column_type, nullable, default_value, comment).await,
                }
            }

            pub async fn drop_column(&self, database: &str, table: &str, column_name: &str) -> Result<(), String> {
                match self {
                    AnyDbPool::MySQL(p) => p.drop_column(database, table, column_name).await,
                    AnyDbPool::PG(p) => p.drop_column(database, table, column_name).await,
                }
            }

            pub async fn modify_column(
                &self, database: &str, table: &str, old_name: &str, new_name: &str, column_type: &str,
                nullable: bool, default_value: Option<&str>, comment: Option<&str>,
            ) -> Result<(), String> {
                match self {
                    AnyDbPool::MySQL(p) => p.modify_column(database, table, old_name, new_name, column_type, nullable, default_value, comment).await,
                    AnyDbPool::PG(p) => p.modify_column(database, table, old_name, new_name, column_type, nullable, default_value, comment).await,
                }
            }
        }
    };
}

impl_any_db_pool!();
