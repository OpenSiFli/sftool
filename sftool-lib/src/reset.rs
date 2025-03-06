use crate::ram_command::{Command, RamCommand};
use crate::SifliTool;

pub trait Reset {
    fn soft_reset(&mut self) -> Result<(), std::io::Error>;
}

impl Reset for SifliTool {
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        self.command(Command::SoftReset)?;
        Ok(())
    }
}