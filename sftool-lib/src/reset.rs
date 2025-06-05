use crate::ram_command::Command;
use crate::SifliTool;

pub trait Reset {
    fn soft_reset(&mut self) -> Result<(), std::io::Error>;
}

impl<T: SifliTool + crate::ram_command::RamCommand> Reset for T {
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        self.command(Command::SoftReset)?;
        Ok(())
    }
}