//! SF32LB52 芯片特定实现模块

pub mod erase_flash;
pub mod ram_command;
pub mod read_flash;
pub mod reset;
pub mod sifli_debug;
pub mod speed;
pub mod write_flash;

use self::sifli_debug::SifliDebug;
use crate::sf32lb52::ram_command::DownloadStub;
use crate::{SifliTool, SifliToolBase, SifliToolTrait};
use serialport::SerialPort;
use std::time::Duration;

pub struct SF32LB52Tool {
    pub base: SifliToolBase,
    pub port: Box<dyn SerialPort>,
    pub step: i32,
}

impl SF32LB52Tool {
    /// 执行全部flash擦除的内部方法
    pub fn internal_erase_all(&mut self, address: u32) -> Result<(), std::io::Error> {
        use indicatif::{ProgressBar, ProgressStyle};
        use ram_command::{Command, RamCommand};

        let progress_bar = ProgressBar::new_spinner();
        if !self.base().quiet {
            progress_bar.set_style(
                ProgressStyle::default_spinner()
                    .template("[{prefix}] Erasing entire flash at {msg}... {spinner}")
                    .unwrap(),
            );
            progress_bar.set_message(format!("0x{:08X}", address));
            progress_bar.set_prefix(format!("0x{:02X}", self.step));
            self.step = self.step.wrapping_add(1);
        }

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

        if !self.base().quiet {
            progress_bar
                .finish_with_message(format!("Erase flash successfully: 0x{:08X}", address));
        }

        Ok(())
    }

    /// 执行区域擦除的内部方法
    pub fn internal_erase_region(&mut self, address: u32, len: u32) -> Result<(), std::io::Error> {
        use indicatif::{ProgressBar, ProgressStyle};
        use ram_command::{Command, RamCommand};

        let progress_bar = ProgressBar::new(len as u64);
        if !self.base().quiet {
            progress_bar.set_style(
                ProgressStyle::default_bar()
                    .template("[{prefix}] Erasing region at {msg}... {wide_bar} {percent_precise}%")
                    .unwrap()
                    .progress_chars("=>-"),
            );
            progress_bar.set_message(format!("0x{:08X}", address));
            progress_bar.set_prefix(format!("0x{:02X}", self.step));
            self.step = self.step.wrapping_add(1);
        }

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

        if !self.base().quiet {
            progress_bar.finish_with_message(format!(
                "Erase region successfully: 0x{:08X} (length: 0x{:08X})",
                address, len
            ));
        }

        Ok(())
    }

    fn attempt_connect(&mut self) -> Result<(), std::io::Error> {
        use self::sifli_debug::{SifliUartCommand, SifliUartResponse};
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
        use self::sifli_debug::SifliUartCommand;
        use crate::ram_stub::{self, CHIP_FILE_NAME};
        use indicatif::{ProgressBar, ProgressStyle};
        use probe_rs::MemoryMappedRegister;
        use probe_rs::architecture::arm::core::armv7m::{Aircr, Demcr};
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
                .get(format!("sf32lb52_{}", self.base.memory_type).as_str())
                .expect("REASON"),
        );
        let Some(stub) = stub else {
            if !self.base.quiet {
                spinner
                    .finish_with_message("No stub file found for the given chip and memory type");
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
}

impl SifliTool for SF32LB52Tool {
    fn create_tool(base: SifliToolBase) -> Box<dyn SifliTool> {
        let mut port = serialport::new(&base.port_name, 1000000)
            .timeout(Duration::from_secs(5))
            .open()
            .unwrap();
        port.write_request_to_send(false).unwrap();
        std::thread::sleep(Duration::from_millis(100));

        let mut tool = Box::new(Self {
            base,
            port,
            step: 0,
        });
        tool.download_stub().expect("Failed to download stub");
        tool
    }
}

impl SifliToolTrait for SF32LB52Tool {
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

    fn set_speed(&mut self, baud: u32) -> Result<(), std::io::Error> {
        use crate::speed::SpeedTrait;
        SpeedTrait::set_speed(self, baud)
    }

    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        use crate::reset::Reset;
        Reset::soft_reset(self)
    }
}
