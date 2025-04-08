pub mod ram_command;
mod ram_stub;
pub mod reset;
mod sifli_debug;
pub mod speed;
pub mod write_flash;

use crate::sifli_debug::SifliUartCommand;
use indicatif::{ProgressBar, ProgressStyle};
use ram_stub::CHIP_FILE_NAME;
use serialport::SerialPort;
use std::env;
use std::io::Write;
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

impl SifliTool {
    pub fn new(base_param: SifliToolBase, write_flash_params: Option<WriteFlashParams>) -> Self {
        let mut port = serialport::new(&base_param.port_name, 1000000)
            .timeout(Duration::from_secs(5))
            .open()
            .unwrap();
        port.write_request_to_send(false).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let step = 0;

        let tool = Self {
            port,
            step,
            base: base_param,
            write_flash_params,
        };

        tool
    }
}
