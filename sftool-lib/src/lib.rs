mod ram_command;
mod ram_stub;
pub mod reset;
pub mod speed;
pub mod write_flash;

use indicatif::{ProgressBar, ProgressStyle};
use probe_rs::architecture::arm::armv8m::Dhcsr;
use probe_rs::architecture::arm::core::registers::cortex_m::{PC, SP};
use probe_rs::architecture::arm::dp::DpAddress;
use probe_rs::architecture::arm::FullyQualifiedApAddress;
use probe_rs::config::Chip;
use probe_rs::config::DebugSequence::Arm;
use probe_rs::probe::list::Lister;
use probe_rs::probe::sifliuart::SifliUart;
use probe_rs::probe::{DebugProbe, DebugProbeError, DebugProbeInfo};
use probe_rs::vendor::sifli::Sifli;
use probe_rs::vendor::Vendor;
use probe_rs::{
    Error, MemoryInterface, MemoryMappedRegister, Permissions, Session,
};
use ram_stub::CHIP_FILE_NAME;
use serialport::SerialPort;
use std::env;
use std::io::{Write};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum Operation {
    #[cfg_attr(feature = "cli", clap(name = "no_reset"))]
    None,
    #[cfg_attr(feature = "cli", clap(name = "soft_reset"))]
    SoftReset,
    #[cfg_attr(feature = "cli", clap(name = "default_reset"))]
    DefaultReset,
}

#[derive(Clone)]
pub struct SifliToolBase {
    pub port_name: String,
    pub before: Operation,
    pub chip: String,
    pub memory_type: String,
    pub baud: u32,
    pub connect_attempts: i8,
    pub compat: bool,
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
    step: i32,
    write_flash_params: Option<WriteFlashParams>,
}

