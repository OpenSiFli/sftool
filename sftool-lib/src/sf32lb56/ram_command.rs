use crate::Result;
use crate::common::ram_command::{CommandConfig, RamOps};
use crate::common::serial_io::{for_tool, sleep_with_cancel};
use crate::common::sifli_debug::{SifliDebug, SifliUartCommand};
use crate::sf32lb56::SF32LB56Tool;

// 重新导出公共类型
pub use crate::common::ram_command::{Command, DownloadStub, RamCommand, Response};

impl RamCommand for SF32LB56Tool {
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

impl DownloadStub for SF32LB56Tool {
    fn download_stub(&mut self) -> Result<()> {
        // Use SifliTool trait methods
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

        // 根据memory_type选择不同的等待条件
        let is_sd = self.base.memory_type == "sd";
        let mut io = for_tool(self);
        if is_sd {
            // SD卡模式：等待 "sd0 OPEN success"，超时5秒
            RamOps::wait_for_shell_prompt(
                &mut io,
                b"sd0 OPEN success",
                1000, // 1秒间隔
                5,    // 最多重试5次 (总计5秒)
            )
        } else {
            // 非SD模式：等待shell提示符 "msh >"
            RamOps::wait_for_shell_prompt(
                &mut io, b"msh >", 200, // 200ms间隔
                5,   // 最多重试5次
            )
        }
    }
}
