//! Tauri-команды для Modbus TCP Slave Simulator.
//!
//! Эти команды обеспечивают интерфейс между Vue-фронтендом и Rust-бэкендом.

use tauri::{AppHandle, State};

use crate::data_store::SharedDataStore;
use crate::server::SharedModbusServer;
use crate::types::{ModbusConnectionProfile, ModbusValue, ModbusVariable, ServerStatus};

/// Состояние приложения, управляемое Tauri.
pub struct AppState {
    pub server: SharedModbusServer,
    pub data_store: SharedDataStore,
}

/// Запустить Modbus TCP сервер с указанным профилем и переменными.
#[tauri::command]
pub async fn start_server(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    profile: ModbusConnectionProfile,
    variables: Vec<ModbusVariable>,
) -> Result<ServerStatus, String> {
    log::info!(
        "Запуск сервера на {}:{} с unit_id={}, {} переменных",
        profile.host,
        profile.port,
        profile.unit_id,
        variables.len()
    );

    // Загружаем переменные в хранилище данных
    state.data_store.load_variables(&variables);

    // Устанавливаем AppHandle для отправки событий логирования
    state.server.set_app_handle(app_handle);

    // Настраиваем и запускаем сервер
    state
        .server
        .set_config(profile.host, profile.port, profile.unit_id);

    state.server.start().await?;

    Ok(state.server.get_status())
}

/// Остановить Modbus TCP сервер.
#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>) -> Result<ServerStatus, String> {
    log::info!("Остановка сервера");

    state.server.stop()?;

    Ok(state.server.get_status())
}

/// Получить текущий статус сервера.
#[tauri::command]
pub fn get_server_status(state: State<'_, AppState>) -> ServerStatus {
    state.server.get_status()
}

/// Обновить значение переменной по её ID.
/// Обновляет как хранилище данных, так и соответствующие регистры/коилы.
#[tauri::command]
pub fn update_variable(
    state: State<'_, AppState>,
    id: String,
    value: ModbusValue,
) -> Result<bool, String> {
    log::debug!("Обновление переменной {} на {:?}", id, value);

    let updated = state.data_store.update_variable(&id, value);

    if updated {
        Ok(true)
    } else {
        Err(format!("Переменная с id '{}' не найдена", id))
    }
}

/// Получить все текущие переменные с их runtime-значениями.
/// Возвращает переменные в том виде, как они хранятся в data_store,
/// включая изменения, внесённые операциями записи от мастера.
#[tauri::command]
pub fn get_variables(state: State<'_, AppState>) -> Vec<ModbusVariable> {
    state.data_store.get_variables()
}

/// Перезагрузить переменные в хранилище данных без перезапуска сервера.
/// Полезно для обновления определений переменных во время работы сервера.
#[tauri::command]
pub fn reload_variables(
    state: State<'_, AppState>,
    variables: Vec<ModbusVariable>,
) -> Result<(), String> {
    log::info!("Перезагрузка {} переменных", variables.len());

    state.data_store.load_variables(&variables);

    Ok(())
}

/// Очистить все данные в хранилище (сбросить все регистры и коилы к значениям по умолчанию).
#[tauri::command]
pub fn clear_data_store(state: State<'_, AppState>) -> Result<(), String> {
    log::info!("Очистка хранилища данных");

    state.data_store.clear();

    Ok(())
}
