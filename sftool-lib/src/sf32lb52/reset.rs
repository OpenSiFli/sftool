use super::SF32LB52Tool;
use super::ram_command::{Command, RamCommand};
use crate::reset::Reset;

impl Reset for SF32LB52Tool {
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        self.command(Command::SoftReset)?;
        Ok(())
    }
}
