use crate::ram_command::{Command, RamCommand, Response};
use crate::{SifliTool, SubcommandParams, utils};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Write};

pub trait ReadFlashTrait {
    fn read_flash(&mut self) -> Result<(), std::io::Error>;
}

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

fn print_ascii(v: Vec<u8>) {
    for byte in v {
        if (byte.is_ascii() && !byte.is_ascii_control()) || byte == 0x0d || byte == 0x0a {
            print!("{}", byte as char);
        } else {
            print!("{:02x} ", byte);
        }
    }
    println!(); // 添加换行以便输出更整洁
}

impl ReadFlashTrait for SifliTool {
    fn read_flash(&mut self) -> Result<(), std::io::Error> {
        let mut step = self.step;

        let SubcommandParams::ReadFlashParams(params) = self.subcommand_params.clone() else {
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
            if !self.base.quiet {
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
            let response = self.command(Command::Read {
                address: file.address,
                len: file.size,
            })?;

            if response != Response::Ok {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to initiate read operation",
                ));
            }

            let mut received_data = Vec::new();

            let now = std::time::SystemTime::now();
            let mut tick = now.elapsed().unwrap().as_millis();

            // 通过超时检测来结束数据接收
            while now.elapsed().unwrap().as_millis() - tick < 100 {
                let mut buffer = [0u8; 1024];
                match self.port.read(&mut buffer) {
                    Ok(n) if n > 0 => {
                        // 重置计时
                        tick = now.elapsed().unwrap().as_millis();

                        // appends data
                        received_data.extend_from_slice(&buffer[..n]);

                        // 进度条
                        if !self.base.quiet {
                            progress_bar.inc(n as u64);
                        }
                    }
                    Ok(_) => continue,
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::TimedOut {
                            continue;
                        }
                        return Err(e);
                    }
                }
            }

            // print_ascii(received_data.clone());
            // println!("received_data.len={}", received_data.len());
            // println!("received_data={:02x?}", received_data);
            // println!("{}", received_data.to_());
            // 截取结尾30字节
            let end_str = String::from_utf8_lossy(&received_data[received_data.len() - 30..]);

            // OK
            if false == end_str.contains("OK") {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Receive fail(lost \"OK\")",
                ));
            }

            let data_str = String::from_utf8_lossy(&received_data);
            let bin_data: Vec<u8>;
            let crc_str;
            if let Some(crc_pos) = data_str.find("CRC:0x") {
                if !self.base.quiet {
                    progress_bar.finish_with_message("Read complete!");// TODO: 显示问题
                }
                // 处理数据，只保留CRC之前的部分
                crc_str = data_str[crc_pos..crc_pos + "CRC:0x".len() + 8].to_string();
                let data_pos = data_str.find("start_trans").unwrap() + "start_trans".len();
                bin_data = received_data[data_pos..crc_pos].into();
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Receive fail(lost \"CRC\")",
                ));
            }

            // 验证CRC
            if let Ok(expected_crc) = u32::from_str_radix(crc_str.as_str(), 16) {
                if !utils::Utils::verify_crc32(&bin_data, expected_crc) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "CRC verif fail",
                    ));
                }
            }

            // 创建输出文件
            let mut output_file = File::create(&file.file_path)?;
            // 写入文件
            output_file.write_all(&bin_data)?;
        }

        Ok(())
    }
}
