use crate::WriteFlashFile;
use crc::Algorithm;
use memmap2::Mmap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::num::ParseIntError;
use std::path::Path;
use tempfile::tempfile;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FileType {
    Bin,
    Hex,
    Elf,
}

pub const ELF_MAGIC: &[u8] = &[0x7F, 0x45, 0x4C, 0x46]; // ELF file magic number

pub struct Utils;
impl Utils {
    pub fn str_to_u32(s: &str) -> Result<u32, ParseIntError> {
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

    pub(crate) fn get_file_crc32(file: &File) -> Result<u32, std::io::Error> {
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
    pub fn detect_file_type(path: &Path) -> Result<FileType, std::io::Error> {
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

    /// 解析文件信息，支持file@address格式
    pub fn parse_file_info(file_str: &str) -> Result<Vec<WriteFlashFile>, std::io::Error> {
        // file@address
        let parts: Vec<_> = file_str.split('@').collect();
        // 如果存在@符号，需要先检查文件类型
        if parts.len() == 2 {
            let addr = Self::str_to_u32(parts[1])
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            
            let file_type = Self::detect_file_type(Path::new(parts[0]))?;
            
            match file_type {
                FileType::Hex => {
                    // 对于HEX文件，使用带基地址覆盖的处理函数
                    return Self::hex_with_base_to_write_flash_files(Path::new(parts[0]), Some(addr));
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
            FileType::Bin => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "For binary files, please use the <file@address> format",
            )),
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
    pub fn hex_to_write_flash_files(
        hex_file: &Path,
    ) -> Result<Vec<WriteFlashFile>, std::io::Error> {
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

            let ihex_record = ihex::Record::from_record_string(&line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            match ihex_record {
                ihex::Record::ExtendedLinearAddress(addr) => {
                    let new_base_address = (addr as u32) << 16;

                    // If base address changes, finalize current segment and start a new one
                    if new_base_address != current_base_address && current_temp_file.is_some() {
                        // Finalize current segment
                        if let Some(temp_file) = current_temp_file.take() {
                            Self::finalize_segment(
                                temp_file,
                                current_segment_start,
                                &mut write_flash_files,
                            )?;
                        }
                        current_file_offset = 0;
                    }

                    current_base_address = new_base_address;
                }
                ihex::Record::Data { offset, value } => {
                    let absolute_address = current_base_address + offset as u32;

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
    ) -> Result<Vec<WriteFlashFile>, std::io::Error> {
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

            let ihex_record = ihex::Record::from_record_string(&line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            match ihex_record {
                ihex::Record::ExtendedLinearAddress(addr) => {
                    let new_base_address = if let Some(override_addr) = base_address_override {
                        // 只替换高8位：(原值 & 0x00FF) | ((新地址 >> 16) & 0xFF00)
                        let modified_addr = (addr & 0x00FF) | ((override_addr >> 16) as u16 & 0xFF00);
                        (modified_addr as u32) << 16
                    } else {
                        (addr as u32) << 16
                    };

                    // If base address changes, finalize current segment and start a new one
                    if new_base_address != current_base_address && current_temp_file.is_some() {
                        // Finalize current segment
                        if let Some(temp_file) = current_temp_file.take() {
                            Self::finalize_segment(
                                temp_file,
                                current_segment_start,
                                &mut write_flash_files,
                            )?;
                        }
                        current_file_offset = 0;
                    }

                    current_base_address = new_base_address;
                }
                ihex::Record::Data { offset, value } => {
                    let absolute_address = current_base_address + offset as u32;

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
    pub fn elf_to_write_flash_files(
        elf_file: &Path,
    ) -> Result<Vec<WriteFlashFile>, std::io::Error> {
        let mut write_flash_files: Vec<WriteFlashFile> = Vec::new();
        const SECTOR_SIZE: u32 = 0x1000; // 扇区大小
        const FILL_BYTE: u8 = 0xFF; // 填充字节

        let file = File::open(elf_file)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let elf = goblin::elf::Elf::parse(&mmap[..])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

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
    ) -> Result<(), std::io::Error> {
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
    pub fn parse_read_file_info(file_spec: &str) -> Result<crate::ReadFlashFile, std::io::Error> {
        let Some((file_path, addr_size)) = file_spec.split_once('@') else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Invalid format: {}. Expected: filename@address:size",
                    file_spec
                ),
            ));
        };

        let Some((address_str, size_str)) = addr_size.split_once(':') else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Invalid address:size format: {}. Expected: address:size",
                    addr_size
                ),
            ));
        };

        let address = Self::str_to_u32(address_str).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid address '{}': {}", address_str, e),
            )
        })?;

        let size = Self::str_to_u32(size_str).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid size '{}': {}", size_str, e),
            )
        })?;

