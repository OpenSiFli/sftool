use crate::{Error, Result, SifliToolTrait};
use crate::common::ram_command::{Command, RamCommand};
use crate::utils::Utils;
use std::fs::File;
use std::io::{Read, Seek, Write};
use tempfile::tempfile;

/// 通用的Flash读取文件结构
#[derive(Debug)]
pub struct ReadFlashFile {
    pub file_path: String,
    pub address: u32,
    pub size: u32,
}

/// 通用的Flash读取操作实现
pub struct FlashReader;

impl FlashReader {
    /// 解析读取文件信息 (filename@address:size格式)
    pub fn parse_file_info(file_spec: &str) -> Result<ReadFlashFile> {
        let Some((file_path, addr_size)) = file_spec.split_once('@') else {
            return Err(Error::invalid_input(
                format!(
                    "Invalid format: {}. Expected: filename@address:size",
                    file_spec
                ),
            ));
        };
        let Some((addr, size)) = addr_size.split_once(':') else {
            return Err(Error::invalid_input(
                format!(
                    "Invalid format: {}. Expected: filename@address:size",
                    file_spec
                ),
            ));
        };

        let address = Utils::str_to_u32(addr)
            .map_err(|e| Error::invalid_input(format!("Invalid address '{}': {}", addr, e)))?;
        let size = Utils::str_to_u32(size)
            .map_err(|e| Error::invalid_input(format!("Invalid size '{}': {}", size, e)))?;

        Ok(ReadFlashFile {
            file_path: file_path.to_string(),
            address,
            size,
        })
    }

    /// 从Flash读取数据的通用实现
    pub fn read_flash_data<T>(
        tool: &mut T,
        address: u32,
        size: u32,
        output_path: &str,
    ) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        let progress = tool.progress();
        let progress_bar =
            progress.create_bar(size as u64, format!("Reading from 0x{:08X}...", address));

        // 创建临时文件
        let mut temp_file = tempfile()?;
        let packet_size = 128 * 1024; // 128KB chunks
        let mut remaining = size;
        let mut current_address = address;

        while remaining > 0 {
            let chunk_size = std::cmp::min(remaining, packet_size);

            // 发送读取命令
            let _ = tool.command(Command::Read {
                address: current_address,
                len: chunk_size,
            });

            // 读取数据
            let mut buffer = vec![0u8; chunk_size as usize];
            let mut total_read = 0;
            let start_time = std::time::SystemTime::now();

            while total_read < chunk_size {
                let remaining_in_chunk = chunk_size - total_read;
                let mut chunk_buffer = vec![0u8; remaining_in_chunk as usize];

                match tool.port().read_exact(&mut chunk_buffer) {
                    Ok(_) => {
                        buffer[total_read as usize..(total_read + remaining_in_chunk) as usize]
                            .copy_from_slice(&chunk_buffer);
                        total_read += remaining_in_chunk;
                    }
                    Err(_) => {
                        // 超时检查
                        if start_time.elapsed().unwrap().as_millis() > 10000 {
                            return Err(Error::timeout(format!(
                                "reading flash at 0x{:08X}",
                                current_address
                            )));
                        }
                        continue;
                    }
                }
            }

            // 写入临时文件
            temp_file.write_all(&buffer)?;

            remaining -= chunk_size;
            current_address += chunk_size;

            progress_bar.inc(chunk_size as u64);
        }

        progress_bar.finish_with_message("Read complete");

        // 将临时文件内容写入目标文件
        temp_file.seek(std::io::SeekFrom::Start(0))?;
        let mut output_file = File::create(output_path)?;
        std::io::copy(&mut temp_file, &mut output_file)?;

        Ok(())
    }
}
