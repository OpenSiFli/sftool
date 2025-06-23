use crate::common::ram_command::{Command, RamCommand};
use super::SF32LB58Tool;
use crate::reset::Reset;

impl Reset for SF32LB58Tool {
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        self.command(Command::SoftReset)?;
        Ok(())
    }
}