        Ok(crate::ReadFlashFile {
            file_path: file_path.to_string(),
            address,
            size,
        })
    }

    /// 解析擦除地址
    pub fn parse_erase_address(address_str: &str) -> Result<u32, std::io::Error> {
        Self::str_to_u32(address_str).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid address '{}': {}", address_str, e),
            )
        })
    }

    /// 解析擦除区域信息 (address:size格式)
    pub fn parse_erase_region(region_spec: &str) -> Result<crate::EraseRegionFile, std::io::Error> {
        let Some((address_str, size_str)) = region_spec.split_once(':') else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Invalid region format: {}. Expected: address:size",
                    region_spec
                ),
            ));
        };

        let address = Self::str_to_u32(address_str).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid address '{}': {}", address_str, e),
            )
        })?;

        let size = Self::str_to_u32(size_str).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid size '{}': {}", size_str, e),
            )
        })?;

        Ok(crate::EraseRegionFile { address, size })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};
    use tempfile::NamedTempFile;

    #[test]
    fn test_hex_to_bin_single_segment() {
        // Create a simple hex file with one segment using correct Intel HEX checksums
        let hex_content = ":0400000001020304F2\n:0410000005060708D2\n:00000001FF\n";

        let mut temp_hex = NamedTempFile::new().unwrap();
        temp_hex.write_all(hex_content.as_bytes()).unwrap();

        let result = Utils::hex_to_write_flash_files(temp_hex.path()).unwrap();

        // Should have one segment
        assert_eq!(result.len(), 1);

        let segment = &result[0];
        assert_eq!(segment.address, 0x00000000);

        // Check data size (gap filled from 0x0000 to 0x1003)
        let file_size = segment.file.metadata().unwrap().len() as usize;
        assert_eq!(file_size, 0x1004);

        // Read file content to verify data
        let mut file_data = Vec::new();
        let mut file = &segment.file;
        file.read_to_end(&mut file_data).unwrap();

        // Verify gap filling
        // First 4 bytes should be the original data: 01 02 03 04
        assert_eq!(&file_data[0..4], &[0x01, 0x02, 0x03, 0x04]);
        // Gap between 0x04 and 0x1000 should be filled with 0xFF
        assert!(file_data[4..0x1000].iter().all(|&b| b == 0xFF));
        // Last 4 bytes should be: 05 06 07 08
        assert_eq!(&file_data[0x1000..0x1004], &[0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn test_hex_to_bin_multiple_segments() {
        // Create a hex file with multiple segments using correct checksums
        let hex_content =
            ":0400000001020304F2\n:020000040001F9\n:0400000011121314B2\n:00000001FF\n";

        let mut temp_hex = NamedTempFile::new().unwrap();
        temp_hex.write_all(hex_content.as_bytes()).unwrap();

        let result = Utils::hex_to_write_flash_files(temp_hex.path()).unwrap();

        // Should have two segments
        assert_eq!(result.len(), 2);

        // First segment at 0x00000000
        assert_eq!(result[0].address, 0x00000000);
        let file_size_0 = result[0].file.metadata().unwrap().len() as usize;
        assert_eq!(file_size_0, 4);

        let mut file_data_0 = Vec::new();
        let mut file_0 = &result[0].file;
        file_0.read_to_end(&mut file_data_0).unwrap();
        assert_eq!(&file_data_0, &[0x01, 0x02, 0x03, 0x04]);

        // Second segment at 0x00010000
        assert_eq!(result[1].address, 0x00010000);
        let file_size_1 = result[1].file.metadata().unwrap().len() as usize;
        assert_eq!(file_size_1, 4);

        let mut file_data_1 = Vec::new();
        let mut file_1 = &result[1].file;
        file_1.read_to_end(&mut file_data_1).unwrap();
        assert_eq!(&file_data_1, &[0x11, 0x12, 0x13, 0x14]);
    }

    #[test]
    fn test_hex_to_bin_with_gaps() {
        // Create a hex file with gaps that should be filled with 0xFF
        let hex_content = ":04000000AABBCCDDEE\n:04100000EEFF0011EE\n:00000001FF\n";

        let mut temp_hex = NamedTempFile::new().unwrap();
        temp_hex.write_all(hex_content.as_bytes()).unwrap();

        let result = Utils::hex_to_write_flash_files(temp_hex.path()).unwrap();

        // Debug: print actual results
        println!("Number of segments: {}", result.len());
        for (i, segment) in result.iter().enumerate() {
            let file_size = segment.file.metadata().unwrap().len() as usize;
            println!(
                "Segment {}: address=0x{:08X}, size={}",
                i, segment.address, file_size
            );
        }

        // Should have one segment
        assert_eq!(result.len(), 1);

        let segment = &result[0];
        assert_eq!(segment.address, 0x00000000);

        // Should have 4 bytes data + 4092 bytes gap + 4 bytes data = 4100 bytes
        let file_size = segment.file.metadata().unwrap().len() as usize;
        println!(
            "Expected size: 0x1004 ({}), Actual size: {}",
            0x1004, file_size
        );
        assert_eq!(file_size, 0x1004);

        // Read file content to verify data
        let mut file_data = Vec::new();
        let mut file = &segment.file;
        file.read_to_end(&mut file_data).unwrap();

        // Verify first 4 bytes
        assert_eq!(&file_data[0..4], &[0xAA, 0xBB, 0xCC, 0xDD]);
        // Verify gap is filled with 0xFF
        assert!(file_data[4..0x1000].iter().all(|&b| b == 0xFF));
        // Verify last 4 bytes
        assert_eq!(&file_data[0x1000..0x1004], &[0xEE, 0xFF, 0x00, 0x11]);

        // Read the file and check gap is filled with 0xFF
        let mut file = segment.file.try_clone().unwrap();
        file.seek(SeekFrom::Start(4)).unwrap();
        let mut gap_data = vec![0; 0x1000 - 4];
        file.read_exact(&mut gap_data).unwrap();

        // All gap bytes should be 0xFF
        assert!(gap_data.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn test_hex_to_bin_complex_multi_segment() {
        // Create a complex hex file with multiple segments, gaps, and different sizes
        let hex_content = ":100000000102030405060708090A0B0C0D0E0F1068\n:08100000111213141516171844\n:020000040001F9\n:040000002122232472\n:041000003132333422\n:020000040010EA\n:080000004142434445464748D4\n:00000001FF\n";

        let mut temp_hex = NamedTempFile::new().unwrap();
        temp_hex.write_all(hex_content.as_bytes()).unwrap();

        let result = Utils::hex_to_write_flash_files(temp_hex.path()).unwrap();

        // Should have three segments
        assert_eq!(result.len(), 3);

        // First segment at 0x00000000 (contains data at 0x0000 and 0x1000 with gap)
        assert_eq!(result[0].address, 0x00000000);
        let file_size_0 = result[0].file.metadata().unwrap().len() as usize;
        assert_eq!(file_size_0, 0x1008); // 0x1000 + 8 bytes

        // Second segment at 0x00010000 (contains data at 0x0000 and 0x1000 with gap)
        assert_eq!(result[1].address, 0x00010000);
        let file_size_1 = result[1].file.metadata().unwrap().len() as usize;
        assert_eq!(file_size_1, 0x1004); // 0x1000 + 4 bytes

        // Third segment at 0x00100000
        assert_eq!(result[2].address, 0x00100000);
        let file_size_2 = result[2].file.metadata().unwrap().len() as usize;
        assert_eq!(file_size_2, 8);

        // Read file content to verify data for first segment
        let mut file_data_0 = Vec::new();
        let mut file_0 = &result[0].file;
        file_0.read_to_end(&mut file_data_0).unwrap();

        // Verify gap filling in first segment
        // First 16 bytes should be the original data: 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F 10
        assert_eq!(
            &file_data_0[0..16],
            &[
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
                0x0F, 0x10
            ]
        );
        // Gap between 0x10 and 0x1000 should be filled with 0xFF
        assert!(file_data_0[16..0x1000].iter().all(|&b| b == 0xFF));
        // Last 8 bytes should be the second data block
        assert_eq!(
            &file_data_0[0x1000..0x1008],
            &[0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18]
        );
    }

    #[test]
    fn test_str_to_u32() {
        assert_eq!(Utils::str_to_u32("123").unwrap(), 123);
        assert_eq!(Utils::str_to_u32("0x10").unwrap(), 16);
        assert_eq!(Utils::str_to_u32("0b1010").unwrap(), 10);
        assert_eq!(Utils::str_to_u32("0o17").unwrap(), 15);
        assert_eq!(Utils::str_to_u32("1k").unwrap(), 1000);
        assert_eq!(Utils::str_to_u32("1K").unwrap(), 1000);
        assert_eq!(Utils::str_to_u32("1m").unwrap(), 1000000);
        assert_eq!(Utils::str_to_u32("1M").unwrap(), 1000000);
    }

    #[test]
    fn test_parse_read_file_info() {
        let result = Utils::parse_read_file_info("output.bin@0x1000:0x100").unwrap();
        assert_eq!(result.file_path, "output.bin");
        assert_eq!(result.address, 0x1000);
        assert_eq!(result.size, 0x100);

        let result = Utils::parse_read_file_info("data.bin@0x20000000:1k").unwrap();
        assert_eq!(result.file_path, "data.bin");
        assert_eq!(result.address, 0x20000000);
        assert_eq!(result.size, 1000);

        // Test error cases
        assert!(Utils::parse_read_file_info("invalid_format").is_err());
        assert!(Utils::parse_read_file_info("file@0x1000").is_err()); // missing size
        assert!(Utils::parse_read_file_info("file@invalid:0x100").is_err()); // invalid address
    }

    #[test]
    fn test_parse_erase_address() {
        assert_eq!(Utils::parse_erase_address("0x1000").unwrap(), 0x1000);
        assert_eq!(Utils::parse_erase_address("1000").unwrap(), 1000);
        assert_eq!(Utils::parse_erase_address("1k").unwrap(), 1000);

        // Test error cases
        assert!(Utils::parse_erase_address("invalid").is_err());
    }

    #[test]
    fn test_parse_erase_region() {
        let result = Utils::parse_erase_region("0x1000:0x100").unwrap();
        assert_eq!(result.address, 0x1000);
        assert_eq!(result.size, 0x100);

        let result = Utils::parse_erase_region("0x20000000:1k").unwrap();
        assert_eq!(result.address, 0x20000000);
        assert_eq!(result.size, 1000);

        // Test error cases
        assert!(Utils::parse_erase_region("invalid_format").is_err());
        assert!(Utils::parse_erase_region("0x1000").is_err()); // missing size
        assert!(Utils::parse_erase_region("invalid:0x100").is_err()); // invalid address
    }

    #[test]
    fn test_hex_with_base_to_write_flash_files() {
        // Create a hex file with ExtendedLinearAddress that should be modified
        // ExtendedLinearAddress(0x0801) -> should become 0x1001 when base_address_override = 0x10000000
        let hex_content = ":020000040801F1\n:0400000001020304F2\n:00000001FF\n";

        let mut temp_hex = NamedTempFile::new().unwrap();
        temp_hex.write_all(hex_content.as_bytes()).unwrap();

        // Test with base address override
        let result = Utils::hex_with_base_to_write_flash_files(temp_hex.path(), Some(0x10000000)).unwrap();

        // Should have one segment
        assert_eq!(result.len(), 1);

        let segment = &result[0];
        // Original ExtendedLinearAddress was 0x0801 (base address 0x08010000)
        // With override 0x10000000, should become 0x1001 (base address 0x10010000)
        // (0x0801 & 0x00FF) | ((0x10000000 >> 16) & 0xFF00) = 0x01 | 0x1000 = 0x1001
        assert_eq!(segment.address, 0x10010000);

        // Test without base address override (should work like original function)
        let result_no_override = Utils::hex_with_base_to_write_flash_files(temp_hex.path(), None).unwrap();
        assert_eq!(result_no_override.len(), 1);
        assert_eq!(result_no_override[0].address, 0x08010000);
    }

    #[test]
    fn test_parse_file_info_hex_with_address() {
        // Create a hex file for testing
        let hex_content = ":020000040801F1\n:0400000001020304F2\n:00000001FF\n";
        
        let mut temp_hex = NamedTempFile::new().unwrap();
        temp_hex.write_all(hex_content.as_bytes()).unwrap();

        // Create a file with .hex extension for proper type detection
        let hex_file_path = temp_hex.path().with_extension("hex");
        std::fs::copy(temp_hex.path(), &hex_file_path).unwrap();

        // Test parsing HEX file with @address format
        let file_spec = format!("{}@0x10000000", hex_file_path.display());
        let result = Utils::parse_file_info(&file_spec).unwrap();

        // Should have one segment with modified address
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].address, 0x10010000);

        // Clean up
        std::fs::remove_file(&hex_file_path).unwrap();
    }

    #[test]
    fn test_parse_file_info_elf_with_address_error() {
        // Create a temporary ELF-like file (just with ELF magic)
        let mut temp_elf = NamedTempFile::new().unwrap();
        temp_elf.write_all(&[0x7F, 0x45, 0x4C, 0x46]).unwrap(); // ELF magic

        let elf_file_path = temp_elf.path().with_extension("elf");
        std::fs::copy(temp_elf.path(), &elf_file_path).unwrap();

        // Test that ELF files with @address format return an error
        let file_spec = format!("{}@0x10000000", elf_file_path.display());
        let result = Utils::parse_file_info(&file_spec);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ELF files do not support"));

        // Clean up
        std::fs::remove_file(&elf_file_path).unwrap();
    }

    #[test]
    fn test_extended_linear_address_replacement_edge_cases() {
        // Test various ExtendedLinearAddress values with different override addresses
        
        // Case 1: ExtendedLinearAddress(0x0000) with override 0x12000000
        // (0x0000 & 0x00FF) | ((0x12000000 >> 16) & 0xFF00) = 0x00 | 0x1200 = 0x1200
        let hex_content1 = ":020000040000FA\n:0400000001020304F2\n:00000001FF\n";
        let mut temp_hex1 = NamedTempFile::new().unwrap();
        temp_hex1.write_all(hex_content1.as_bytes()).unwrap();
        
        let result1 = Utils::hex_with_base_to_write_flash_files(temp_hex1.path(), Some(0x12000000)).unwrap();
        assert_eq!(result1[0].address, 0x12000000);

        // Case 2: ExtendedLinearAddress(0x00FF) with override 0x34000000  
        // (0x00FF & 0x00FF) | ((0x34000000 >> 16) & 0xFF00) = 0xFF | 0x3400 = 0x34FF
        let hex_content2 = ":0200000400FFFB\n:0400000001020304F2\n:00000001FF\n";
        let mut temp_hex2 = NamedTempFile::new().unwrap();
        temp_hex2.write_all(hex_content2.as_bytes()).unwrap();
        
        let result2 = Utils::hex_with_base_to_write_flash_files(temp_hex2.path(), Some(0x34FF0000)).unwrap();
        assert_eq!(result2[0].address, 0x34FF0000);
    }
}
