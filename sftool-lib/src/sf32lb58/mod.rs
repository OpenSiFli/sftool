//! SF32LB58 芯片特定实现模块

pub mod erase_flash;
pub mod read_flash;
pub mod reset;
pub mod speed;
pub mod write_flash;

use crate::{SifliTool, SifliToolBase, SifliToolTrait};
use serialport::SerialPort;

pub struct SF32LB58Tool {
    pub base: SifliToolBase,
    pub port: Box<dyn SerialPort>,
    pub step: i32,
}

impl SifliTool for SF32LB58Tool {
    fn create_tool(_base: SifliToolBase) -> Box<dyn SifliTool> {
        todo!("SF32LB58Tool::new not implemented yet");
    }
}

impl SifliToolTrait for SF32LB58Tool {
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

    fn set_speed(&mut self, _baud: u32) -> Result<(), std::io::Error> {
        todo!("SF32LB58Tool::set_speed not implemented yet")
    }

    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB58Tool::soft_reset not implemented yet")
    }
}
