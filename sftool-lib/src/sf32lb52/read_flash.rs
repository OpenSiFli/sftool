use super::ram_command::{Command, RamCommand};
use crate::{SifliTool, SubcommandParams, utils};
use crate::read_flash::ReadFlashTrait;
use super::SF32LB52Tool;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Seek, Write};
use tempfile::tempfile;

struct ReadFlashFile {
    file_path: String,
    address: u32,
    size: u32,
}

fn parse_file_info(file_spec: &str) -> Result<ReadFlashFile, std::io::Error> {
    // 解析格式: filename@address:size
    let Some((file_path, addr_size)) = file_spec.split_once('@') else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "Invalid format: {}. Expected: filename@address:size",
                file_spec
            ),
        ));
    };
    let Some((addr, size)) = addr_size.split_once(':') else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "Invalid format: {}. Expected: filename@address:size",
                file_spec
            ),
        ));
    };

    let address = utils::Utils::str_to_u32(addr)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let size = utils::Utils::str_to_u32(size)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    Ok(ReadFlashFile {
        file_path: file_path.to_string(),
        address,
        size,
    })
}

impl ReadFlashTrait for SF32LB52Tool {
    fn read_flash(&mut self) -> Result<(), std::io::Error> {
        let mut step = self.step();

        let SubcommandParams::ReadFlashParams(params) = self.subcommand_params().clone() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid params for read flash",
            ));
        };

        let mut read_flash_files: Vec<ReadFlashFile> = Vec::new();

        // 解析所有文件读取
        for file_spec in params.file_path.iter() {
            read_flash_files.push(parse_file_info(file_spec)?);
        }

        // 处理每个读取
        for file in read_flash_files {
            let progress_bar = ProgressBar::new(file.size as u64);
            if !self.base().quiet {
                progress_bar.set_style(
                    ProgressStyle::default_bar()
                        .template("[{prefix}] Reading at {msg}... {wide_bar} {bytes_per_sec} {percent_precise}%")
                        .unwrap()
                        .progress_chars("=>-"),
                );
                progress_bar.set_message(format!("0x{:08X}", file.address));
                progress_bar.set_prefix(format!("0x{:02X}", step));
                step += 1;
            }

            // 发送读取命令
            let _ = self.command(Command::Read {
                address: file.address,
                len: file.size,
            });

            let mut buffer = Vec::new();

            let now = std::time::SystemTime::now();

            // 判断是否可以开始读取数据
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

                // 一旦buffer出现RESPONSE_STR_TABLE中的任意一个，不一定是结束字节，也可能是在buffer中间出现，就认为接收完毕
                let response_str = "start_trans\r\n";
                let response_bytes = response_str.as_bytes();
                let exists = buffer
                    .windows(response_bytes.len())
                    .any(|window| window == response_bytes);
                if exists {
                    break;
                }
            }
            // 接下来都是裸数据
            let mut current_file = tempfile()?;
            let mut total_read = 0;
            while total_read < file.size {
                const READ_SIZE: usize = 1024;
                let mut read_buffer = [0; 1024];
                // 只读file.size 大小
                let read_size = if file.size - total_read < READ_SIZE as u32 {
                    (file.size - total_read) as usize
                } else {
                    READ_SIZE
                };
                self.port().read_exact(&mut read_buffer[..read_size])?;
                current_file.write_all(&read_buffer[..read_size])?;
                total_read += read_size as u32;

                if !self.base().quiet {
                    progress_bar.inc(read_size as u64);
                }
            }
            
            current_file.seek(std::io::SeekFrom::Start(0))?;
            let read_file_crc32 = utils::Utils::get_file_crc32(&current_file)?;
            let mut read_crc_str_bytes = [0u8; 14];
            self.port().read_exact(&mut read_crc_str_bytes)?;
            let read_crc_str = String::from_utf8_lossy(&read_crc_str_bytes);
            let read_crc32 = utils::Utils::str_to_u32(&read_crc_str[4..])
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            if read_file_crc32 != read_crc32 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "CRC mismatch: expected 0x{:08X}, got 0x{:08X}",
                        read_file_crc32, read_crc32
                    ),
                ));
            }
            
            // 将读取的文件保存到指定路径
            let mut output_file = File::create(&file.file_path)?;
            current_file.seek(std::io::SeekFrom::Start(0))?;
            std::io::copy(&mut current_file, &mut output_file)?;

            if !self.base().quiet {
                progress_bar.finish_with_message(format!(
                    "Read flash successfully: {} (0x{:08X})",
                    file.file_path, file.address
                ));
            }
        }

        Ok(())
    }
}
