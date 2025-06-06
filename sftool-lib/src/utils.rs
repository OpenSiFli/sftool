use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::num::ParseIntError;
use std::path::Path;
use crc::Algorithm;
use tempfile::tempfile;

// Re-export WriteFlashFile from write_flash module
pub use crate::write_flash::WriteFlashFile;

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

    /// Finalize a segment by calculating CRC32 and adding it to the write flash files
    fn finalize_segment(
        temp_file: File,
        segment_start: u32,
        write_flash_files: &mut Vec<WriteFlashFile>,
    ) -> Result<(), std::io::Error> {
        let mut file = temp_file;
        file.seek(SeekFrom::Start(0))?;
        let crc32 = Self::get_file_crc32(&file.try_clone()?)?;
        write_flash_files.push(WriteFlashFile {
            address: segment_start,
            file,
            crc32,
        });
        Ok(())
    }

    /// Convert Intel HEX file to binary files
    /// 
    /// This function parses an Intel HEX file and converts it into one or more binary files.
    /// It handles multiple segments, automatic gap filling with 0xFF, and generates separate
    /// WriteFlashFile entries for each memory segment.
    pub fn hex_to_bin(hex_file: &Path) -> Result<Vec<WriteFlashFile>, std::io::Error> {
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
                            Self::finalize_segment(temp_file, current_segment_start, &mut write_flash_files)?;
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
                        Self::finalize_segment(temp_file, current_segment_start, &mut write_flash_files)?;
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
        
        let result = Utils::hex_to_bin(temp_hex.path()).unwrap();
        
        // Should have one segment
        assert_eq!(result.len(), 1);
        
        let segment = &result[0];
        assert_eq!(segment.address, 0x00000000);
        
        // Check file size (gap filled from 0x0000 to 0x1003)
        assert_eq!(segment.file.metadata().unwrap().len(), 0x1004);
        
        // Verify gap filling
        let mut file = segment.file.try_clone().unwrap();
        file.seek(SeekFrom::Start(4)).unwrap(); // Skip first 4 bytes
        let mut gap_data = vec![0; 0x1000 - 4]; // Gap between 0x04 and 0x1000
        file.read_exact(&mut gap_data).unwrap();
        assert!(gap_data.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn test_hex_to_bin_multiple_segments() {
        // Create a hex file with multiple segments using correct checksums
        let hex_content = ":0400000001020304F2\n:020000040001F9\n:0400000011121314B2\n:00000001FF\n";
        
        let mut temp_hex = NamedTempFile::new().unwrap();
        temp_hex.write_all(hex_content.as_bytes()).unwrap();
        
        let result = Utils::hex_to_bin(temp_hex.path()).unwrap();
        
        // Should have two segments
        assert_eq!(result.len(), 2);
        
        // First segment at 0x00000000
        assert_eq!(result[0].address, 0x00000000);
        assert_eq!(result[0].file.metadata().unwrap().len(), 4);
        
        // Second segment at 0x00010000
        assert_eq!(result[1].address, 0x00010000);
        assert_eq!(result[1].file.metadata().unwrap().len(), 4);
    }

    #[test]
    fn test_hex_to_bin_with_gaps() {
        // Create a hex file with gaps that should be filled with 0xFF
        let hex_content = ":04000000AABBCCDDEE\n:04100000EEFF0011EE\n:00000001FF\n";
        
        let mut temp_hex = NamedTempFile::new().unwrap();
        temp_hex.write_all(hex_content.as_bytes()).unwrap();
        
        let result = Utils::hex_to_bin(temp_hex.path()).unwrap();
        
        // Debug: print actual results
        println!("Number of segments: {}", result.len());
        for (i, segment) in result.iter().enumerate() {
            println!("Segment {}: address=0x{:08X}, size={}", 
                i, segment.address, segment.file.metadata().unwrap().len());
        }
        
        // Should have one segment
        assert_eq!(result.len(), 1);
        
        let segment = &result[0];
        assert_eq!(segment.address, 0x00000000);
        
        // Should have 4 bytes data + 4092 bytes gap + 4 bytes data = 4100 bytes
        println!("Expected size: 0x1004 ({}), Actual size: {}", 0x1004, segment.file.metadata().unwrap().len());
        assert_eq!(segment.file.metadata().unwrap().len(), 0x1004);
        
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
        
        let result = Utils::hex_to_bin(temp_hex.path()).unwrap();
        
        // Should have three segments
        assert_eq!(result.len(), 3);
        
        // First segment at 0x00000000 (contains data at 0x0000 and 0x1000 with gap)
        assert_eq!(result[0].address, 0x00000000);
        assert_eq!(result[0].file.metadata().unwrap().len(), 0x1008); // 0x1000 + 8 bytes
        
        // Second segment at 0x00010000 (contains data at 0x0000 and 0x1000 with gap)
        assert_eq!(result[1].address, 0x00010000);
        assert_eq!(result[1].file.metadata().unwrap().len(), 0x1004); // 0x1000 + 4 bytes
        
        // Third segment at 0x00100000
        assert_eq!(result[2].address, 0x00100000);
        assert_eq!(result[2].file.metadata().unwrap().len(), 8);
        
        // Verify gap filling in first segment
        let mut file = result[0].file.try_clone().unwrap();
        file.seek(SeekFrom::Start(16)).unwrap(); // Skip first 16 bytes of data
        let mut gap_data = vec![0; 0x1000 - 16]; // Gap between 0x10 and 0x1000
        file.read_exact(&mut gap_data).unwrap();
        assert!(gap_data.iter().all(|&b| b == 0xFF));
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
}
