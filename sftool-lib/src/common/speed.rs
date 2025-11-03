use crate::common::ram_command::{Command, RamCommand};
use crate::{Result, SifliToolTrait};
use std::io::Write;
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
            delay: 500,
        })?;

        // 设置串口波特率
        tool.port().set_baud_rate(speed)?;

        // 等待一段时间让设置生效
        std::thread::sleep(Duration::from_millis(300));

        // 发送回车换行测试连接
        tool.port().write_all("\r\n".as_bytes())?;
        tool.port().flush()?;

        // 再等待一段时间
        std::thread::sleep(Duration::from_millis(300));

        // 清空缓冲区
        tool.port().clear(serialport::ClearBuffer::All)?;

        Ok(())
    }
}
