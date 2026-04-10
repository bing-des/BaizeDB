// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;
mod error;

use state::AppState;
use tauri::Manager;

fn main() {
    // 默认 debug 级别，可通过 RUST_LOG 环境变量覆盖
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::connection::add_connection,
            commands::connection::remove_connection,
            commands::connection::test_connection,
            commands::connection::list_connections,
            commands::connection::connect_db,
            commands::connection::disconnect_db,
            commands::database::list_databases,
            commands::database::list_schemas,
            commands::database::list_tables,
            commands::database::list_columns,
            commands::database::get_table_data,
            commands::database::get_table_row_count,
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
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
