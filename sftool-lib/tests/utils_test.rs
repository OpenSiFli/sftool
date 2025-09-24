use sftool_lib::utils::Utils;
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
    let hex_content = ":0400000001020304F2\n:020000040001F9\n:0400000011121314B2\n:00000001FF\n";

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
    let result =
        Utils::hex_with_base_to_write_flash_files(temp_hex.path(), Some(0x10000000)).unwrap();

    // Should have one segment
    assert_eq!(result.len(), 1);

    let segment = &result[0];
    // Original ExtendedLinearAddress was 0x0801 (base address 0x08010000)
    // With override 0x10000000, should become 0x1001 (base address 0x10010000)
    // (0x0801 & 0x00FF) | ((0x10000000 >> 16) & 0xFF00) = 0x01 | 0x1000 = 0x1001
    assert_eq!(segment.address, 0x10010000);

    // Test without base address override (should work like original function)
    let result_no_override =
        Utils::hex_with_base_to_write_flash_files(temp_hex.path(), None).unwrap();
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
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("ELF files do not support")
    );

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

    let result1 =
        Utils::hex_with_base_to_write_flash_files(temp_hex1.path(), Some(0x12000000)).unwrap();
    assert_eq!(result1[0].address, 0x12000000);

    // Case 2: ExtendedLinearAddress(0x00FF) with override 0x34000000
    // (0x00FF & 0x00FF) | ((0x34000000 >> 16) & 0xFF00) = 0xFF | 0x3400 = 0x34FF
    let hex_content2 = ":0200000400FFFB\n:0400000001020304F2\n:00000001FF\n";
    let mut temp_hex2 = NamedTempFile::new().unwrap();
    temp_hex2.write_all(hex_content2.as_bytes()).unwrap();

    let result2 =
        Utils::hex_with_base_to_write_flash_files(temp_hex2.path(), Some(0x34FF0000)).unwrap();
    assert_eq!(result2[0].address, 0x34FF0000);
}

#[test]
fn test_hex_continuous_segments_merging() {
    // Test that continuous segments are merged into one file
    // Single ExtendedLinearAddress(0x0800) with continuous data
    let hex_content = ":020000040800F2\n:0400000001020304F2\n:0400040005060708DE\n:00000001FF\n";

    let mut temp_hex = NamedTempFile::new().unwrap();
    temp_hex.write_all(hex_content.as_bytes()).unwrap();

    let result = Utils::hex_to_write_flash_files(temp_hex.path()).unwrap();

    // Should have only one segment (merged)
    assert_eq!(result.len(), 1);

    let segment = &result[0];
    assert_eq!(segment.address, 0x08000000);

    // Should have 8 bytes total (4 + 4)
    let file_size = segment.file.metadata().unwrap().len() as usize;
    assert_eq!(file_size, 8);

    // Read file content to verify continuous data
    let mut file_data = Vec::new();
    let mut file = &segment.file;
    file.read_to_end(&mut file_data).unwrap();
    assert_eq!(
        &file_data,
        &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
    );
}

#[test]
fn test_hex_different_base_continuous_segments_merging() {
    // Test that segments from different ExtendedLinearAddress but continuous addresses are merged
    // First segment: ExtendedLinearAddress(0x0800) with 4 bytes at offset 0x0000 -> address 0x08000000-0x08000003
    // Second segment: ExtendedLinearAddress(0x0800) with 4 bytes at offset 0x0004 -> address 0x08000004-0x08000007
    let hex_content =
        ":020000040800F2\n:0400000001020304F2\n:020000040800F2\n:0400040005060708DE\n:00000001FF\n";

    let mut temp_hex = NamedTempFile::new().unwrap();
    temp_hex.write_all(hex_content.as_bytes()).unwrap();

    let result = Utils::hex_to_write_flash_files(temp_hex.path()).unwrap();

    // Should have only one segment (merged across same ExtendedLinearAddress)
    assert_eq!(result.len(), 1);

    let segment = &result[0];
    assert_eq!(segment.address, 0x08000000);

    // Should have 8 bytes total
    let file_size = segment.file.metadata().unwrap().len() as usize;
    assert_eq!(file_size, 8);

    // Read file content to verify continuous data
    let mut file_data = Vec::new();
    let mut file = &segment.file;
    file.read_to_end(&mut file_data).unwrap();
    assert_eq!(
        &file_data,
        &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
    );
}

