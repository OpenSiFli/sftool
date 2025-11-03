//! SF32LB52 芯片特定实现模块

pub mod erase_flash;
pub mod ram_command;
pub mod read_flash;
pub mod reset;
pub mod sifli_debug;
pub mod speed;
pub mod write_flash;

use crate::common::sifli_debug::SifliDebug;
use crate::sf32lb52::ram_command::DownloadStub;
use crate::{Result, SifliTool, SifliToolBase, SifliToolTrait};
use serialport::SerialPort;
use std::time::Duration;

pub struct SF32LB52Tool {
    pub base: SifliToolBase,
    pub port: Box<dyn SerialPort>,
}

// 为 SF32LB52Tool 实现 Send 和 Sync
// 注意：这假设串口操作在设计上是线程安全的，或者我们会确保同一时间只有一个线程访问
unsafe impl Send for SF32LB52Tool {}
unsafe impl Sync for SF32LB52Tool {}

impl SF32LB52Tool {
    /// 执行全部flash擦除的内部方法
    pub fn internal_erase_all(&mut self, address: u32) -> Result<()> {
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
                return Err(
                    std::io::Error::new(std::io::ErrorKind::TimedOut, "Erase timeout").into(),
                );
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
    pub fn internal_erase_region(&mut self, address: u32, len: u32) -> Result<()> {
        use ram_command::{Command, RamCommand};

        let progress = self.progress();
        let spinner =
            progress.create_spinner(format!("Erasing entire flash at 0x{:08X}...", address));

        // 发送擦除区域命令
        let _ = self.command(Command::Erase { address, len });

        let mut buffer = Vec::new();
        let now = std::time::SystemTime::now();

        let timeout_ms = (len as u128 / (4 * 1024) + 1) * 800; // 我们假设每擦除1个sector（4KB）最长时间不超过800ms
        tracing::info!(
            "Erase region at 0x{:08X} with length 0x{:08X}, timeout: {} ms",
            address,
            len,
            timeout_ms
        );

        // 等待擦除完成
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > timeout_ms {
                // 擦除可能需要更长时间
                tracing::error!("response string is {}", String::from_utf8_lossy(&buffer));
                return Err(
                    std::io::Error::new(std::io::ErrorKind::TimedOut, "Erase timeout").into(),
                );
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
            "Erase region successfully: 0x{:08X} (length: {} bytes)",
            address, len
        ));

        Ok(())
    }

    fn attempt_connect(&mut self) -> Result<()> {
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
            let value: Result<()> = match self.debug_command(SifliUartCommand::Enter) {
                Ok(SifliUartResponse::Enter) => Ok(()),
                _ => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to enter debug mode",
                )
                .into()),
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
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to connect to the chip").into())
    }

    fn download_stub_impl(&mut self) -> Result<()> {
        use crate::common::sifli_debug::SifliUartCommand;
        use crate::ram_stub::{self, CHIP_FILE_NAME};
        use probe_rs::MemoryMappedRegister;
        use probe_rs::architecture::arm::core::armv7m::{Aircr, Demcr};
        use probe_rs::architecture::arm::core::registers::cortex_m::{PC, SP};

        let progress = self.progress();
        let spinner = progress.create_spinner("Download stub...");

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
            spinner.finish_with_message("No stub file found for the given chip and memory type");
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No stub file found for the given chip and memory type",
            )
            .into());
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

        // 2.1.1 Set RTC->BKP0R to 0xA640
        // RTC->BKP0R address is 0x500cb000 + 0x30
        let bkp0r_addr = 0x500cb000 + 0x30;
        let bkp0r_value = 0xA640;
        self.debug_write_word32(bkp0r_addr, bkp0r_value)?;

        // 2.1.2 Set PA21 GPIO DOSR0
        let gpio_dosr0_addr = 0x500a0008;
        let mut gpio_dosr0_value = self.debug_read_word32(gpio_dosr0_addr)?;
        // PA21 is bit 21, set it to 1
        gpio_dosr0_value |= 1 << 21;
        self.debug_write_word32(gpio_dosr0_addr, gpio_dosr0_value)?;

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

impl SifliTool for SF32LB52Tool {
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

impl SifliToolTrait for SF32LB52Tool {
    fn port(&mut self) -> &mut Box<dyn SerialPort> {
        &mut self.port
    }

    fn base(&self) -> &SifliToolBase {
        &self.base
    }

    fn set_speed(&mut self, baud: u32) -> Result<()> {
        use crate::speed::SpeedTrait;
        SpeedTrait::set_speed(self, baud)
    }

    fn soft_reset(&mut self) -> Result<()> {
        use crate::reset::Reset;
        Reset::soft_reset(self)
    }
}
