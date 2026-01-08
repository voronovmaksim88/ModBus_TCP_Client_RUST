//! Modbus TCP protocol implementation.
//!
//! This module handles parsing and building Modbus TCP frames.
//! Modbus TCP frame structure:
//! - Transaction ID: 2 bytes
//! - Protocol ID: 2 bytes (always 0x0000 for Modbus)
//! - Length: 2 bytes (remaining bytes count)
//! - Unit ID: 1 byte
//! - Function Code: 1 byte
//! - Data: variable length

#![allow(dead_code)]

use std::io;

/// Modbus function codes supported by this slave simulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FunctionCode {
    /// Read Coils (0x01)
    ReadCoils = 0x01,
    /// Read Discrete Inputs (0x02)
    ReadDiscreteInputs = 0x02,
    /// Read Holding Registers (0x03)
    ReadHoldingRegisters = 0x03,
    /// Read Input Registers (0x04)
    ReadInputRegisters = 0x04,
    /// Write Single Coil (0x05)
    WriteSingleCoil = 0x05,
    /// Write Single Register (0x06)
    WriteSingleRegister = 0x06,
    /// Write Multiple Coils (0x0F)
    WriteMultipleCoils = 0x0F,
    /// Write Multiple Registers (0x10)
    WriteMultipleRegisters = 0x10,
}

impl FunctionCode {
    pub fn from_u8(code: u8) -> Option<Self> {
        match code {
            0x01 => Some(FunctionCode::ReadCoils),
            0x02 => Some(FunctionCode::ReadDiscreteInputs),
            0x03 => Some(FunctionCode::ReadHoldingRegisters),
            0x04 => Some(FunctionCode::ReadInputRegisters),
            0x05 => Some(FunctionCode::WriteSingleCoil),
            0x06 => Some(FunctionCode::WriteSingleRegister),
            0x0F => Some(FunctionCode::WriteMultipleCoils),
            0x10 => Some(FunctionCode::WriteMultipleRegisters),
            _ => None,
        }
    }
}

/// Modbus exception codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExceptionCode {
    /// Illegal Function (01)
    IllegalFunction = 0x01,
    /// Illegal Data Address (02)
    IllegalDataAddress = 0x02,
    /// Illegal Data Value (03)
    IllegalDataValue = 0x03,
    /// Server Device Failure (04)
    ServerDeviceFailure = 0x04,
}

/// MBAP (Modbus Application Protocol) header.
#[derive(Debug, Clone, Copy)]
pub struct MbapHeader {
    pub transaction_id: u16,
    pub protocol_id: u16,
    pub length: u16,
    pub unit_id: u8,
}

impl MbapHeader {
    pub const SIZE: usize = 7;

    pub fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "MBAP header too short",
            ));
        }

        let transaction_id = u16::from_be_bytes([data[0], data[1]]);
        let protocol_id = u16::from_be_bytes([data[2], data[3]]);
        let length = u16::from_be_bytes([data[4], data[5]]);
        let unit_id = data[6];

        // Protocol ID must be 0 for Modbus TCP
        if protocol_id != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid protocol ID (must be 0 for Modbus TCP)",
            ));
        }

        Ok(Self {
            transaction_id,
            protocol_id,
            length,
            unit_id,
        })
    }

    pub fn write_to(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.transaction_id.to_be_bytes());
        buf.extend_from_slice(&self.protocol_id.to_be_bytes());
        buf.extend_from_slice(&self.length.to_be_bytes());
        buf.push(self.unit_id);
    }
}

/// Parsed Modbus request.
#[derive(Debug, Clone)]
pub struct ModbusRequest {
    pub header: MbapHeader,
    pub function_code: u8,
    pub data: Vec<u8>,
}

impl ModbusRequest {
    /// Parse a complete Modbus TCP frame from bytes.
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < MbapHeader::SIZE + 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Frame too short",
            ));
        }

        let header = MbapHeader::parse(data)?;

        // Check if we have complete frame
        let expected_len = MbapHeader::SIZE - 1 + header.length as usize;
        if data.len() < expected_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Incomplete frame: expected {} bytes, got {}",
                    expected_len,
                    data.len()
                ),
            ));
        }

        let function_code = data[MbapHeader::SIZE];
        let pdu_data_start = MbapHeader::SIZE + 1;
        let pdu_data_end = MbapHeader::SIZE - 1 + header.length as usize;
        let request_data = data[pdu_data_start..pdu_data_end].to_vec();

        Ok(Self {
            header,
            function_code,
            data: request_data,
        })
    }

    /// Get the expected frame length from MBAP header.
    /// Returns None if buffer is too short to read header.
    pub fn expected_frame_length(data: &[u8]) -> Option<usize> {
        if data.len() < 6 {
            return None;
        }
        let length = u16::from_be_bytes([data[4], data[5]]) as usize;
        Some(MbapHeader::SIZE - 1 + length)
    }
}

