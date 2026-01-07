//! Type definitions for Modbus TCP Slave Simulator.
//! These types mirror the TypeScript models defined in the frontend.

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
    pub auto_reconnect: bool,
}

impl Default for ModbusConnectionProfile {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            name: "Локальный сервер".to_string(),
            host: "127.0.0.1".to_string(),
            port: 502,
            unit_id: 1,
            auto_reconnect: true,
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
