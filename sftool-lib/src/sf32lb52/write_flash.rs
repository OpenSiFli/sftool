use super::ram_command::{Command, RamCommand, Response};
use super::SF32LB52Tool;
use crate::utils::{FileType, Utils, ELF_MAGIC};
use crate::write_flash::WriteFlashTrait;
use crate::WriteFlashParams;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;

#[derive(Debug)]
pub struct WriteFlashFile {
    pub address: u32,
    pub file: File,
    pub crc32: u32,
}

fn detect_file_type(path: &Path) -> Result<FileType, std::io::Error> {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        match ext.to_lowercase().as_str() {
            "bin" => return Ok(FileType::Bin),
            "hex" => return Ok(FileType::Hex),
            "elf" | "axf" => return Ok(FileType::Elf),
            _ => {} // 如果扩展名无法识别，继续检查MAGIC
        }
    }

    // 如果没有可识别的扩展名，则检查文件MAGIC
    let mut file = File::open(path)?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;

    if magic == ELF_MAGIC {
        return Ok(FileType::Elf);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "Unrecognized file type",
    ))
}

fn parse_file_info(file_str: &str) -> Result<Vec<WriteFlashFile>, std::io::Error> {
    // file@address
    let parts: Vec<_> = file_str.split('@').collect();
    // 如果存在@符号，则证明是bin文件
    if parts.len() == 2 {
        let addr = Utils::str_to_u32(parts[1])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        let file = File::open(parts[0])?;
        let crc32 = Utils::get_file_crc32(&file.try_clone()?)?;

        return Ok(vec![WriteFlashFile {
            address: addr,
            file,
            crc32,
        }]);
    }

    let file_type = detect_file_type(Path::new(parts[0]))?;

    match file_type {
        FileType::Hex => Utils::hex_to_bin(Path::new(parts[0])),
        FileType::Elf => Utils::elf_to_bin(Path::new(parts[0])),
        FileType::Bin => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "For binary files, please use the <file@address> format",
        )),
    }
}

impl SF32LB52Tool {
    fn erase_all(&mut self, write_flash_files: &[WriteFlashFile]) -> Result<(), std::io::Error> {
        let spinner = ProgressBar::new_spinner();
        if !self.base.quiet {
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));
            spinner.set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            spinner.set_prefix(format!("0x{:02X}", self.step));
            spinner.set_message("Erasing all flash regions...");
            self.step = self.step.wrapping_add(1);
        }
        let mut erase_address: Vec<u32> = Vec::new();
        for f in write_flash_files.iter() {
            let address = f.address & 0xFF00_0000;
            // 如果ERASE_ADDRESS中的地址已经被擦除过，则跳过
            if erase_address.contains(&address) {
                continue;
            }
            self.command(Command::EraseAll { address: f.address })?;
            erase_address.push(address);
        }
        if !self.base.quiet {
            spinner.finish_with_message("All flash regions erased");
        }
        Ok(())
    }

    fn verify(&mut self, address: u32, len: u32, crc: u32) -> Result<(), std::io::Error> {
        let spinner = ProgressBar::new_spinner();
        if !self.base.quiet {
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));
            spinner.set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            spinner.set_prefix(format!("0x{:02X}", self.step));
            spinner.set_message("Verifying data...");
        }
        let response = self.command(Command::Verify { address, len, crc })?;
        if response != Response::Ok {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Verify failed",
            ));
        }
        if !self.base.quiet {
            spinner.finish_with_message("Verify success!");
        }
        self.step = self.step.wrapping_add(1);
        Ok(())
    }
}

impl WriteFlashTrait for SF32LB52Tool {
    fn write_flash(&mut self, params: &WriteFlashParams) -> Result<(), std::io::Error> {
        let mut step = self.step;

        let mut write_flash_files: Vec<WriteFlashFile> = Vec::new();

        let packet_size = if self.base.compat { 256 } else { 128 * 1024 };

        for file in params.file_path.iter() {
            write_flash_files.append(&mut parse_file_info(file)?);
        }

        if params.erase_all {
            self.erase_all(&write_flash_files)?;
        }

        for file in write_flash_files.iter() {
            let re_download_spinner = ProgressBar::new_spinner();
            let download_bar = ProgressBar::new(file.file.metadata()?.len());

            let download_bar_template = ProgressStyle::default_bar()
                .template("[{prefix}] {msg} {wide_bar} {bytes_per_sec} {percent_precise}%")
                .unwrap()
                .progress_chars("=>-");

            if !params.erase_all {
                if !self.base.quiet {
                    re_download_spinner.enable_steady_tick(std::time::Duration::from_millis(100));
                    re_download_spinner.set_style(
                        ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap(),
                    );
                    re_download_spinner.set_prefix(format!("0x{:02X}", step));
                    re_download_spinner.set_message(format!(
                        "Checking whether a re-download is necessary at address 0x{:08X}...",
                        file.address
                    ));
                    step += 1;
                }
                let response = self.command(Command::Verify {
                    address: file.address,
                    len: file.file.metadata()?.len() as u32,
                    crc: file.crc32,
                })?;
                if response == Response::Ok {
                    if !self.base.quiet {
                        re_download_spinner.finish_with_message("No need to re-download, skip!");
                    }
                    continue;
                }
                if !self.base.quiet {
                    re_download_spinner.finish_with_message("Need to re-download");

                    download_bar.set_style(download_bar_template);
                    download_bar.set_message(format!("Download at 0x{:08X}...", file.address));
                    download_bar.set_prefix(format!("0x{:02X}", step));
                    step += 1;
                }

                let res = self.command(Command::WriteAndErase {
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
                    let res = self.send_data(&buffer[..bytes_read])?;
                    if res == Response::RxWait {
                        if !self.base.quiet {
                            download_bar.inc(bytes_read as u64);
                            // downloaded += bytes_read;
                        }
                        continue;
                    } else if res != Response::Ok {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Write flash failed",
                        ));
                    }
                }

                if !self.base.quiet {
                    download_bar.finish_with_message("Download success!");
                }
            } else {
                let mut buffer = vec![0u8; packet_size];
                let mut reader = BufReader::new(&file.file);

                if !self.base.quiet {
                    download_bar.set_style(download_bar_template);
                    download_bar.set_message(format!("Download at 0x{:08X}...", file.address));
                    download_bar.set_prefix(format!("0x{:02X}", step));
                    step += 1;
                }

                let mut address = file.address;
                loop {
                    let bytes_read = reader.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    self.port.write_all(
                        Command::Write {
                            address: address,
                            len: bytes_read as u32,
                        }
                        .to_string()
                        .as_bytes(),
                    )?;
                    self.port.flush()?;
                    let res = self.send_data(&buffer[..bytes_read])?;
                    if res != Response::Ok {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Write flash failed",
                        ));
                    }
                    address += bytes_read as u32;
                    if !self.base.quiet {
                        download_bar.inc(bytes_read as u64);
                    }
                }
                if !self.base.quiet {
                    download_bar.finish_with_message("Download success!");
                }
            }
            // verify
            if params.verify {
                self.verify(file.address, file.file.metadata()?.len() as u32, file.crc32)?;
            }
        }
        Ok(())
    }
}