fn attempt_connect(
    probe: DebugProbeInfo,
    base_param: &SifliToolBase,
    step: &mut i32,
) -> Result<Session, Error> {
    // 当 connect_attempts 小于等于 0 时视为无限重试，否则设定有限重试次数
    let infinite_attempts = base_param.connect_attempts <= 0;
    let mut remaining_attempts = if infinite_attempts {
        None
    } else {
        Some(base_param.connect_attempts)
    };

    loop {
        if base_param.before == Operation::DefaultReset {
            // 使用RTS引脚复位
            let mut port = serialport::new(&base_param.port_name, base_param.baud)
                .dtr_on_open(false)
                .timeout(Duration::from_secs(5))
                .open()
                .unwrap();
            port.write_data_terminal_ready(false).unwrap();
            port.write_request_to_send(true).unwrap();
            std::thread::sleep(Duration::from_millis(100));
            port.write_request_to_send(false).unwrap();
            std::thread::sleep(Duration::from_millis(100));
        }
        let value = probe.open()?.attach(base_param.chip.clone(), Permissions::default());
        // 如果有限重试，检查是否还有机会
        if let Some(ref mut attempts) = remaining_attempts {
            if *attempts == 0 {
                break; // 超过最大重试次数则退出循环
            }
            *attempts -= 1;
        }

        let spinner = ProgressBar::new_spinner();
        if !base_param.quiet {
            spinner.enable_steady_tick(Duration::from_millis(100));
            spinner.set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            spinner.set_prefix(format!("0x{:02X}", step));
            *step = step.wrapping_add(1);
            spinner.set_message("Connecting to chip...");
        }

        // 尝试连接
        match value {
            Ok(session) => {
                if !base_param.quiet {
                    spinner.finish_with_message("Connected success!");
                }
                return Ok(session);
            }
            Err(_) => {
                if !base_param.quiet {
                    spinner.finish_with_message("Failed to connect to the chip, retrying...");
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }

    Err(Error::Probe(DebugProbeError::Other(
        "Failed to connect to the chip".to_string(),
    )))
}

impl SifliTool {
    pub fn new(base_param: SifliToolBase, write_flash_params: Option<WriteFlashParams>) -> Self {
        let step = Self::download_stub(&base_param).unwrap();
        let mut port = serialport::new(&base_param.port_name, 1000000)
            .preserve_dtr_on_open()
            .timeout(Duration::from_secs(5))
            .open()
            .unwrap();
        port.write_request_to_send(false).unwrap();
        // Self::run(&port).unwrap();
        // std::thread::sleep(Duration::from_millis(500));
        let buf: [u8; 14] = [
            0x7E, 0x79, 0x08, 0x00, 0x10, 0x00, 0x41, 0x54, 0x53, 0x46, 0x33, 0x32, 0x18, 0x21,
        ];
        // Turn off the uart debug module again before transferring the data.
        port.write_all(&buf).unwrap();
        port.write_all("\r\n".as_bytes()).unwrap();
        port.flush().unwrap();
        port.clear(serialport::ClearBuffer::All).unwrap();

        Self {
            port,
            step,
            base: base_param,
            write_flash_params,
        }
    }

    fn run(serial: &Box<dyn SerialPort>) -> Result<(), std::io::Error> {
        let reader = serial.try_clone()?;
        let writer = reader.try_clone()?;
        let ser = serial.try_clone()?;
        let mut debug = SifliUart::new(Box::new(reader), Box::new(writer), ser).unwrap();
        debug.attach().unwrap();

        let mut interface = Box::new(debug)
            .try_get_arm_interface()
            .unwrap()
            .initialize(
                match (Sifli {}
                    .try_create_debug_sequence(&Chip {
                        name: "SF32LB52".to_string(),
                        part: None,
                        svd: None,
                        documentation: Default::default(),
                        package_variants: Default::default(),
                        cores: Default::default(),
                        memory_map: Default::default(),
                        flash_algorithms: Default::default(),
                        rtt_scan_ranges: Default::default(),
                        jtag: Default::default(),
                        default_binary_format: Default::default(),
                    })
                    .unwrap())
                {
                    Arm(arm) => arm,
                    _ => panic!("Invalid sequence"),
                },
                DpAddress::Default,
            )
            .unwrap();
        let mut interface = interface
            .memory_interface(&FullyQualifiedApAddress::v1_with_dp(DpAddress::Default, 0))
            .unwrap();
        let mut value = Dhcsr(0);
        // Leave halted state.
        // Step one instruction.
        value.set_c_step(true);
        value.set_c_halt(false);
        value.set_c_debugen(true);
        value.set_c_maskints(true);
        value.enable_write();

        interface
            .write_word_32(Dhcsr::get_mmio_address(), value.into())
            .unwrap();
        interface.flush().unwrap();

        let mut value = Dhcsr(0);
        value.set_c_halt(false);
        value.set_c_debugen(true);
        value.enable_write();

        interface
            .write_word_32(Dhcsr::get_mmio_address(), value.into())
            .unwrap();
        interface.flush().unwrap();

        Ok(())
    }

    fn download_stub(base_param: &SifliToolBase) -> Result<i32, std::io::Error> {
        let spinner = ProgressBar::new_spinner();
        let mut step = 0;

        unsafe {
            env::set_var("SIFLI_UART_DEBUG", "1");
        }

        let lister = Lister::new();
        let probes = lister.list_all();

        let index = probes.iter().enumerate().find_map(|(index, probe)| {
            probe.serial_number.as_ref().and_then(|s| {
                if s.contains(base_param.port_name.clone().as_str()) {
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
        let probe = probes[index].clone();

        let mut session = attempt_connect(probe, base_param, &mut step)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        if !base_param.quiet {
            spinner.enable_steady_tick(Duration::from_millis(100));
            spinner.set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            spinner.set_prefix(format!("0x{:02X}", step));
            step = step.wrapping_add(1);
            spinner.set_message("Downloading stub...");
        }

        let mut core = session
            .core(0)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        core.reset_and_halt(std::time::Duration::from_secs(5))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Download the stub
        let stub = ram_stub::RamStubFile::get(
            CHIP_FILE_NAME
                .get(format!("{}_{}", base_param.chip, base_param.memory_type).as_str())
                .expect("REASON"),
        );
        let Some(stub) = stub else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No stub file found for the given chip and memory type",
            ));
        };

        let packet_size = if base_param.compat { 256 } else { 64 * 1024 };

        let mut addr = 0x2005_A000;
        let mut data = &stub.data[..];
        while !data.is_empty() {
            let chunk = &data[..std::cmp::min(data.len(), packet_size)];
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
        std::thread::sleep(Duration::from_secs(1));

        if !base_param.quiet {
            spinner.finish_with_message("Stub download success!");
        }
        Ok(step)
    }
}