#[test]
fn test_hex_non_continuous_segments_not_merged() {
    // Test that non-continuous segments are NOT merged
    // Using existing working hex content from another test
    let hex_content = ":0400000001020304F2\n:020000040001F9\n:0400000011121314B2\n:00000001FF\n";

    let mut temp_hex = NamedTempFile::new().unwrap();
    temp_hex.write_all(hex_content.as_bytes()).unwrap();

    let result = Utils::hex_to_write_flash_files(temp_hex.path()).unwrap();

    // Should have two separate segments (not merged due to gap)
    assert_eq!(result.len(), 2);

    // First segment
    assert_eq!(result[0].address, 0x00000000);
    let file_size_0 = result[0].file.metadata().unwrap().len() as usize;
    assert_eq!(file_size_0, 4);

    // Second segment
    assert_eq!(result[1].address, 0x00010000);
    let file_size_1 = result[1].file.metadata().unwrap().len() as usize;
    assert_eq!(file_size_1, 4);
}

#[test]
fn test_hex_with_base_continuous_segments_merging() {
    // Test continuous segment merging with base address override
    // Similar to test_hex_continuous_segments_merging but with base override
    let hex_content = ":020000040801F1\n:0400000001020304F2\n:0400040005060708DE\n:00000001FF\n";

    let mut temp_hex = NamedTempFile::new().unwrap();
    temp_hex.write_all(hex_content.as_bytes()).unwrap();

    // With base address override 0x10000000
    let result =
        Utils::hex_with_base_to_write_flash_files(temp_hex.path(), Some(0x10000000)).unwrap();

    // Should have only one segment (merged)
    assert_eq!(result.len(), 1);

    let segment = &result[0];
    // Original would be 0x08010000, with override becomes 0x10010000
    assert_eq!(segment.address, 0x10010000);

    // Should have 8 bytes total
    let file_size = segment.file.metadata().unwrap().len() as usize;
    assert_eq!(file_size, 8);

    // Read file content to verify continuous data
    let mut file_data = Vec::new();
    let mut file = &segment.file;
    file.read_to_end(&mut file_data).unwrap();
    assert_eq!(
        &file_data,
        &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
    );
}

#[test]
fn test_hex_continuous_with_gaps_still_merged() {
    // Test that segments are merged even when there are internal gaps (filled with 0xFF)
    // ExtendedLinearAddress(0x0800) with data at 0x0000, 0x0008, and 0x000C (small gaps)
    let hex_content = ":020000040800F2\n:0400000001020304F2\n:0400080005060708DA\n:04000C000A0B0C0DC2\n:00000001FF\n";

    let mut temp_hex = NamedTempFile::new().unwrap();
    temp_hex.write_all(hex_content.as_bytes()).unwrap();

    let result = Utils::hex_to_write_flash_files(temp_hex.path()).unwrap();

    // Should have only one segment (all continuous with small gaps)
    assert_eq!(result.len(), 1);

    let segment = &result[0];
    assert_eq!(segment.address, 0x08000000);

    // Should have data from 0x0000 to 0x000F (16 bytes total)
    let file_size = segment.file.metadata().unwrap().len() as usize;
    assert_eq!(file_size, 16);

    // Read file content to verify data and gap filling
    let mut file_data = Vec::new();
    let mut file = &segment.file;
    file.read_to_end(&mut file_data).unwrap();

    // First 4 bytes
    assert_eq!(&file_data[0..4], &[0x01, 0x02, 0x03, 0x04]);
    // Gap from 0x04 to 0x07 should be filled with 0xFF
    assert!(file_data[4..8].iter().all(|&b| b == 0xFF));
    // Data at 0x08-0x0B
    assert_eq!(&file_data[8..12], &[0x05, 0x06, 0x07, 0x08]);
    // Data at 0x0C-0x0F
    assert_eq!(&file_data[12..16], &[0x0A, 0x0B, 0x0C, 0x0D]);
}
