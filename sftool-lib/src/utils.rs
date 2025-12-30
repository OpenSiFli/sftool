use crate::{Error, Result, WriteFlashFile};
use crc::Algorithm;
use memmap2::Mmap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use tempfile::tempfile;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FileType {
    Bin,
    Hex,
    Elf,
    Unknown,
}

pub const ELF_MAGIC: &[u8] = &[0x7F, 0x45, 0x4C, 0x46]; // ELF file magic number

pub struct Utils;
impl Utils {
    pub fn str_to_u32(s: &str) -> Result<u32> {
        let s = s.trim();

        let (num_str, multiplier) = match s.chars().last() {
            Some('k') | Some('K') => (&s[..s.len() - 1], 1_000u32),
            Some('m') | Some('M') => (&s[..s.len() - 1], 1_000_000u32),
            Some('g') | Some('G') => (&s[..s.len() - 1], 1_000_000_000u32),
            _ => (s, 1),
        };

        let unsigned: u32 = if let Some(hex) = num_str.strip_prefix("0x") {
            u32::from_str_radix(hex, 16)?
        } else if let Some(bin) = num_str.strip_prefix("0b") {
            u32::from_str_radix(bin, 2)?
        } else if let Some(oct) = num_str.strip_prefix("0o") {
            u32::from_str_radix(oct, 8)?
        } else {
            num_str.parse()?
        };

        Ok(unsigned * multiplier)
    }

