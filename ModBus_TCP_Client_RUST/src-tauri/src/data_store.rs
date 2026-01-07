//! Data store for Modbus registers and coils.
//!
//! This module provides thread-safe storage for Modbus data areas:
//! - Coils (0x) - read/write single bits
//! - Discrete Inputs (1x) - read-only single bits
//! - Input Registers (3x) - read-only 16-bit registers
//! - Holding Registers (4x) - read/write 16-bit registers

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::modbus_protocol::ExceptionCode;
use crate::types::{ModbusArea, ModbusDataType, ModbusValue, ModbusVariable};

/// Default size for each data area (can be expanded dynamically).
const DEFAULT_COILS_SIZE: usize = 10000;
const DEFAULT_DISCRETE_INPUTS_SIZE: usize = 10000;
const DEFAULT_INPUT_REGISTERS_SIZE: usize = 10000;
const DEFAULT_HOLDING_REGISTERS_SIZE: usize = 10000;

/// Thread-safe Modbus data store.
#[derive(Debug)]
pub struct ModbusDataStore {
    /// Coils (0x) - bit array
    coils: RwLock<Vec<bool>>,
    /// Discrete Inputs (1x) - bit array
    discrete_inputs: RwLock<Vec<bool>>,
    /// Input Registers (3x) - u16 array
    input_registers: RwLock<Vec<u16>>,
    /// Holding Registers (4x) - u16 array
    holding_registers: RwLock<Vec<u16>>,
    /// Mapping from variable ID to its definition (for quick lookup)
    variables: RwLock<HashMap<String, ModbusVariable>>,
}

impl Default for ModbusDataStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ModbusDataStore {
    /// Create a new data store with default sizes.
    pub fn new() -> Self {
        Self {
            coils: RwLock::new(vec![false; DEFAULT_COILS_SIZE]),
            discrete_inputs: RwLock::new(vec![false; DEFAULT_DISCRETE_INPUTS_SIZE]),
            input_registers: RwLock::new(vec![0u16; DEFAULT_INPUT_REGISTERS_SIZE]),
            holding_registers: RwLock::new(vec![0u16; DEFAULT_HOLDING_REGISTERS_SIZE]),
            variables: RwLock::new(HashMap::new()),
        }
    }

    /// Initialize the data store from a list of variables.
    /// This sets up the initial values based on variable definitions.
    pub fn load_variables(&self, variables: &[ModbusVariable]) {
        let mut vars_map = self.variables.write();
        vars_map.clear();

        for var in variables {
            vars_map.insert(var.id.clone(), var.clone());
            self.write_variable_value(var);
        }
    }

    /// Write a single variable's value to the appropriate data area.
    fn write_variable_value(&self, var: &ModbusVariable) {
        match var.area {
            ModbusArea::Coil => {
                let value = var.value.as_bool();
                let mut coils = self.coils.write();
                if (var.address as usize) < coils.len() {
                    coils[var.address as usize] = value;
                }
            }
            ModbusArea::DiscreteInput => {
                let value = var.value.as_bool();
                let mut inputs = self.discrete_inputs.write();
                if (var.address as usize) < inputs.len() {
                    inputs[var.address as usize] = value;
                }
            }
            ModbusArea::InputRegister => {
                self.write_register_value(
                    &self.input_registers,
                    var.address,
                    &var.data_type,
                    &var.value,
                );
            }
            ModbusArea::HoldingRegister => {
                self.write_register_value(
                    &self.holding_registers,
                    var.address,
                    &var.data_type,
                    &var.value,
                );
            }
        }
    }

    /// Write a value to a register array based on data type.
    fn write_register_value(
        &self,
        registers: &RwLock<Vec<u16>>,
        address: u16,
        data_type: &ModbusDataType,
        value: &ModbusValue,
    ) {
        let mut regs = registers.write();
        let addr = address as usize;

        match data_type {
            ModbusDataType::Bool => {
                if addr < regs.len() {
                    regs[addr] = if value.as_bool() { 1 } else { 0 };
                }
            }
            ModbusDataType::Uint16 => {
                if addr < regs.len() {
                    regs[addr] = value.as_u16();
                }
            }
            ModbusDataType::Int16 => {
                if addr < regs.len() {
                    regs[addr] = value.as_i16() as u16;
                }
            }
            ModbusDataType::Uint32 => {
                let val = value.as_u32();
                if addr + 1 < regs.len() {
                    // Big-endian: high word first
                    regs[addr] = (val >> 16) as u16;
                    regs[addr + 1] = (val & 0xFFFF) as u16;
                }
            }
            ModbusDataType::Float32 => {
                let val = value.as_f32();
                let bits = val.to_bits();
                if addr + 1 < regs.len() {
                    // Big-endian: high word first
                    regs[addr] = (bits >> 16) as u16;
                    regs[addr + 1] = (bits & 0xFFFF) as u16;
                }
            }
        }
    }

