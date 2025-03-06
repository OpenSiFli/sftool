use crate::SifliTool;
use crate::ram_command::{Command, RamCommand, Response};
use crc::Algorithm;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use memmap2::Mmap;
use phf::phf_map;
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::fmt::format;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use tempfile::tempfile;

pub trait WriteFlashTrait {
    fn write_flash(&mut self) -> Result<(), std::io::Error>;
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum FileType {
    Bin,
    Hex,
    Elf,
}

struct WriteFlashFile {
    address: u32,
    file: File,
    crc32: u32,
}

fn str_to_u32(s: &str) -> Result<u32, std::num::ParseIntError> {
    if let Some(hex_digits) = s.strip_prefix("0x") {
        u32::from_str_radix(hex_digits, 16)
    } else if let Some(bin_digits) = s.strip_prefix("0b") {
        u32::from_str_radix(bin_digits, 2)
    } else if let Some(oct_digits) = s.strip_prefix("0o") {
        u32::from_str_radix(oct_digits, 8)
    } else {
        s.parse::<u32>()
    }
}

fn get_file_type(s: &str) -> Result<FileType, std::io::Error> {
    match s.split('.').last().unwrap().to_lowercase().as_str() {
        "bin" => Ok(FileType::Bin),
        "hex" => Ok(FileType::Hex),
        "elf" | "axf" => Ok(FileType::Elf),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Wrong file type",
        )),
    }
}

fn hex_to_bin(hex_file: &Path) -> Result<Vec<WriteFlashFile>, std::io::Error> {
    let mut write_flash_files: Vec<WriteFlashFile> = Vec::new();

    let file = std::fs::File::open(hex_file)?;
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();

    let mut address = 0;
    let mut temp_file = tempfile()?;

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }
        let ihex_record = ihex::Record::from_record_string(&line)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        match ihex_record {
            ihex::Record::ExtendedLinearAddress(addr) => {
                address = (addr as u32) << 16;
            }
            ihex::Record::Data { offset, value } => {
                // 获取当前文件长度
                let metadata = temp_file.metadata()?;
                let current_len = metadata.len();
                let offset_u64 = offset as u64;

                // 如果当前文件长度小于 offset，则说明文件中存在空隙，需要填充 0xFF
                if current_len < offset_u64 {
                    // 先定位到文件末尾（也就是 current_len 位置）
                    temp_file.seek(SeekFrom::End(0))?;

                    // 计算需要填充的字节数
                    let gap_size = offset_u64 - current_len;

                    // 构造一个填充缓冲区，该缓冲区内容全为 0xFF
                    let fill_data = vec![0xFF; gap_size as usize];
                    temp_file.write_all(&fill_data)?;
                }

                // 定位到指定的 offset 开始写入数据
                temp_file.seek(SeekFrom::Start(offset_u64))?;
                temp_file.write_all(&value)?;
            }
            ihex::Record::EndOfFile => {
                temp_file.seek(SeekFrom::Start(0))?;
                let crc32 = get_file_crc32(&temp_file.try_clone()?)?;
                write_flash_files.push(WriteFlashFile {
                    address,
                    file: temp_file.try_clone()?,
                    crc32,
                });
            }
            _ => {}
        }
    }

    Ok(write_flash_files)
}

fn elf_to_bin(elf_file: &Path) -> Result<Vec<WriteFlashFile>, std::io::Error> {
    let mut write_flash_files: Vec<WriteFlashFile> = Vec::new();

    let file = File::open(elf_file)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let elf = goblin::elf::Elf::parse(&mmap[..])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let mut address = 0;
    let mut temp_file = tempfile()?;
    for ph in elf.program_headers.iter() {
        if ph.p_type == goblin::elf::program_header::PT_LOAD {
            let offset = ph.p_offset;
            let size = ph.p_filesz;
            let data = &mmap[offset as usize..(offset + size) as usize];
            temp_file.write_all(data)?;
            let crc32 = get_file_crc32(&temp_file.try_clone()?)?;
            write_flash_files.push(WriteFlashFile {
                address,
                file: temp_file.try_clone()?,
                crc32,
            });
            address += size as u32;
        }
    }

    Ok(write_flash_files)
}

