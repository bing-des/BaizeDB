// 子模块声明
#[path = "migration/types.rs"]
mod types;
#[path = "migration/mysql_source.rs"]
mod mysql_source;
#[path = "migration/postgres_target.rs"]
mod postgres_target;
#[path = "migration/postgres_source.rs"]
mod postgres_source;
#[path = "migration/mysql_target.rs"]
mod mysql_target;

use tauri::{State, AppHandle, Emitter};
use crate::state::{AppState, DbPool};
use crate::commands::database::ensure_pg_db_pool;
use mysql_source::MySQLDataSource;
use mysql_target::MySQLTarget;
use postgres_source::PostgreSQLDataSource;
use postgres_target::PostgreSQLTarget;
use types::{DataSource, DataTarget, MigrationInput, MigrationProgress, MigrationStatus, TableMapping};

/// 获取连接池辅助函数
async fn get_pool(state: &AppState, connection_id: &str) -> Result<DbPool, String> {
    let pools = state.pools.read().await;
    pools.get(connection_id).cloned().ok_or_else(|| "连接未激活".to_string())
}

/// 通用数据源工厂函数
async fn create_data_source(
    state: &AppState,
    connection_id: &str,
    database: &str,
) -> Result<Box<dyn DataSource + Send + Sync>, String> {
    // 对于 PostgreSQL 数据源，需要使用 db_pools（按目标数据库创建的连接池）
    let pool = {
        let pools = state.pools.read().await;
        match pools.get(connection_id) {
            Some(DbPool::PostgreSQL(_)) => {
                // PG：使用 db_pools 获取指定数据库的连接池
                drop(pools);
                let db_key = ensure_pg_db_pool(connection_id, database, state).await?;
                let db_pools = state.db_pools.read().await;
                db_pools.get(&db_key).cloned().ok_or_else(|| "PG 数据库连接池未找到".to_string())?
            }
            Some(_) => {
                // MySQL/Redis：直接使用主连接池
                pools.get(connection_id).cloned().unwrap()
            }
            None => return Err("连接未激活".to_string()),
        }
    };
    match pool {
        DbPool::MySQL(_) => Ok(Box::new(MySQLDataSource::from_pool(&pool)?)),
        DbPool::PostgreSQL(_) => Ok(Box::new(PostgreSQLDataSource::from_pool(&pool)?)),
        DbPool::Redis(_) => Err("Redis 不支持作为迁移源".to_string()),
    }
}

/// 通用数据目标工厂函数
/// 对于 PostgreSQL 目标，会为目标数据库创建独立的连接池（通过 ensure_pg_db_pool）
async fn create_data_target(
    state: &AppState,
    connection_id: &str,
    database: &str,
) -> Result<Box<dyn DataTarget + Send + Sync>, String> {
    // 先检查连接类型
    let db_type = {
        let pools = state.pools.read().await;
        let pool = pools.get(connection_id).ok_or("连接未激活")?;
        match pool {
            DbPool::MySQL(_) => crate::state::DbType::MySQL,
            DbPool::PostgreSQL(_) => crate::state::DbType::PostgreSQL,
            DbPool::Redis(_) => return Err("Redis 不支持作为迁移目标".to_string()),
        }
    };

    match db_type {
        crate::state::DbType::MySQL => {
            let pool = get_pool(state, connection_id).await?;
            Ok(Box::new(MySQLTarget::from_pool(&pool)?))
        }
        crate::state::DbType::PostgreSQL => {
            // PG：为目标数据库创建独立连接池，确保连接到正确的 database
            let db_key = ensure_pg_db_pool(connection_id, database, state).await?;
            let db_pools = state.db_pools.read().await;
            let pool = db_pools.get(&db_key).cloned().ok_or_else(|| "PG 数据库连接池未找到".to_string())?;
            Ok(Box::new(PostgreSQLTarget::from_pool(&pool)?))
        }
        _ => Err("不支持的目标数据库类型".to_string()),
    }
}

