//! Реализация Modbus TCP сервера.
//!
//! Этот модуль предоставляет асинхронный TCP-сервер, который обрабатывает
//! Modbus TCP запросы от мастер-устройств. Сервер работает в фоновой задаче
//! и может быть запущен/остановлен через команды.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

use crate::data_store::SharedDataStore;
use crate::modbus_protocol::{
    pack_bits, pack_registers, ExceptionCode, FunctionCode, ModbusRequest, ModbusResponse,
    ReadRequest, WriteMultipleCoilsRequest, WriteMultipleRegistersRequest, WriteSingleCoilRequest,
    WriteSingleRegisterRequest,
};
use crate::types::{function_code_name, LogEntry, LogEntryType, ServerStatus};

/// Максимальный размер фрейма Modbus TCP (256 байт ADU максимум).
const MAX_FRAME_SIZE: usize = 260;

/// Размер буфера чтения.
const READ_BUFFER_SIZE: usize = 1024;

/// Название события для отправки логов в UI.
const LOG_EVENT_NAME: &str = "modbus-log";

/// Состояние сервера, которое может быть разделено между задачами.
pub struct ModbusServer {
    /// Флаг, указывающий, запущен ли сервер.
    running: AtomicBool,
    /// Текущее количество подключённых клиентов.
    connections_count: AtomicUsize,
    /// Конфигурация сервера.
    config: RwLock<ServerConfig>,
    /// Отправитель сигнала завершения.
    shutdown_tx: RwLock<Option<broadcast::Sender<()>>>,
    /// Последнее сообщение об ошибке.
    last_error: RwLock<Option<String>>,
    /// Хранилище данных для регистров и коилов.
    data_store: SharedDataStore,
    /// Счётчик для генерации уникальных ID логов.
    log_id_counter: AtomicU64,
    /// Handle приложения Tauri для отправки событий.
    app_handle: RwLock<Option<AppHandle>>,
}

