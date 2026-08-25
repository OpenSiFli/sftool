use crate::common::debug_sequence::DebugSequenceReplacementInfo;
use crate::common::ram_command::{CommandConfig, RamOps, is_sd_memory};
use crate::common::serial_io::{for_tool, sleep_with_cancel};
use crate::common::sifli_debug::{SifliDebug, SifliUartCommand};
use crate::sf32lb56::SF32LB56Tool;
use crate::{Error, Result};

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

    fn supports_debug_sequence_replacement(&mut self) -> Result<bool> {
        if let Some(supported) = self.debug_sequence_replacement_supported {
            return Ok(supported);
        }

        let supported = match self
            .send_debug_sequence_replacement_info(&DebugSequenceReplacementInfo::empty())
        {
            Ok(Response::Ok) => true,
            Ok(response) => {
                tracing::debug!(?response, "RAM patch does not support burn_info");
                false
            }
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(error) => {
                tracing::debug!(%error, "RAM patch does not support burn_info");
                false
            }
        };
        self.debug_sequence_replacement_supported = Some(supported);
        Ok(supported)
    }

    fn send_debug_sequence_replacement_info(
        &mut self,
        info: &DebugSequenceReplacementInfo,
    ) -> Result<Response> {
        let data = info.encode()?;
        let memory_type = self.base.memory_type.clone();
        let mut io = for_tool(self);
        RamOps::send_debug_sequence_replacement_info(&mut io, &data, memory_type.as_str())
    }
}

impl DownloadStub for SF32LB56Tool {
    fn download_stub(&mut self) -> Result<()> {
        self.debug_sequence_replacement_supported = None;
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
        let is_sd = is_sd_memory(&self.base.memory_type);
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
