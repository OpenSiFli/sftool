use std::io::Write;
use std::time::Duration;
use crate::speed::SpeedTrait;
use super::ram_command::{Command, RamCommand};
use super::SF32LB52Tool;

impl SpeedTrait for SF32LB52Tool {
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