    /// Update a variable's value by its ID.
    /// Returns true if the variable was found and updated.
    pub fn update_variable(&self, id: &str, value: ModbusValue) -> bool {
        let mut vars = self.variables.write();
        if let Some(var) = vars.get_mut(id) {
            var.value = value.clone();
            let var_clone = var.clone();
            drop(vars); // Release lock before writing to registers
            self.write_variable_value(&var_clone);
            true
        } else {
            false
        }
    }

    /// Get all current variables with their values.
    pub fn get_variables(&self) -> Vec<ModbusVariable> {
        self.variables.read().values().cloned().collect()
    }

    // ========== Coils (0x) ==========

    /// Read coils starting from address.
    pub fn read_coils(&self, start: u16, count: u16) -> Result<Vec<bool>, ExceptionCode> {
        let coils = self.coils.read();
        let start = start as usize;
        let end = start + count as usize;

        if end > coils.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        Ok(coils[start..end].to_vec())
    }

    /// Write a single coil.
    pub fn write_single_coil(&self, address: u16, value: bool) -> Result<(), ExceptionCode> {
        let mut coils = self.coils.write();
        let addr = address as usize;

        if addr >= coils.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        coils[addr] = value;
        self.sync_variable_from_coil(address, value);
        Ok(())
    }

    /// Write multiple coils.
    pub fn write_multiple_coils(&self, start: u16, values: &[bool]) -> Result<(), ExceptionCode> {
        let mut coils = self.coils.write();
        let start_addr = start as usize;
        let end_addr = start_addr + values.len();

        if end_addr > coils.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        for (i, &value) in values.iter().enumerate() {
            coils[start_addr + i] = value;
        }

        // Sync variables
        drop(coils);
        for (i, &value) in values.iter().enumerate() {
            self.sync_variable_from_coil(start + i as u16, value);
        }

        Ok(())
    }

    /// Sync a variable when a coil is written by master.
    fn sync_variable_from_coil(&self, address: u16, value: bool) {
        let mut vars = self.variables.write();
        for var in vars.values_mut() {
            if var.area == ModbusArea::Coil && var.address == address {
                var.value = ModbusValue::Bool(value);
            }
        }
    }

    // ========== Discrete Inputs (1x) ==========

    /// Read discrete inputs starting from address.
    pub fn read_discrete_inputs(&self, start: u16, count: u16) -> Result<Vec<bool>, ExceptionCode> {
        let inputs = self.discrete_inputs.read();
        let start = start as usize;
        let end = start + count as usize;

        if end > inputs.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        Ok(inputs[start..end].to_vec())
    }

    // ========== Holding Registers (4x) ==========

    /// Read holding registers starting from address.
    pub fn read_holding_registers(
        &self,
        start: u16,
        count: u16,
    ) -> Result<Vec<u16>, ExceptionCode> {
        let regs = self.holding_registers.read();
        let start = start as usize;
        let end = start + count as usize;

        if end > regs.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        Ok(regs[start..end].to_vec())
    }

    /// Write a single holding register.
    pub fn write_single_register(&self, address: u16, value: u16) -> Result<(), ExceptionCode> {
        let mut regs = self.holding_registers.write();
        let addr = address as usize;

        if addr >= regs.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        regs[addr] = value;
        drop(regs);
        self.sync_variable_from_register(ModbusArea::HoldingRegister, address);
        Ok(())
    }

    /// Write multiple holding registers.
    pub fn write_multiple_registers(
        &self,
        start: u16,
        values: &[u16],
    ) -> Result<(), ExceptionCode> {
        let mut regs = self.holding_registers.write();
        let start_addr = start as usize;
        let end_addr = start_addr + values.len();

        if end_addr > regs.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        for (i, &value) in values.iter().enumerate() {
            regs[start_addr + i] = value;
        }

        drop(regs);
        // Sync variables for each register that might have been written
        for i in 0..values.len() {
            self.sync_variable_from_register(ModbusArea::HoldingRegister, start + i as u16);
        }

        Ok(())
    }

    // ========== Input Registers (3x) ==========

    /// Read input registers starting from address.
    pub fn read_input_registers(&self, start: u16, count: u16) -> Result<Vec<u16>, ExceptionCode> {
        let regs = self.input_registers.read();
        let start = start as usize;
        let end = start + count as usize;

        if end > regs.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        Ok(regs[start..end].to_vec())
    }

