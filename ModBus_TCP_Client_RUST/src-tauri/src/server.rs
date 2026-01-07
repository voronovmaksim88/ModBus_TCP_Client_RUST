//! Modbus TCP Server implementation.
//!
//! This module provides an async TCP server that handles Modbus TCP requests
//! from master devices. The server runs in a background task and can be
//! started/stopped via commands.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

use crate::data_store::SharedDataStore;
use crate::modbus_protocol::{
    pack_bits, pack_registers, ExceptionCode, FunctionCode, ModbusRequest, ModbusResponse,
    ReadRequest, WriteMultipleCoilsRequest, WriteMultipleRegistersRequest, WriteSingleCoilRequest,
    WriteSingleRegisterRequest,
};
use crate::types::ServerStatus;

/// Maximum frame size for Modbus TCP (256 bytes ADU max).
const MAX_FRAME_SIZE: usize = 260;

/// Read buffer size.
const READ_BUFFER_SIZE: usize = 1024;

/// Server state that can be shared across tasks.
#[derive(Debug)]
pub struct ModbusServer {
    /// Whether the server is currently running.
    running: AtomicBool,
    /// Current number of connected clients.
    connections_count: AtomicUsize,
    /// Server configuration.
    config: RwLock<ServerConfig>,
    /// Shutdown signal sender.
    shutdown_tx: RwLock<Option<broadcast::Sender<()>>>,
    /// Last error message.
    last_error: RwLock<Option<String>>,
    /// Data store for registers and coils.
    data_store: SharedDataStore,
}

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub unit_id: u8,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 502,
            unit_id: 1,
        }
    }
}

impl ModbusServer {
    /// Create a new Modbus server instance.
    pub fn new(data_store: SharedDataStore) -> Self {
        Self {
            running: AtomicBool::new(false),
            connections_count: AtomicUsize::new(0),
            config: RwLock::new(ServerConfig::default()),
            shutdown_tx: RwLock::new(None),
            last_error: RwLock::new(None),
            data_store,
        }
    }

    /// Update server configuration.
    pub fn set_config(&self, host: String, port: u16, unit_id: u8) {
        let mut config = self.config.write();
        config.host = host;
        config.port = port;
        config.unit_id = unit_id;
    }

    /// Check if the server is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get current server status.
    pub fn get_status(&self) -> ServerStatus {
        let config = self.config.read();
        let error = self.last_error.read().clone();

        ServerStatus {
            running: self.running.load(Ordering::SeqCst),
            host: config.host.clone(),
            port: config.port,
            unit_id: config.unit_id,
            connections_count: self.connections_count.load(Ordering::SeqCst),
            error,
        }
    }

    /// Start the server.
    pub async fn start(&self) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("Server is already running".to_string());
        }

        let config = self.config.read().clone();
        let bind_addr = format!("{}:{}", config.host, config.port);

        // Try to bind to the address
        let listener = TcpListener::bind(&bind_addr)
            .await
            .map_err(|e| format!("Failed to bind to {}: {}", bind_addr, e))?;

        log::info!("Modbus TCP server listening on {}", bind_addr);

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        *self.shutdown_tx.write() = Some(shutdown_tx.clone());

        // Clear any previous error
        *self.last_error.write() = None;

        // Mark server as running
        self.running.store(true, Ordering::SeqCst);

        // Clone references for the accept loop
        let server_running = Arc::new(AtomicBool::new(true));
        let server_running_clone = server_running.clone();
        let data_store = self.data_store.clone();
        let connections_count = Arc::new(AtomicUsize::new(0));
        let unit_id = config.unit_id;

        // Spawn the accept loop
        let connections_count_clone = connections_count;
        tokio::spawn(async move {
            let mut shutdown_rx = shutdown_tx.subscribe();

            loop {
                tokio::select! {
                    // Accept new connections
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((socket, addr)) => {
                                log::info!("New connection from {}", addr);
                                connections_count_clone.fetch_add(1, Ordering::SeqCst);

                                let data_store = data_store.clone();
                                let connections_count = connections_count_clone.clone();
                                let mut client_shutdown_rx = shutdown_tx.subscribe();

                                // Spawn handler for this connection
                                tokio::spawn(async move {
                                    handle_connection(socket, addr, data_store, unit_id, &mut client_shutdown_rx).await;
                                    connections_count.fetch_sub(1, Ordering::SeqCst);
                                    log::info!("Connection closed: {}", addr);
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to accept connection: {}", e);
                            }
                        }
                    }
                    // Shutdown signal received
                    _ = shutdown_rx.recv() => {
                        log::info!("Server shutdown signal received");
                        server_running_clone.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            }

            log::info!("Server accept loop terminated");
        });

        // Sync connection count periodically (simplified - just use atomic)
        // The actual count is managed by the spawned tasks

        Ok(())
    }

    /// Stop the server.
    pub fn stop(&self) -> Result<(), String> {
        if !self.running.load(Ordering::SeqCst) {
            return Err("Server is not running".to_string());
        }

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.read().as_ref() {
            let _ = tx.send(());
        }

        // Clear shutdown sender
        *self.shutdown_tx.write() = None;

        // Mark as not running
        self.running.store(false, Ordering::SeqCst);
        self.connections_count.store(0, Ordering::SeqCst);

        log::info!("Modbus TCP server stopped");

        Ok(())
    }

    /// Set last error message.
    pub fn set_error(&self, error: String) {
        *self.last_error.write() = Some(error);
    }
}

