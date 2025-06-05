use super::ram_command::{Command, RamCommand};
use crate::{SifliTool, SubcommandParams, utils};
use crate::write_flash::WriteFlashTrait;
use super::SF32LB52Tool;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Seek, Write};

struct WriteFlashFile {
    file_path: String,
    address: u32,
    verify: bool,
}

fn parse_file_info(file_spec: &str, default_verify: bool) -> Result<WriteFlashFile, std::io::Error> {
    // 解析格式: filename[@address]
    if let Some((file_path, addr)) = file_spec.split_once('@') {
        let address = utils::Utils::str_to_u32(addr)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        Ok(WriteFlashFile {
            file_path: file_path.to_string(),
            address,
            verify: default_verify,
        })
    } else {
        Ok(WriteFlashFile {
            file_path: file_spec.to_string(),
            address: 0x10000000, // 默认flash地址
            verify: default_verify,
        })
    }
}

impl WriteFlashTrait for SF32LB52Tool {
    fn write_flash(&mut self) -> Result<(), std::io::Error> {
        let mut step = self.step();

        let SubcommandParams::WriteFlashParams(params) = self.subcommand_params().clone() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid params for write flash",
            ));
        };

        let mut write_flash_files: Vec<WriteFlashFile> = Vec::new();

        // 解析所有文件
        for file_spec in params.file_path.iter() {
            write_flash_files.push(parse_file_info(file_spec, params.verify)?);
        }

        // 写入每个文件
        for file in write_flash_files {
            let current_file = File::open(&file.file_path)?;
            let file_size = current_file.metadata()?.len();

            let progress_bar = ProgressBar::new(file_size);
            if !self.base().quiet {
                progress_bar.set_style(
                    ProgressStyle::default_bar()
                        .template("[{prefix}] Writing at {msg}... {wide_bar} {bytes_per_sec} {percent_precise}%")
                        .unwrap()
                        .progress_chars("=>-"),
                );
                progress_bar.set_message(format!("0x{:08X}", file.address));
                progress_bar.set_prefix(format!("0x{:02X}", step));
                step += 1;
            }

            // 发送写入命令 (暂时不支持压缩)
            let _ = self.command(Command::WriteAndErase {
                address: file.address,
                len: file_size as u32,
            });

            let mut current_file = File::open(&file.file_path)?;

            let mut buffer = Vec::new();
            let now = std::time::SystemTime::now();

            // 等待响应
            loop {
                let elapsed = now.elapsed().unwrap().as_millis();
                if elapsed > 1000 {
                    tracing::error!("response string is {}", String::from_utf8_lossy(&buffer));
                    return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout"));
                }

                let mut byte = [0];
                let ret = self.port().read_exact(&mut byte);
                if ret.is_err() {
                    continue;
                }
                buffer.push(byte[0]);

                // 等待 "start_trans\r\n"
                let response_str = "start_trans\r\n";
                let response_bytes = response_str.as_bytes();
                let exists = buffer
                    .windows(response_bytes.len())
                    .any(|window| window == response_bytes);
                if exists {
                    break;
                }
            }

            // 发送文件数据
            let mut total_sent = 0;
            loop {
                const WRITE_SIZE: usize = 1024;
                let mut write_buffer = [0; WRITE_SIZE];
                let bytes_read = current_file.read(&mut write_buffer)?;
                if bytes_read == 0 {
                    break;
                }

                self.port().write_all(&write_buffer[..bytes_read])?;
                self.port().flush()?;
                total_sent += bytes_read;

                if !self.base().quiet {
                    progress_bar.inc(bytes_read as u64);
                }
            }

            // 读取 CRC 验证
            let mut crc_str_bytes = [0u8; 14];
            self.port().read_exact(&mut crc_str_bytes)?;
            let crc_str = String::from_utf8_lossy(&crc_str_bytes);
            let received_crc32 = utils::Utils::str_to_u32(&crc_str[4..])
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            // 计算文件 CRC
            current_file.seek(std::io::SeekFrom::Start(0))?;
            let file_crc32 = utils::Utils::get_file_crc32(&current_file)?;

            if file_crc32 != received_crc32 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "CRC mismatch: expected 0x{:08X}, got 0x{:08X}",
                        file_crc32, received_crc32
                    ),
                ));
            }

            if !self.base().quiet {
                progress_bar.finish_with_message(format!(
                    "Write flash successfully: {} (0x{:08X})",
                    file.file_path, file.address
                ));
            }
        }

        Ok(())
    }
}
