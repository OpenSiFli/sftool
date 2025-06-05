//! SF32LB56 芯片特定实现模块

pub mod write_flash;
pub mod read_flash;
pub mod erase_flash;

use crate::{SifliToolBase, SubcommandParams, SifliTool};
use serialport::SerialPort;
use std::time::Duration;

pub struct SF32LB56Tool {
    pub base: SifliToolBase,
    pub port: Box<dyn SerialPort>,
    pub step: i32,
    pub subcommand_params: SubcommandParams,
}

impl SF32LB56Tool {
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

impl SifliTool for SF32LB56Tool {
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
        todo!("SF32LB56 execute_command implementation")
    }
    
    fn debug_command(&mut self, _cmd: crate::sifli_debug::SifliUartCommand) -> Result<crate::sifli_debug::SifliUartResponse, std::io::Error> {
        todo!("SF32LB56 debug_command implementation")
    }
    
    fn debug_write_word32(&mut self, _addr: u32, _data: u32) -> Result<(), std::io::Error> {
        todo!("SF32LB56 debug_write_word32 implementation")
    }
    
    fn debug_read_word32(&mut self, _addr: u32) -> Result<u32, std::io::Error> {
        todo!("SF32LB56 debug_read_word32 implementation")
    }
    
    fn debug_write_core_reg(&mut self, _reg: u16, _data: u32) -> Result<(), std::io::Error> {
        todo!("SF32LB56 debug_write_core_reg implementation")
    }
    
    fn debug_run(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB56 debug_run implementation")
    }
    
    fn debug_halt(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB56 debug_halt implementation")
    }
    
    fn debug_write_memory(&mut self, _addr: u32, _data: &[u8]) -> Result<(), std::io::Error> {
        todo!("SF32LB56 debug_write_memory implementation")
    }
    
    fn attempt_connect(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB56 attempt_connect implementation")
    }
    
    fn download_stub_impl(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB56 download_stub_impl implementation")
    }
    
    fn download_stub(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB56Tool::download_stub not implemented yet")
    }
    
    fn set_speed(&mut self, _baud: u32) -> Result<(), std::io::Error> {
        todo!("SF32LB56Tool::set_speed not implemented yet")
    }
    
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB56Tool::soft_reset not implemented yet")
    }
}

impl crate::ram_command::RamCommand for SF32LB56Tool {
    fn command(&mut self, _cmd: crate::ram_command::Command) -> Result<crate::ram_command::Response, std::io::Error> {
        todo!("SF32LB56Tool::command not implemented yet")
    }

    fn send_data(&mut self, _data: &[u8]) -> Result<crate::ram_command::Response, std::io::Error> {
        todo!("SF32LB56Tool::send_data not implemented yet")
    }
}