    pub(crate) fn get_file_crc32(file: &File) -> Result<u32> {
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

        let mut buffer = [0u8; 4 * 1024];
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

    /// 文件类型检测
    pub fn detect_file_type(path: &Path) -> Result<FileType> {
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

        // 如果MAGIC也无法识别，返回Unknown
        Ok(FileType::Unknown)
    }

    /// 解析文件信息，支持file@address格式
    pub fn parse_file_info(file_str: &str) -> Result<Vec<WriteFlashFile>> {
        // file@address
        let parts: Vec<_> = file_str.split('@').collect();
        // 如果存在@符号，需要先检查文件类型
        if parts.len() == 2 {
            let addr = Self::str_to_u32(parts[1])?;

            let file_type = Self::detect_file_type(Path::new(parts[0]))?;

            match file_type {
                FileType::Hex => {
                    // 对于HEX文件，使用带基地址覆盖的处理函数
                    return Self::hex_with_base_to_write_flash_files(
                        Path::new(parts[0]),
                        Some(addr),
                    );
                }
                FileType::Elf => {
                    // ELF文件不支持@地址格式
                    return Err(Error::invalid_input(
                        "ELF files do not support @address format",
                    ));
                }
                _ => {
                    // 对于其他文件类型，使用原来的处理方式
                    let file = std::fs::File::open(parts[0])?;
                    let crc32 = Self::get_file_crc32(&file)?;

                    return Ok(vec![WriteFlashFile {
                        address: addr,
                        file,
                        crc32,
                    }]);
                }
            }
        }

        let file_type = Self::detect_file_type(Path::new(parts[0]))?;

        match file_type {
            FileType::Hex => Self::hex_to_write_flash_files(Path::new(parts[0])),
            FileType::Elf => Self::elf_to_write_flash_files(Path::new(parts[0])),
            _ => Err(Error::invalid_input(
                "For binary files, please use the <file@address> format",
            )),
        }
    }

    /// 解析写入文件信息，直接使用路径与可选地址
    pub fn parse_write_file(path: &str, address: Option<u32>) -> Result<Vec<WriteFlashFile>> {
        let file_path = Path::new(path);
        match address {
            Some(addr) => {
                let file_type = Self::detect_file_type(file_path)?;
                match file_type {
                    FileType::Hex => {
                        Self::hex_with_base_to_write_flash_files(file_path, Some(addr))
                    }
                    FileType::Elf => Err(Error::invalid_input(
                        "ELF files do not support @address format",
                    )),
                    _ => {
                        let file = std::fs::File::open(file_path)?;
                        let crc32 = Self::get_file_crc32(&file)?;
                        Ok(vec![WriteFlashFile {
                            address: addr,
                            file,
                            crc32,
                        }])
                    }
                }
            }
            None => {
                let file_type = Self::detect_file_type(file_path)?;
                match file_type {
                    FileType::Hex => Self::hex_to_write_flash_files(file_path),
                    FileType::Elf => Self::elf_to_write_flash_files(file_path),
                    _ => Err(Error::invalid_input(
                        "For binary files, please use the <file@address> format",
                    )),
                }
            }
        }
    }

    /// 计算数据的CRC32
    pub fn calculate_crc32(data: &[u8]) -> u32 {
        const CRC_32_ALGO: Algorithm<u32> = Algorithm {
            width: 32,
            poly: 0x04C11DB7,
            init: 0,
            refin: true,
            refout: true,
            xorout: 0,
            check: 0,
            residue: 0,
        };
        crc::Crc::<u32>::new(&CRC_32_ALGO).checksum(data)
    }

    /// 将HEX文件转换为WriteFlashFile
    pub fn hex_to_write_flash_files(hex_file: &Path) -> Result<Vec<WriteFlashFile>> {
        let mut write_flash_files: Vec<WriteFlashFile> = Vec::new();

        let file = std::fs::File::open(hex_file)?;
        let reader = std::io::BufReader::new(file);

        let mut current_base_address = 0u32;
        let mut current_temp_file: Option<File> = None;
        let mut current_segment_start = 0u32;
        let mut current_file_offset = 0u32;

        for line in reader.lines() {
            let line = line?;
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }

            let ihex_record = ihex::Record::from_record_string(line)?;

            match ihex_record {
                ihex::Record::ExtendedLinearAddress(addr) => {
                    let new_base_address = (addr as u32) << 16;

                    // We don't need to do anything special for ExtendedLinearAddress anymore
                    // Just update the current_base_address for calculating absolute addresses
                    current_base_address = new_base_address;
                }
                ihex::Record::Data { offset, value } => {
                    let absolute_address = current_base_address + offset as u32;

                    // Check if we need to start a new segment based on address continuity
                    let should_start_new_segment = if let Some(ref _temp_file) = current_temp_file {
                        let current_end_address = current_segment_start + current_file_offset;
                        let expected_start_address = absolute_address;

                        // If the new data is not continuous with existing data, start new segment
                        // Allow for some reasonable gap (e.g., 4KB) to be filled, but beyond that start new segment
                        let gap_size = if expected_start_address >= current_end_address {
                            expected_start_address - current_end_address
                        } else {
                            // Overlapping or backwards, definitely need new segment
                            u32::MAX
                        };

                        // If gap is too large (> 4KB), start new segment
                        gap_size > 0x1000
                    } else {
                        false // No current file, will create one below
                    };

                    if should_start_new_segment {
                        // Finalize current segment
                        if let Some(temp_file) = current_temp_file.take() {
                            Self::finalize_segment(
                                temp_file,
                                current_segment_start,
                                &mut write_flash_files,
                            )?;
                        }
                    }

                    // If this is the first data record or start of a new segment
                    if current_temp_file.is_none() {
                        current_temp_file = Some(tempfile()?);
                        current_segment_start = absolute_address;
                        current_file_offset = 0;
                    }

                    if let Some(ref mut temp_file) = current_temp_file {
                        let expected_file_offset = absolute_address - current_segment_start;

                        // Fill gaps with 0xFF if they exist
                        if expected_file_offset > current_file_offset {
                            let gap_size = expected_file_offset - current_file_offset;
                            let fill_data = vec![0xFF; gap_size as usize];
                            temp_file.write_all(&fill_data)?;
                            current_file_offset = expected_file_offset;
                        }

                        // Write data
                        temp_file.write_all(&value)?;
                        current_file_offset += value.len() as u32;
                    }
                }
                ihex::Record::EndOfFile => {
                    // Finalize the last segment
                    if let Some(temp_file) = current_temp_file.take() {
                        Self::finalize_segment(
                            temp_file,
                            current_segment_start,
                            &mut write_flash_files,
                        )?;
                    }
                    break;
                }
                _ => {}
            }
        }

        // If file ends without encountering EndOfFile record, finalize current segment
        if let Some(temp_file) = current_temp_file.take() {
            Self::finalize_segment(temp_file, current_segment_start, &mut write_flash_files)?;
        }

        Ok(write_flash_files)
    }

