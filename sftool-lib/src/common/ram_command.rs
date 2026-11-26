use crate::common::debug_sequence::DebugSequenceReplacementInfo;
use crate::common::serial_io::SerialIo;
use crate::{Error, Result};
use std::str::FromStr;
use strum::{Display, EnumString};

/// 通用的RAM命令枚举，可在不同芯片间复用
#[derive(EnumString, Display, Debug, Clone, PartialEq, Eq)]
pub enum Command {
    #[strum(to_string = "burn_erase_all 0x{address:08x}\r")]
    EraseAll { address: u32 },

    #[strum(to_string = "burn_verify 0x{address:08x} 0x{len:08x} 0x{crc:08x}\r")]
    Verify { address: u32, len: u32, crc: u32 },

    #[strum(to_string = "burn_erase 0x{address:08x} 0x{len:08x}\r")]
    Erase { address: u32, len: u32 },

    #[strum(to_string = "burn_erase_write 0x{address:08x} 0x{len:08x}\r")]
    WriteAndErase { address: u32, len: u32 },

    #[strum(to_string = "burn_write 0x{address:08x} 0x{len:08x}\r")]
    Write { address: u32, len: u32 },

    #[strum(to_string = "burn_read 0x{address:08x} 0x{len:08x}\r")]
    Read { address: u32, len: u32 },

    #[strum(to_string = "burn_reset\r")]
    SoftReset,

    #[strum(to_string = "burn_speed {baud} {delay}\r")]
    SetBaud { baud: u32, delay: u32 },
}

/// 通用的命令响应枚举
#[derive(EnumString, Display, Debug, Clone, PartialEq, Eq)]
pub enum Response {
    #[strum(serialize = "OK")]
    Ok,
    #[strum(serialize = "Fail")]
    Fail,
    #[strum(serialize = "RX_WAIT")]
    RxWait,
}

/// 响应字符串查找表
pub const RESPONSE_STR_TABLE: [&str; 3] = ["OK", "Fail", "RX_WAIT"];

/// RAM命令处理trait，定义了发送命令和数据的接口
pub trait RamCommand {
    fn command(&mut self, cmd: Command) -> Result<Response>;
    fn send_data(&mut self, data: &[u8]) -> Result<Response>;

    /// Whether the connected RAM patch supports `burn_info`. The default keeps
    /// legacy and non-Debug-IP chips on their existing raw write path.
    fn supports_debug_sequence_replacement(&mut self) -> Result<bool> {
        Ok(false)
    }

    fn send_debug_sequence_replacement_info(
        &mut self,
        _info: &DebugSequenceReplacementInfo,
    ) -> Result<Response> {
        Ok(Response::Fail)
    }

    fn format_command(&self, cmd: &Command) -> String {
        cmd.to_string()
    }
}

/// Stub下载trait，定义了下载stub的接口
pub trait DownloadStub {
    fn download_stub(&mut self) -> Result<()>;
}

/// 命令处理的配置参数
pub struct CommandConfig {
    pub compat_mode: bool,
    pub chunk_size: usize,
    pub chunk_delay_ms: u64,
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            compat_mode: false,
            chunk_size: 256,
            chunk_delay_ms: 10,
        }
    }
}

/// 通用的RAM操作处理器，包含可复用的逻辑
pub struct RamOps;

pub fn is_sd_memory(memory_type: &str) -> bool {
    let memory_type = memory_type.to_ascii_lowercase();
    memory_type == "sd" || memory_type.starts_with("sd_")
}

impl RamOps {
    const DEFAULT_TIMEOUT_MS: u128 = 4000;
    const ERASE_ALL_TIMEOUT_MS: u128 = 30 * 1000;
    const REPLACEMENT_INFO_TIMEOUT_MS: u128 = 2000;

    /// Quiet period that ends the drain after a failed `burn_info` probe.
    const PROBE_RECOVERY_IDLE_MS: std::time::Duration = std::time::Duration::from_millis(100);
    /// Upper bound on the drain, so a chatty peer cannot stall the recovery.
    const PROBE_RECOVERY_MAX_MS: std::time::Duration = std::time::Duration::from_millis(1000);

