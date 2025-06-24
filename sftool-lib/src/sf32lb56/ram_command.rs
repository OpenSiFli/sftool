use crate::common::ram_command::{CommandConfig, RamOps};
use crate::sf32lb56::SF32LB56Tool;

// 重新导出公共类型
pub use crate::common::ram_command::{Command, DownloadStub, RamCommand, Response};

impl RamCommand for SF32LB56Tool {
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

impl DownloadStub for SF32LB56Tool {
    fn download_stub(&mut self) -> Result<(), std::io::Error> {
        // SF32LB56的具体实现可能与SF32LB52不同
        // 这里先提供一个基础实现，可以根据具体需求调整
        todo!("SF32LB56Tool::download_stub not implemented yet")
    }
}
