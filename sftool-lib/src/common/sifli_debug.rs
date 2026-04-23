use crate::common::serial_io::{
    CancelableReader, CancelableWriter, for_tool, is_cancelled_io_error, sleep_with_cancel,
};
use crate::{Error, Result, SifliTool};
use probe_rs::architecture::arm::armv8m::Dcrdr;
use probe_rs::{MemoryMappedRegister, memory_mapped_bitfield_register};
use std::cmp::{max, min};
use std::fmt;
use std::io::{BufReader, BufWriter, Read, Write};
use std::time::{Duration, Instant};

pub const START_WORD: [u8; 2] = [0x7E, 0x79];
pub const DEFUALT_RECV_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub enum SifliUartCommand<'a> {
    Enter,
    Exit,
    MEMRead { addr: u32, len: u16 },
    MEMWrite { addr: u32, data: &'a [u32] },
}

#[derive(Debug)]
pub enum SifliUartResponse {
    Enter,
    Exit,
    MEMRead { data: Vec<u8> },
    MEMWrite,
}

#[derive(Debug)]
pub enum RecvError {
    Cancelled,
    Timeout,
    InvalidHeaderLength,
    InvalidHeaderChannel,
    ReadError(std::io::Error),
    InvalidResponse(u8),
}

impl From<RecvError> for Error {
    fn from(err: RecvError) -> Self {
        match err {
            RecvError::Cancelled => Error::Cancelled,
            RecvError::Timeout => Error::timeout("receiving UART frame"),
            RecvError::InvalidHeaderLength => Error::protocol("invalid frame length"),
            RecvError::InvalidHeaderChannel => Error::protocol("invalid frame channel information"),
            RecvError::ReadError(e) => Error::from(e),
            RecvError::InvalidResponse(code) => {
                Error::protocol(format!("invalid response code: {:#04X}", code))
            }
        }
    }
}

impl fmt::Display for SifliUartCommand<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SifliUartCommand::Enter => write!(f, "Enter"),
            SifliUartCommand::Exit => write!(f, "Exit"),
            SifliUartCommand::MEMRead { addr, len } => {
                write!(f, "MEMRead {{ addr: {:#X}, len: {:#X} }}", addr, len)
            }
            SifliUartCommand::MEMWrite { addr, data } => {
                write!(f, "MEMWrite {{ addr: {:#X}, data: [", addr)?;
                for (i, d) in data.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:#X}", d)?;
                }
                write!(f, "] }}")
            }
        }
    }
}

impl fmt::Display for SifliUartResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SifliUartResponse::Enter => write!(f, "Enter"),
            SifliUartResponse::Exit => write!(f, "Exit"),
            SifliUartResponse::MEMRead { data } => {
                write!(f, "MEMRead {{ data: [")?;
                for (i, byte) in data.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:#04X}", byte)?;
                }
                write!(f, "] }}")
            }
            SifliUartResponse::MEMWrite => write!(f, "MEMWrite"),
        }
    }
}

memory_mapped_bitfield_register! {
    pub struct Dcrsr(u32);
    0xE000_EDF4, "DCRSR",
    impl From;
    pub _, set_regwnr: 16;
    // If the processor does not implement the FP extension the REGSEL field is bits `[4:0]`, and bits `[6:5]` are Reserved, SBZ.
    pub _, set_regsel: 6,0;
}

memory_mapped_bitfield_register! {
    pub struct Dhcsr(u32);
    0xE000_EDF0, "DHCSR",
    impl From;
    pub s_reset_st, _: 25;
    pub s_retire_st, _: 24;
    pub s_lockup, _: 19;
    pub s_sleep, _: 18;
    pub s_halt, _: 17;
    pub s_regrdy, _: 16;
    pub c_maskints, set_c_maskints: 3;
    pub c_step, set_c_step: 2;
    pub c_halt, set_c_halt: 1;
    pub c_debugen, set_c_debugen: 0;
}

impl Dhcsr {
    /// This function sets the bit to enable writes to this register.
    ///
    /// C1.6.3 Debug Halting Control and Status Register, DHCSR:
    /// Debug key:
    /// Software must write 0xA05F to this field to enable write accesses to bits
    /// `[15:0]`, otherwise the processor ignores the write access.
    pub fn enable_write(&mut self) {
        self.0 &= !(0xffff << 16);
        self.0 |= 0xa05f << 16;
    }
}

