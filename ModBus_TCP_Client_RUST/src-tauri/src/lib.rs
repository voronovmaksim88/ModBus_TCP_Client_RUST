//! Modbus TCP Slave Simulator - Tauri Application
//!
//! This is the main library entry point that sets up the Tauri application
//! with all necessary modules and commands.

mod commands;
mod data_store;
mod modbus_protocol;
mod server;
mod types;

use commands::AppState;
use data_store::create_shared_data_store;
use server::create_shared_server;

/// Initialize and run the Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Modbus TCP Slave Simulator");

    // Create shared data store
    let data_store = create_shared_data_store();

    // Create shared server
    let server = create_shared_server(data_store.clone());

    // Create app state
    let app_state = AppState { server, data_store };

    // Build and run Tauri application
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::get_server_status,
            commands::update_variable,
            commands::get_variables,
            commands::reload_variables,
            commands::clear_data_store,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