    /// 将HEX文件转换为WriteFlashFile，支持基地址覆盖
    /// base_address_override: 如果提供，将用其高8位替换ExtendedLinearAddress中的高8位
    pub fn hex_with_base_to_write_flash_files(
        hex_file: &Path,
        base_address_override: Option<u32>,
    ) -> Result<Vec<WriteFlashFile>> {
        let mut write_flash_files: Vec<WriteFlashFile> = Vec::new();

        let file = std::fs::File::open(hex_file)?;
        let reader = std::io::BufReader::new(file);

        let mut current_base_address = 0u32;
        let mut current_temp_file: Option<File> = None;
        let mut current_segment_start = 0u32;
        let mut current_file_offset = 0u32;

        for line in reader.lines() {
            let line = line?;
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }

            let ihex_record = ihex::Record::from_record_string(line)?;

            match ihex_record {
                ihex::Record::ExtendedLinearAddress(addr) => {
                    let new_base_address = if let Some(override_addr) = base_address_override {
                        // 只替换高8位：(原值 & 0x00FF) | ((新地址 >> 16) & 0xFF00)
                        let modified_addr =
                            (addr & 0x00FF) | ((override_addr >> 16) as u16 & 0xFF00);
                        (modified_addr as u32) << 16
                    } else {
                        (addr as u32) << 16
                    };

                    // We don't need to do anything special for ExtendedLinearAddress anymore
                    // Just update the current_base_address for calculating absolute addresses
                    current_base_address = new_base_address;
                }
                ihex::Record::Data { offset, value } => {
                    let absolute_address = current_base_address + offset as u32;

                    // Check if we need to start a new segment based on address continuity
                    let should_start_new_segment = if let Some(ref _temp_file) = current_temp_file {
                        let current_end_address = current_segment_start + current_file_offset;
                        let expected_start_address = absolute_address;

                        // If the new data is not continuous with existing data, start new segment
                        // Allow for some reasonable gap (e.g., 4KB) to be filled, but beyond that start new segment
                        let gap_size = if expected_start_address >= current_end_address {
                            expected_start_address - current_end_address
                        } else {
                            // Overlapping or backwards, definitely need new segment
                            u32::MAX
                        };

                        // If gap is too large (> 4KB), start new segment
                        gap_size > 0x1000
                    } else {
                        false // No current file, will create one below
                    };

                    if should_start_new_segment {
                        // Finalize current segment
                        if let Some(temp_file) = current_temp_file.take() {
                            Self::finalize_segment(
                                temp_file,
                                current_segment_start,
                                &mut write_flash_files,
                            )?;
                        }
                    }

                    // If this is the first data record or start of a new segment
                    if current_temp_file.is_none() {
                        current_temp_file = Some(tempfile()?);
                        current_segment_start = absolute_address;
                        current_file_offset = 0;
                    }

                    if let Some(ref mut temp_file) = current_temp_file {
                        let expected_file_offset = absolute_address - current_segment_start;

                        // Fill gaps with 0xFF if they exist
                        if expected_file_offset > current_file_offset {
                            let gap_size = expected_file_offset - current_file_offset;
                            let fill_data = vec![0xFF; gap_size as usize];
                            temp_file.write_all(&fill_data)?;
                            current_file_offset = expected_file_offset;
                        }

                        // Write data
                        temp_file.write_all(&value)?;
                        current_file_offset += value.len() as u32;
                    }
                }
                ihex::Record::EndOfFile => {
                    // Finalize the last segment
                    if let Some(temp_file) = current_temp_file.take() {
                        Self::finalize_segment(
                            temp_file,
                            current_segment_start,
                            &mut write_flash_files,
                        )?;
                    }
                    break;
                }
                _ => {}
            }
        }

        // If file ends without encountering EndOfFile record, finalize current segment
        if let Some(temp_file) = current_temp_file.take() {
            Self::finalize_segment(temp_file, current_segment_start, &mut write_flash_files)?;
        }

