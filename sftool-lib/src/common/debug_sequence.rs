use crate::{Error, Result};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};

pub const DEBUG_SEQUENCE: [u8; 8] = [0x41, 0x54, 0x53, 0x46, 0x33, 0x32, 0x05, 0x21];
pub const REPLACEMENT_BYTE: u8 = 0xAA;
pub const FLASH_WRITE_PAGE_SIZE: u64 = 128 * 1024;
pub const REPLACE_MAX: usize = 100;
pub const REPLACE_HEADER1: u32 = 0xABAB_BABA;
pub const REPLACE_HEADER2: u32 = 0x3535_5353;
pub const REPLACE_INFO_SIZE: usize = 4 + 4 + REPLACE_MAX * 8 + 4;

/// Positions in one firmware file where the RAM patch must restore the first
/// byte of the Debug IP enable sequence after the bulk write completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSequenceReplacementInfo {
    offsets: Vec<u64>,
}

impl DebugSequenceReplacementInfo {
    pub fn empty() -> Self {
        Self {
            offsets: Vec::new(),
        }
    }

    pub fn scan_file(file: &File) -> Result<Self> {
        let mut reader = BufReader::new(file.try_clone()?);
        reader.seek(SeekFrom::Start(0))?;

        let mut offsets = Vec::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut tail = Vec::with_capacity(DEBUG_SEQUENCE.len() - 1);
        let mut bytes_read_total = 0u64;

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let window_start = bytes_read_total.saturating_sub(tail.len() as u64);
            let mut window = Vec::with_capacity(tail.len() + bytes_read);
            window.extend_from_slice(&tail);
            window.extend_from_slice(&buffer[..bytes_read]);

            for (index, candidate) in window.windows(DEBUG_SEQUENCE.len()).enumerate() {
                if candidate != DEBUG_SEQUENCE {
                    continue;
                }

                if offsets.len() == REPLACE_MAX {
                    return Err(Error::invalid_input(format!(
                        "debug sequence replacement count exceeds {REPLACE_MAX}"
                    )));
                }
                offsets.push(window_start + index as u64);
            }

            tail.clear();
            let tail_start = window.len().saturating_sub(DEBUG_SEQUENCE.len() - 1);
            tail.extend_from_slice(&window[tail_start..]);
            bytes_read_total += bytes_read as u64;
        }

