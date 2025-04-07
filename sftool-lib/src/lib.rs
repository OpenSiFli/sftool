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

// fn attempt_connect(base_param: &SifliToolBase, step: &mut i32) -> Result<(), Error> {
//     // 当 connect_attempts 小于等于 0 时视为无限重试，否则设定有限重试次数
//     let infinite_attempts = base_param.connect_attempts <= 0;
//     let mut remaining_attempts = if infinite_attempts {
//         None
//     } else {
//         Some(base_param.connect_attempts)
//     };
//
//     loop {
//         if base_param.before == Operation::DefaultReset {
//             // 使用RTS引脚复位
//             let mut port = serialport::new(&base_param.port_name, base_param.baud)
//                 .dtr_on_open(true)
//                 .timeout(Duration::from_secs(5))
//                 .open()
//                 .unwrap();
//             port.write_data_terminal_ready(false).unwrap();
//             port.write_request_to_send(true).unwrap();
//             std::thread::sleep(Duration::from_millis(100));
//             port.write_request_to_send(false).unwrap();
//             std::thread::sleep(Duration::from_millis(100));
//         }
//         // let value = probe.open()?.attach(base_param.chip.clone(), Permissions::default());
//         // 如果有限重试，检查是否还有机会
//         if let Some(ref mut attempts) = remaining_attempts {
//             if *attempts == 0 {
//                 break; // 超过最大重试次数则退出循环
//             }
//             *attempts -= 1;
//         }
//
//         let spinner = ProgressBar::new_spinner();
//         if !base_param.quiet {
//             spinner.enable_steady_tick(Duration::from_millis(100));
//             spinner.set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
//             spinner.set_prefix(format!("0x{:02X}", step));
//             *step = step.wrapping_add(1);
//             spinner.set_message("Connecting to chip...");
//         }
//
//         // 尝试连接
//         // match value {
//         //     Ok(session) => {
//         //         if !base_param.quiet {
//         //             spinner.finish_with_message("Connected success!");
//         //         }
//         //         return Ok(session);
//         //     }
//         //     Err(_) => {
//         //         if !base_param.quiet {
//         //             spinner.finish_with_message("Failed to connect to the chip, retrying...");
//         //         }
//         //         std::thread::sleep(Duration::from_millis(500));
//         //     }
//         // }
//     }
//
//     Err(Error::Probe(DebugProbeError::Other(
//         "Failed to connect to the chip".to_string(),
//     )))
// }

impl SifliTool {
    pub fn new(base_param: SifliToolBase, write_flash_params: Option<WriteFlashParams>) -> Self {
        let mut port = serialport::new(&base_param.port_name, 1000000)
            .timeout(Duration::from_secs(5))
            .open()
            .unwrap();
        port.write_request_to_send(false).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let step = 0;

        let mut tool = Self {
            port,
            step,
            base: base_param,
            write_flash_params,
        };
        tool.debug_command(SifliUartCommand::Enter).unwrap();
        tool
    }
}