pub trait SifliDebug {
    fn debug_command(&mut self, command: SifliUartCommand) -> Result<SifliUartResponse>;
    fn debug_write_word32(&mut self, addr: u32, data: u32) -> Result<()>;
    fn debug_read_word32(&mut self, addr: u32) -> Result<u32>;
    fn debug_write_core_reg(&mut self, reg: u16, data: u32) -> Result<()>;
    fn debug_write_memory(&mut self, addr: u32, data: &[u8]) -> Result<()>;
    fn debug_run(&mut self) -> Result<()>;
    fn debug_halt(&mut self) -> Result<()>;
    fn debug_step(&mut self) -> Result<()>;
}

// Trait defining chip-specific frame formatting behavior
pub trait ChipFrameFormat {
    /// Create chip-specific header with appropriate endianness and fields
    fn create_header(len: u16) -> Vec<u8>;

    /// Parse received frame header and return payload size
    fn parse_frame_header<R: Read>(reader: &mut BufReader<R>)
    -> std::result::Result<usize, RecvError>;

    /// Encode command data with chip-specific endianness
    fn encode_command_data(command: &SifliUartCommand) -> Vec<u8>;

    /// Decode response data with chip-specific endianness
    fn decode_response_data(data: &[u8]) -> u32;

    /// Apply chip-specific address mapping (default: no mapping)
    fn map_address(addr: u32) -> u32 {
        addr
    }
}

// Common implementation for communication
pub fn send_command<F: ChipFrameFormat, W: Write>(
    writer: &mut BufWriter<W>,
    command: &SifliUartCommand,
) -> Result<()> {
    let send_data = F::encode_command_data(command);
    let header = F::create_header(send_data.len() as u16);

    writer.write_all(&header).map_err(|error| {
        if is_cancelled_io_error(&error) {
            Error::Cancelled
        } else {
            error.into()
        }
    })?;
    writer.write_all(&send_data).map_err(|error| {
        if is_cancelled_io_error(&error) {
            Error::Cancelled
        } else {
            error.into()
        }
    })?;
    writer.flush().map_err(|error| {
        if is_cancelled_io_error(&error) {
            Error::Cancelled
        } else {
            error.into()
        }
    })?;
    Ok(())
}

