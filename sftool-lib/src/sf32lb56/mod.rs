//! SF32LB56 芯片特定实现模块

pub mod erase_flash;
pub mod ram_command;
pub mod read_flash;
pub mod reset;
pub mod sifli_debug;
pub mod speed;
pub mod write_flash;

use crate::common::sifli_debug::{
    ChipFrameFormat, RecvError, START_WORD, SifliDebug, SifliUartCommand, SifliUartResponse,
    common_debug,
};
use crate::sf32lb56::ram_command::DownloadStub;
use crate::{SifliTool, SifliToolBase, SifliToolTrait};
use serialport::SerialPort;
use std::io::{BufReader, Read};
use std::time::Duration;

// Define SF32LB56FrameFormat here to avoid import issues
pub struct SF32LB56FrameFormat;

impl ChipFrameFormat for SF32LB56FrameFormat {
    fn create_header(len: u16) -> Vec<u8> {
        let mut header = vec![];
        header.extend_from_slice(&START_WORD);
        // SF32LB56 uses big-endian for data length
        header.extend_from_slice(&len.to_be_bytes());
        // SF32LB56 adds 4-byte timestamp (fixed to 0)
        header.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        // Channel number fixed to 0x10
        header.push(0x10);
        // CRC fixed to 0x00
        header.push(0x00);
        // reserved field (2 bytes, fixed to 0x00)
        header.extend_from_slice(&[0x00, 0x00]);
        header
    }

    fn parse_frame_header(
        reader: &mut BufReader<Box<dyn Read + Send>>,
    ) -> Result<usize, RecvError> {
        // 读取长度 (2字节) - SF32LB56 uses big-endian
        let mut length_bytes = [0; 2];
        if let Err(e) = reader.read_exact(&mut length_bytes) {
            tracing::error!("Failed to read length bytes: {}", e);
            return Err(RecvError::InvalidHeaderLength);
        }

        let payload_size = u16::from_be_bytes(length_bytes) as usize;

        // 读取时间戳 (4字节) - SF32LB56 specific
        let mut timestamp_bytes = [0; 4];
        if let Err(e) = reader.read_exact(&mut timestamp_bytes) {
            tracing::error!("Failed to read timestamp bytes: {}", e);
            return Err(RecvError::InvalidHeaderChannel);
        }

        // 读取通道和CRC (2字节)
        let mut channel_crc = [0; 2];
        if let Err(e) = reader.read_exact(&mut channel_crc) {
            tracing::error!("Failed to read channel and CRC bytes: {}", e);
            return Err(RecvError::InvalidHeaderChannel);
        }

        // SF32LB56: Read 2-byte reserved field
        let mut reserved_bytes = [0; 2];
        if let Err(e) = reader.read_exact(&mut reserved_bytes) {
            tracing::error!("Failed to read reserved bytes: {}", e);
            return Err(RecvError::ReadError(e));
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
                // SF32LB56 uses big-endian for address and length
                send_data.extend_from_slice(&addr.to_be_bytes());
                send_data.extend_from_slice(&len.to_be_bytes());
            }
            SifliUartCommand::MEMWrite { addr, data } => {
                send_data.push(0x40);
                send_data.push(0x77);
                // SF32LB56 uses big-endian for address and data length
                send_data.extend_from_slice(&addr.to_be_bytes());
                send_data.extend_from_slice(&(data.len() as u16).to_be_bytes());
                for d in data.iter() {
                    send_data.extend_from_slice(&d.to_be_bytes());
                }
            }
        }
        send_data
    }

    fn decode_response_data(data: &[u8]) -> u32 {
        // SF32LB56 uses big-endian
        u32::from_be_bytes([data[0], data[1], data[2], data[3]])
    }

    // SF32LB56 specific address mapping function
    fn map_address(addr: u32) -> u32 {
        let mut l_addr = addr;
        if addr >= 0xE0000000 && addr < 0xF0000000 {
            l_addr = (addr & 0x0fffffff) | 0xF0000000;
            // l_addr = addr
        } else if addr >= 0x00400000 && addr <= 0x0041FFFF {
            // L_RAM
            l_addr += 0x20000000;
        } else if addr >= 0x20C00000 && addr <= 0x20C1FFFF {
            // L_RAM
            l_addr -= 0x00800000;
        } else if addr >= 0x20000000 && addr <= 0x200C7FFF {
            // H_RAM
            l_addr += 0x0A000000;
        } else if addr >= 0x20800000 && addr <= 0x20BFFFFF {
            // H_RAM
            l_addr -= 0x20800000;
        } else if addr >= 0x10000000 && addr <= 0x1FFFFFFF {
            // EXT_MEM
            l_addr += 0x50000000;
        }
        l_addr
    }
}

pub struct SF32LB56Tool {
    pub base: SifliToolBase,
    pub port: Box<dyn SerialPort>,
}

// 为 SF32LB56Tool 实现 Send 和 Sync
unsafe impl Send for SF32LB56Tool {}
unsafe impl Sync for SF32LB56Tool {}

