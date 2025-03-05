mod ram_command;
mod ram_stub;
pub mod write_flash;

use console::Term;
use indicatif::{ProgressBar, ProgressStyle};
use probe_rs::architecture::arm::core::registers::cortex_m::{PC, SP};
use probe_rs::architecture::arm::sequences::ArmDebugSequence;
use probe_rs::probe::list::Lister;
use probe_rs::{MemoryInterface, Permissions, RegisterId, RegisterRole};
use ram_stub::CHIP_FILE_NAME;
use serialport;
use serialport::SerialPort;
use std::io::{Read, Write};
use std::time::Duration;

#[derive(Clone)]
pub struct SifliToolBase {
    pub port_name: String,
    pub chip: String,
    pub memory_type: String,
    pub quiet: bool,
}

#[derive(Clone)]
pub struct WriteFlashParams {
    pub file_path: Vec<String>,
    pub verify: bool,
    pub no_compress: bool,
    pub erase_all: bool,
}

pub struct SifliTool {
    port: Box<dyn SerialPort>,
    base: SifliToolBase,
    write_flash_params: Option<WriteFlashParams>,
}

impl SifliTool {
    pub fn new(base_param: SifliToolBase, write_flash_params: Option<WriteFlashParams>) -> Self {
        Self::download_stub(
            &base_param.port_name,
            &base_param.chip,
            &base_param.memory_type,
            base_param.quiet,
        )
        .unwrap();
        let mut port = serialport::new(&base_param.port_name, 1000000)
            .timeout(Duration::from_secs(5))
            .open()
            .unwrap();
        port.write_all("\r\n".as_bytes()).unwrap();
        port.flush().unwrap();
        port.clear(serialport::ClearBuffer::All).unwrap();

        Self {
            port,
            base: base_param,
            write_flash_params,
        }
    }

    fn download_stub(
        port_name: &str,
        chip: &str,
        memory_type: &str,
        quiet: bool,
    ) -> Result<(), std::io::Error> {
        let spinner = ProgressBar::new_spinner();
        if !quiet {
            spinner.enable_steady_tick(Duration::from_millis(100));
            spinner.set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            spinner.set_prefix("0x00");
            spinner.set_message("Connecting to chip...");
        }

        let lister = Lister::new();
        let probes = lister.list_all();

        let index = probes.iter().enumerate().find_map(|(index, probe)| {
            probe.serial_number.as_ref().and_then(|s| {
                if s.contains(port_name) {
                    Some(index)
                } else {
                    None
                }
            })
        });
        let Some(index) = index else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No probe found with the given serial number",
            ));
        };
        let probe = probes[index]
            .open()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let mut session = probe
            .attach(chip, Permissions::default())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let mut core = session
            .core(0)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        core.reset_and_halt(std::time::Duration::from_secs(5))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Download the stub
        let stub = ram_stub::RamStubFile::get(
            CHIP_FILE_NAME
                .get(format!("{}_{}", chip, memory_type).as_str())
                .expect("REASON"),
        );
        let Some(stub) = stub else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No stub file found for the given chip and memory type",
            ));
        };

        let mut addr = 0x2005_A000;
        let mut data = &stub.data[..];
        while !data.is_empty() {
            let chunk = &data[..std::cmp::min(data.len(), 64 * 1024)];
            core.write_8(addr, chunk)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            addr += chunk.len() as u64;
            data = &data[chunk.len()..];
        }

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
        tracing::info!("SP: {:#010x}, PC: {:#010x}", sp, pc);
        // set SP
        core.write_core_reg(SP.id, sp)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        // set PC
        core.write_core_reg(PC.id, pc)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        core.run()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::thread::sleep(Duration::from_millis(500));

        if !quiet {
            spinner.finish_with_message("Connected success!");
        }
        Ok(())
    }
}