/// Modbus response builder.
pub struct ModbusResponse;

impl ModbusResponse {
    /// Build a successful response with data.
    pub fn build_response(request: &ModbusRequest, function_code: u8, data: &[u8]) -> Vec<u8> {
        let mut response = Vec::with_capacity(MbapHeader::SIZE + 1 + data.len());

        // MBAP header
        let length = 2 + data.len() as u16; // unit_id + function_code + data
        let header = MbapHeader {
            transaction_id: request.header.transaction_id,
            protocol_id: 0,
            length,
            unit_id: request.header.unit_id,
        };
        header.write_to(&mut response);

        // PDU
        response.push(function_code);
        response.extend_from_slice(data);

        response
    }

    /// Build an exception response.
    pub fn build_exception(
        request: &ModbusRequest,
        function_code: u8,
        exception_code: ExceptionCode,
    ) -> Vec<u8> {
        let mut response = Vec::with_capacity(MbapHeader::SIZE + 2);

        // MBAP header
        let header = MbapHeader {
            transaction_id: request.header.transaction_id,
            protocol_id: 0,
            length: 3, // unit_id + error_code + exception_code
            unit_id: request.header.unit_id,
        };
        header.write_to(&mut response);

        // PDU with error flag (function_code | 0x80)
        response.push(function_code | 0x80);
        response.push(exception_code as u8);

        response
    }
}

/// Read request parameters (for functions 0x01-0x04).
#[derive(Debug, Clone, Copy)]
pub struct ReadRequest {
    pub start_address: u16,
    pub quantity: u16,
}

impl ReadRequest {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Read request data too short",
            ));
        }

        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);

        Ok(Self {
            start_address,
            quantity,
        })
    }

    /// Validate read coils/discrete inputs request (max 2000 bits).
    pub fn validate_bits(&self) -> Result<(), ExceptionCode> {
        if self.quantity == 0 || self.quantity > 2000 {
            return Err(ExceptionCode::IllegalDataValue);
        }
        Ok(())
    }

    /// Validate read registers request (max 125 registers).
    pub fn validate_registers(&self) -> Result<(), ExceptionCode> {
        if self.quantity == 0 || self.quantity > 125 {
            return Err(ExceptionCode::IllegalDataValue);
        }
        Ok(())
    }
}

/// Write single coil request (function 0x05).
#[derive(Debug, Clone, Copy)]
pub struct WriteSingleCoilRequest {
    pub address: u16,
    pub value: bool,
}

impl WriteSingleCoilRequest {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Write single coil request data too short",
            ));
        }

        let address = u16::from_be_bytes([data[0], data[1]]);
        let value_raw = u16::from_be_bytes([data[2], data[3]]);

        // Value must be 0x0000 (OFF) or 0xFF00 (ON)
        let value = match value_raw {
            0x0000 => false,
            0xFF00 => true,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid coil value (must be 0x0000 or 0xFF00)",
                ))
            }
        };

        Ok(Self { address, value })
    }

    pub fn to_response_data(&self) -> [u8; 4] {
        let value_raw: u16 = if self.value { 0xFF00 } else { 0x0000 };
        let mut data = [0u8; 4];
        data[0..2].copy_from_slice(&self.address.to_be_bytes());
        data[2..4].copy_from_slice(&value_raw.to_be_bytes());
        data
    }
}

/// Write single register request (function 0x06).
#[derive(Debug, Clone, Copy)]
pub struct WriteSingleRegisterRequest {
    pub address: u16,
    pub value: u16,
}

impl WriteSingleRegisterRequest {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Write single register request data too short",
            ));
        }

        let address = u16::from_be_bytes([data[0], data[1]]);
        let value = u16::from_be_bytes([data[2], data[3]]);

        Ok(Self { address, value })
    }

    pub fn to_response_data(&self) -> [u8; 4] {
        let mut data = [0u8; 4];
        data[0..2].copy_from_slice(&self.address.to_be_bytes());
        data[2..4].copy_from_slice(&self.value.to_be_bytes());
        data
    }
}

/// Write multiple coils request (function 0x0F).
#[derive(Debug, Clone)]
pub struct WriteMultipleCoilsRequest {
    pub start_address: u16,
    pub quantity: u16,
    pub values: Vec<bool>,
}

