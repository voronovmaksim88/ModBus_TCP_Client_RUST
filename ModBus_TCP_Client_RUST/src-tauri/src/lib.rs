//! Modbus TCP Slave Simulator — Tauri-приложение
//!
//! Это главная точка входа библиотеки, которая настраивает Tauri-приложение
//! со всеми необходимыми модулями и командами.

mod commands;
mod data_store;
mod modbus_protocol;
mod server;
mod types;

use commands::AppState;
use data_store::create_shared_data_store;
use server::create_shared_server;

/// Инициализация и запуск Tauri-приложения.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Инициализируем логгер
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Запуск Modbus TCP Slave Simulator");

    // Создаём общее хранилище данных для регистров и коилов
    let data_store = create_shared_data_store();

    // Создаём общий экземпляр Modbus TCP сервера
    let server = create_shared_server(data_store.clone());

    // Создаём состояние приложения, которое будет доступно во всех командах
    let app_state = AppState { server, data_store };

    // Собираем и запускаем Tauri-приложение
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
        .expect("Ошибка при запуске Tauri-приложения");
}