        Ok(write_flash_files)
    }

    /// 将ELF文件转换为WriteFlashFile  
    pub fn elf_to_write_flash_files(elf_file: &Path) -> Result<Vec<WriteFlashFile>> {
        let mut write_flash_files: Vec<WriteFlashFile> = Vec::new();
        const SECTOR_SIZE: u32 = 0x1000; // 扇区大小
        const FILL_BYTE: u8 = 0xFF; // 填充字节

        let file = File::open(elf_file)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let elf = goblin::elf::Elf::parse(&mmap[..])?;

        // 收集所有需要烧录的段
        let mut load_segments: Vec<_> = elf
            .program_headers
            .iter()
            .filter(|ph| {
                ph.p_type == goblin::elf::program_header::PT_LOAD && ph.p_paddr < 0x2000_0000
            })
            .collect();
        load_segments.sort_by_key(|ph| ph.p_paddr);

        if load_segments.is_empty() {
            return Ok(write_flash_files);
        }

        let mut current_file = tempfile()?;
        let mut current_base = (load_segments[0].p_paddr as u32) & !(SECTOR_SIZE - 1);
        let mut current_offset = 0; // 跟踪当前文件中的偏移量

        for ph in load_segments.iter() {
            let vaddr = ph.p_paddr as u32;
            let offset = ph.p_offset as usize;
            let size = ph.p_filesz as usize;
            let data = &mmap[offset..offset + size];

            // 计算当前段的对齐基地址
            let segment_base = vaddr & !(SECTOR_SIZE - 1);

            // 如果超出了当前对齐块，创建新文件
            if segment_base > current_base + current_offset {
                current_file.seek(std::io::SeekFrom::Start(0))?;
                let crc32 = Self::get_file_crc32(&current_file)?;
                write_flash_files.push(WriteFlashFile {
                    address: current_base,
                    file: std::mem::replace(&mut current_file, tempfile()?),
                    crc32,
                });
                current_base = segment_base;
                current_offset = 0;
            }

            // 计算相对于当前文件基地址的偏移
            let relative_offset = vaddr - current_base;

            // 如果当前偏移小于目标偏移，填充间隙
            if current_offset < relative_offset {
                let padding = relative_offset - current_offset;
                current_file.write_all(&vec![FILL_BYTE; padding as usize])?;
                current_offset = relative_offset;
            }

            // 写入数据
            current_file.write_all(data)?;
            current_offset += size as u32;
        }

        // 处理最后一个文件
        if current_offset > 0 {
            current_file.seek(std::io::SeekFrom::Start(0))?;
            let crc32 = Self::get_file_crc32(&current_file)?;
            write_flash_files.push(WriteFlashFile {
                address: current_base,
                file: current_file,
                crc32,
            });
        }

        Ok(write_flash_files)
    }

    /// 完成一个段的处理，将临时文件转换为WriteFlashFile
    fn finalize_segment(
        mut temp_file: File,
        address: u32,
        write_flash_files: &mut Vec<WriteFlashFile>,
    ) -> Result<()> {
        temp_file.seek(std::io::SeekFrom::Start(0))?;
        let crc32 = Self::get_file_crc32(&temp_file)?;
        write_flash_files.push(WriteFlashFile {
            address,
            file: temp_file,
            crc32,
        });
        Ok(())
    }

    /// 解析读取文件信息 (filename@address:size格式)
    pub fn parse_read_file_info(file_spec: &str) -> Result<crate::ReadFlashFile> {
        let Some((file_path, addr_size)) = file_spec.split_once('@') else {
            return Err(Error::invalid_input(format!(
                "Invalid format: {}. Expected: filename@address:size",
                file_spec
            )));
        };

        let Some((address_str, size_str)) = addr_size.split_once(':') else {
            return Err(Error::invalid_input(format!(
                "Invalid address:size format: {}. Expected: address:size",
                addr_size
            )));
        };

        let address = Self::str_to_u32(address_str).map_err(|e| {
            Error::invalid_input(format!("Invalid address '{}': {}", address_str, e))
        })?;

        let size = Self::str_to_u32(size_str)
            .map_err(|e| Error::invalid_input(format!("Invalid size '{}': {}", size_str, e)))?;

        Ok(crate::ReadFlashFile {
            file_path: file_path.to_string(),
            address,
            size,
        })
    }

    /// 解析擦除地址
    pub fn parse_erase_address(address_str: &str) -> Result<u32> {
        Self::str_to_u32(address_str)
            .map_err(|e| Error::invalid_input(format!("Invalid address '{}': {}", address_str, e)))
    }

    /// 解析擦除区域信息 (address:size格式)
    pub fn parse_erase_region(region_spec: &str) -> Result<crate::EraseRegionFile> {
        let Some((address_str, size_str)) = region_spec.split_once(':') else {
            return Err(Error::invalid_input(format!(
                "Invalid region format: {}. Expected: address:size",
                region_spec
            )));
        };

        let address = Self::str_to_u32(address_str).map_err(|e| {
            Error::invalid_input(format!("Invalid address '{}': {}", address_str, e))
        })?;

        let size = Self::str_to_u32(size_str)
            .map_err(|e| Error::invalid_input(format!("Invalid size '{}': {}", size_str, e)))?;

        Ok(crate::EraseRegionFile { address, size })
    }
}
