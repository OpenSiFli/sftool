use crate::reset::Reset;
use super::ram_command::{Command, RamCommand};
use super::SF32LB52Tool;

impl Reset for SF32LB52Tool {
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        self.command(Command::SoftReset)?;
        Ok(())
    }
}
