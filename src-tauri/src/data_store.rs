//! Хранилище данных для Modbus регистров и коилов.
//!
//! Этот модуль предоставляет потокобезопасное хранилище для областей данных Modbus:
//! - Coils (0x) - чтение/запись одиночных битов
//! - Discrete Inputs (1x) - только чтение одиночных битов
//! - Input Registers (3x) - только чтение 16-битных регистров
//! - Holding Registers (4x) - чтение/запись 16-битных регистров
//!
//! СТРОГАЯ ПРОВЕРКА АДРЕСОВ:
//! Сервер возвращает ошибку IllegalDataAddress для адресов,
//! по которым нет определённых переменных.

use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::modbus_protocol::ExceptionCode;
use crate::types::{ModbusArea, ModbusDataType, ModbusValue, ModbusVariable};

/// Размер по умолчанию для каждой области данных.
/// 65536 адресов (0..=65535), чтобы покрыть полный диапазон Modbus.
const DEFAULT_COILS_SIZE: usize = 65536;
const DEFAULT_DISCRETE_INPUTS_SIZE: usize = 65536;
const DEFAULT_INPUT_REGISTERS_SIZE: usize = 65536;
const DEFAULT_HOLDING_REGISTERS_SIZE: usize = 65536;

/// Потокобезопасное хранилище данных Modbus.
#[derive(Debug)]
pub struct ModbusDataStore {
    /// Coils (0x) - массив битов
    coils: RwLock<Vec<bool>>,
    /// Discrete Inputs (1x) - массив битов
    discrete_inputs: RwLock<Vec<bool>>,
    /// Input Registers (3x) - массив u16
    input_registers: RwLock<Vec<u16>>,
    /// Holding Registers (4x) - массив u16
    holding_registers: RwLock<Vec<u16>>,
    /// Соответствие ID переменной её определению (для быстрого поиска)
    variables: RwLock<HashMap<String, ModbusVariable>>,

    // === Множества определённых адресов для строгой проверки ===
    /// Определённые адреса coils
    defined_coils: RwLock<HashSet<u16>>,
    /// Определённые адреса discrete inputs
    defined_discrete_inputs: RwLock<HashSet<u16>>,
    /// Определённые адреса holding registers
    defined_holding_registers: RwLock<HashSet<u16>>,
    /// Определённые адреса input registers
    defined_input_registers: RwLock<HashSet<u16>>,
}

impl Default for ModbusDataStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ModbusDataStore {
    /// Создать новое хранилище данных с размерами по умолчанию.
    pub fn new() -> Self {
        Self {
            coils: RwLock::new(vec![false; DEFAULT_COILS_SIZE]),
            discrete_inputs: RwLock::new(vec![false; DEFAULT_DISCRETE_INPUTS_SIZE]),
            input_registers: RwLock::new(vec![0u16; DEFAULT_INPUT_REGISTERS_SIZE]),
            holding_registers: RwLock::new(vec![0u16; DEFAULT_HOLDING_REGISTERS_SIZE]),
            variables: RwLock::new(HashMap::new()),
            defined_coils: RwLock::new(HashSet::new()),
            defined_discrete_inputs: RwLock::new(HashSet::new()),
            defined_holding_registers: RwLock::new(HashSet::new()),
            defined_input_registers: RwLock::new(HashSet::new()),
        }
    }

    /// Инициализировать хранилище данных из списка переменных.
    /// Устанавливает начальные значения на основе определений переменных.
    pub fn load_variables(&self, variables: &[ModbusVariable]) {
        // Очищаем все данные
        {
            let mut vars_map = self.variables.write();
            vars_map.clear();
        }
        {
            let mut defined = self.defined_coils.write();
            defined.clear();
        }
        {
            let mut defined = self.defined_discrete_inputs.write();
            defined.clear();
        }
        {
            let mut defined = self.defined_holding_registers.write();
            defined.clear();
        }
        {
            let mut defined = self.defined_input_registers.write();
            defined.clear();
        }

        // Загружаем переменные
        for var in variables {
            // Сохраняем переменную
            {
                let mut vars_map = self.variables.write();
                vars_map.insert(var.id.clone(), var.clone());
            }

            // Отмечаем адреса как определённые
            self.mark_addresses_defined(var);

            // Записываем значение
            self.write_variable_value(var);
        }
    }