    /// 发送命令并等待响应的通用实现
    pub fn send_command_and_wait_response(
        io: &mut SerialIo<'_>,
        cmd: Command,
        command_str: &str,
        memory_type: &str,
    ) -> Result<Response> {
        tracing::debug!("command: {:?}", cmd);

        // 发送命令
        io.write_all(command_str.as_bytes())?;
        io.flush()?;
        // 在macOS上，FTDI的驱动似乎不高兴我们清除输入缓冲区，这可能会导致后续要发送的内容被截断
        // 因此这个地方我们不再需要清理缓冲区，应该在后续的操作中滤除掉额外的信息
        // port.clear(serialport::ClearBuffer::All)?;

        // 确定超时时间
        let timeout = match cmd {
            Command::EraseAll { .. } => Self::ERASE_ALL_TIMEOUT_MS,
            _ => Self::DEFAULT_TIMEOUT_MS,
        };
        let timeout = if is_sd_memory(memory_type) {
            timeout * 3
        } else {
            timeout
        };

        // 某些命令直接返回成功，不等待响应
        match cmd {
            Command::SetBaud { .. } | Command::Read { .. } | Command::Erase { .. } => {
                return Ok(Response::Ok);
            }
            _ => (),
        }

        Self::wait_for_response(io, timeout)
    }

    /// 发送数据并等待响应的通用实现
    pub fn send_data_and_wait_response(
        io: &mut SerialIo<'_>,
        data: &[u8],
        config: &CommandConfig,
    ) -> Result<Response> {
        // 根据配置发送数据
        if !config.compat_mode {
            io.write_all(data)?;
            io.flush()?;
        } else {
            // 兼容模式：分块发送
            for chunk in data.chunks(config.chunk_size) {
                io.write_all(chunk)?;
                io.flush()?;
                io.sleep(std::time::Duration::from_millis(config.chunk_delay_ms))?;
            }
        }

        Self::wait_for_response(io, Self::DEFAULT_TIMEOUT_MS)
    }

    /// Sends the `burn_info` command followed by the fixed-size replacement
    /// table expected by the new 52X/56X/57X RAM patches.
    pub fn send_debug_sequence_replacement_info(
        io: &mut SerialIo<'_>,
        data: &[u8],
        memory_type: &str,
    ) -> Result<Response> {
        tracing::debug!("sending burn_info replacement table ({} bytes)", data.len());
        io.write_all(b"burn_info\r")?;
        io.flush()?;
        io.sleep(std::time::Duration::from_millis(100))?;
        io.write_all(data)?;
        io.flush()?;

        let timeout = if is_sd_memory(memory_type) {
            Self::REPLACEMENT_INFO_TIMEOUT_MS * 3
        } else {
            Self::REPLACEMENT_INFO_TIMEOUT_MS
        };
        Self::wait_for_response(io, timeout)
    }

    /// Probes whether the connected RAM patch understands `burn_info`.
    ///
    /// A patch built without that command treats the probe as unknown shell
    /// input: it echoes an error and then swallows the fixed-size replacement
    /// table as text. The reply never matches a known response, so the probe
    /// times out and those leftovers stay in the port, where they corrupt the
    /// next command. Draining the input and re-syncing on the shell prompt
    /// puts the port back into a usable state so the legacy write path can
    /// still run. Returns `false` when the patch does not support it.
    ///
    /// When `skip_probe` is set the handshake is skipped entirely and the
    /// legacy path is used without paying the probe timeout first. This is
    /// what `--no-burn-info` sets for patches known to lack the command.
    pub fn probe_debug_sequence_replacement_support(
        io: &mut SerialIo<'_>,
        memory_type: &str,
        skip_probe: bool,
    ) -> Result<bool> {
        if skip_probe {
            tracing::debug!("skipping burn_info probe by request; using the legacy write flow");
            return Ok(false);
        }

        Self::send_debug_sequence_replacement_info(
            io,
            &DebugSequenceReplacementInfo::empty().encode()?,
            memory_type,
        )
        .or_else(|error| {
            if matches!(error, Error::Cancelled) {
                return Err(error);
            }

            tracing::debug!(%error, "RAM patch does not support burn_info; restoring port state");

            Self::recover_port_after_unsupported_probe(io, memory_type);
            Ok(Response::Fail)
        })
        .map(|response| response == Response::Ok)
    }