/// Конфигурация сервера.
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
    /// Создать новый экземпляр Modbus сервера.
    pub fn new(data_store: SharedDataStore) -> Self {
        Self {
            running: AtomicBool::new(false),
            connections_count: AtomicUsize::new(0),
            config: RwLock::new(ServerConfig::default()),
            shutdown_tx: RwLock::new(None),
            last_error: RwLock::new(None),
            data_store,
            log_id_counter: AtomicU64::new(1),
            app_handle: RwLock::new(None),
        }
    }

    /// Установить handle приложения Tauri для отправки событий.
    pub fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.write() = Some(handle);
    }

    /// Обновить конфигурацию сервера.
    pub fn set_config(&self, host: String, port: u16, unit_id: u8) {
        let mut config = self.config.write();
        config.host = host;
        config.port = port;
        config.unit_id = unit_id;
    }

    /// Проверить, запущен ли сервер.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Получить текущий статус сервера.
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

    /// Сгенерировать следующий ID для записи лога.
    fn next_log_id(&self) -> u64 {
        self.log_id_counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Отправить запись лога в UI.
    pub fn emit_log(&self, entry: LogEntry) {
        if let Some(handle) = self.app_handle.read().as_ref() {
            if let Err(e) = handle.emit(LOG_EVENT_NAME, &entry) {
                log::warn!("Не удалось отправить лог в UI: {}", e);
            }
        }
    }

    /// Создать и отправить информационный лог.
    pub fn log_info(&self, client_addr: &str, message: &str) {
        let entry = LogEntry::new(
            self.next_log_id(),
            LogEntryType::Info,
            client_addr.to_string(),
            message.to_string(),
        );
        log::info!("[{}] {}", client_addr, message);
        self.emit_log(entry);
    }

    /// Создать и отправить лог ошибки.
    pub fn log_error(&self, client_addr: &str, message: &str) {
        let entry = LogEntry::new(
            self.next_log_id(),
            LogEntryType::Error,
            client_addr.to_string(),
            message.to_string(),
        );
        log::error!("[{}] {}", client_addr, message);
        self.emit_log(entry);
    }

    /// Запустить сервер.
    pub async fn start(&self) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("Сервер уже запущен".to_string());
        }

        let config = self.config.read().clone();
        let bind_addr = format!("{}:{}", config.host, config.port);

        // Пытаемся привязаться к адресу
        let listener = TcpListener::bind(&bind_addr)
            .await
            .map_err(|e| format!("Не удалось привязаться к {}: {}", bind_addr, e))?;

        log::info!("Modbus TCP сервер слушает на {}", bind_addr);

        // Создаём канал завершения
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        *self.shutdown_tx.write() = Some(shutdown_tx.clone());

        // Очищаем предыдущую ошибку
        *self.last_error.write() = None;

        // Отмечаем сервер как запущенный
        self.running.store(true, Ordering::SeqCst);

        // Логируем запуск
        self.log_info("SERVER", &format!("Сервер запущен на {}", bind_addr));

        // Клонируем ссылки для цикла принятия соединений
        let server_running = Arc::new(AtomicBool::new(true));
        let server_running_clone = server_running.clone();
        let data_store = self.data_store.clone();
        let connections_count = Arc::new(AtomicUsize::new(0));
        let unit_id = config.unit_id;
        let app_handle = self.app_handle.read().clone();
        let log_id_counter = Arc::new(AtomicU64::new(self.log_id_counter.load(Ordering::SeqCst)));

        // Запускаем цикл принятия соединений
        let connections_count_clone = connections_count;
        tokio::spawn(async move {
            let mut shutdown_rx = shutdown_tx.subscribe();

            loop {
                tokio::select! {
                    // Принимаем новые соединения
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((socket, addr)) => {
                                log::info!("Новое соединение от {}", addr);
                                connections_count_clone.fetch_add(1, Ordering::SeqCst);

                                // Отправляем лог о подключении
                                if let Some(ref handle) = app_handle {
                                    let entry = LogEntry::new(
                                        log_id_counter.fetch_add(1, Ordering::SeqCst),
                                        LogEntryType::Info,
                                        addr.to_string(),
                                        "Клиент подключился".to_string(),
                                    );
                                    let _ = handle.emit(LOG_EVENT_NAME, &entry);
                                }

                                let data_store = data_store.clone();
                                let connections_count = connections_count_clone.clone();
                                let mut client_shutdown_rx = shutdown_tx.subscribe();
                                let client_app_handle = app_handle.clone();
                                let client_log_counter = log_id_counter.clone();

                                // Запускаем обработчик для этого соединения
                                tokio::spawn(async move {
                                    handle_connection(
                                        socket,
                                        addr,
                                        data_store,
                                        unit_id,
                                        &mut client_shutdown_rx,
                                        client_app_handle,
                                        client_log_counter,
                                    ).await;
                                    connections_count.fetch_sub(1, Ordering::SeqCst);
                                    log::info!("Соединение закрыто: {}", addr);
                                });
                            }
                            Err(e) => {
                                log::error!("Не удалось принять соединение: {}", e);
                            }
                        }
                    }
                    // Получен сигнал завершения
                    _ = shutdown_rx.recv() => {
                        log::info!("Получен сигнал завершения сервера");
                        server_running_clone.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            }

            log::info!("Цикл принятия соединений завершён");
        });

        Ok(())
    }

    /// Остановить сервер.
    pub fn stop(&self) -> Result<(), String> {
        if !self.running.load(Ordering::SeqCst) {
            return Err("Сервер не запущен".to_string());
        }

        // Отправляем сигнал завершения
        if let Some(tx) = self.shutdown_tx.read().as_ref() {
            let _ = tx.send(());
        }

        // Очищаем отправитель сигнала
        *self.shutdown_tx.write() = None;

        // Отмечаем как остановленный
        self.running.store(false, Ordering::SeqCst);
        self.connections_count.store(0, Ordering::SeqCst);

        // Логируем остановку
        self.log_info("SERVER", "Сервер остановлен");

        log::info!("Modbus TCP сервер остановлен");

        Ok(())
    }

    /// Установить сообщение об ошибке.
    pub fn set_error(&self, error: String) {
        *self.last_error.write() = Some(error);
    }
}

