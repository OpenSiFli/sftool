use super::SF32LB52Tool;
use crate::Result;
use crate::common::sifli_debug::{
    ChipFrameFormat, RecvError, START_WORD, SifliUartCommand, SifliUartResponse, common_debug,
};
use std::io::{BufReader, Read};

// Re-export for the module
pub use crate::common::sifli_debug::SifliDebug;

// SF32LB52 specific frame format implementation
pub struct SF32LB52FrameFormat;

impl ChipFrameFormat for SF32LB52FrameFormat {
    fn create_header(len: u16) -> Vec<u8> {
        let mut header = vec![];
        header.extend_from_slice(&START_WORD);
        // SF32LB52 uses little-endian for data length
        header.extend_from_slice(&len.to_le_bytes());
        header.push(0x10);
        header.push(0x00);
        header
    }

    fn parse_frame_header(
        reader: &mut BufReader<Box<dyn Read + Send>>,
    ) -> std::result::Result<usize, RecvError> {
        // 读取长度 (2字节) - SF32LB52 uses little-endian
        let mut length_bytes = [0; 2];
        if let Err(e) = reader.read_exact(&mut length_bytes) {
            tracing::error!("Failed to read length bytes: {}", e);
            return Err(RecvError::InvalidHeaderLength);
        }

        let payload_size = u16::from_le_bytes(length_bytes) as usize;

        // 读取通道和CRC (2字节)
        let mut channel_crc = [0; 2];
        if let Err(e) = reader.read_exact(&mut channel_crc) {
            tracing::error!("Failed to read channel and CRC bytes: {}", e);
            return Err(RecvError::InvalidHeaderChannel);
        }

        Ok(payload_size)
    }

    fn encode_command_data(command: &SifliUartCommand) -> Vec<u8> {
        let mut send_data = vec![];
        match command {
            SifliUartCommand::Enter => {
                let temp = [0x41, 0x54, 0x53, 0x46, 0x33, 0x32, 0x05, 0x21];
                send_data.extend_from_slice(&temp);
            }
            SifliUartCommand::Exit => {
                let temp = [0x41, 0x54, 0x53, 0x46, 0x33, 0x32, 0x18, 0x21];
                send_data.extend_from_slice(&temp);
            }
            SifliUartCommand::MEMRead { addr, len } => {
                send_data.push(0x40);
                send_data.push(0x72);
                // SF32LB52 uses little-endian for address and length
                send_data.extend_from_slice(&addr.to_le_bytes());
                send_data.extend_from_slice(&len.to_le_bytes());
            }
            SifliUartCommand::MEMWrite { addr, data } => {
                send_data.push(0x40);
                send_data.push(0x77);
                // SF32LB52 uses little-endian for address and data length
                send_data.extend_from_slice(&addr.to_le_bytes());
                send_data.extend_from_slice(&(data.len() as u16).to_le_bytes());
                for d in data.iter() {
                    send_data.extend_from_slice(&d.to_le_bytes());
                }
            }
        }
        send_data
    }

    fn decode_response_data(data: &[u8]) -> u32 {
        // SF32LB52 uses little-endian
        u32::from_le_bytes([data[0], data[1], data[2], data[3]])
    }

    // SF32LB52 uses no address mapping
    fn map_address(addr: u32) -> u32 {
        addr
    }
}

impl crate::common::sifli_debug::SifliDebug for SF32LB52Tool {
    fn debug_command(&mut self, command: SifliUartCommand) -> Result<SifliUartResponse> {
        common_debug::debug_command_impl::<SF32LB52Tool, SF32LB52FrameFormat>(self, command)
    }

    fn debug_read_word32(&mut self, addr: u32) -> Result<u32> {
        common_debug::debug_read_word32_impl::<SF32LB52Tool, SF32LB52FrameFormat>(self, addr)
    }

    fn debug_write_word32(&mut self, addr: u32, data: u32) -> Result<()> {
        common_debug::debug_write_word32_impl::<SF32LB52Tool, SF32LB52FrameFormat>(self, addr, data)
    }

    fn debug_write_memory(&mut self, addr: u32, data: &[u8]) -> Result<()> {
        common_debug::debug_write_memory_impl::<SF32LB52Tool, SF32LB52FrameFormat>(self, addr, data)
    }

    fn debug_write_core_reg(&mut self, reg: u16, data: u32) -> Result<()> {
        common_debug::debug_write_core_reg_impl::<SF32LB52Tool, SF32LB52FrameFormat>(
            self, reg, data,
        )
    }

    fn debug_step(&mut self) -> Result<()> {
        common_debug::debug_step_impl::<SF32LB52Tool, SF32LB52FrameFormat>(self)
    }

    fn debug_run(&mut self) -> Result<()> {
        common_debug::debug_run_impl::<SF32LB52Tool, SF32LB52FrameFormat>(self)
    }

    fn debug_halt(&mut self) -> Result<()> {
        common_debug::debug_halt_impl::<SF32LB52Tool, SF32LB52FrameFormat>(self)
    }
}
