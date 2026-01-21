use crate::common::ram_command::{Command, RamCommand};
use crate::progress::{EraseFlashStyle, EraseRegionStyle, ProgressOperation, ProgressStatus};
use crate::utils::Utils;
use crate::{Error, Result, SifliToolTrait};

/// 通用的Flash擦除操作实现
pub struct EraseOps;

impl EraseOps {
    /// 擦除整个Flash的通用实现
    pub fn erase_all<T>(tool: &mut T, address: u32) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        let progress = tool.progress();
        let progress_bar = progress.create_spinner(ProgressOperation::EraseFlash {
            address,
            style: EraseFlashStyle::Complete,
        });

        // 发送擦除所有命令
        let _ = tool.command(Command::EraseAll { address });

        let mut buffer = Vec::new();
        let now = std::time::SystemTime::now();

        // 等待擦除完成
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > 30000 {
                return Err(Error::timeout(format!(
                    "erasing flash at 0x{:08X}",
                    address
                )));
            }

            let mut byte = [0];
            let ret = tool.port().read_exact(&mut byte);
            if ret.is_err() {
                continue;
            }
            buffer.push(byte[0]);

            if buffer.windows(2).any(|window| window == b"OK") {
                break;
            }
        }

        progress_bar.finish(ProgressStatus::Success);

        Ok(())
    }

    /// 擦除指定区域的通用实现
    pub fn erase_region<T>(tool: &mut T, address: u32, len: u32) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        let progress = tool.progress();
        let progress_bar = progress.create_spinner(ProgressOperation::EraseRegion {
            address,
            len,
            style: EraseRegionStyle::Range,
        });

        // 发送擦除区域命令
        let _ = tool.command(Command::Erase { address, len });

        let mut buffer = Vec::new();
        let now = std::time::SystemTime::now();

        // 等待擦除完成
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > 30000 {
                return Err(Error::timeout(format!(
                    "erasing region 0x{:08X}..0x{:08X}",
                    address,
                    address + len.saturating_sub(1)
                )));
            }

            let mut byte = [0];
            let ret = tool.port().read_exact(&mut byte);
            if ret.is_err() {
                continue;
            }
            buffer.push(byte[0]);

            if buffer.windows(2).any(|window| window == b"OK") {
                break;
            }
        }

        progress_bar.finish(ProgressStatus::Success);

        Ok(())
    }

    /// 解析擦除地址参数
    pub fn parse_address(address_str: &str) -> Result<u32> {
        Utils::str_to_u32(address_str)
            .map_err(|e| Error::invalid_input(format!("Invalid address '{}': {}", address_str, e)))
    }

    /// 解析区域参数 (address:size格式)
    pub fn parse_region(region_spec: &str) -> Result<(u32, u32)> {
        let Some((addr_str, size_str)) = region_spec.split_once(':') else {
            return Err(Error::invalid_input(format!(
                "Invalid region format: {}. Expected: address:size",
                region_spec
            )));
        };

        let address = Utils::str_to_u32(addr_str)
            .map_err(|e| Error::invalid_input(format!("Invalid address '{}': {}", addr_str, e)))?;
        let len = Utils::str_to_u32(size_str)
            .map_err(|e| Error::invalid_input(format!("Invalid size '{}': {}", size_str, e)))?;

        Ok((address, len))
    }
}
