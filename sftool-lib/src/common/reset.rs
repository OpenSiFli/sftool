use crate::common::ram_command::{Command, RamCommand};

/// 通用的复位操作实现
pub struct ResetOps;

impl ResetOps {
    /// 发送软件复位命令
    pub fn soft_reset<T>(tool: &mut T) -> Result<(), std::io::Error>
    where
        T: RamCommand,
    {
        tool.command(Command::SoftReset)?;
        Ok(())
    }
}