        Ok(Self { offsets })
    }

    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    pub fn encode(&self) -> Result<[u8; REPLACE_INFO_SIZE]> {
        if self.offsets.len() > REPLACE_MAX {
            return Err(Error::invalid_input(format!(
                "debug sequence replacement count exceeds {REPLACE_MAX}"
            )));
        }

        let mut data = [0u8; REPLACE_INFO_SIZE];
        data[0..4].copy_from_slice(&REPLACE_HEADER1.to_le_bytes());
        data[4..8].copy_from_slice(&(self.offsets.len() as u32).to_le_bytes());

        for (index, offset) in self.offsets.iter().copied().enumerate() {
            let block = u32::try_from(offset / FLASH_WRITE_PAGE_SIZE).map_err(|_| {
                Error::invalid_input("debug sequence replacement block exceeds u32")
            })?;
            let position = u32::try_from(offset % FLASH_WRITE_PAGE_SIZE).map_err(|_| {
                Error::invalid_input("debug sequence replacement position exceeds u32")
            })?;
            let start = 8 + index * 8;
            data[start..start + 4].copy_from_slice(&block.to_le_bytes());
            data[start + 4..start + 8].copy_from_slice(&position.to_le_bytes());
        }

        data[REPLACE_INFO_SIZE - 4..].copy_from_slice(&REPLACE_HEADER2.to_le_bytes());
        Ok(data)
    }

    /// Replaces only the first byte of each recorded sequence in the outgoing
    /// transfer chunk. The source file remains unchanged.
    pub fn apply_to_chunk(&self, chunk_offset: u64, data: &mut [u8]) {
        let chunk_end = chunk_offset.saturating_add(data.len() as u64);
        for offset in &self.offsets {
            if *offset >= chunk_offset && *offset < chunk_end {
                data[(*offset - chunk_offset) as usize] = REPLACEMENT_BYTE;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scan(data: &[u8]) -> DebugSequenceReplacementInfo {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(data).unwrap();
        DebugSequenceReplacementInfo::scan_file(file.as_file()).unwrap()
    }

    #[test]
    fn encodes_empty_info_with_fixed_layout() {
        let info = scan(b"no debug marker here");

        let encoded = info.encode().unwrap();

        assert!(info.is_empty());
        assert_eq!(REPLACE_MAX, 100);
        assert_eq!(encoded.len(), 812);
        assert_eq!(encoded.len(), REPLACE_INFO_SIZE);
        assert_eq!(&encoded[0..4], &REPLACE_HEADER1.to_le_bytes());
        assert_eq!(&encoded[4..8], &0u32.to_le_bytes());
        assert_eq!(
            &encoded[REPLACE_INFO_SIZE - 4..],
            &REPLACE_HEADER2.to_le_bytes()
        );
    }

    #[test]
    fn detects_markers_across_scan_and_flash_write_boundaries() {
        let mut data = vec![0u8; FLASH_WRITE_PAGE_SIZE as usize + 64];
        let scan_boundary = 64 * 1024 - 3;
        let page_boundary = FLASH_WRITE_PAGE_SIZE as usize - 4;
        let second_page = FLASH_WRITE_PAGE_SIZE as usize + 10;
        data[scan_boundary..scan_boundary + DEBUG_SEQUENCE.len()].copy_from_slice(&DEBUG_SEQUENCE);
        data[page_boundary..page_boundary + DEBUG_SEQUENCE.len()].copy_from_slice(&DEBUG_SEQUENCE);
        data[second_page..second_page + DEBUG_SEQUENCE.len()].copy_from_slice(&DEBUG_SEQUENCE);

        let info = scan(&data);
        let encoded = info.encode().unwrap();

        assert_eq!(
            info.offsets,
            vec![
                scan_boundary as u64,
                page_boundary as u64,
                second_page as u64
            ]
        );
        assert_eq!(&encoded[8..12], &0u32.to_le_bytes());
        assert_eq!(&encoded[12..16], &(scan_boundary as u32).to_le_bytes());
        assert_eq!(&encoded[16..20], &0u32.to_le_bytes());
        assert_eq!(&encoded[20..24], &(page_boundary as u32).to_le_bytes());
        assert_eq!(&encoded[24..28], &1u32.to_le_bytes());
        assert_eq!(&encoded[28..32], &10u32.to_le_bytes());
    }

    #[test]
    fn only_replaces_recorded_bytes_in_the_transfer_buffer() {
        let mut data = Vec::from(&b"prefix"[..]);
        data.extend_from_slice(&DEBUG_SEQUENCE);
        data.extend_from_slice(b"suffix");
        let info = scan(&data);
        let source = data.clone();

        info.apply_to_chunk(0, &mut data);

        assert_eq!(data[6], REPLACEMENT_BYTE);
        assert_eq!(&data[7..14], &DEBUG_SEQUENCE[1..]);
        assert_eq!(source[6], DEBUG_SEQUENCE[0]);
    }

    #[test]
    fn supports_exactly_the_protocol_limit() {
        let mut data = Vec::new();
        for _ in 0..REPLACE_MAX {
            data.extend_from_slice(&DEBUG_SEQUENCE);
        }

        let info = scan(&data);
        let encoded = info.encode().unwrap();

        assert_eq!(info.len(), REPLACE_MAX);
        assert_eq!(&encoded[4..8], &(REPLACE_MAX as u32).to_le_bytes());
        assert_eq!(encoded.len(), 812);
    }

    #[test]
    fn rejects_more_than_the_protocol_limit() {
        let mut data = Vec::new();
        for _ in 0..=REPLACE_MAX {
            data.extend_from_slice(&DEBUG_SEQUENCE);
        }

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&data).unwrap();
        let error = DebugSequenceReplacementInfo::scan_file(file.as_file()).unwrap_err();

        let expected_message = format!("exceeds {REPLACE_MAX}");
        assert!(
            matches!(error, Error::InvalidInput(message) if message.contains(&expected_message))
        );
    }
}
