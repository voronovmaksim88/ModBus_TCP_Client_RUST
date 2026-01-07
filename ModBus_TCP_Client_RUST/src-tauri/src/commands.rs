//! Tauri commands for Modbus TCP Slave Simulator.
//!
//! These commands provide the interface between the Vue frontend and the Rust backend.

use tauri::State;

use crate::data_store::SharedDataStore;
use crate::server::SharedModbusServer;
use crate::types::{ModbusConnectionProfile, ModbusValue, ModbusVariable, ServerStatus};

/// Application state managed by Tauri.
pub struct AppState {
    pub server: SharedModbusServer,
    pub data_store: SharedDataStore,
}

/// Start the Modbus TCP server with the given profile and variables.
#[tauri::command]
pub async fn start_server(
    state: State<'_, AppState>,
    profile: ModbusConnectionProfile,
    variables: Vec<ModbusVariable>,
) -> Result<ServerStatus, String> {
    log::info!(
        "Starting server on {}:{} with unit_id={}, {} variables",
        profile.host,
        profile.port,
        profile.unit_id,
        variables.len()
    );

    // Load variables into data store
    state.data_store.load_variables(&variables);

    // Configure and start server
    state
        .server
        .set_config(profile.host, profile.port, profile.unit_id);

    state.server.start().await?;

    Ok(state.server.get_status())
}

/// Stop the Modbus TCP server.
#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>) -> Result<ServerStatus, String> {
    log::info!("Stopping server");

    state.server.stop()?;

    Ok(state.server.get_status())
}

/// Get current server status.
#[tauri::command]
pub fn get_server_status(state: State<'_, AppState>) -> ServerStatus {
    state.server.get_status()
}

/// Update a variable's value by ID.
/// This updates both the data store and the underlying registers/coils.
#[tauri::command]
pub fn update_variable(
    state: State<'_, AppState>,
    id: String,
    value: ModbusValue,
) -> Result<bool, String> {
    log::debug!("Updating variable {} to {:?}", id, value);

    let updated = state.data_store.update_variable(&id, value);

    if updated {
        Ok(true)
    } else {
        Err(format!("Variable with id '{}' not found", id))
    }
}

/// Get all current variables with their runtime values.
/// This returns the variables as they are in the data store,
/// which may have been modified by master write operations.
#[tauri::command]
pub fn get_variables(state: State<'_, AppState>) -> Vec<ModbusVariable> {
    state.data_store.get_variables()
}

/// Reload variables into the data store without restarting the server.
/// Useful for updating variable definitions while server is running.
#[tauri::command]
pub fn reload_variables(
    state: State<'_, AppState>,
    variables: Vec<ModbusVariable>,
) -> Result<(), String> {
    log::info!("Reloading {} variables", variables.len());

    state.data_store.load_variables(&variables);

    Ok(())
}

/// Clear all data in the store (reset all registers and coils to defaults).
#[tauri::command]
pub fn clear_data_store(state: State<'_, AppState>) -> Result<(), String> {
    log::info!("Clearing data store");

    state.data_store.clear();

    Ok(())
}
