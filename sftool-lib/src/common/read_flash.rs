use crate::common::ram_command::{Command, RamCommand};
use crate::common::serial_io::{SerialIo, for_tool};
use crate::progress::{ProgressHandle, ProgressOperation, ProgressStatus};
use crate::utils::Utils;
use crate::{Error, Result, SifliToolTrait};
use crc::{Algorithm, Crc};
use std::fs::File;
use std::io::{Seek, Write};
use std::time::Duration;
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
    const START_TRANS_MARKER: &'static [u8] = b"start_trans\r\n";
    const READ_TIMEOUT_MS: u128 = 10_000;
    const READ_CHUNK_SIZE: usize = 16 * 1024;
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

    /// 解析读取文件信息 (filename@address:size格式)
    pub fn parse_file_info(file_spec: &str) -> Result<ReadFlashFile> {
        let Some((file_path, addr_size)) = file_spec.split_once('@') else {
            return Err(Error::invalid_input(format!(
                "Invalid format: {}. Expected: filename@address:size",
                file_spec
            )));
        };
        let Some((addr, size)) = addr_size.split_once(':') else {
            return Err(Error::invalid_input(format!(
                "Invalid format: {}. Expected: filename@address:size",
                file_spec
            )));
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
        tool.check_cancelled()?;
        let progress = tool.progress();
        let progress_bar =
            progress.create_bar(size as u64, ProgressOperation::ReadFlash { address, size });

        let mut temp_file = tempfile()?;

        // 读取一次即可，由设备负责连续发送数据
        tool.command(Command::Read { address, len: size })?;

        let (expected_crc, actual_crc) = {
            let mut io = for_tool(tool);

            Self::wait_for_marker(&mut io, Self::START_TRANS_MARKER, "start_trans marker")?;

            let actual_crc = Self::receive_payload(
                &mut io,
                size,
                &mut temp_file,
                &progress_bar,
                address,
            )?;

            let expected_crc = Self::read_crc_value(&mut io)?;
            Self::expect_ok(&mut io)?;

            (expected_crc, actual_crc)
        };

        if actual_crc != expected_crc {
            return Err(Error::CrcMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }

        progress_bar.finish(ProgressStatus::Success);

        temp_file.seek(std::io::SeekFrom::Start(0))?;
        let mut output_file = File::create(output_path)?;
        std::io::copy(&mut temp_file, &mut output_file)?;

        Ok(())
    }

    fn wait_for_marker(io: &mut SerialIo<'_>, marker: &[u8], context: &str) -> Result<()> {
        io.wait_for_pattern(
            marker,
            Duration::from_millis(Self::READ_TIMEOUT_MS as u64),
            context,
        )?;
        Ok(())
    }

    fn receive_payload(
        io: &mut SerialIo<'_>,
        size: u32,
        temp_file: &mut File,
        progress_bar: &ProgressHandle,
        address: u32,
    ) -> Result<u32> {
        let mut remaining = size as usize;
        let buffer_len = remaining.clamp(1usize, Self::READ_CHUNK_SIZE);
        let mut buffer = vec![0u8; buffer_len];

        let crc = Crc::<u32>::new(&Self::CRC_32_ALGO);
        let mut digest = crc.digest();
        let mut processed = 0usize;

        while remaining > 0 {
            io.check_cancelled()?;
            let chunk_len = std::cmp::min(buffer.len(), remaining);
            let chunk = &mut buffer[..chunk_len];
            let current_address = address.saturating_add(processed as u32);
            io.read_exact_with_timeout(
                chunk,
                Duration::from_millis(Self::READ_TIMEOUT_MS as u64),
                &format!("reading flash at 0x{:08X}", current_address),
            )?;

            temp_file.write_all(chunk)?;
            digest.update(chunk);

            remaining -= chunk_len;
            processed += chunk_len;
            progress_bar.inc(chunk_len as u64);
        }

        Ok(digest.finalize())
    }

    fn read_crc_value(io: &mut SerialIo<'_>) -> Result<u32> {
        let line = Self::read_non_empty_line(io, "CRC response")?;
        let lower = line.to_ascii_lowercase();
        let prefix = "crc:0x";

        if !lower.starts_with(prefix) {
            return Err(Error::protocol(format!("unexpected CRC line: {}", line)));
        }

        let hex_part = &line[prefix.len()..];
        u32::from_str_radix(hex_part, 16)
            .map_err(|e| Error::protocol(format!("invalid CRC '{}': {}", line, e)))
    }

    fn expect_ok(io: &mut SerialIo<'_>) -> Result<()> {
        let line = Self::read_non_empty_line(io, "OK response")?;
        if line != "OK" {
            return Err(Error::protocol(format!("unexpected response: {}", line)));
        }
        Ok(())
    }

    fn read_non_empty_line(io: &mut SerialIo<'_>, context: &str) -> Result<String> {
        loop {
            let line = Self::read_line(io, context)?;
            let trimmed = line.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            return Ok(trimmed);
        }
    }

    fn read_line(io: &mut SerialIo<'_>, context: &str) -> Result<String> {
        io.read_line_with_timeout(
            Duration::from_millis(Self::READ_TIMEOUT_MS as u64),
            context,
        )
    }
}
