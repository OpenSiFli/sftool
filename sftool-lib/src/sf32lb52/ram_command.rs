use crate::common::ram_command::{CommandConfig, RamOps};
use crate::common::sifli_debug::{SifliDebug, SifliUartCommand};
use crate::sf32lb52::SF32LB52Tool;

// 重新导出公共类型，保持向后兼容
pub use crate::common::ram_command::{Command, DownloadStub, RamCommand, Response};

impl RamCommand for SF32LB52Tool {
    fn command(&mut self, cmd: Command) -> Result<Response, std::io::Error> {
        RamOps::send_command_and_wait_response(&mut self.port, cmd)
    }

    fn send_data(&mut self, data: &[u8]) -> Result<Response, std::io::Error> {
        let config = CommandConfig {
            compat_mode: self.base.compat,
            ..Default::default()
        };
        RamOps::send_data_and_wait_response(&mut self.port, data, &config)
    }
}

impl DownloadStub for SF32LB52Tool {
    fn download_stub(&mut self) -> Result<(), std::io::Error> {
        // Use SifliTool trait methods
        self.attempt_connect()?;
        self.download_stub_impl()?;

        std::thread::sleep(std::time::Duration::from_millis(100));
        self.port.clear(serialport::ClearBuffer::All)?;
        self.debug_command(SifliUartCommand::Exit)?;

        // 等待shell提示符 "msh >"
        RamOps::wait_for_shell_prompt(
            &mut self.port,
            b"msh >",
            200, // 200ms间隔
            5,   // 最多重试5次
        )
    }
}