    /// Отметить адреса переменной как определённые.
    /// Для типов uint32 и float32 отмечаем 2 регистра.
    fn mark_addresses_defined(&self, var: &ModbusVariable) {
        let register_count = match var.data_type {
            ModbusDataType::Uint32 | ModbusDataType::Float32 => 2,
            _ => 1,
        };

        match var.area {
            ModbusArea::Coil => {
                let mut defined = self.defined_coils.write();
                defined.insert(var.address);
            }
            ModbusArea::DiscreteInput => {
                let mut defined = self.defined_discrete_inputs.write();
                defined.insert(var.address);
            }
            ModbusArea::HoldingRegister => {
                let mut defined = self.defined_holding_registers.write();
                for i in 0..register_count {
                    defined.insert(var.address + i);
                }
            }
            ModbusArea::InputRegister => {
                let mut defined = self.defined_input_registers.write();
                for i in 0..register_count {
                    defined.insert(var.address + i);
                }
            }
        }
    }

    /// Проверить, что все адреса в диапазоне определены.
    fn check_addresses_defined(
        &self,
        defined_set: &HashSet<u16>,
        start: u16,
        count: u16,
    ) -> Result<(), ExceptionCode> {
        for addr in start..(start + count) {
            if !defined_set.contains(&addr) {
                return Err(ExceptionCode::IllegalDataAddress);
            }
        }
        Ok(())
    }

    /// Записать значение одной переменной в соответствующую область данных.
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