impl WriteMultipleCoilsRequest {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Write multiple coils request data too short",
            ));
        }

        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        let byte_count = data[4] as usize;

        let expected_bytes = (quantity as usize + 7) / 8;
        if byte_count != expected_bytes || data.len() < 5 + byte_count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid byte count in write multiple coils request",
            ));
        }

        // Unpack bits
        let mut values = Vec::with_capacity(quantity as usize);
        for i in 0..quantity as usize {
            let byte_index = 5 + i / 8;
            let bit_index = i % 8;
            let bit_value = (data[byte_index] >> bit_index) & 1;
            values.push(bit_value == 1);
        }

        Ok(Self {
            start_address,
            quantity,
            values,
        })
    }

    pub fn validate(&self) -> Result<(), ExceptionCode> {
        if self.quantity == 0 || self.quantity > 1968 {
            return Err(ExceptionCode::IllegalDataValue);
        }
        Ok(())
    }

    pub fn to_response_data(&self) -> [u8; 4] {
        let mut data = [0u8; 4];
        data[0..2].copy_from_slice(&self.start_address.to_be_bytes());
        data[2..4].copy_from_slice(&self.quantity.to_be_bytes());
        data
    }
}

/// Write multiple registers request (function 0x10).
#[derive(Debug, Clone)]
pub struct WriteMultipleRegistersRequest {
    pub start_address: u16,
    pub quantity: u16,
    pub values: Vec<u16>,
}

impl WriteMultipleRegistersRequest {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Write multiple registers request data too short",
            ));
        }

        let start_address = u16::from_be_bytes([data[0], data[1]]);
        let quantity = u16::from_be_bytes([data[2], data[3]]);
        let byte_count = data[4] as usize;

        let expected_bytes = quantity as usize * 2;
        if byte_count != expected_bytes || data.len() < 5 + byte_count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid byte count in write multiple registers request",
            ));
        }

        // Read register values
        let mut values = Vec::with_capacity(quantity as usize);
        for i in 0..quantity as usize {
            let offset = 5 + i * 2;
            let value = u16::from_be_bytes([data[offset], data[offset + 1]]);
            values.push(value);
        }

        Ok(Self {
            start_address,
            quantity,
            values,
        })
    }

    pub fn validate(&self) -> Result<(), ExceptionCode> {
        if self.quantity == 0 || self.quantity > 123 {
            return Err(ExceptionCode::IllegalDataValue);
        }
        Ok(())
    }

    pub fn to_response_data(&self) -> [u8; 4] {
        let mut data = [0u8; 4];
        data[0..2].copy_from_slice(&self.start_address.to_be_bytes());
        data[2..4].copy_from_slice(&self.quantity.to_be_bytes());
        data
    }
}

/// Helper to pack boolean values into bytes (LSB first within each byte).
pub fn pack_bits(bits: &[bool]) -> Vec<u8> {
    let byte_count = (bits.len() + 7) / 8;
    let mut bytes = vec![0u8; byte_count];

    for (i, &bit) in bits.iter().enumerate() {
        if bit {
            let byte_index = i / 8;
            let bit_index = i % 8;
            bytes[byte_index] |= 1 << bit_index;
        }
    }

    bytes
}

/// Helper to pack u16 values into bytes (big-endian).
pub fn pack_registers(registers: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(registers.len() * 2);
    for &reg in registers {
        bytes.extend_from_slice(&reg.to_be_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mbap_header_parse() {
        let data = [0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01];
        let header = MbapHeader::parse(&data).unwrap();
        assert_eq!(header.transaction_id, 1);
        assert_eq!(header.protocol_id, 0);
        assert_eq!(header.length, 6);
        assert_eq!(header.unit_id, 1);
    }

    #[test]
    fn test_read_request_parse() {
        let data = [0x00, 0x00, 0x00, 0x0A]; // start=0, quantity=10
        let req = ReadRequest::parse(&data).unwrap();
        assert_eq!(req.start_address, 0);
        assert_eq!(req.quantity, 10);
    }

    #[test]
    fn test_pack_bits() {
        let bits = vec![true, false, true, true, false, false, false, false, true];
        let packed = pack_bits(&bits);
        assert_eq!(packed, vec![0b00001101, 0b00000001]);
    }

    #[test]
    fn test_pack_registers() {
        let regs = vec![0x0102, 0x0304];
        let packed = pack_registers(&regs);
        assert_eq!(packed, vec![0x01, 0x02, 0x03, 0x04]);
    }
}