/// 启动迁移任务（异步执行，通过事件推送进度）
#[tauri::command]
pub async fn start_migration_v2(
    input: MigrationInput,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<String, String> {
    // 生成迁移任务 ID
    let migration_id = uuid::Uuid::new_v4().to_string();
    let mid = migration_id.clone();

    // 初始进度
    let initial_progress = MigrationProgress {
        migration_id: migration_id.clone(),
        current_table: String::new(),
        total_tables: 0,
        tables_completed: 0,
        rows_migrated: 0,
        current_table_rows: 0,
        status: MigrationStatus::Preparing,
        error: None,
    };
    let _ = app_handle.emit("migration-progress", &initial_progress);

    // 默认值
    let migrate_structure = input.migrate_structure.unwrap_or(true);
    let migrate_data = input.migrate_data.unwrap_or(true);
    let truncate_target = input.truncate_target.unwrap_or(false);
    let target_database = input.target_database.clone().unwrap_or_else(|| input.source_database.clone());
    let batch_size = input.batch_size.unwrap_or(1000);

    // 克隆 Arc 内部数据（AppState 内部都是 Arc<RwLock<...>>，clone 只是增加引用计数）
    let state_inner = AppState {
        connections: state.connections.clone(),
        pools: state.pools.clone(),
        db_pools: state.db_pools.clone(),
    };
    let app_handle_inner = app_handle.clone();

    tokio::spawn(async move {
        let result = run_migration(
            &state_inner,
            &app_handle_inner,
            &mid,
            input,
            migrate_structure,
            migrate_data,
            truncate_target,
            &target_database,
            batch_size,
        ).await;

        match result {
            Ok(final_progress) => {
                let _ = app_handle_inner.emit("migration-progress", &final_progress);
            }
            Err(e) => {
                let failed = MigrationProgress {
                    migration_id: mid.clone(),
                    current_table: String::new(),
                    total_tables: 0,
                    tables_completed: 0,
                    rows_migrated: 0,
                    current_table_rows: 0,
                    status: MigrationStatus::Failed,
                    error: Some(e),
                };
                let _ = app_handle_inner.emit("migration-progress", &failed);
            }
        }
    });

    Ok(migration_id)
}

/// 发送进度事件
fn emit_progress(app: &AppHandle, progress: &MigrationProgress) {
    let _ = app.emit("migration-progress", progress);
}

/// 实际执行迁移逻辑，返回最终的 MigrationProgress
async fn run_migration(
    state: &AppState,
    app: &AppHandle,
    migration_id: &str,
    input: MigrationInput,
    migrate_structure: bool,
    migrate_data: bool,
    truncate_target: bool,
    target_database: &str,
    batch_size: usize,
) -> std::result::Result<MigrationProgress, String> {
    // 创建数据源和数据目标
    let data_source = create_data_source(state, &input.source_connection_id, &input.source_database).await?;
    let data_target = create_data_target(state, &input.target_connection_id, target_database).await?;

    log::info!(
        "开始迁移: {} -> {}, 数据库: {} -> {}",
        data_source.source_type(),
        data_target.target_type(),
        input.source_database,
        target_database
    );

    // 获取要迁移的表列表
    let tables = if let Some(tables) = input.tables {
        tables
    } else {
        data_source.list_tables(&input.source_database).await?
    };
    let total_tables = tables.len();

    // 获取表映射配置
    let table_mappings = input.table_mappings.unwrap_or_default();

    let mut progress = MigrationProgress {
        migration_id: migration_id.to_string(),
        current_table: String::new(),
        total_tables,
        tables_completed: 0,
        rows_migrated: 0,
        current_table_rows: 0,
        status: MigrationStatus::Preparing,
        error: None,
    };

    // 迁移结构
    if migrate_structure {
        progress.status = MigrationStatus::MigratingStructure;

        for (idx, table) in tables.iter().enumerate() {
            // 查找映射
            let mapping = TableMapping::find_for_table(&table_mappings, table);
            let target_table = mapping
                .map(|m: &TableMapping| m.target_table_name().to_string())
                .unwrap_or_else(|| table.clone());
            let col_map = mapping
                .map(|m: &TableMapping| m.column_map())
                .unwrap_or_default();
            let ignored_cols = mapping
                .map(|m: &TableMapping| m.ignored_columns())
                .unwrap_or_default();

            progress.current_table = if target_table != *table {
                format!("{} → {}", table, target_table)
            } else {
                table.clone()
            };
            progress.tables_completed = idx;
            emit_progress(app, &progress);

            // 获取源表结构
            let mut schema = data_source.get_table_schema(&input.source_database, table).await?;

            // 过滤忽略的列
            if !ignored_cols.is_empty() {
                schema.columns.retain(|col| !ignored_cols.contains(&col.name));
            }

            // 应用列映射：修改 schema 中的列名
            if !col_map.is_empty() {
                for col in &mut schema.columns {
                    if let Some(target_name) = col_map.get(&col.name) {
                        col.name = target_name.clone();
                    }
                }
            }

            // 修改表名为目标表名
            schema.name = target_table.clone();

            // 在目标数据库创建表
            data_target.create_table(target_database, &schema).await?;

            // 添加列注释
            for column in &schema.columns {
                if let Some(comment) = &column.comment {
                    if !comment.is_empty() {
                        data_target.add_column_comment(
                            target_database,
                            &target_table,
                            &column.name,
                            comment,
                        ).await?;
                    }
                }
            }
        }
    }

    // 迁移数据
    if migrate_data {
        progress.status = MigrationStatus::MigratingData;

        for (idx, table) in tables.iter().enumerate() {
            // 查找映射
            let mapping = TableMapping::find_for_table(&table_mappings, table);
            let target_table = mapping
                .map(|m: &TableMapping| m.target_table_name().to_string())
                .unwrap_or_else(|| table.clone());
            let col_map = mapping
                .map(|m: &TableMapping| m.column_map())
                .unwrap_or_default();
            let ignored_cols = mapping
                .map(|m: &TableMapping| m.ignored_columns())
                .unwrap_or_default();

            progress.current_table = if target_table != *table {
                format!("{} → {}", table, target_table)
            } else {
                table.clone()
            };
            progress.tables_completed = idx;
            progress.current_table_rows = 0;
            emit_progress(app, &progress);

            // 清空目标表数据（如果选项开启）
            if truncate_target {
                data_target.truncate_table(target_database, &target_table).await?;
                log::info!("已清空目标表 {}.{} 的数据", target_database, target_table);
            }

            // 获取源表结构（用原始列名读取源数据）
            let source_schema = data_source.get_table_schema(&input.source_database, table).await?;

            // 计算需要忽略的列索引（用于从 DataRow 中过滤值）
            let ignored_indices: Vec<usize> = source_schema.columns.iter().enumerate()
                .filter(|(_, col)| ignored_cols.contains(&col.name))
                .map(|(i, _)| i)
                .collect();

            // 构建映射后的目标 schema（列名替换 + 过滤忽略列）
            let target_schema = {
                let mut mapped = source_schema.clone();
                mapped.name = target_table.clone();
                if !col_map.is_empty() {
                    for col in &mut mapped.columns {
                        if let Some(target_name) = col_map.get(&col.name) {
                            col.name = target_name.clone();
                        }
                    }
                }
                // 过滤忽略的列
                if !ignored_cols.is_empty() {
                    mapped.columns.retain(|col| !ignored_cols.contains(&col.name));
                }
                mapped
            };

            let mut offset = 0;
            let mut total_rows = 0;

            loop {
                // 读取一批数据（用源 schema 的列名读取）
                let mut rows = data_source.read_table_data(
                    &input.source_database,
                    table,
                    &source_schema,
                    offset,
                    batch_size,
                ).await?;

                if rows.is_empty() {
                    break;
                }

                // 过滤掉忽略列的值
                if !ignored_indices.is_empty() {
                    for row in &mut rows {
                        // 从后向前删除，避免索引偏移
                        for &idx in ignored_indices.iter().rev() {
                            if idx < row.values.len() {
                                row.values.remove(idx);
                            }
                        }
                    }
                }

                // 插入到目标数据库（用映射后的 schema）
                let inserted = data_target.insert_rows(
                    target_database,
                    &target_table,
                    &target_schema,
                    &rows,
                ).await?;

                total_rows += inserted;
                offset += batch_size;
                progress.current_table_rows = total_rows;
                progress.rows_migrated += inserted;
                emit_progress(app, &progress);

                if rows.len() < batch_size {
                    break;
                }
            }
        }
        progress.tables_completed = total_tables;
        progress.current_table = String::new();
        progress.current_table_rows = 0;
    }

    progress.status = MigrationStatus::Completed;
    log::info!("迁移完成: 迁移了 {} 个表，共 {} 行数据", total_tables, progress.rows_migrated);
    Ok(progress)
}
