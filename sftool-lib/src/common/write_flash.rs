use crate::SifliToolTrait;
use crate::WriteFlashFile;
use crate::common::ram_command::{Command, RamCommand, Response};
use std::io::{BufReader, Read, Write};

/// 通用的Flash写入操作实现
pub struct FlashWriter;

impl FlashWriter {
    /// 擦除所有Flash区域
    pub fn erase_all<T>(
        tool: &mut T,
        write_flash_files: &[WriteFlashFile],
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let progress = tool.progress();
        let spinner = progress.create_spinner("Erasing all flash regions...");
        
        let mut erase_address: Vec<u32> = Vec::new();
        for f in write_flash_files.iter() {
            let address = f.address & 0xFF00_0000;
            // 如果ERASE_ADDRESS中的地址已经被擦除过，则跳过
            if erase_address.contains(&address) {
                continue;
            }
            tool.command(Command::EraseAll { address: f.address })?;
            erase_address.push(address);
        }
        
        spinner.finish_with_message("All flash regions erased");
        Ok(())
    }

    /// 验证数据
    pub fn verify<T>(tool: &mut T, address: u32, len: u32, crc: u32) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let progress = tool.progress();
        let spinner = progress.create_spinner("Verifying data...");
        
        let response = tool.command(Command::Verify { address, len, crc })?;
        if response != Response::Ok {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Verify failed",
            ));
        }
        
        spinner.finish_with_message("Verify success!");
        Ok(())
    }

    /// 写入单个文件到Flash（非全擦除模式）
    pub fn write_file_incremental<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        verify: bool,
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let progress = tool.progress();
        let re_download_spinner = progress.create_spinner(format!(
            "Checking whether a re-download is necessary at address 0x{:08X}...",
            file.address
        ));

        let response = tool.command(Command::Verify {
            address: file.address,
            len: file.file.metadata()?.len() as u32,
            crc: file.crc32,
        })?;

        if response == Response::Ok {
            re_download_spinner.finish_with_message("No need to re-download, skip!");
            return Ok(());
        }

        re_download_spinner.finish_with_message("Need to re-download");
        
        let download_bar = progress.create_bar(
            file.file.metadata()?.len(),
            format!("Download at 0x{:08X}...", file.address)
        );

        let res = tool.command(Command::WriteAndErase {
            address: file.address,
            len: file.file.metadata()?.len() as u32,
        })?;
        if res != Response::RxWait {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Write flash failed",
            ));
        }

        let mut buffer = vec![0u8; 128 * 1024];
        let mut reader = BufReader::new(&file.file);

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            let res = tool.send_data(&buffer[..bytes_read])?;
            if res == Response::RxWait {
                download_bar.inc(bytes_read as u64);
                continue;
            } else if res != Response::Ok {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Write flash failed",
                ));
            }
        }

        download_bar.finish_with_message("Download success!");

        // verify
        if verify {
            Self::verify(
                tool,
                file.address,
                file.file.metadata()?.len() as u32,
                file.crc32,
            )?;
        }

        Ok(())
    }

    /// 写入单个文件到Flash（全擦除模式）
    pub fn write_file_full_erase<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        verify: bool,
        packet_size: usize,
    ) -> Result<(), std::io::Error>
    where
        T: SifliToolTrait + RamCommand,
    {
        let progress = tool.progress();
        let download_bar = progress.create_bar(
            file.file.metadata()?.len(),
            format!("Download at 0x{:08X}...", file.address)
        );

        let mut buffer = vec![0u8; packet_size];
        let mut reader = BufReader::new(&file.file);

        let mut address = file.address;
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            tool.port().write_all(
                Command::Write {
                    address: address,
                    len: bytes_read as u32,
                }
                .to_string()
                .as_bytes(),
            )?;
            tool.port().flush()?;
            let res = tool.send_data(&buffer[..bytes_read])?;
            if res != Response::Ok {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Write flash failed",
                ));
            }
            address += bytes_read as u32;
            download_bar.inc(bytes_read as u64);
        }
        
        download_bar.finish_with_message("Download success!");

        // verify
        if verify {
            Self::verify(
                tool,
                file.address,
                file.file.metadata()?.len() as u32,
                file.crc32,
            )?;
        }

        Ok(())
    }
}