// SifliDebug trait implementation for SF32LB56Tool
impl SifliDebug for SF32LB56Tool {
    fn debug_command(
        &mut self,
        command: SifliUartCommand,
    ) -> Result<SifliUartResponse, std::io::Error> {
        common_debug::debug_command_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self, command)
    }

    fn debug_read_word32(&mut self, addr: u32) -> Result<u32, std::io::Error> {
        common_debug::debug_read_word32_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self, addr)
    }

    fn debug_write_word32(&mut self, addr: u32, data: u32) -> Result<(), std::io::Error> {
        common_debug::debug_write_word32_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self, addr, data)
    }

    fn debug_write_memory(&mut self, addr: u32, data: &[u8]) -> Result<(), std::io::Error> {
        common_debug::debug_write_memory_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self, addr, data)
    }

    fn debug_write_core_reg(&mut self, reg: u16, data: u32) -> Result<(), std::io::Error> {
        common_debug::debug_write_core_reg_impl::<SF32LB56Tool, SF32LB56FrameFormat>(
            self, reg, data,
        )
    }

    fn debug_step(&mut self) -> Result<(), std::io::Error> {
        common_debug::debug_step_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self)
    }

    fn debug_run(&mut self) -> Result<(), std::io::Error> {
        common_debug::debug_run_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self)
    }

    fn debug_halt(&mut self) -> Result<(), std::io::Error> {
        common_debug::debug_halt_impl::<SF32LB56Tool, SF32LB56FrameFormat>(self)
    }
}