    /// Записать значение в массив регистров в зависимости от типа данных.
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
                    // Big-endian: старшее слово первым
                    regs[addr] = (val >> 16) as u16;
                    regs[addr + 1] = (val & 0xFFFF) as u16;
                }
            }
            ModbusDataType::Float32 => {
                let val = value.as_f32();
                let bits = val.to_bits();
                if addr + 1 < regs.len() {
                    // Big-endian: старшее слово первым
                    regs[addr] = (bits >> 16) as u16;
                    regs[addr + 1] = (bits & 0xFFFF) as u16;
                }
            }
        }
    }

    /// Обновить значение переменной по её ID.
    /// Возвращает true, если переменная найдена и обновлена.
    pub fn update_variable(&self, id: &str, value: ModbusValue) -> bool {
        let mut vars = self.variables.write();
        if let Some(var) = vars.get_mut(id) {
            var.value = value.clone();
            let var_clone = var.clone();
            drop(vars); // Освобождаем блокировку перед записью в регистры
            self.write_variable_value(&var_clone);
            true
        } else {
            false
        }
    }

    /// Получить все текущие переменные с их значениями.
    pub fn get_variables(&self) -> Vec<ModbusVariable> {
        self.variables.read().values().cloned().collect()
    }

    // ========== Coils (0x) ==========

    /// Читать coils начиная с адреса.
    /// СТРОГАЯ ПРОВЕРКА: возвращает ошибку для неопределённых адресов.
    pub fn read_coils(&self, start: u16, count: u16) -> Result<Vec<bool>, ExceptionCode> {
        // Проверяем, что все адреса определены
        {
            let defined = self.defined_coils.read();
            self.check_addresses_defined(&defined, start, count)?;
        }

        let coils = self.coils.read();
        let start_idx = start as usize;
        let end_idx = start_idx + count as usize;

        if end_idx > coils.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        Ok(coils[start_idx..end_idx].to_vec())
    }

    /// Записать один coil.
    /// СТРОГАЯ ПРОВЕРКА: возвращает ошибку для неопределённых адресов.
    pub fn write_single_coil(&self, address: u16, value: bool) -> Result<(), ExceptionCode> {
        // Проверяем, что адрес определён
        {
            let defined = self.defined_coils.read();
            if !defined.contains(&address) {
                return Err(ExceptionCode::IllegalDataAddress);
            }
        }

        let mut coils = self.coils.write();
        let addr = address as usize;

        if addr >= coils.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        coils[addr] = value;
        drop(coils);
        self.sync_variable_from_coil(address, value);
        Ok(())
    }

    /// Записать несколько coils.
    /// СТРОГАЯ ПРОВЕРКА: возвращает ошибку для неопределённых адресов.
    pub fn write_multiple_coils(&self, start: u16, values: &[bool]) -> Result<(), ExceptionCode> {
        // Проверяем, что все адреса определены
        {
            let defined = self.defined_coils.read();
            self.check_addresses_defined(&defined, start, values.len() as u16)?;
        }

        let mut coils = self.coils.write();
        let start_addr = start as usize;
        let end_addr = start_addr + values.len();

        if end_addr > coils.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        for (i, &value) in values.iter().enumerate() {
            coils[start_addr + i] = value;
        }

        // Синхронизируем переменные
        drop(coils);
        for (i, &value) in values.iter().enumerate() {
            self.sync_variable_from_coil(start + i as u16, value);
        }

        Ok(())
    }

    /// Синхронизировать переменную когда coil записан мастером.
    fn sync_variable_from_coil(&self, address: u16, value: bool) {
        let mut vars = self.variables.write();
        for var in vars.values_mut() {
            if var.area == ModbusArea::Coil && var.address == address {
                var.value = ModbusValue::Bool(value);
            }
        }
    }

    // ========== Discrete Inputs (1x) ==========

    /// Читать discrete inputs начиная с адреса.
    /// СТРОГАЯ ПРОВЕРКА: возвращает ошибку для неопределённых адресов.
    pub fn read_discrete_inputs(&self, start: u16, count: u16) -> Result<Vec<bool>, ExceptionCode> {
        // Проверяем, что все адреса определены
        {
            let defined = self.defined_discrete_inputs.read();
            self.check_addresses_defined(&defined, start, count)?;
        }

        let inputs = self.discrete_inputs.read();
        let start_idx = start as usize;
        let end_idx = start_idx + count as usize;

        if end_idx > inputs.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        Ok(inputs[start_idx..end_idx].to_vec())
    }

    // ========== Holding Registers (4x) ==========

    /// Читать holding registers начиная с адреса.
    /// СТРОГАЯ ПРОВЕРКА: возвращает ошибку для неопределённых адресов.
    pub fn read_holding_registers(
        &self,
        start: u16,
        count: u16,
    ) -> Result<Vec<u16>, ExceptionCode> {
        // Проверяем, что все адреса определены
        {
            let defined = self.defined_holding_registers.read();
            self.check_addresses_defined(&defined, start, count)?;
        }

        let regs = self.holding_registers.read();
        let start_idx = start as usize;
        let end_idx = start_idx + count as usize;

        if end_idx > regs.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        Ok(regs[start_idx..end_idx].to_vec())
    }

    /// Записать один holding register.
    /// СТРОГАЯ ПРОВЕРКА: возвращает ошибку для неопределённых адресов.
    pub fn write_single_register(&self, address: u16, value: u16) -> Result<(), ExceptionCode> {
        // Проверяем, что адрес определён
        {
            let defined = self.defined_holding_registers.read();
            if !defined.contains(&address) {
                return Err(ExceptionCode::IllegalDataAddress);
            }
        }

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

    /// Записать несколько holding registers.
    /// СТРОГАЯ ПРОВЕРКА: возвращает ошибку для неопределённых адресов.
    pub fn write_multiple_registers(
        &self,
        start: u16,
        values: &[u16],
    ) -> Result<(), ExceptionCode> {
        // Проверяем, что все адреса определены
        {
            let defined = self.defined_holding_registers.read();
            self.check_addresses_defined(&defined, start, values.len() as u16)?;
        }

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
        // Синхронизируем переменные для каждого записанного регистра
        for i in 0..values.len() {
            self.sync_variable_from_register(ModbusArea::HoldingRegister, start + i as u16);
        }

        Ok(())
    }

    // ========== Input Registers (3x) ==========

    /// Читать input registers начиная с адреса.
    /// СТРОГАЯ ПРОВЕРКА: возвращает ошибку для неопределённых адресов.
    pub fn read_input_registers(&self, start: u16, count: u16) -> Result<Vec<u16>, ExceptionCode> {
        // Проверяем, что все адреса определены
        {
            let defined = self.defined_input_registers.read();
            self.check_addresses_defined(&defined, start, count)?;
        }

        let regs = self.input_registers.read();
        let start_idx = start as usize;
        let end_idx = start_idx + count as usize;

        if end_idx > regs.len() {
            return Err(ExceptionCode::IllegalDataAddress);
        }

        Ok(regs[start_idx..end_idx].to_vec())
    }

    /// Синхронизировать переменную когда регистр записан мастером.
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

    /// Очистить все данные в хранилище (сбросить все регистры и коилы к значениям по умолчанию).
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
        // Очищаем множества определённых адресов
        {
            let mut defined = self.defined_coils.write();
            defined.clear();
        }
        {
            let mut defined = self.defined_discrete_inputs.write();
            defined.clear();
        }
        {
            let mut defined = self.defined_holding_registers.write();
            defined.clear();
        }
        {
            let mut defined = self.defined_input_registers.write();
            defined.clear();
        }
    }
}

