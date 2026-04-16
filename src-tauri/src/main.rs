// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod database;
mod state;
mod store;

use state::AppState;
use tauri::Manager;

fn main() {
    // 默认 info 级别，可通过 RUST_LOG 环境变量覆盖
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new(
            std::sync::Arc::new(store::connection_store::SqliteConnectionStore::new()),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::connection::add_connection,
            commands::connection::remove_connection,
            commands::connection::test_connection,
            commands::connection::list_connections,
            commands::connection::connect_db,
            commands::connection::disconnect_db,
            commands::connection::save_connections,
            commands::connection::load_connections,
            commands::database::list_databases,
            commands::database::list_schemas,
            commands::database::list_tables,
            commands::database::list_columns,
            commands::database::get_table_data,
            commands::database::get_table_row_count,
            commands::database::update_table_data,
            commands::database::delete_table_data,
            commands::database::insert_table_data,
            commands::database::drop_database,
            commands::database::drop_table,
            commands::database::create_table,
            commands::database::add_column,
            commands::database::drop_column,
            commands::database::modify_column,
            commands::query::execute_query,
            commands::query::execute_query_paged,
            commands::redis::redis_list_dbs,
            commands::redis::redis_list_keys,
            commands::redis::redis_get_key,
            commands::redis::redis_set_key,
            commands::redis::redis_del_key,
            commands::migration::start_migration_v2,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.set_title("BaizeDB").unwrap();

            // 初始化存储层（建表 + 加载连接配置到内存）
            let state = app.state::<AppState>();
            tauri::async_runtime::block_on(async {
                let loaded = store::init_store(&state.store, app)
                    .await
                    .expect("初始化存储层失败");

                // 写入内存 HashMap
                let mut conns = state.connections.write().await;
                for conn in &loaded {
                    conns.insert(conn.id.clone(), conn.clone());
                }
                drop(conns);
                log::info!("加载了 {} 个连接配置", loaded.len());
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