    /// Clears whatever an unsupported `burn_info` probe left behind.
    ///
    /// This is best effort: the port may already be quiet, or the prompt may
    /// have scrolled past before we started looking. Failing to re-sync must
    /// not abort the write, because the legacy path still works as long as
    /// the stale text is gone.
    fn recover_port_after_unsupported_probe(io: &mut SerialIo<'_>, memory_type: &str) {
        if let Err(error) =
            io.drain_input(Self::PROBE_RECOVERY_IDLE_MS, Self::PROBE_RECOVERY_MAX_MS)
        {
            tracing::debug!(%error, "failed to drain serial input after burn_info probe");
            return;
        }

        let (prompt, interval_ms, retries): (&[u8], u64, u32) = if is_sd_memory(memory_type) {
            (b"sd0 OPEN success", 5000, 1)
        } else {
            (b"msh >", 200, 5)
        };

        if let Err(error) = Self::wait_for_shell_prompt(io, prompt, interval_ms, retries) {
            tracing::debug!(%error, "shell prompt not seen after burn_info probe");
        }
    }

    /// 等待响应的通用实现
    fn wait_for_response(io: &mut SerialIo<'_>, timeout_ms: u128) -> Result<Response> {
        let matched = io.wait_for_patterns(
            &RESPONSE_STR_TABLE.map(str::as_bytes),
            std::time::Duration::from_millis(timeout_ms as u64),
            "RAM command response",
        )?;
        tracing::debug!(
            "Response buffer: {:?}",
            String::from_utf8_lossy(&matched.buffer)
        );
        Response::from_str(RESPONSE_STR_TABLE[matched.index])
            .map_err(|e| Error::invalid_input(e.to_string()))
    }

    /// 等待shell提示符的通用实现
    pub fn wait_for_shell_prompt(
        io: &mut SerialIo<'_>,
        prompt: &[u8],
        retry_interval_ms: u64,
        max_retries: u32,
    ) -> Result<()> {
        io.wait_for_prompt(
            prompt,
            std::time::Duration::from_millis(retry_interval_ms),
            max_retries,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CancelToken;
    use crate::common::serial_io::test_support::TestSerialPort;

    /// A patch without `burn_info` echoes an error for the unknown command and
    /// then emits the shell prompt. The probe must report "unsupported" and
    /// leave the port drained, so the following command is not fed stale text.
    #[test]
    fn probe_reports_unsupported_and_drains_leftovers() {
        let (mut port, state) =
            TestSerialPort::from_bytes(b"burn_info: command not found\r\nmsh >");
        let mut io = SerialIo::new(&mut port, CancelToken::new());

        let supported = RamOps::probe_debug_sequence_replacement_support(&mut io, "nor", false)
            .expect("probe should not fail when the patch merely lacks burn_info");

        assert!(!supported);
        assert_eq!(
            state.lock().unwrap().read_data.len(),
            0,
            "stale bytes must not survive the recovery"
        );
        assert!(
            state.lock().unwrap().writes.starts_with(b"burn_info\r"),
            "the probe should still be attempted"
        );
    }

    /// A patch that answers `OK` must be reported as supported and must not
    /// trigger the recovery drain.
    #[test]
    fn probe_reports_supported_when_patch_answers_ok() {
        let (mut port, state) = TestSerialPort::from_bytes(b"OK");
        let mut io = SerialIo::new(&mut port, CancelToken::new());

        let supported =
            RamOps::probe_debug_sequence_replacement_support(&mut io, "nor", false).expect("probe");

        assert!(supported);
        assert_eq!(
            state.lock().unwrap().clear_calls,
            0,
            "a successful probe should not have to clear the port"
        );
    }

    /// `--no-burn-info` must bypass the handshake entirely: no bytes written,
    /// no probe timeout paid, and the legacy write flow selected.
    #[test]
    fn probe_is_skipped_when_requested() {
        let (mut port, state) = TestSerialPort::from_bytes(b"");
        let mut io = SerialIo::new(&mut port, CancelToken::new());

        let supported = RamOps::probe_debug_sequence_replacement_support(&mut io, "nor", true)
            .expect("skipping the probe cannot fail");

        assert!(!supported, "legacy flow must be selected");
        assert!(
            state.lock().unwrap().writes.is_empty(),
            "nothing should be sent to the patch when the probe is skipped"
        );
    }

    /// Cancellation during the probe must propagate instead of being swallowed
    /// as "unsupported", otherwise a cancelled run would keep writing.
    #[test]
    fn probe_propagates_cancellation() {
        let (mut port, _) = TestSerialPort::from_bytes(&[]);
        let token = CancelToken::new();
        token.cancel();
        let mut io = SerialIo::new(&mut port, token);

        let result = RamOps::probe_debug_sequence_replacement_support(&mut io, "nor", false);

        assert!(matches!(result, Err(Error::Cancelled)));
    }
}
