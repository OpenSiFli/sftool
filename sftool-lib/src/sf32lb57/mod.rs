//! SF32LB57 chip-specific implementation.

pub mod erase_flash;
pub mod ram_command;
pub mod read_flash;
pub mod reset;
pub mod sifli_debug;
pub mod speed;
pub mod write_flash;

use crate::common::serial_io::{for_tool, sleep_with_cancel};
use crate::common::sifli_debug::SifliDebug;
use crate::progress::{
    EraseFlashStyle, EraseRegionStyle, ProgressOperation, ProgressStatus, StubStage,
};
use crate::sf32lb57::ram_command::DownloadStub;
use crate::{Result, SifliTool, SifliToolBase, SifliToolTrait};
use serialport::SerialPort;
use std::time::Duration;

pub struct SF32LB57Tool {
    pub base: SifliToolBase,
    pub port: Box<dyn SerialPort>,
}

unsafe impl Send for SF32LB57Tool {}
unsafe impl Sync for SF32LB57Tool {}

impl SF32LB57Tool {
    pub fn internal_erase_all(&mut self, address: u32) -> Result<()> {
        use ram_command::{Command, RamCommand};

        let progress = self.progress();
        let spinner = progress.create_spinner(ProgressOperation::EraseFlash {
            address,
            style: EraseFlashStyle::Addressed,
        });

        let _ = self.command(Command::EraseAll { address });

        let mut io = for_tool(self);
        io.wait_for_pattern(
            b"OK",
            Duration::from_millis(30_000),
            &format!("erasing flash at 0x{:08X}", address),
        )?;

        spinner.finish(ProgressStatus::Success);

        Ok(())
    }

    pub fn internal_erase_region(&mut self, address: u32, len: u32) -> Result<()> {
        use ram_command::{Command, RamCommand};

        let progress = self.progress();
        let spinner = progress.create_spinner(ProgressOperation::EraseRegion {
            address,
            len,
            style: EraseRegionStyle::LegacyFlashStartDecimalLength,
        });

        let _ = self.command(Command::Erase { address, len });

        let timeout_ms = (len as u128 / (4 * 1024) + 1) * 800;
        tracing::info!(
            "Erase region at 0x{:08X} with length 0x{:08X}, timeout: {} ms",
            address,
            len,
            timeout_ms
        );

        let mut io = for_tool(self);
        io.wait_for_pattern(
            b"OK",
            Duration::from_millis(timeout_ms as u64),
            &format!("erasing region at 0x{:08X}", address),
        )?;

        spinner.finish(ProgressStatus::Success);

        Ok(())
    }

    fn attempt_connect(&mut self) -> Result<()> {
        use crate::common::sifli_debug::{SifliUartCommand, SifliUartResponse};

        let infinite_attempts = self.base.connect_attempts <= 0;
        let mut remaining_attempts = if infinite_attempts {
            None
        } else {
            Some(self.base.connect_attempts)
        };
        loop {
            if self.base.before.requires_reset() {
                let mut io = for_tool(self);
                io.write_request_to_send(true)?;
                io.sleep(Duration::from_millis(100))?;
                io.write_request_to_send(false)?;
                io.sleep(Duration::from_millis(100))?;
            }
            let value: Result<()> = match self.debug_command(SifliUartCommand::Enter) {
                Ok(SifliUartResponse::Enter) => Ok(()),
                _ => Err(std::io::Error::other("Failed to enter debug mode").into()),
            };
            if let Some(ref mut attempts) = remaining_attempts {
                if *attempts == 0 {
                    break;
                }
                *attempts -= 1;
            }

            let progress = self.progress();
            let spinner = progress.create_spinner(ProgressOperation::Connect);

            match value {
                Ok(_) => {
                    spinner.finish(ProgressStatus::Success);
                    return Ok(());
                }
                Err(_) => {
                    spinner.finish(ProgressStatus::Retry);
                    sleep_with_cancel(&self.base.cancel_token, Duration::from_millis(500))?;
                }
            }
        }
        Err(std::io::Error::other("Failed to connect to the chip").into())
    }

    fn download_stub_impl(&mut self) -> Result<()> {
        use crate::common::sifli_debug::SifliUartCommand;
        use crate::ram_stub::load_stub_file;
        use probe_rs::MemoryMappedRegister;
        use probe_rs::architecture::arm::core::armv7m::{Aircr, Demcr};
        use probe_rs::architecture::arm::core::registers::cortex_m::{PC, SP};

        let progress = self.progress();
        let spinner = progress.create_spinner(ProgressOperation::DownloadStub {
            stage: StubStage::Start,
        });

        let demcr = self.debug_read_word32(Demcr::get_mmio_address() as u32)?;
        let mut demcr = Demcr(demcr);
        demcr.set_vc_corereset(true);
        self.debug_write_word32(Demcr::get_mmio_address() as u32, demcr.into())?;

        let mut aircr = Aircr(0);
        aircr.vectkey();
        aircr.set_sysresetreq(true);
        let _ = self.debug_write_word32(Aircr::get_mmio_address() as u32, aircr.into());
        sleep_with_cancel(
            &self.base.cancel_token,
            std::time::Duration::from_millis(10),
        )?;

        self.debug_command(SifliUartCommand::Enter)?;
        self.debug_halt()?;

        let demcr = self.debug_read_word32(Demcr::get_mmio_address() as u32)?;
        let mut demcr = Demcr(demcr);
        demcr.set_vc_corereset(false);
        self.debug_write_word32(Demcr::get_mmio_address() as u32, demcr.into())?;

        sleep_with_cancel(
            &self.base.cancel_token,
            std::time::Duration::from_millis(100),
        )?;

        let chip_memory_key = format!("sf32lb57_{}", self.base.memory_type);
        let stub = match load_stub_file(self.base.external_stub_path.as_deref(), &chip_memory_key) {
            Ok(s) => s,
            Err(e) => {
                spinner.finish(ProgressStatus::NotFound);
                return Err(e.into());
            }
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

        let bkp0r_addr = 0x500c_b000 + 0x30;
        let bkp0r_value = 0xA640;
        self.debug_write_word32(bkp0r_addr, bkp0r_value)?;

        let gpio_dosr0_addr = 0x500a_0008;
        let mut gpio_dosr0_value = self.debug_read_word32(gpio_dosr0_addr)?;
        gpio_dosr0_value |= 1 << 21;
        self.debug_write_word32(gpio_dosr0_addr, gpio_dosr0_value)?;

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

        self.debug_run()?;

        spinner.finish(ProgressStatus::Success);

        Ok(())
    }
}

impl SifliTool for SF32LB57Tool {
    fn create_tool(base: SifliToolBase) -> Box<dyn SifliTool> {
        let mut port = serialport::new(&base.port_name, 1000000)
            .timeout(Duration::from_secs(5))
            .open()
            .unwrap();
        port.write_request_to_send(false).unwrap();
        std::thread::sleep(Duration::from_millis(100));

        let mut tool = Box::new(Self { base, port });
        if tool.base.before.should_download_stub() {
            tool.download_stub().expect("Failed to download stub");
        }
        tool
    }
}

impl SifliToolTrait for SF32LB57Tool {
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
