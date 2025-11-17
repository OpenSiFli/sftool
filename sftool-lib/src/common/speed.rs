use crate::common::ram_command::{Command, RamCommand, RamOps};
use crate::{Result, SifliToolTrait};
use std::time::Duration;

/// 通用的速度设置操作实现
pub struct SpeedOps;

impl SpeedOps {
    /// 设置串口速度的通用实现
    pub fn set_speed<T>(tool: &mut T, speed: u32) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        // 发送设置波特率命令
        tool.command(Command::SetBaud {
            baud: speed,
            delay: 10,
        })?;

        // 等待一段时间让设置生效
        std::thread::sleep(Duration::from_millis(50));

        // 设置串口波特率
        tool.port().set_baud_rate(speed)?;

        tool.port().clear(serialport::ClearBuffer::All)?;

        RamOps::wait_for_shell_prompt(tool.port(), b"msh >", 200, 5)?;

        Ok(())
    }
}
