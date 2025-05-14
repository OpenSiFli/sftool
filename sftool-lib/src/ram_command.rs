use crate::ram_stub::CHIP_FILE_NAME;
use crate::sifli_debug::{SifliUartCommand, SifliUartResponse};
use crate::{Operation, SifliTool, ram_stub};
use indicatif::{ProgressBar, ProgressStyle};
use probe_rs::MemoryMappedRegister;
use probe_rs::architecture::arm::core::registers::cortex_m::{PC, SP};
use std::cmp::PartialEq;
use std::io::{Error, Write};
use std::str::FromStr;
use std::time::Duration;
use strum::{Display, EnumString};

#[derive(EnumString, Display, Debug, Clone, PartialEq, Eq)]
pub enum Command {
    #[strum(to_string = "burn_erase_all 0x{address:08x}\r")]
    EraseAll { address: u32 },

    #[strum(to_string = "burn_verify 0x{address:08x} 0x{len:08x} 0x{crc:08x}\r")]
    Verify { address: u32, len: u32, crc: u32 },

    #[strum(to_string = "burn_erase_write 0x{address:08x} 0x{len:08x}\r")]
    WriteAndErase { address: u32, len: u32 },

    #[strum(to_string = "burn_write 0x{address:08x} 0x{len:08x}\r")]
    Write { address: u32, len: u32 },

    #[strum(to_string = "burn_reset\r")]
    SoftReset,

    #[strum(to_string = "burn_speed {baud} {delay}\r")]
    SetBaud { baud: u32, delay: u32 },
}

#[derive(EnumString, Display, Debug, Clone, PartialEq, Eq)]
pub enum Response {
    #[strum(serialize = "OK")]
    Ok,
    #[strum(serialize = "Fail")]
    Fail,
    #[strum(serialize = "RX_WAIT")]
    RxWait,
}

const RESPONSE_STR_TABLE: [&str; 3] = ["OK", "Fail", "RX_WAIT"];

pub trait RamCommand {
    fn command(&mut self, cmd: Command) -> Result<Response, std::io::Error>;
    fn send_data(&mut self, data: &[u8]) -> Result<Response, std::io::Error>;
}

const TIMEOUT: u128 = 4000; //ms

impl RamCommand for SifliTool {
    fn command(&mut self, cmd: Command) -> Result<Response, std::io::Error> {
        self.port.write_all(cmd.to_string().as_bytes())?;
        self.port.flush()?;
        self.port.clear(serialport::ClearBuffer::All)?;

        let timeout = match cmd {
            Command::EraseAll { .. } => 30 * 1000,
            _ => TIMEOUT,
        };

        if let Command::SetBaud { .. } = cmd {
            return Ok(Response::Ok);
        }

        let mut buffer = Vec::new();
        let now = std::time::SystemTime::now();
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > timeout {
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout"));
            }

            let mut byte = [0];
            let ret = self.port.read_exact(&mut byte);
            if ret.is_err() {
                continue;
            }
            buffer.push(byte[0]);

            for response_str in RESPONSE_STR_TABLE.iter() {
                let response_bytes = response_str.as_bytes();
                // 对比buffer和response_bytes，如果buffer中包含response_str，就认为接收完毕
                // 不需要转成字符串，直接对比字节
                let exists = buffer
                    .windows(response_bytes.len())
                    .any(|window| window == response_bytes);
                if exists {
                    return Response::from_str(response_str).map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                    });
                }
            }
        }
    }

    fn send_data(&mut self, data: &[u8]) -> Result<Response, Error> {
        if !self.base.compat {
            self.port.write_all(data)?;
            self.port.flush()?;
        } else {
            // 每次只发256字节
            for chunk in data.chunks(256) {
                self.port.write_all(chunk)?;
                self.port.flush()?;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        let mut buffer = Vec::new();
        let now = std::time::SystemTime::now();
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > TIMEOUT {
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout"));
            }

            let mut byte = [0];
            let ret = self.port.read_exact(&mut byte);
            if ret.is_err() {
                continue;
            }
            buffer.push(byte[0]);

            // 一旦buffer出现RESPONSE_STR_TABLE中的任意一个，不一定是结束字节，也可能是在buffer中间出现，就认为接收完毕
            for response_str in RESPONSE_STR_TABLE.iter() {
                let response_bytes = response_str.as_bytes();
                let exists = buffer
                    .windows(response_bytes.len())
                    .any(|window| window == response_bytes);
                if exists {
                    return Response::from_str(response_str).map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                    });
                }
            }
        }
    }
}