/// Обработать одно клиентское соединение.
async fn handle_connection(
    mut socket: TcpStream,
    addr: SocketAddr,
    data_store: SharedDataStore,
    unit_id: u8,
    shutdown_rx: &mut broadcast::Receiver<()>,
    app_handle: Option<AppHandle>,
    log_counter: Arc<AtomicU64>,
) {
    let mut buffer = vec![0u8; READ_BUFFER_SIZE];
    let mut frame_buffer = Vec::with_capacity(MAX_FRAME_SIZE);
    let client_addr = addr.to_string();

    loop {
        tokio::select! {
            // Читаем данные из сокета
            read_result = socket.read(&mut buffer) => {
                match read_result {
                    Ok(0) => {
                        // Соединение закрыто
                        emit_log_entry(&app_handle, &log_counter, LogEntry::new(
                            log_counter.fetch_add(1, Ordering::SeqCst),
                            LogEntryType::Info,
                            client_addr.clone(),
                            "Клиент отключился".to_string(),
                        ));
                        break;
                    }
                    Ok(n) => {
                        frame_buffer.extend_from_slice(&buffer[..n]);

                        // Обрабатываем полные фреймы
                        while let Some(frame_len) = ModbusRequest::expected_frame_length(&frame_buffer) {
                            if frame_buffer.len() >= frame_len {
                                // Извлекаем и обрабатываем фрейм
                                let frame_data: Vec<u8> = frame_buffer.drain(..frame_len).collect();
                                let request_start = Instant::now();

                                match ModbusRequest::parse(&frame_data) {
                                    Ok(request) => {
                                        // Проверяем Unit ID
                                        if request.header.unit_id != unit_id && request.header.unit_id != 0 {
                                            log::debug!(
                                                "Игнорируем запрос для unit ID {} (мы {})",
                                                request.header.unit_id,
                                                unit_id
                                            );
                                            continue;
                                        }

                                        // Логируем запрос
                                        let func_name = function_code_name(request.function_code);
                                        let request_summary = format_request_summary(&request);

                                        let request_log = LogEntry::new(
                                            log_counter.fetch_add(1, Ordering::SeqCst),
                                            LogEntryType::Request,
                                            client_addr.clone(),
                                            request_summary,
                                        )
                                        .with_function(request.function_code, func_name)
                                        .with_raw_data(&frame_data);

                                        emit_log_entry(&app_handle, &log_counter, request_log);

                                        // Обрабатываем запрос и отправляем ответ
                                        let response = process_request(&request, &data_store);
                                        let duration_us = request_start.elapsed().as_micros() as u64;

                                        // Логируем ответ
                                        let response_summary = format_response_summary(&request, &response);
                                        let is_error = response.len() > 7 && (response[7] & 0x80) != 0;

                                        let response_log = LogEntry::new(
                                            log_counter.fetch_add(1, Ordering::SeqCst),
                                            if is_error { LogEntryType::Error } else { LogEntryType::Response },
                                            client_addr.clone(),
                                            response_summary,
                                        )
                                        .with_function(request.function_code, func_name)
                                        .with_raw_data(&response)
                                        .with_duration(duration_us);

                                        emit_log_entry(&app_handle, &log_counter, response_log);

                                        if let Err(e) = socket.write_all(&response).await {
                                            log::error!("Не удалось отправить ответ {}: {}", addr, e);
                                            return;
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Не удалось разобрать запрос от {}: {}", addr, e);
                                        emit_log_entry(&app_handle, &log_counter, LogEntry::new(
                                            log_counter.fetch_add(1, Ordering::SeqCst),
                                            LogEntryType::Error,
                                            client_addr.clone(),
                                            format!("Ошибка разбора запроса: {}", e),
                                        ).with_raw_data(&frame_data));
                                        // Очищаем буфер при ошибке разбора для ресинхронизации
                                        frame_buffer.clear();
                                    }
                                }
                            } else {
                                // Нужно больше данных
                                break;
                            }
                        }

                        // Предотвращаем переполнение буфера
                        if frame_buffer.len() > MAX_FRAME_SIZE * 2 {
                            log::warn!("Переполнение буфера фреймов от {}, очистка", addr);
                            frame_buffer.clear();
                        }
                    }
                    Err(e) => {
                        log::error!("Ошибка чтения от {}: {}", addr, e);
                        break;
                    }
                }
            }
            // Сигнал завершения
            _ = shutdown_rx.recv() => {
                log::debug!("Соединение {} получило сигнал завершения", addr);
                break;
            }
        }
    }
}

/// Вспомогательная функция для отправки записи лога.
fn emit_log_entry(app_handle: &Option<AppHandle>, _log_counter: &Arc<AtomicU64>, entry: LogEntry) {
    if let Some(handle) = app_handle {
        let _ = handle.emit(LOG_EVENT_NAME, &entry);
    }
}

/// Форматировать краткое описание запроса.
fn format_request_summary(request: &ModbusRequest) -> String {
    match FunctionCode::from_u8(request.function_code) {
        Some(FunctionCode::ReadCoils) | Some(FunctionCode::ReadDiscreteInputs) => {
            if let Ok(req) = ReadRequest::parse(&request.data) {
                format!(
                    "Чтение с адреса {} количество {}",
                    req.start_address, req.quantity
                )
            } else {
                "Чтение (ошибка разбора)".to_string()
            }
        }
        Some(FunctionCode::ReadHoldingRegisters) | Some(FunctionCode::ReadInputRegisters) => {
            if let Ok(req) = ReadRequest::parse(&request.data) {
                format!(
                    "Чтение регистров с адреса {} количество {}",
                    req.start_address, req.quantity
                )
            } else {
                "Чтение регистров (ошибка разбора)".to_string()
            }
        }
        Some(FunctionCode::WriteSingleCoil) => {
            if let Ok(req) = WriteSingleCoilRequest::parse(&request.data) {
                format!("Запись coil по адресу {} = {}", req.address, req.value)
            } else {
                "Запись coil (ошибка разбора)".to_string()
            }
        }
        Some(FunctionCode::WriteSingleRegister) => {
            if let Ok(req) = WriteSingleRegisterRequest::parse(&request.data) {
                format!("Запись регистра по адресу {} = {}", req.address, req.value)
            } else {
                "Запись регистра (ошибка разбора)".to_string()
            }
        }
        Some(FunctionCode::WriteMultipleCoils) => {
            if let Ok(req) = WriteMultipleCoilsRequest::parse(&request.data) {
                format!(
                    "Запись {} coils с адреса {}",
                    req.quantity, req.start_address
                )
            } else {
                "Запись coils (ошибка разбора)".to_string()
            }
        }
        Some(FunctionCode::WriteMultipleRegisters) => {
            if let Ok(req) = WriteMultipleRegistersRequest::parse(&request.data) {
                format!(
                    "Запись {} регистров с адреса {}",
                    req.quantity, req.start_address
                )
            } else {
                "Запись регистров (ошибка разбора)".to_string()
            }
        }
        None => {
            format!("Неизвестная функция 0x{:02X}", request.function_code)
        }
    }
}

/// Форматировать краткое описание ответа.
fn format_response_summary(request: &ModbusRequest, response: &[u8]) -> String {
    // Проверяем, является ли ответ ошибкой
    if response.len() > 8 && (response[7] & 0x80) != 0 {
        let exception_code = response[8];
        let exception_name = match exception_code {
            0x01 => "Illegal Function",
            0x02 => "Illegal Data Address",
            0x03 => "Illegal Data Value",
            0x04 => "Server Device Failure",
            _ => "Unknown Exception",
        };
        return format!("Ошибка: {} (0x{:02X})", exception_name, exception_code);
    }

    match FunctionCode::from_u8(request.function_code) {
        Some(FunctionCode::ReadCoils) | Some(FunctionCode::ReadDiscreteInputs) => {
            if response.len() > 8 {
                let byte_count = response[8] as usize;
                format!("OK: {} байт данных", byte_count)
            } else {
                "OK".to_string()
            }
        }
        Some(FunctionCode::ReadHoldingRegisters) | Some(FunctionCode::ReadInputRegisters) => {
            if response.len() > 8 {
                let byte_count = response[8] as usize;
                format!("OK: {} регистров", byte_count / 2)
            } else {
                "OK".to_string()
            }
        }
        Some(FunctionCode::WriteSingleCoil) => "OK: Coil записан".to_string(),
        Some(FunctionCode::WriteSingleRegister) => "OK: Регистр записан".to_string(),
        Some(FunctionCode::WriteMultipleCoils) => "OK: Coils записаны".to_string(),
        Some(FunctionCode::WriteMultipleRegisters) => "OK: Регистры записаны".to_string(),
        None => "Ответ отправлен".to_string(),
    }
}

/// Обработать Modbus запрос и сгенерировать ответ.
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
            log::warn!("Неподдерживаемый код функции: 0x{:02X}", function_code);
            ModbusResponse::build_exception(request, function_code, ExceptionCode::IllegalFunction)
        }
    }
}

/// Обработать Read Coils (0x01).
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

/// Обработать Read Discrete Inputs (0x02).
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

/// Обработать Read Holding Registers (0x03).
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

/// Обработать Read Input Registers (0x04).
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

/// Обработать Write Single Coil (0x05).
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
            // Эхо данных запроса в ответ
            ModbusResponse::build_response(request, request.function_code, &request.data)
        }
        Err(e) => ModbusResponse::build_exception(request, request.function_code, e),
    }
}

/// Обработать Write Single Register (0x06).
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
            // Эхо данных запроса в ответ
            ModbusResponse::build_response(request, request.function_code, &request.data)
        }
        Err(e) => ModbusResponse::build_exception(request, request.function_code, e),
    }
}

/// Обработать Write Multiple Coils (0x0F).
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

/// Обработать Write Multiple Registers (0x10).
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

/// Общая ссылка на сервер.
pub type SharedModbusServer = Arc<ModbusServer>;

/// Создать новый общий экземпляр сервера.
pub fn create_shared_server(data_store: SharedDataStore) -> SharedModbusServer {
    Arc::new(ModbusServer::new(data_store))
}