/// Handle a single client connection.
async fn handle_connection(
    mut socket: TcpStream,
    addr: SocketAddr,
    data_store: SharedDataStore,
    unit_id: u8,
    shutdown_rx: &mut broadcast::Receiver<()>,
) {
    let mut buffer = vec![0u8; READ_BUFFER_SIZE];
    let mut frame_buffer = Vec::with_capacity(MAX_FRAME_SIZE);

    loop {
        tokio::select! {
            // Read data from socket
            read_result = socket.read(&mut buffer) => {
                match read_result {
                    Ok(0) => {
                        // Connection closed
                        break;
                    }
                    Ok(n) => {
                        frame_buffer.extend_from_slice(&buffer[..n]);

                        // Process complete frames
                        while let Some(frame_len) = ModbusRequest::expected_frame_length(&frame_buffer) {
                            if frame_buffer.len() >= frame_len {
                                // Extract and process frame
                                let frame_data: Vec<u8> = frame_buffer.drain(..frame_len).collect();

                                match ModbusRequest::parse(&frame_data) {
                                    Ok(request) => {
                                        // Check unit ID
                                        if request.header.unit_id != unit_id && request.header.unit_id != 0 {
                                            // Ignore requests for other unit IDs (broadcast 0 is accepted)
                                            log::debug!(
                                                "Ignoring request for unit ID {} (we are {})",
                                                request.header.unit_id,
                                                unit_id
                                            );
                                            continue;
                                        }

                                        // Process request and send response
                                        let response = process_request(&request, &data_store);

                                        if let Err(e) = socket.write_all(&response).await {
                                            log::error!("Failed to send response to {}: {}", addr, e);
                                            return;
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Failed to parse request from {}: {}", addr, e);
                                        // Clear buffer on parse error to resync
                                        frame_buffer.clear();
                                    }
                                }
                            } else {
                                // Need more data
                                break;
                            }
                        }

                        // Prevent buffer from growing too large
                        if frame_buffer.len() > MAX_FRAME_SIZE * 2 {
                            log::warn!("Frame buffer overflow from {}, clearing", addr);
                            frame_buffer.clear();
                        }
                    }
                    Err(e) => {
                        log::error!("Read error from {}: {}", addr, e);
                        break;
                    }
                }
            }
            // Shutdown signal
            _ = shutdown_rx.recv() => {
                log::debug!("Connection {} received shutdown signal", addr);
                break;
            }
        }
    }
}

/// Process a Modbus request and generate a response.
fn process_request(request: &ModbusRequest, data_store: &SharedDataStore) -> Vec<u8> {
    let function_code = request.function_code;

    match FunctionCode::from_u8(function_code) {
        Some(FunctionCode::ReadCoils) => handle_read_coils(request, data_store),
        Some(FunctionCode::ReadDiscreteInputs) => handle_read_discrete_inputs(request, data_store),
        Some(FunctionCode::ReadHoldingRegisters) => {
            handle_read_holding_registers(request, data_store)
        }
        Some(FunctionCode::ReadInputRegisters) => handle_read_input_registers(request, data_store),
        Some(FunctionCode::WriteSingleCoil) => handle_write_single_coil(request, data_store),
        Some(FunctionCode::WriteSingleRegister) => {
            handle_write_single_register(request, data_store)
        }
        Some(FunctionCode::WriteMultipleCoils) => handle_write_multiple_coils(request, data_store),
        Some(FunctionCode::WriteMultipleRegisters) => {
            handle_write_multiple_registers(request, data_store)
        }
        None => {
            // Unsupported function code
            log::warn!("Unsupported function code: 0x{:02X}", function_code);
            ModbusResponse::build_exception(request, function_code, ExceptionCode::IllegalFunction)
        }
    }
}

/// Handle Read Coils (0x01).
fn handle_read_coils(request: &ModbusRequest, data_store: &SharedDataStore) -> Vec<u8> {
    let read_req = match ReadRequest::parse(&request.data) {
        Ok(r) => r,
        Err(_) => {
            return ModbusResponse::build_exception(
                request,
                request.function_code,
                ExceptionCode::IllegalDataValue,
            );
        }
    };

    if let Err(e) = read_req.validate_bits() {
        return ModbusResponse::build_exception(request, request.function_code, e);
    }

    match data_store.read_coils(read_req.start_address, read_req.quantity) {
        Ok(coils) => {
            let packed = pack_bits(&coils);
            let mut data = vec![packed.len() as u8];
            data.extend_from_slice(&packed);
            ModbusResponse::build_response(request, request.function_code, &data)
        }
        Err(e) => ModbusResponse::build_exception(request, request.function_code, e),
    }
}

/// Handle Read Discrete Inputs (0x02).
fn handle_read_discrete_inputs(request: &ModbusRequest, data_store: &SharedDataStore) -> Vec<u8> {
    let read_req = match ReadRequest::parse(&request.data) {
        Ok(r) => r,
        Err(_) => {
            return ModbusResponse::build_exception(
                request,
                request.function_code,
                ExceptionCode::IllegalDataValue,
            );
        }
    };

    if let Err(e) = read_req.validate_bits() {
        return ModbusResponse::build_exception(request, request.function_code, e);
    }

    match data_store.read_discrete_inputs(read_req.start_address, read_req.quantity) {
        Ok(inputs) => {
            let packed = pack_bits(&inputs);
            let mut data = vec![packed.len() as u8];
            data.extend_from_slice(&packed);
            ModbusResponse::build_response(request, request.function_code, &data)
        }
        Err(e) => ModbusResponse::build_exception(request, request.function_code, e),
    }
}

/// Handle Read Holding Registers (0x03).
fn handle_read_holding_registers(request: &ModbusRequest, data_store: &SharedDataStore) -> Vec<u8> {
    let read_req = match ReadRequest::parse(&request.data) {
        Ok(r) => r,
        Err(_) => {
            return ModbusResponse::build_exception(
                request,
                request.function_code,
                ExceptionCode::IllegalDataValue,
            );
        }
    };

    if let Err(e) = read_req.validate_registers() {
        return ModbusResponse::build_exception(request, request.function_code, e);
    }

    match data_store.read_holding_registers(read_req.start_address, read_req.quantity) {
        Ok(regs) => {
            let packed = pack_registers(&regs);
            let mut data = vec![packed.len() as u8];
            data.extend_from_slice(&packed);
            ModbusResponse::build_response(request, request.function_code, &data)
        }
        Err(e) => ModbusResponse::build_exception(request, request.function_code, e),
    }
}

/// Handle Read Input Registers (0x04).
fn handle_read_input_registers(request: &ModbusRequest, data_store: &SharedDataStore) -> Vec<u8> {
    let read_req = match ReadRequest::parse(&request.data) {
        Ok(r) => r,
        Err(_) => {
            return ModbusResponse::build_exception(
                request,
                request.function_code,
                ExceptionCode::IllegalDataValue,
            );
        }
    };

    if let Err(e) = read_req.validate_registers() {
        return ModbusResponse::build_exception(request, request.function_code, e);
    }

    match data_store.read_input_registers(read_req.start_address, read_req.quantity) {
        Ok(regs) => {
            let packed = pack_registers(&regs);
            let mut data = vec![packed.len() as u8];
            data.extend_from_slice(&packed);
            ModbusResponse::build_response(request, request.function_code, &data)
        }
        Err(e) => ModbusResponse::build_exception(request, request.function_code, e),
    }
}

/// Handle Write Single Coil (0x05).
fn handle_write_single_coil(request: &ModbusRequest, data_store: &SharedDataStore) -> Vec<u8> {
    let write_req = match WriteSingleCoilRequest::parse(&request.data) {
        Ok(r) => r,
        Err(_) => {
            return ModbusResponse::build_exception(
                request,
                request.function_code,
                ExceptionCode::IllegalDataValue,
            );
        }
    };

    match data_store.write_single_coil(write_req.address, write_req.value) {
        Ok(()) => {
            // Echo the request data as response
            ModbusResponse::build_response(request, request.function_code, &request.data)
        }
        Err(e) => ModbusResponse::build_exception(request, request.function_code, e),
    }
}

/// Handle Write Single Register (0x06).
fn handle_write_single_register(request: &ModbusRequest, data_store: &SharedDataStore) -> Vec<u8> {
    let write_req = match WriteSingleRegisterRequest::parse(&request.data) {
        Ok(r) => r,
        Err(_) => {
            return ModbusResponse::build_exception(
                request,
                request.function_code,
                ExceptionCode::IllegalDataValue,
            );
        }
    };

    match data_store.write_single_register(write_req.address, write_req.value) {
        Ok(()) => {
            // Echo the request data as response
            ModbusResponse::build_response(request, request.function_code, &request.data)
        }
        Err(e) => ModbusResponse::build_exception(request, request.function_code, e),
    }
}

/// Handle Write Multiple Coils (0x0F).
fn handle_write_multiple_coils(request: &ModbusRequest, data_store: &SharedDataStore) -> Vec<u8> {
    let write_req = match WriteMultipleCoilsRequest::parse(&request.data) {
        Ok(r) => r,
        Err(_) => {
            return ModbusResponse::build_exception(
                request,
                request.function_code,
                ExceptionCode::IllegalDataValue,
            );
        }
    };

    if let Err(e) = write_req.validate() {
        return ModbusResponse::build_exception(request, request.function_code, e);
    }

    match data_store.write_multiple_coils(write_req.start_address, &write_req.values) {
        Ok(()) => {
            let response_data = write_req.to_response_data();
            ModbusResponse::build_response(request, request.function_code, &response_data)
        }
        Err(e) => ModbusResponse::build_exception(request, request.function_code, e),
    }
}

/// Handle Write Multiple Registers (0x10).
fn handle_write_multiple_registers(
    request: &ModbusRequest,
    data_store: &SharedDataStore,
) -> Vec<u8> {
    let write_req = match WriteMultipleRegistersRequest::parse(&request.data) {
        Ok(r) => r,
        Err(_) => {
            return ModbusResponse::build_exception(
                request,
                request.function_code,
                ExceptionCode::IllegalDataValue,
            );
        }
    };

    if let Err(e) = write_req.validate() {
        return ModbusResponse::build_exception(request, request.function_code, e);
    }

    match data_store.write_multiple_registers(write_req.start_address, &write_req.values) {
        Ok(()) => {
            let response_data = write_req.to_response_data();
            ModbusResponse::build_response(request, request.function_code, &response_data)
        }
        Err(e) => ModbusResponse::build_exception(request, request.function_code, e),
    }
}

/// Shared reference to the server.
pub type SharedModbusServer = Arc<ModbusServer>;

/// Create a new shared server instance.
pub fn create_shared_server(data_store: SharedDataStore) -> SharedModbusServer {
    Arc::new(ModbusServer::new(data_store))
}