fn get_file_crc32(file: &File) -> Result<u32, std::io::Error> {
    const CRC_32_ALGO: Algorithm<u32> = Algorithm {
        width: 32,
        poly: 0x04C11DB7,
        init: 0,
        refin: true,
        refout: true,
        xorout: 0,
        check: 0x2DFD2D88,
        residue: 0,
    };

    const CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&CRC_32_ALGO);
    let mut reader = BufReader::new(file);

    let mut digest = CRC.digest();

    let mut buffer = [0u8; 4 * 1024 * 1024];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }

    let checksum = digest.finalize();
    reader.seek(SeekFrom::Start(0))?;
    Ok(checksum)
}

lazy_static! {
    static ref CHIP_MEMORY_LAYOUT: HashMap<&'static str, Vec<u32>> = {
        let mut m = HashMap::new();
        m.insert("sf32lb52", vec![0x10000000, 0x12000000]);
        m
    };
}

impl SifliTool {
    fn erase_all(
        &mut self,
        write_flash_files: &[WriteFlashFile],
        mut step: i32,
    ) -> Result<(), std::io::Error> {
        let spinner = ProgressBar::new_spinner();
        if !self.base.quiet {
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));
            spinner.set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            spinner.set_prefix(format!("0x{:02X}", step));
            spinner.set_message("Erasing all flash regions...");
            step += 1;
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
}

impl WriteFlashTrait for SifliTool {
    fn write_flash(&mut self) -> Result<(), std::io::Error> {
        let mut step = 1;
        let params = self
            .write_flash_params
            .as_ref()
            .cloned()
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "No write flash params",
            ))?;
        let mut write_flash_files: Vec<WriteFlashFile> = Vec::new();

        for file in params.file_path.iter() {
            // file@address
            let parts: Vec<_> = file.split('@').collect();
            // 如果存在@符号，则证明是bin文件
            if parts.len() == 2 {
                let addr = str_to_u32(parts[1])
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
                let file = File::open(parts[0])?;
                let crc32 = get_file_crc32(&file.try_clone()?)?;
                write_flash_files.push(WriteFlashFile {
                    address: addr,
                    file,
                    crc32,
                });
                continue;
            }

            let wrong_file_type_err = || -> std::io::Error {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Wrong file type, if you want to download a bin file without a suffix, use the <file@address> form",
                )
            };
            // 判断文件后缀
            let file_suffix = parts[0].split('.').last().ok_or(wrong_file_type_err())?;

            let file_type = get_file_type(file_suffix)?;

            match file_type {
                FileType::Hex => {
                    write_flash_files.append(&mut hex_to_bin(Path::new(parts[0]))?);
                }
                FileType::Elf => {
                    write_flash_files.append(&mut elf_to_bin(Path::new(parts[0]))?);
                }
                FileType::Bin => {
                    return Err(wrong_file_type_err());
                }
            }
        }

        if params.erase_all {
            self.erase_all(&write_flash_files, step)?;
        }

        for file in write_flash_files.iter() {
            let re_download_spinner = ProgressBar::new_spinner();
            let download_bar = ProgressBar::new(file.file.metadata()?.len());
            let mut downloaded = 0;

            let download_bar_template = ProgressStyle::default_bar()
                .template("[{prefix}] Download at {msg}... {wide_bar} {bytes}/{total_bytes}")
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
                    download_bar.set_message(format!("0x{:08X}", file.address));
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

                let mut buffer = [0u8; 128 * 1024];
                let mut reader = BufReader::new(&file.file);

                loop {
                    let bytes_read = reader.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    let res = self.send_data(&buffer[..bytes_read])?;
                    if res == Response::RxWait {
                        if !self.base.quiet {
                            download_bar.inc(downloaded as u64);
                            downloaded += bytes_read;
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
                let mut buffer = [0u8; 128 * 1024];
                let mut reader = BufReader::new(&file.file);

                if !self.base.quiet {
                    download_bar.set_style(download_bar_template);
                    download_bar.set_message(format!("0x{:08X}", file.address));
                    download_bar.set_prefix(format!("0x{:02X}", step));
                    step += 1;
                }

                loop {
                    let bytes_read = reader.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    self.port.write_all(
                        Command::Write {
                            address: file.address,
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
                    if !self.base.quiet {
                        download_bar.inc(downloaded as u64);
                        downloaded += bytes_read;
                    }
                }
                if !self.base.quiet {
                    download_bar.finish_with_message("Download success!");
                }
            }
        }
        Ok(())
    }
}
