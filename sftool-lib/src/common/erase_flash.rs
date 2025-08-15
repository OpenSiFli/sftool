use crate::SifliToolTrait;
use crate::common::ram_command::{Command, RamCommand};
use crate::utils::Utils;

/// 通用的Flash擦除操作实现
pub struct EraseOps;

impl EraseOps {
    /// 擦除整个Flash的通用实现
    pub fn erase_all<T>(tool: &mut T, address: u32) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let progress = tool.progress();
        let progress_bar =
            progress.create_spinner(format!("Erasing entire flash at 0x{:08X}...", address));

        // 发送擦除所有命令
        let _ = tool.command(Command::EraseAll { address });

        let mut buffer = Vec::new();
        let now = std::time::SystemTime::now();

        // 等待擦除完成
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > 30000 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Erase timeout",
                ));
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

        progress_bar.finish_with_message("Erase complete");

        Ok(())
    }

    /// 擦除指定区域的通用实现
    pub fn erase_region<T>(tool: &mut T, address: u32, len: u32) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let progress = tool.progress();
        let progress_bar = progress.create_spinner(format!(
            "Erasing region at 0x{:08X} (size: 0x{:08X})...",
            address, len
        ));

        // 发送擦除区域命令
        let _ = tool.command(Command::Erase { address, len });

        let mut buffer = Vec::new();
        let now = std::time::SystemTime::now();

        // 等待擦除完成
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > 30000 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Erase timeout",
                ));
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

        progress_bar.finish_with_message("Region erase complete");

        Ok(())
    }

    /// 解析擦除地址参数
    pub fn parse_address(address_str: &str) -> Result<u32, std::io::Error> {
        Utils::str_to_u32(address_str)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
    }

    /// 解析区域参数 (address:size格式)
    pub fn parse_region(region_spec: &str) -> Result<(u32, u32), std::io::Error> {
        let Some((addr_str, size_str)) = region_spec.split_once(':') else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Invalid region format: {}. Expected: address:size",
                    region_spec
                ),
            ));
        };

        let address = Utils::str_to_u32(addr_str)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        let len = Utils::str_to_u32(size_str)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        Ok((address, len))
    }
}