/// Общая ссылка на хранилище данных.
pub type SharedDataStore = Arc<ModbusDataStore>;

/// Создать новое общее хранилище данных.
pub fn create_shared_data_store() -> SharedDataStore {
    Arc::new(ModbusDataStore::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_validation_undefined_address() {
        let store = ModbusDataStore::new();

        // Без загруженных переменных чтение должно вернуть ошибку
        let result = store.read_holding_registers(0, 1);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ExceptionCode::IllegalDataAddress);
    }

    #[test]
    fn test_strict_validation_defined_address() {
        let store = ModbusDataStore::new();

        // Загружаем переменную
        let vars = vec![ModbusVariable {
            id: "var1".to_string(),
            name: "Test Register".to_string(),
            area: ModbusArea::HoldingRegister,
            address: 100,
            data_type: ModbusDataType::Uint16,
            value: ModbusValue::Number(12345.0),
            bit: None,
            readonly: None,
            note: None,
        }];

        store.load_variables(&vars);

        // Чтение определённого адреса должно работать
        let result = store.read_holding_registers(100, 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()[0], 12345);

        // Чтение неопределённого адреса должно вернуть ошибку
        let result = store.read_holding_registers(101, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_strict_validation_uint32_occupies_two_registers() {
        let store = ModbusDataStore::new();

        // Загружаем переменную uint32 (занимает 2 регистра)
        let vars = vec![ModbusVariable {
            id: "var1".to_string(),
            name: "Test Register".to_string(),
            area: ModbusArea::HoldingRegister,
            address: 50,
            data_type: ModbusDataType::Uint32,
            value: ModbusValue::Number(0x12345678 as f64),
            bit: None,
            readonly: None,
            note: None,
        }];

        store.load_variables(&vars);

        // Чтение обоих регистров должно работать
        let result = store.read_holding_registers(50, 2);
        assert!(result.is_ok());

        // Чтение только первого регистра тоже должно работать
        let result = store.read_holding_registers(50, 1);
        assert!(result.is_ok());

        // Чтение только второго регистра тоже должно работать
        let result = store.read_holding_registers(51, 1);
        assert!(result.is_ok());

        // Чтение третьего регистра (не определён) должно вернуть ошибку
        let result = store.read_holding_registers(52, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_coils_strict_validation() {
        let store = ModbusDataStore::new();

        // Загружаем coil
        let vars = vec![ModbusVariable {
            id: "coil1".to_string(),
            name: "Test Coil".to_string(),
            area: ModbusArea::Coil,
            address: 0,
            data_type: ModbusDataType::Bool,
            value: ModbusValue::Bool(true),
            bit: None,
            readonly: None,
            note: None,
        }];

        store.load_variables(&vars);

        // Чтение определённого coil должно работать
        let result = store.read_coils(0, 1);
        assert!(result.is_ok());
        assert!(result.unwrap()[0]);

        // Чтение неопределённого coil должно вернуть ошибку
        let result = store.read_coils(1, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_to_undefined_address_fails() {
        let store = ModbusDataStore::new();

        // Без загруженных переменных запись должна вернуть ошибку
        let result = store.write_single_register(0, 100);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ExceptionCode::IllegalDataAddress);
    }

    #[test]
    fn test_write_to_defined_address_works() {
        let store = ModbusDataStore::new();

        // Загружаем переменную
        let vars = vec![ModbusVariable {
            id: "var1".to_string(),
            name: "Test Register".to_string(),
            area: ModbusArea::HoldingRegister,
            address: 10,
            data_type: ModbusDataType::Uint16,
            value: ModbusValue::Number(0.0),
            bit: None,
            readonly: None,
            note: None,
        }];

        store.load_variables(&vars);

        // Запись в определённый адрес должна работать
        let result = store.write_single_register(10, 999);
        assert!(result.is_ok());

        // Проверяем, что значение записалось
        let result = store.read_holding_registers(10, 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()[0], 999);
    }
}
