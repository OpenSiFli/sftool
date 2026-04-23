use crate::Result;
use crate::common::ram_command::{CommandConfig, RamOps};
use crate::common::serial_io::for_tool;
use crate::sf32lb55::SF32LB55Tool;

// 重新导出公共类型
pub use crate::common::ram_command::{Command, DownloadStub, RamCommand, Response};

impl RamCommand for SF32LB55Tool {
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
}

impl DownloadStub for SF32LB55Tool {
    fn download_stub(&mut self) -> Result<()> {
        // Use SifliTool trait methods
        self.download_stub_impl()
    }
}
