//! SF32LB58 芯片特定实现模块

pub mod write_flash;
pub mod read_flash;
pub mod erase_flash;
pub mod reset;
pub mod speed;

use crate::{SifliTool, SifliToolBase, SubcommandParams};
use serialport::SerialPort;

pub struct SF32LB58Tool {
    pub base: SifliToolBase,
    pub port: Box<dyn SerialPort>,
    pub step: i32,
    pub subcommand_params: SubcommandParams,
}

impl SF32LB58Tool {
    pub fn new(_base: SifliToolBase, _subcommand_params: SubcommandParams) -> Box<dyn SifliTool> {
        todo!("SF32LB58Tool::new not implemented yet");
    }
}

impl SifliTool for SF32LB58Tool {
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
        todo!("SF32LB58 execute_command implementation")
    }
    
    fn attempt_connect(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB58 attempt_connect implementation")
    }
    
    fn download_stub_impl(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB58 download_stub_impl implementation")
    }
    
    fn download_stub(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB58Tool::download_stub not implemented yet")
    }
    
    fn set_speed(&mut self, _baud: u32) -> Result<(), std::io::Error> {
        todo!("SF32LB58Tool::set_speed not implemented yet")
    }
    
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB58Tool::soft_reset not implemented yet")
    }
}


