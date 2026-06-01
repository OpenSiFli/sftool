use crate::Result;
use crate::common::ram_command::{CommandConfig, RamOps, is_sd_memory};
use crate::common::serial_io::{for_tool, sleep_with_cancel};
use crate::common::sifli_debug::{SifliDebug, SifliUartCommand};
use crate::sf32lb57::SF32LB57Tool;

pub use crate::common::ram_command::{Command, DownloadStub, RamCommand, Response};

impl RamCommand for SF32LB57Tool {
    fn command(&mut self, cmd: Command) -> Result<Response> {
        let cmd_string = self.format_command(&cmd);
        let memory_type = self.base.memory_type.clone();
        let mut io = for_tool(self);
        RamOps::send_command_and_wait_response(&mut io, cmd, &cmd_string, memory_type.as_str())
    }

    fn send_data(&mut self, data: &[u8]) -> Result<Response> {
        let config = CommandConfig {
            compat_mode: self.base.compat,
            ..Default::default()
        };
        let mut io = for_tool(self);
        RamOps::send_data_and_wait_response(&mut io, data, &config)
    }

    fn format_command(&self, cmd: &Command) -> String {
        match cmd {
            Command::EraseAll { address } => {
                format!("burn_erase_all_factory 0x{address:08x}\r")
            }
            _ => cmd.to_string(),
        }
    }
}

impl DownloadStub for SF32LB57Tool {
    fn download_stub(&mut self) -> Result<()> {
        self.attempt_connect()?;
        self.download_stub_impl()?;

        sleep_with_cancel(
            &self.base.cancel_token,
            std::time::Duration::from_millis(100),
        )?;
        {
            let mut io = for_tool(self);
            io.clear(serialport::ClearBuffer::All)?;
        }
        self.debug_command(SifliUartCommand::Exit)?;

        let is_sd = is_sd_memory(&self.base.memory_type);
        let mut io = for_tool(self);
        if is_sd {
            RamOps::wait_for_shell_prompt(&mut io, b"sd0 OPEN success", 5000, 1)
        } else {
            RamOps::wait_for_shell_prompt(&mut io, b"msh >", 200, 5)
        }
    }
}
