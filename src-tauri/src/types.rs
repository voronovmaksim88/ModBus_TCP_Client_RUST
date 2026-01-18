//! Определения типов для Modbus TCP Slave Simulator.
//! Эти типы соответствуют TypeScript-моделям, определённым во фронтенде.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Modbus memory area type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModbusArea {
    /// Coils (0x) - read/write single bit
    Coil,
    /// Discrete Inputs (1x) - read-only single bit
    DiscreteInput,
    /// Input Registers (3x) - read-only 16-bit
    InputRegister,
    /// Holding Registers (4x) - read/write 16-bit
    HoldingRegister,
}

/// Data type for interpreting register values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModbusDataType {
    Bool,
    Uint16,
    Int16,
    Uint32,
    Float32,
}

impl ModbusDataType {
    /// Returns the number of 16-bit registers this data type occupies.
    pub fn register_count(&self) -> u16 {
        match self {
            ModbusDataType::Bool => 1,
            ModbusDataType::Uint16 => 1,
            ModbusDataType::Int16 => 1,
            ModbusDataType::Uint32 => 2,
            ModbusDataType::Float32 => 2,
        }
    }
}

/// Connection profile for the Modbus slave.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModbusConnectionProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub unit_id: u8,
}

impl Default for ModbusConnectionProfile {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            name: "Локальный сервер".to_string(),
            host: "127.0.0.1".to_string(),
            port: 502,
            unit_id: 1,
        }
    }
}

/// A single Modbus variable definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModbusVariable {
    pub id: String,
    pub name: String,
    pub area: ModbusArea,
    /// Address of the register/coil (0-based).
    pub address: u16,
    pub data_type: ModbusDataType,
    /// Current value that will be returned to master.
    /// For bool: true/false, for numeric types: number.
    pub value: ModbusValue,
    /// Bit within register (for bool in holding/input register), optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit: Option<u8>,
    /// Whether this variable is read-only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readonly: Option<bool>,
    /// User note/comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Value that can be either boolean or numeric.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ModbusValue {
    Bool(bool),
    Number(f64),
    Null,
}

impl ModbusValue {
    /// Convert value to boolean (for coils/discrete inputs).
    pub fn as_bool(&self) -> bool {
        match self {
            ModbusValue::Bool(b) => *b,
            ModbusValue::Number(n) => *n != 0.0,
            ModbusValue::Null => false,
        }
    }

    /// Convert value to u16 (for registers).
    pub fn as_u16(&self) -> u16 {
        match self {
            ModbusValue::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            ModbusValue::Number(n) => *n as u16,
            ModbusValue::Null => 0,
        }
    }

    /// Convert value to i16.
    pub fn as_i16(&self) -> i16 {
        match self {
            ModbusValue::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            ModbusValue::Number(n) => *n as i16,
            ModbusValue::Null => 0,
        }
    }

    /// Convert value to u32.
    pub fn as_u32(&self) -> u32 {
        match self {
            ModbusValue::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            ModbusValue::Number(n) => *n as u32,
            ModbusValue::Null => 0,
        }
    }

    /// Convert value to f32.
    pub fn as_f32(&self) -> f32 {
        match self {
            ModbusValue::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            ModbusValue::Number(n) => *n as f32,
            ModbusValue::Null => 0.0,
        }
    }
}

impl Default for ModbusValue {
    fn default() -> Self {
        ModbusValue::Number(0.0)
    }
}

/// Full project configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModbusProject {
    pub profiles: Vec<ModbusConnectionProfile>,
    pub current_profile_id: Option<String>,
    pub variables: Vec<ModbusVariable>,
}

impl Default for ModbusProject {
    fn default() -> Self {
        let profile = ModbusConnectionProfile::default();
        Self {
            current_profile_id: Some(profile.id.clone()),
            profiles: vec![profile],
            variables: Vec::new(),
        }
    }
}

/// Server status information sent to frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub running: bool,
    pub host: String,
    pub port: u16,
    pub unit_id: u8,
    pub connections_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Default for ServerStatus {
    fn default() -> Self {
        Self {
            running: false,
            host: "0.0.0.0".to_string(),
            port: 502,
            unit_id: 1,
            connections_count: 0,
            error: None,
        }
    }
}

/// Тип записи лога: запрос или ответ.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogEntryType {
    /// Входящий запрос от мастера
    Request,
    /// Исходящий ответ слэйва
    Response,
    /// Ошибка обработки
    Error,
    /// Информационное сообщение (подключение/отключение)
    Info,
}

/// Запись лога для отображения в UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// Уникальный ID записи
    pub id: u64,
    /// Временная метка (ISO 8601)
    pub timestamp: String,
    /// Тип записи (request/response/error/info)
    pub entry_type: LogEntryType,
    /// IP-адрес клиента
    pub client_addr: String,
    /// Код функции Modbus (если применимо)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_code: Option<u8>,
    /// Название функции (человекочитаемое)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_name: Option<String>,
    /// Краткое описание запроса/ответа
    pub summary: String,
    /// Сырые данные в hex (опционально)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_data: Option<String>,
    /// Время обработки в микросекундах (для ответов)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_us: Option<u64>,
}

impl LogEntry {
    /// Создать новую запись лога.
    pub fn new(id: u64, entry_type: LogEntryType, client_addr: String, summary: String) -> Self {
        Self {
            id,
            timestamp: chrono_now_iso(),
            entry_type,
            client_addr,
            function_code: None,
            function_name: None,
            summary,
            raw_data: None,
            duration_us: None,
        }
    }

    /// Установить код и название функции.
    pub fn with_function(mut self, code: u8, name: &str) -> Self {
        self.function_code = Some(code);
        self.function_name = Some(name.to_string());
        self
    }

    /// Установить сырые данные в hex.
    pub fn with_raw_data(mut self, data: &[u8]) -> Self {
        self.raw_data = Some(bytes_to_hex(data));
        self
    }

    /// Установить время обработки.
    pub fn with_duration(mut self, duration_us: u64) -> Self {
        self.duration_us = Some(duration_us);
        self
    }
}

/// Получить текущее время в формате ISO 8601.
fn chrono_now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = now.as_secs();
    let millis = now.subsec_millis();

    // Простой формат: секунды с эпохи + миллисекунды
    // Для полного ISO 8601 нужна библиотека chrono, но для простоты используем timestamp
    format!("{}.{:03}", secs, millis)
}

/// Преобразовать байты в hex-строку.
fn bytes_to_hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Получить человекочитаемое название функции Modbus.
pub fn function_code_name(code: u8) -> &'static str {
    match code {
        0x01 => "Read Coils",
        0x02 => "Read Discrete Inputs",
        0x03 => "Read Holding Registers",
        0x04 => "Read Input Registers",
        0x05 => "Write Single Coil",
        0x06 => "Write Single Register",
        0x0F => "Write Multiple Coils",
        0x10 => "Write Multiple Registers",
        _ => "Unknown Function",
    }
}