pub fn recv_response<F: ChipFrameFormat, R: Read>(
    reader: &mut BufReader<R>,
) -> Result<SifliUartResponse> {
    let start_time = Instant::now();
    let mut temp: Vec<u8> = vec![];

    // 步骤1: 找到帧起始标记 (START_WORD)
    tracing::debug!("Waiting for frame start marker...");
    let mut buffer = vec![];

    loop {
        if start_time.elapsed() >= DEFUALT_RECV_TIMEOUT {
            tracing::warn!(
                "Receive timeout: {} seconds",
                DEFUALT_RECV_TIMEOUT.as_secs()
            );
            return Err(RecvError::Timeout.into());
        }

        let mut byte = [0; 1];
        match reader.read_exact(&mut byte) {
            Ok(_) => {
                // 处理帧检测逻辑
                if byte[0] == START_WORD[0] || (buffer.len() == 1 && byte[0] == START_WORD[1]) {
                    buffer.push(byte[0]);

                    // 检查是否找到完整的START_WORD
                    if buffer.ends_with(&START_WORD) {
                        tracing::debug!("Frame start marker found: {:02X?}", START_WORD);
                        break;
                    }
                } else {
                    // 重置缓冲区
                    buffer.clear();
                }

                // 缓冲区超过2个字节但没有匹配START_WORD，清除旧数据
                if buffer.len() > 2 {
                    buffer.clear();
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // 对于非阻塞IO，继续尝试
                continue;
            }
            Err(e) if is_cancelled_io_error(&e) => {
                return Err(RecvError::Cancelled.into());
            }
            Err(e) => {
                tracing::error!("Error reading frame start marker: {}", e);
                continue; // 继续尝试读取下一个字节
            }
        }
    }

    temp.extend_from_slice(&buffer);

    // 步骤2: 使用芯片特定的帧头解析
    let payload_size = F::parse_frame_header(reader)?;
    tracing::debug!("Received packet length: {} bytes", payload_size);

    // 步骤3: 读取有效载荷数据
    tracing::debug!("Reading payload data ({} bytes)...", payload_size);
    let mut recv_data = vec![];

    while recv_data.len() < payload_size {
        let mut byte = [0; 1];
        match reader.read_exact(&mut byte) {
            Ok(_) => {
                recv_data.push(byte[0]);
            }
            Err(e) if is_cancelled_io_error(&e) => {
                return Err(RecvError::Cancelled.into());
            }
            Err(e) => {
                tracing::error!("Failed to read payload data: {}", e);
                return Err(RecvError::ReadError(e).into());
            }
        }
    }

    temp.extend_from_slice(&recv_data);

    // 步骤4: 解析响应数据
    if recv_data.is_empty() {
        tracing::error!("Received empty payload data");
        return Err(RecvError::InvalidResponse(0).into());
    }

    let response_code = recv_data[0];
    match response_code {
        0xD1 => {
            tracing::info!("Received Enter command response");
            Ok(SifliUartResponse::Enter)
        }
        0xD0 => {
            tracing::info!("Received Exit command response");
            Ok(SifliUartResponse::Exit)
        }
        0xD2 => {
            // 提取数据部分，跳过响应代码和最后的校验字节
            let data = if recv_data.len() > 1 {
                recv_data[1..recv_data.len() - 1].to_vec()
            } else {
                Vec::new()
            };
            tracing::info!(
                "Received memory read response, data length: {} bytes",
                data.len()
            );
            Ok(SifliUartResponse::MEMRead { data })
        }
        0xD3 => {
            tracing::info!("Received memory write response");
            Ok(SifliUartResponse::MEMWrite)
        }
        _ => {
            tracing::error!("Received unknown response code: {:#04X}", response_code);
            Err(RecvError::InvalidResponse(response_code).into())
        }
    }
}

// Common helper functions that implement shared debug operations
pub mod common_debug {
    use super::*;

    /// Common implementation for debug_command
    pub fn debug_command_impl<T: SifliTool, F: ChipFrameFormat>(
        tool: &mut T,
        command: SifliUartCommand,
    ) -> Result<SifliUartResponse> {
        tracing::info!("Command: {}", command);
        let writer: CancelableWriter = {
            let mut io = for_tool(tool);
            io.try_clone_writer()?
        };
        let mut buf_writer = BufWriter::new(writer);

        let reader: CancelableReader = {
            let mut io = for_tool(tool);
            io.try_clone_reader()?
        };
        let mut buf_reader = BufReader::new(reader);

        let ret = send_command::<F, _>(&mut buf_writer, &command);
        if let Err(e) = ret {
            tracing::error!("Command send error: {:?}", e);
            return Err(e);
        }

        match command {
            SifliUartCommand::Exit => Ok(SifliUartResponse::Exit),
            _ => recv_response::<F, _>(&mut buf_reader),
        }
    }

    /// Common implementation for debug_read_word32
    pub fn debug_read_word32_impl<T: SifliTool, F: ChipFrameFormat>(
        tool: &mut T,
        addr: u32,
    ) -> Result<u32> {
        let mapped_addr = F::map_address(addr);
        let command = SifliUartCommand::MEMRead {
            addr: mapped_addr,
            len: 1,
        };

        // Call debug_command_impl directly instead of using the trait method
        match debug_command_impl::<T, F>(tool, command) {
            Ok(SifliUartResponse::MEMRead { data }) => {
                if data.len() != 4 {
                    return Err(Error::invalid_input("invalid response length"));
                }
                let value = F::decode_response_data(&data);
                Ok(value)
            }
            Ok(_) => Err(Error::invalid_input("invalid response")),
            Err(e) => Err(e),
        }
    }

    /// Common implementation for debug_write_word32
    pub fn debug_write_word32_impl<T: SifliTool, F: ChipFrameFormat>(
        tool: &mut T,
        addr: u32,
        data: u32,
    ) -> Result<()> {
        let mapped_addr = F::map_address(addr);
        let command = SifliUartCommand::MEMWrite {
            addr: mapped_addr,
            data: &[data],
        };
        match debug_command_impl::<T, F>(tool, command) {
            Ok(SifliUartResponse::MEMWrite) => Ok(()),
            Ok(_) => Err(Error::invalid_input("invalid response")),
            Err(e) => Err(e),
        }
    }

    /// Common implementation for debug_write_memory with chip-specific mapping
    pub fn debug_write_memory_impl<T: SifliTool, F: ChipFrameFormat>(
        tool: &mut T,
        address: u32,
        data: &[u8],
    ) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        // Apply chip-specific address mapping first
        let mut mapped_address = F::map_address(address);

        // Then apply the existing mapping logic (common to all chips)
        mapped_address = if (mapped_address & 0xff000000) == 0x12000000 {
            (mapped_address & 0x00ffffff) | 0x62000000
        } else {
            mapped_address
        };

        let addr_usize = mapped_address as usize;
        // Calculate the start address and end address after alignment
        let start_aligned = addr_usize - (addr_usize % 4);
        let end_aligned = (addr_usize + data.len()).div_ceil(4) * 4;
        let total_bytes = end_aligned - start_aligned;
        let total_words = total_bytes / 4;

        let mut buffer = vec![0u8; total_bytes];

        for i in 0..total_words {
            let block_addr = start_aligned + i * 4;
            let block_end = block_addr + 4;

            // Determine if the current 4-byte block is 'completely overwritten' by the new data written to it
            // If the block is completely in the new data area, then copy the new data directly
            if block_addr >= addr_usize && block_end <= addr_usize + data.len() {
                let offset_in_data = block_addr - addr_usize;
                buffer[i * 4..i * 4 + 4].copy_from_slice(&data[offset_in_data..offset_in_data + 4]);
            } else {
                // For the rest of the cases (header or tail incomplete overwrite):
                // Call MEMRead first to read out the original 4-byte block.
                let resp = debug_command_impl::<T, F>(
                    tool,
                    SifliUartCommand::MEMRead {
                        addr: block_addr as u32,
                        len: 1,
                    },
                )?;
                let mut block: [u8; 4] = match resp {
                    SifliUartResponse::MEMRead { data: d } if d.len() == 4 => {
                        // Apply chip-specific decoding for proper endianness
                        let value = F::decode_response_data(&d);
                        value.to_le_bytes()
                    }
                    _ => {
                        return Err(Error::invalid_input("invalid response length"));
                    }
                };
                // Calculate the overlap of the block with the new data area
                let overlap_start = max(block_addr, addr_usize);
                let overlap_end = min(block_end, addr_usize + data.len());
                if overlap_start < overlap_end {
                    let in_block_offset = overlap_start - block_addr;
                    let in_data_offset = overlap_start - addr_usize;
                    let overlap_len = overlap_end - overlap_start;
                    block[in_block_offset..in_block_offset + overlap_len]
                        .copy_from_slice(&data[in_data_offset..in_data_offset + overlap_len]);
                }
                buffer[i * 4..i * 4 + 4].copy_from_slice(&block);
            }
        }

        let words: Vec<u32> = buffer
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("chunk length is 4")))
            .collect();

        // Write the entire alignment area at once
        debug_command_impl::<T, F>(
            tool,
            SifliUartCommand::MEMWrite {
                addr: start_aligned as u32,
                data: &words,
            },
        )?;

        Ok(())
    }

    /// Common implementation for debug_write_core_reg
    pub fn debug_write_core_reg_impl<T: SifliTool, F: ChipFrameFormat>(
        tool: &mut T,
        addr: u16,
        value: u32,
    ) -> Result<()> {
        debug_write_word32_impl::<T, F>(tool, Dcrdr::get_mmio_address() as u32, value)?;

        let mut dcrsr_val = Dcrsr(0);
        dcrsr_val.set_regwnr(true); // Perform a write.
        dcrsr_val.set_regsel(addr.into()); // The address of the register to write.

        debug_write_word32_impl::<T, F>(tool, Dcrsr::get_mmio_address() as u32, dcrsr_val.into())?;

        // self.wait_for_core_register_transfer(Duration::from_millis(100))?;
        sleep_with_cancel(&tool.base().cancel_token, Duration::from_millis(10))?;
        Ok(())
    }

    /// Common implementation for debug_step
    pub fn debug_step_impl<T: SifliTool, F: ChipFrameFormat>(tool: &mut T) -> Result<()> {
        // 这里我们忽略了很多必要的检查，请参考probe-rs源码
        let mut value = Dhcsr(0);
        // Leave halted state.
        // Step one instruction.
        value.set_c_step(true);
        value.set_c_halt(false);
        value.set_c_debugen(true);
        value.set_c_maskints(true);
        value.enable_write();

        debug_write_word32_impl::<T, F>(tool, Dhcsr::get_mmio_address() as u32, value.into())?;

        sleep_with_cancel(&tool.base().cancel_token, Duration::from_millis(10))?;
        Ok(())
    }

    /// Common implementation for debug_run
    pub fn debug_run_impl<T: SifliTool, F: ChipFrameFormat>(tool: &mut T) -> Result<()> {
        debug_step_impl::<T, F>(tool)?;
        sleep_with_cancel(&tool.base().cancel_token, Duration::from_millis(100))?;
        let mut value = Dhcsr(0);
        value.set_c_halt(false);
        value.set_c_debugen(true);
        value.enable_write();

        debug_write_word32_impl::<T, F>(tool, Dhcsr::get_mmio_address() as u32, value.into())?;

        sleep_with_cancel(&tool.base().cancel_token, Duration::from_millis(100))?;
        Ok(())
    }

    /// Common implementation for debug_halt
    pub fn debug_halt_impl<T: SifliTool, F: ChipFrameFormat>(tool: &mut T) -> Result<()> {
        let mut value = Dhcsr(0);
        value.set_c_halt(true);
        value.set_c_debugen(true);
        value.enable_write();

        debug_write_word32_impl::<T, F>(tool, Dhcsr::get_mmio_address() as u32, value.into())?;
        sleep_with_cancel(&tool.base().cancel_token, Duration::from_millis(10))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{SifliUartCommand, common_debug};
    use crate::common::serial_io::test_support::TestSerialPort;
    use crate::progress::no_op_progress_sink;
    use crate::{
        BeforeOperation, CancelToken, EraseFlashParams, EraseFlashTrait, EraseRegionParams,
        ReadFlashParams, ReadFlashTrait, Result, SifliTool, SifliToolBase, SifliToolTrait,
        WriteFlashParams, WriteFlashTrait,
    };
    use serialport::SerialPort;
    use std::sync::{Arc, Mutex};

    struct TestTool {
        base: SifliToolBase,
        port: Box<dyn SerialPort>,
    }

    unsafe impl Send for TestTool {}
    unsafe impl Sync for TestTool {}

    impl SifliToolTrait for TestTool {
        fn port(&mut self) -> &mut Box<dyn SerialPort> {
            &mut self.port
        }

        fn base(&self) -> &SifliToolBase {
            &self.base
        }

        fn set_speed(&mut self, _baud: u32) -> Result<()> {
            Ok(())
        }

        fn soft_reset(&mut self) -> Result<()> {
            Ok(())
        }
    }

    impl WriteFlashTrait for TestTool {
        fn write_flash(&mut self, _params: &WriteFlashParams) -> Result<()> {
            Ok(())
        }
    }

    impl ReadFlashTrait for TestTool {
        fn read_flash(&mut self, _params: &ReadFlashParams) -> Result<()> {
            Ok(())
        }
    }

    impl EraseFlashTrait for TestTool {
        fn erase_flash(&mut self, _params: &EraseFlashParams) -> Result<()> {
            Ok(())
        }

        fn erase_region(&mut self, _params: &EraseRegionParams) -> Result<()> {
            Ok(())
        }
    }

    impl SifliTool for TestTool {
        fn create_tool(_base_param: SifliToolBase) -> Box<dyn SifliTool>
        where
            Self: Sized,
        {
            panic!("not used in tests")
        }
    }

    fn make_test_tool() -> (TestTool, Arc<Mutex<crate::common::serial_io::test_support::TestSerialPortState>>, CancelToken) {
        let token = CancelToken::new();
        let (port, state) = TestSerialPort::from_bytes(&[]);
        let base = SifliToolBase::new_with_external_stub_and_cancel(
            "test-port".to_string(),
            BeforeOperation::NoReset,
            "nor".to_string(),
            1_000_000,
            1,
            false,
            no_op_progress_sink(),
            None,
            token.clone(),
        );
        (
            TestTool {
                base,
                port: Box::new(port),
            },
            state,
            token,
        )
    }

    #[test]
    fn debug_command_impl_propagates_cancellation_from_cloned_streams() {
        let (mut tool, state, token) = make_test_tool();
        state.lock().unwrap().cancel_on_write_call = Some((1, token));

        let result = common_debug::debug_command_impl::<
            TestTool,
            crate::sf32lb52::sifli_debug::SF32LB52FrameFormat,
        >(&mut tool, SifliUartCommand::Enter);

        assert!(matches!(result, Err(crate::Error::Cancelled)));
    }
}
