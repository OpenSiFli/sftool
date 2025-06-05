//! SF32LB52 芯片特定实现模块

use crate::{SifliToolBase, SubcommandParams, SifliTool};
use serialport::SerialPort;
use std::time::Duration;

pub struct SF32LB52Tool {
    pub base: SifliToolBase,
    pub port: Box<dyn SerialPort>,
    pub step: i32,
    pub subcommand_params: SubcommandParams,
}

impl SF32LB52Tool {
    pub fn new(base: SifliToolBase, subcommand_params: SubcommandParams) -> Box<dyn SifliTool> {
        let mut port = serialport::new(&base.port_name, 1000000)
            .timeout(Duration::from_secs(5))
            .open()
            .unwrap();
        port.write_request_to_send(false).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        
        Box::new(Self {
            base,
            port,
            step: 0,
            subcommand_params,
        })
    }
}

impl SifliTool for SF32LB52Tool {
    fn port(&mut self) -> &mut Box<dyn SerialPort> {
        &mut self.port
    }

    fn base(&self) -> &SifliToolBase {
        &self.base
    }

    fn step(&self) -> i32 {
        self.step
    }

    fn step_mut(&mut self) -> &mut i32 {
        &mut self.step
    }

    fn subcommand_params(&self) -> &SubcommandParams {
        &self.subcommand_params
    }

    fn execute_command(&mut self) -> Result<(), std::io::Error> {
        use crate::{write_flash, read_flash, erase_flash};
        
        match &self.subcommand_params {
            SubcommandParams::WriteFlashParams(_) => write_flash::WriteFlashTrait::write_flash(self),
            SubcommandParams::ReadFlashParams(_) => read_flash::ReadFlashTrait::read_flash(self),
            SubcommandParams::EraseFlashParams(_) => erase_flash::EraseFlashTrait::erase_flash(self),
            SubcommandParams::EraseRegionParams(_) => erase_flash::EraseFlashTrait::erase_region(self),
        }
    }
    
    fn debug_command(&mut self, cmd: crate::sifli_debug::SifliUartCommand) -> Result<crate::sifli_debug::SifliUartResponse, std::io::Error> {
        crate::sifli_debug::SifliDebug::debug_command(self, cmd)
    }
    
    fn debug_write_word32(&mut self, addr: u32, data: u32) -> Result<(), std::io::Error> {
        crate::sifli_debug::SifliDebug::debug_write_word32(self, addr, data)
    }
    
    fn debug_read_word32(&mut self, addr: u32) -> Result<u32, std::io::Error> {
        crate::sifli_debug::SifliDebug::debug_read_word32(self, addr)
    }
    
    fn debug_write_core_reg(&mut self, reg: u16, data: u32) -> Result<(), std::io::Error> {
        crate::sifli_debug::SifliDebug::debug_write_core_reg(self, reg, data)
    }
    
    fn debug_run(&mut self) -> Result<(), std::io::Error> {
        crate::sifli_debug::SifliDebug::debug_run(self)
    }
    
    fn debug_halt(&mut self) -> Result<(), std::io::Error> {
        crate::sifli_debug::SifliDebug::debug_halt(self)
    }
    
    fn debug_write_memory(&mut self, addr: u32, data: &[u8]) -> Result<(), std::io::Error> {
        crate::sifli_debug::SifliDebug::debug_write_memory(self, addr, data)
    }
    
    fn attempt_connect(&mut self) -> Result<(), std::io::Error> {
        use crate::sifli_debug::{SifliUartCommand, SifliUartResponse};
        use crate::Operation;
        
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

            use indicatif::{ProgressBar, ProgressStyle};
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
    
    fn download_stub_impl(&mut self) -> Result<(), std::io::Error> {
        use indicatif::{ProgressBar, ProgressStyle};
        use crate::sifli_debug::SifliUartCommand;
        use crate::ram_stub::{self, CHIP_FILE_NAME};
        use probe_rs::{MemoryMappedRegister};
        use probe_rs::architecture::arm::core::armv7m::{Demcr, Aircr};
        use probe_rs::architecture::arm::core::registers::cortex_m::{PC, SP};
        
        let spinner = ProgressBar::new_spinner();
        if !self.base.quiet {
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));
            spinner.set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            spinner.set_prefix(format!("0x{:02X}", self.step));
            spinner.set_message("Download stub...");
        }
        self.step = self.step.wrapping_add(1);
        
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
                .get(format!("SF32LB52_{}", self.base.memory_type).as_str())
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
        self.debug_write_core_reg(PC.id.0, pc)?;
        self.debug_write_core_reg(SP.id.0, sp)?;

        // 3.2. run
        self.debug_run()?;

        if !self.base.quiet {
            spinner.finish_with_message("Download stub success!");
        }

        Ok(())
    }
    
    fn download_stub(&mut self) -> Result<(), std::io::Error> {
        use crate::ram_command::DownloadStub;
        DownloadStub::download_stub(self)
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

impl crate::ram_command::RamCommand for SF32LB52Tool {
    fn command(&mut self, cmd: crate::ram_command::Command) -> Result<crate::ram_command::Response, std::io::Error> {
        use crate::ram_command::RESPONSE_STR_TABLE;
        use std::io::{Read, Write};
        use std::time::{SystemTime, UNIX_EPOCH};
        
        const TIMEOUT: u128 = 4000; // ms
        
        self.port.write_all(cmd.to_string().as_bytes())?;
        self.port.flush()?;
        
        let mut buffer = Vec::new();
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        
        let mut byte = [0];
        
        loop {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
            if now - start_time > TIMEOUT {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Timeout waiting for command response",
                ));
            }
            
            if self.port.read_exact(&mut byte).is_ok() {
                buffer.push(byte[0]);
                if buffer.len() >= 2 && buffer.ends_with(b"\r\n") {
                    break;
                }
            }
        }
        
        // Remove \r\n from the end
        if buffer.len() >= 2 {
            buffer.truncate(buffer.len() - 2);
        }
        
        let response_str = String::from_utf8_lossy(&buffer);
        for &expected in &RESPONSE_STR_TABLE {
            if response_str == expected {
                return Ok(expected.parse().unwrap());
            }
        }
        
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Unknown response: {}", response_str),
        ))
    }

    fn send_data(&mut self, data: &[u8]) -> Result<crate::ram_command::Response, std::io::Error> {
        use crate::ram_command::RESPONSE_STR_TABLE;
        use std::io::{Read, Write};
        use std::time::{SystemTime, UNIX_EPOCH};
        
        const TIMEOUT: u128 = 4000; // ms
        
        self.port.write_all(data)?;
        self.port.flush()?;
        
        let mut buffer = Vec::new();
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        
        let mut byte = [0];
        
        loop {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
            if now - start_time > TIMEOUT {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Timeout waiting for send_data response",
                ));
            }
            
            if self.port.read_exact(&mut byte).is_ok() {
                buffer.push(byte[0]);
                if buffer.len() >= 2 && buffer.ends_with(b"\r\n") {
                    break;
                }
            }
        }
        
        // Remove \r\n from the end
        if buffer.len() >= 2 {
            buffer.truncate(buffer.len() - 2);
        }
        
        let response_str = String::from_utf8_lossy(&buffer);
        for &expected in &RESPONSE_STR_TABLE {
            if response_str == expected {
                return Ok(expected.parse().unwrap());
            }
        }
        
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Unknown response: {}", response_str),
        ))
    }
}
