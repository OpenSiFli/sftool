use crate::common::ram_command::{CommandConfig, RamOps};
use crate::sf32lb58::SF32LB58Tool;

// 重新导出公共类型
pub use crate::common::ram_command::{Command, DownloadStub, RamCommand, Response};

impl RamCommand for SF32LB58Tool {
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

impl DownloadStub for SF32LB58Tool {
    fn download_stub(&mut self) -> Result<(), std::io::Error> {
        self.download_stub_impl()
    }
}