pub trait DownloadStub {
    fn download_stub(&mut self) -> Result<(), std::io::Error>;
}

impl SifliTool {
    fn download_stub(&mut self) -> Result<(), std::io::Error> {
        let spinner = ProgressBar::new_spinner();
        if !self.base.quiet {
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));
            spinner.set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            spinner.set_prefix(format!("0x{:02X}", self.step));
            spinner.set_message("Download stub...");
        }
        self.step = self.step.wrapping_add(1);
        
        /// 1. reset and halt
        ///    1.1. reset_catch_set
        use probe_rs::architecture::arm::core::armv7m::Demcr;
        let demcr = self.debug_read_word32(Demcr::get_mmio_address() as u32)?;
        let mut demcr = Demcr(demcr);
        demcr.set_vc_corereset(true);
        self.debug_write_word32(Demcr::get_mmio_address() as u32, demcr.into())?;
        /// 1.2. reset_system
        use probe_rs::architecture::arm::core::armv7m::Aircr;

        let mut aircr = Aircr(0);
        aircr.vectkey();
        aircr.set_sysresetreq(true);
        let _ = self.debug_write_word32(Aircr::get_mmio_address() as u32, aircr.into()); // MCU已经重启，不一定能收到正确回复

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
                .get(format!("{}_{}", self.base.chip, self.base.memory_type).as_str())
                .expect("REASON"),
        );
        let Some(stub) = stub else {
            if !self.base.quiet {
                spinner.finish_with_message("No stub file found for the given chip and memory type");
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No stub file found for the given chip and memory type",
            ));
        };

        let packet_size = if self.base.compat { 256 } else { 64 * 1024 };

        let mut addr = 0x2005_A000;
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
        self.debug_write_core_reg(PC.id, pc)?;
        self.debug_write_core_reg(SP.id, sp)?;

        // 3.2. run
        self.debug_run()?;

        if !self.base.quiet {
            spinner.finish_with_message("Download stub success!");
        }

        Ok(())
    }

    fn attempt_connect(&mut self) -> Result<(), std::io::Error> {
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

            let spinner = ProgressBar::new_spinner();
            if !self.base.quiet {
                spinner.enable_steady_tick(Duration::from_millis(100));
                spinner
                    .set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
                spinner.set_prefix(format!("0x{:02X}", self.step));
                self.step = self.step.wrapping_add(1);
                spinner.set_message("Connecting to chip...");
            }

            // 尝试连接
            match value {
                Ok(_) => {
                    if !self.base.quiet {
                        spinner.finish_with_message("Connected success!");
                    }
                    return Ok(());
                }
                Err(_) => {
                    if !self.base.quiet {
                        spinner.finish_with_message("Failed to connect to the chip, retrying...");
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to connect to the chip",
        ))
    }
}

impl DownloadStub for SifliTool {
    fn download_stub(&mut self) -> Result<(), std::io::Error> {
        self.attempt_connect()?;
        self.download_stub()?;

        std::thread::sleep(std::time::Duration::from_millis(100));
        self.port.clear(serialport::ClearBuffer::All)?;
        self.debug_command(SifliUartCommand::Exit)?;

        // 1s之内串口发b"\r\n"字符串，并等待是否有"msh >"回复，200ms发一次b"\r\n"
        let mut buffer = Vec::new();
        let mut now = std::time::SystemTime::now();
        const RETRY: u32 = 5;
        let mut retry_count = 0;
        self.port.write_all(b"\r\n")?;
        self.port.flush()?;
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > 200 {
                retry_count += 1;
                now = std::time::SystemTime::now();
                self.port.write_all(b"\r\n")?;
                self.port.flush()?;
                buffer.clear();
            }
            if retry_count > RETRY {
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout"));
            }

            let mut byte = [0];
            let ret = self.port.read_exact(&mut byte);
            if ret.is_err() {
                continue;
            }
            buffer.push(byte[0]);

            // 一旦buffer出现"msh >"字符串，就认为接收完毕
            if buffer.windows(5).any(|window| window == b"msh >") {
                break;
            }
        }
        Ok(())
    }
}
