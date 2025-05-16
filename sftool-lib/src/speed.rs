use std::io::Write;
use std::time::Duration;
use crate::SifliTool;
use crate::ram_command::{Command, RamCommand};

pub trait SpeedTrait {
    fn set_speed(&mut self, speed: u32) -> Result<(), std::io::Error>;
}

impl SpeedTrait for SifliTool {
    fn set_speed(&mut self, speed: u32) -> Result<(), std::io::Error> {
        self.command(Command::SetBaud {
            baud: speed,
            delay: 500,
        })?;
        self.port.set_baud_rate(speed)?;
        std::thread::sleep(Duration::from_millis(300));
        self.port.write_all("\r\n".as_bytes())?;
        self.port.flush()?;
        std::thread::sleep(Duration::from_millis(300));
        self.port.clear(serialport::ClearBuffer::All)?;
        Ok(())
    }
}
