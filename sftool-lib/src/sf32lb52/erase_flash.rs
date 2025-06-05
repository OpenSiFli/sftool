use crate::ram_command::{Command, RamCommand};
use crate::{SifliTool, SubcommandParams, utils};
use crate::erase_flash::EraseFlashTrait;
use super::SF32LB52Tool;
use indicatif::{ProgressBar, ProgressStyle};

impl EraseFlashTrait for SF32LB52Tool {
    fn erase_flash(&mut self) -> Result<(), std::io::Error> {
        let mut step = self.step();

        let SubcommandParams::EraseFlashParams(params) = self.subcommand_params().clone() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid params for erase flash",
            ));
        };

        // 解析擦除地址 (这是擦除全部flash的命令，使用EraseAll)
        let address = utils::Utils::str_to_u32(&params.address)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        let progress_bar = ProgressBar::new_spinner();
        if !self.base().quiet {
            progress_bar.set_style(
                ProgressStyle::default_spinner()
                    .template("[{prefix}] Erasing entire flash at {msg}... {spinner}")
                    .unwrap(),
            );
            progress_bar.set_message(format!("0x{:08X}", address));
            progress_bar.set_prefix(format!("0x{:02X}", step));
        }

        // 发送擦除所有命令
        let _ = self.command(Command::EraseAll { address });

        let mut buffer = Vec::new();
        let now = std::time::SystemTime::now();

        // 等待擦除完成
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > 30000 {  // 擦除可能需要更长时间
                tracing::error!("response string is {}", String::from_utf8_lossy(&buffer));
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Erase timeout"));
            }

            let mut byte = [0];
            let ret = self.port().read_exact(&mut byte);
            if ret.is_err() {
                continue;
            }
            buffer.push(byte[0]);

            // 检查擦除完成响应
            if buffer.windows(2).any(|window| window == b"OK") {
                break;
            }
        }

        if !self.base().quiet {
            progress_bar.finish_with_message(format!(
                "Erase flash successfully: 0x{:08X}",
                address
            ));
        }

        Ok(())
    }

    fn erase_region(&mut self) -> Result<(), std::io::Error> {
        let mut step = self.step();

        let SubcommandParams::EraseRegionParams(params) = self.subcommand_params().clone() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid params for erase region",
            ));
        };

        // 处理每个区域
        for region_spec in params.region.iter() {
            // 解析格式: address:size
            let Some((addr_str, size_str)) = region_spec.split_once(':') else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Invalid region format: {}. Expected: address:size", region_spec),
                ));
            };

            let address = utils::Utils::str_to_u32(addr_str)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            let len = utils::Utils::str_to_u32(size_str)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            let progress_bar = ProgressBar::new(len as u64);
            if !self.base().quiet {
                progress_bar.set_style(
                    ProgressStyle::default_bar()
                        .template("[{prefix}] Erasing region at {msg}... {wide_bar} {percent_precise}%")
                        .unwrap()
                        .progress_chars("=>-"),
                );
                progress_bar.set_message(format!("0x{:08X}", address));
                progress_bar.set_prefix(format!("0x{:02X}", step));
                step += 1;
            }

            // 发送擦除区域命令
            let _ = self.command(Command::Erase { address, len });

            let mut buffer = Vec::new();
            let now = std::time::SystemTime::now();

            // 等待擦除完成
            loop {
                let elapsed = now.elapsed().unwrap().as_millis();
                if elapsed > 30000 {  // 擦除可能需要更长时间
                    tracing::error!("response string is {}", String::from_utf8_lossy(&buffer));
                    return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Erase timeout"));
                }

                let mut byte = [0];
                let ret = self.port().read_exact(&mut byte);
                if ret.is_err() {
                    continue;
                }
                buffer.push(byte[0]);

                // 检查擦除完成响应
                if buffer.windows(2).any(|window| window == b"OK") {
                    break;
                }
            }

            if !self.base().quiet {
                progress_bar.finish_with_message(format!(
                    "Erase region successfully: 0x{:08X} (length: 0x{:08X})",
                    address, len
                ));
            }
        }

        Ok(())
    }
}