impl SF32LB56Tool {
    /// 执行全部flash擦除的内部方法
    pub fn internal_erase_all(&mut self, address: u32) -> Result<(), std::io::Error> {
        use ram_command::{Command, RamCommand};

        let progress = self.progress();
        let spinner =
            progress.create_spinner(format!("Erasing entire flash at 0x{:08X}...", address));

        // 发送擦除所有命令
        let _ = self.command(Command::EraseAll { address });

        let mut buffer = Vec::new();
        let now = std::time::SystemTime::now();

        // 等待擦除完成
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > 30000 {
                // 擦除可能需要更长时间
                tracing::error!("response string is {}", String::from_utf8_lossy(&buffer));
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Erase timeout",
                ));
            }

            let mut byte = [0];
            let ret = self.port().read_exact(&mut byte);
            if ret.is_err() {
                continue;
            }
            buffer.push(byte[0]);

            // 检查擦除完成响应
            if buffer.windows(2).any(|window| window == b"OK") {
                break;
            }
        }

        spinner.finish_with_message(format!("Erase flash successfully: 0x{:08X}", address));

        Ok(())
    }

    /// 执行区域擦除的内部方法
    pub fn internal_erase_region(&mut self, address: u32, len: u32) -> Result<(), std::io::Error> {
        use ram_command::{Command, RamCommand};

        let progress = self.progress();
        let spinner = progress.create_spinner(format!(
            "Erasing region at 0x{:08X} (size: 0x{:08X})...",
            address, len
        ));

        // 发送擦除区域命令
        let _ = self.command(Command::Erase { address, len });

        let mut buffer = Vec::new();
        let now = std::time::SystemTime::now();

        // 等待擦除完成
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > 30000 {
                // 擦除可能需要更长时间
                tracing::error!("response string is {}", String::from_utf8_lossy(&buffer));
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Erase timeout",
                ));
            }

            let mut byte = [0];
            let ret = self.port().read_exact(&mut byte);
            if ret.is_err() {
                continue;
            }
            buffer.push(byte[0]);

            // 检查擦除完成响应
            if buffer.windows(2).any(|window| window == b"OK") {
                break;
            }
        }

        spinner.finish_with_message(format!(
            "Erase region successfully: 0x{:08X} (length: 0x{:08X})",
            address, len
        ));

        Ok(())
    }

    pub fn attempt_connect(&mut self) -> Result<(), std::io::Error> {
        use crate::Operation;
        use crate::common::sifli_debug::{SifliUartCommand, SifliUartResponse};

        let infinite_attempts = self.base.connect_attempts <= 0;
        let mut remaining_attempts = if infinite_attempts {
            None
        } else {
            Some(self.base.connect_attempts)
        };
        loop {
            if self.base.before == Operation::DefaultReset {
                // 使用RTS引脚复位
                self.port.write_request_to_send(true)?;
                std::thread::sleep(Duration::from_millis(100));
                self.port.write_request_to_send(false)?;
                std::thread::sleep(Duration::from_millis(100));
            }
            let value = match self.debug_command(SifliUartCommand::Enter) {
                Ok(SifliUartResponse::Enter) => Ok(()),
                _ => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to enter debug mode",
                )),
            };
            // 如果有限重试，检查是否还有机会
            if let Some(ref mut attempts) = remaining_attempts {
                if *attempts == 0 {
                    break; // 超过最大重试次数则退出循环
                }
                *attempts -= 1;
            }

            let progress = self.progress();
            let spinner = progress.create_spinner("Connecting to chip...");

            // 尝试连接
            match value {
                Ok(_) => {
                    spinner.finish_with_message("Connected success!");
                    return Ok(());
                }
                Err(_) => {
                    spinner.finish_with_message("Failed to connect to the chip, retrying...");
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to connect to the chip",
        ))
    }

    pub fn download_stub_impl(&mut self) -> Result<(), std::io::Error> {
        use crate::common::sifli_debug::SifliUartCommand;
        use crate::ram_stub::{self, CHIP_FILE_NAME};
        use probe_rs::MemoryMappedRegister;
        use probe_rs::architecture::arm::core::armv7m::{Aircr, Demcr};
        use probe_rs::architecture::arm::core::registers::cortex_m::{PC, SP};

        let progress = self.progress();
        let spinner = progress.create_spinner("Download stub...");

        // 0.0 HCPU Unconditional halt
        self.debug_halt()?;
        //  0.1 HPSYS_AON->ISSR->LP_ACTIVE
        let mut data = self.debug_read_word32(0x4004_0028)?;
        if data & 0x20 != 0 {
            data = 0xa05f0003;
            // LCPU Halt
            self.debug_write_word32(0x3000_EDF0, data)?;
        }

        // 1. reset and halt
        //    1.1. reset_catch_set
        let demcr = self.debug_read_word32(Demcr::get_mmio_address() as u32)?;
        let mut demcr = Demcr(demcr);
        demcr.set_vc_corereset(true);
        self.debug_write_word32(Demcr::get_mmio_address() as u32, demcr.into())?;

        // 1.2. reset_system
        let mut aircr = Aircr(0);
        aircr.vectkey();
        aircr.set_sysresetreq(true);
        let _ = self.debug_write_word32(Aircr::get_mmio_address() as u32, aircr.into()); // MCU已经重启，不一定能收到正确回复
        std::thread::sleep(std::time::Duration::from_millis(10));

        // 1.3. Re-enter debug mode
        self.debug_command(SifliUartCommand::Enter)?;

        // 1.4. halt
        self.debug_halt()?;

        // 1.5. reset_catch_clear
        let demcr = self.debug_read_word32(Demcr::get_mmio_address() as u32)?;
        let mut demcr = Demcr(demcr);
        demcr.set_vc_corereset(false);
        self.debug_write_word32(Demcr::get_mmio_address() as u32, demcr.into())?;

        std::thread::sleep(std::time::Duration::from_millis(100));
        // 2. Download stub
        let stub = ram_stub::RamStubFile::get(
            CHIP_FILE_NAME
                .get(format!("sf32lb56_{}", self.base.memory_type).as_str())
                .expect("REASON"),
        );
        let Some(stub) = stub else {
            spinner.finish_with_message("No stub file found for the given chip and memory type");
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No stub file found for the given chip and memory type",
            ));
        };

        let packet_size = if self.base.compat { 256 } else { 64 * 1024 };

        let mut addr = 0x2006_7000;
        let mut data = &stub.data[..];
        while !data.is_empty() {
            let chunk = &data[..std::cmp::min(data.len(), packet_size)];
            self.debug_write_memory(addr, chunk)?;
            addr += chunk.len() as u32;
            data = &data[chunk.len()..];
        }

        // 3. run ram stub
        // 3.1. set SP and PC
        let sp = u32::from_le_bytes(
            stub.data[0..4]
                .try_into()
                .expect("slice with exactly 4 bytes"),
        );
        let pc = u32::from_le_bytes(
            stub.data[4..8]
                .try_into()
                .expect("slice with exactly 4 bytes"),
        );
        self.debug_write_core_reg(PC.id.0, pc)?;
        self.debug_write_core_reg(SP.id.0, sp)?;

        // 3.2. run
        self.debug_run()?;

        spinner.finish_with_message("Download stub success!");

        Ok(())
    }
}

impl SifliTool for SF32LB56Tool {
    fn create_tool(base: SifliToolBase) -> Box<dyn SifliTool> {
        let mut port = serialport::new(&base.port_name, 1000000)
            .timeout(Duration::from_secs(5))
            .open()
            .unwrap();
        port.write_request_to_send(false).unwrap();
        std::thread::sleep(Duration::from_millis(100));

        let mut tool = Box::new(Self { base, port });
        tool.download_stub().expect("Failed to download stub");
        tool
    }
}

impl SifliToolTrait for SF32LB56Tool {
    fn port(&mut self) -> &mut Box<dyn SerialPort> {
        &mut self.port
    }

    fn base(&self) -> &SifliToolBase {
        &self.base
    }

    fn set_speed(&mut self, baud: u32) -> Result<(), std::io::Error> {
        use crate::speed::SpeedTrait;
        SpeedTrait::set_speed(self, baud)
    }

    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        use crate::reset::Reset;
        Reset::soft_reset(self)
    }
}
