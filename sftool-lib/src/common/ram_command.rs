use crate::{Error, Result};
use serialport::SerialPort;
use std::io::{Read, Write};
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

impl RamOps {
    const DEFAULT_TIMEOUT_MS: u128 = 4000;
    const ERASE_ALL_TIMEOUT_MS: u128 = 30 * 1000;

    /// 发送命令并等待响应的通用实现
    pub fn send_command_and_wait_response(
        port: &mut Box<dyn SerialPort>,
        cmd: Command,
        memory_type: &str,
    ) -> Result<Response> {
        tracing::debug!("command: {:?}", cmd);

        // 发送命令
        port.write_all(cmd.to_string().as_bytes())?;
        port.flush()?;
        // 在macOS上，FTDI的驱动似乎不高兴我们清除输入缓冲区，这可能会导致后续要发送的内容被截断
        // 因此这个地方我们不再需要清理缓冲区，应该在后续的操作中滤除掉额外的信息
        // port.clear(serialport::ClearBuffer::All)?;

        // 确定超时时间
        let timeout = match cmd {
            Command::EraseAll { .. } => Self::ERASE_ALL_TIMEOUT_MS,
            _ => Self::DEFAULT_TIMEOUT_MS,
        };
        let timeout = if memory_type == "sd" {
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

        Self::wait_for_response(port, timeout)
    }

    /// 发送数据并等待响应的通用实现
    pub fn send_data_and_wait_response(
        port: &mut Box<dyn SerialPort>,
        data: &[u8],
        config: &CommandConfig,
    ) -> Result<Response> {
        // 根据配置发送数据
        if !config.compat_mode {
            port.write_all(data)?;
            port.flush()?;
        } else {
            // 兼容模式：分块发送
            for chunk in data.chunks(config.chunk_size) {
                port.write_all(chunk)?;
                port.flush()?;
                std::thread::sleep(std::time::Duration::from_millis(config.chunk_delay_ms));
            }
        }

        Self::wait_for_response(port, Self::DEFAULT_TIMEOUT_MS)
    }

    /// 等待响应的通用实现
    fn wait_for_response(
        port: &mut Box<dyn SerialPort>,
        timeout_ms: u128,
    ) -> Result<Response> {
        let mut buffer = Vec::new();
        let now = std::time::SystemTime::now();

        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > timeout_ms {
                tracing::debug!("Response buffer: {:?}", String::from_utf8_lossy(&buffer));
                return Err(Error::timeout("waiting for RAM command response"));
            }

            let mut byte = [0];
            let ret = port.read_exact(&mut byte);
            if ret.is_err() {
                continue;
            }
            buffer.push(byte[0]);

            // 检查是否收到预期的响应
            for response_str in RESPONSE_STR_TABLE.iter() {
                let response_bytes = response_str.as_bytes();
                let exists = buffer
                    .windows(response_bytes.len())
                    .any(|window| window == response_bytes);
                if exists {
                    tracing::debug!("Response buffer: {:?}", String::from_utf8_lossy(&buffer));
                    return Response::from_str(response_str)
                        .map_err(|e| Error::invalid_input(e.to_string()));
                }
            }
        }
    }

    /// 等待shell提示符的通用实现
    pub fn wait_for_shell_prompt(
        port: &mut Box<dyn SerialPort>,
        prompt: &[u8],
        retry_interval_ms: u64,
        max_retries: u32,
    ) -> Result<()> {
        let mut buffer = Vec::new();
        let mut now = std::time::SystemTime::now();
        let mut retry_count = 0;

        // 发送初始的回车换行
        port.write_all(b"\r\n")?;
        port.flush()?;

        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > retry_interval_ms as u128 {
                tracing::warn!(
                    "Wait for shell Failed, retry. buffer: {:?}",
                    String::from_utf8_lossy(&buffer)
                );
                port.clear(serialport::ClearBuffer::All)?;
                tracing::debug!("Retrying to find shell prompt...");
                std::thread::sleep(std::time::Duration::from_millis(100));
                retry_count += 1;
                now = std::time::SystemTime::now();
                port.write_all(b"\r\n")?;
                port.flush()?;
                buffer.clear();
            }

            if retry_count > max_retries {
                return Err(Error::timeout("waiting for shell prompt"));
            }

            let mut byte = [0];
            let ret = port.read_exact(&mut byte);
            if ret.is_err() {
                continue;
            }
            buffer.push(byte[0]);

            // 检查是否收到shell提示符
            if buffer.windows(prompt.len()).any(|window| window == prompt) {
                break;
            }
        }

        Ok(())
    }
}