    /// Sync a variable when a register is written by master.
    fn sync_variable_from_register(&self, area: ModbusArea, address: u16) {
        let regs = match area {
            ModbusArea::HoldingRegister => self.holding_registers.read(),
            ModbusArea::InputRegister => self.input_registers.read(),
            _ => return,
        };

        let mut vars = self.variables.write();
        for var in vars.values_mut() {
            if var.area == area && var.address == address {
                let addr = address as usize;
                let new_value = match var.data_type {
                    ModbusDataType::Bool => {
                        if addr < regs.len() {
                            ModbusValue::Bool(regs[addr] != 0)
                        } else {
                            continue;
                        }
                    }
                    ModbusDataType::Uint16 => {
                        if addr < regs.len() {
                            ModbusValue::Number(regs[addr] as f64)
                        } else {
                            continue;
                        }
                    }
                    ModbusDataType::Int16 => {
                        if addr < regs.len() {
                            ModbusValue::Number(regs[addr] as i16 as f64)
                        } else {
                            continue;
                        }
                    }
                    ModbusDataType::Uint32 => {
                        if addr + 1 < regs.len() {
                            let val = ((regs[addr] as u32) << 16) | (regs[addr + 1] as u32);
                            ModbusValue::Number(val as f64)
                        } else {
                            continue;
                        }
                    }
                    ModbusDataType::Float32 => {
                        if addr + 1 < regs.len() {
                            let bits = ((regs[addr] as u32) << 16) | (regs[addr + 1] as u32);
                            let val = f32::from_bits(bits);
                            ModbusValue::Number(val as f64)
                        } else {
                            continue;
                        }
                    }
                };
                var.value = new_value;
            }
        }
    }

    /// Clear all data areas to defaults.
    pub fn clear(&self) {
        {
            let mut coils = self.coils.write();
            for c in coils.iter_mut() {
                *c = false;
            }
        }
        {
            let mut inputs = self.discrete_inputs.write();
            for i in inputs.iter_mut() {
                *i = false;
            }
        }
        {
            let mut regs = self.input_registers.write();
            for r in regs.iter_mut() {
                *r = 0;
            }
        }
        {
            let mut regs = self.holding_registers.write();
            for r in regs.iter_mut() {
                *r = 0;
            }
        }
        {
            let mut vars = self.variables.write();
            vars.clear();
        }
    }
}

/// Shared reference to the data store.
pub type SharedDataStore = Arc<ModbusDataStore>;

/// Create a new shared data store.
pub fn create_shared_data_store() -> SharedDataStore {
    Arc::new(ModbusDataStore::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coils_read_write() {
        let store = ModbusDataStore::new();

        // Write single coil
        store.write_single_coil(0, true).unwrap();
        store.write_single_coil(5, true).unwrap();

        // Read coils
        let coils = store.read_coils(0, 10).unwrap();
        assert!(coils[0]);
        assert!(!coils[1]);
        assert!(coils[5]);
    }

    #[test]
    fn test_holding_registers_read_write() {
        let store = ModbusDataStore::new();

        // Write single register
        store.write_single_register(0, 0x1234).unwrap();
        store.write_single_register(1, 0x5678).unwrap();

        // Read registers
        let regs = store.read_holding_registers(0, 2).unwrap();
        assert_eq!(regs[0], 0x1234);
        assert_eq!(regs[1], 0x5678);
    }

    #[test]
    fn test_write_multiple_registers() {
        let store = ModbusDataStore::new();

        store
            .write_multiple_registers(10, &[100, 200, 300])
            .unwrap();

        let regs = store.read_holding_registers(10, 3).unwrap();
        assert_eq!(regs, vec![100, 200, 300]);
    }

    #[test]
    fn test_address_out_of_bounds() {
        let store = ModbusDataStore::new();

        // Try to read beyond bounds
        let result = store.read_coils(9999, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_variables() {
        let store = ModbusDataStore::new();

        let vars = vec![
            ModbusVariable {
                id: "var1".to_string(),
                name: "Test Coil".to_string(),
                area: ModbusArea::Coil,
                address: 0,
                data_type: ModbusDataType::Bool,
                value: ModbusValue::Bool(true),
                bit: None,
                readonly: None,
                note: None,
            },
            ModbusVariable {
                id: "var2".to_string(),
                name: "Test Register".to_string(),
                area: ModbusArea::HoldingRegister,
                address: 100,
                data_type: ModbusDataType::Uint16,
                value: ModbusValue::Number(12345.0),
                bit: None,
                readonly: None,
                note: None,
            },
        ];

        store.load_variables(&vars);

        // Verify coil
        let coils = store.read_coils(0, 1).unwrap();
        assert!(coils[0]);

        // Verify register
        let regs = store.read_holding_registers(100, 1).unwrap();
        assert_eq!(regs[0], 12345);
    }
}
