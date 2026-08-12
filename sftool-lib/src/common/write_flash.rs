use crate::common::ram_command::{Command, RamCommand, Response};
use crate::common::serial_io::for_tool;
use crate::progress::{ProgressOperation, ProgressStatus};
use crate::{Error, Result, SifliToolTrait, WriteFlashFile};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};

const HAZARDOUS_MARKERS: [[u8; 2]; 3] = [[0x7e, 0x79], [0x40, 0x72], [0x40, 0x77]];
const HAZARDOUS_BYTE_PLACEHOLDER: u8 = 0xff;

/// 通用的Flash写入操作实现
pub struct FlashWriter;

impl FlashWriter {
    /// Check whether raw file data contains an SF32LB52 Debug IP marker.
    ///
    /// The two start bytes may straddle the internal read-buffer boundary, so
    /// retain the last byte of each read. Restore the shared file position
    /// before returning because `WriteFlashFile` stores an already-open file.
    pub fn contains_hazardous_marker(file: &File) -> Result<bool> {
        let mut file = file;
        file.seek(SeekFrom::Start(0))?;

        let result = (|| {
            let mut buffer = [0u8; 16 * 1024];
            let mut previous = None;

            loop {
                let bytes_read = file.read(&mut buffer)?;
                if bytes_read == 0 {
                    return Ok(false);
                }

                for &byte in &buffer[..bytes_read] {
                    if HAZARDOUS_MARKERS
                        .iter()
                        .any(|marker| previous == Some(marker[0]) && byte == marker[1])
                    {
                        return Ok(true);
                    }
                    previous = Some(byte);
                }
            }
        })();

        file.seek(SeekFrom::Start(0))?;
        result
    }

    /// 擦除所有Flash区域
    pub fn erase_all<T>(tool: &mut T, write_flash_files: &[WriteFlashFile]) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        tool.check_cancelled()?;
        let progress = tool.progress();
        let spinner = progress.create_spinner(ProgressOperation::EraseAllRegions);

        let mut erase_address: Vec<u32> = Vec::new();
        for f in write_flash_files.iter() {
            tool.check_cancelled()?;
            let address = f.address & 0xFF00_0000;
            // 如果ERASE_ADDRESS中的地址已经被擦除过，则跳过
            if erase_address.contains(&address) {
                continue;
            }
            tool.command(Command::EraseAll { address: f.address })?;
            erase_address.push(address);
        }

        spinner.finish(ProgressStatus::Success);
        Ok(())
    }

    /// 验证数据
    pub fn verify<T>(tool: &mut T, address: u32, len: u32, crc: u32) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        tool.check_cancelled()?;
        let progress = tool.progress();
        let spinner = progress.create_spinner(ProgressOperation::Verify { address, len });

        let response = tool.command(Command::Verify { address, len, crc })?;
        if response != Response::Ok {
            return Err(Error::protocol(format!(
                "verify failed for 0x{:08X}..0x{:08X}",
                address,
                address + len.saturating_sub(1)
            )));
        }

        spinner.finish(ProgressStatus::Success);
        Ok(())
    }

    /// 写入单个文件到Flash（非全擦除模式）
    pub fn write_file_incremental<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        verify: bool,
    ) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        Self::write_file_incremental_impl(tool, file, verify, false)
    }

    /// Write a file without putting SF32LB52 Debug IP markers on USART1.
    ///
    /// The raw transfer leaves the second byte of each hazardous marker erased,
    /// then programs those bytes individually through the stub's ASCII-hex
    /// command. This keeps the stub's normal erase/write path while ensuring the
    /// wire never contains binary frame, MEMRead, or MEMWrite markers.
    pub fn write_file_incremental_collision_safe<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        verify: bool,
    ) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        warn_collision_safe_transport(file.address);
        Self::write_file_incremental_impl(tool, file, verify, true)
    }

    fn write_file_incremental_impl<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        verify: bool,
        collision_safe: bool,
    ) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        tool.check_cancelled()?;
        let progress = tool.progress();
        let re_download_spinner = progress.create_spinner(ProgressOperation::CheckRedownload {
            address: file.address,
            size: file.file.metadata()?.len(),
        });

        let response = tool.command(Command::Verify {
            address: file.address,
            len: file.file.metadata()?.len() as u32,
            crc: file.crc32,
        })?;

        if response == Response::Ok {
            re_download_spinner.finish(ProgressStatus::Skipped);
            return Ok(());
        }

        re_download_spinner.finish(ProgressStatus::Required);

        let download_bar = progress.create_bar(
            file.file.metadata()?.len(),
            ProgressOperation::WriteFlash {
                address: file.address,
                size: file.file.metadata()?.len(),
            },
        );

        let res = tool.command(Command::WriteAndErase {
            address: file.address,
            len: file.file.metadata()?.len() as u32,
        })?;
        if res != Response::RxWait {
            return Err(Error::protocol(format!(
                "write flash failed to start at 0x{:08X}",
                file.address
            )));
        }

        let mut buffer = vec![0u8; 128 * 1024];
        let mut reader = BufReader::new(&file.file);
        reader.seek(SeekFrom::Start(0))?;
        let mut previous_byte = None;
        let mut stream_offset = 0u32;
        let mut patches = Vec::new();

        loop {
            tool.check_cancelled()?;
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            if collision_safe {
                escape_hazardous_markers(
                    &mut buffer[..bytes_read],
                    file.address.checked_add(stream_offset).ok_or_else(|| {
                        Error::invalid_input("flash address overflow during collision-safe write")
                    })?,
                    &mut previous_byte,
                    &mut patches,
                )?;
            }
            tool.check_cancelled()?;
            let res = tool.send_data(&buffer[..bytes_read])?;
            if res == Response::RxWait {
                download_bar.inc(bytes_read as u64);
                continue;
            } else if res != Response::Ok {
                return Err(Error::protocol(format!(
                    "write flash failed during transfer at 0x{:08X}",
                    file.address
                )));
            }
            stream_offset = stream_offset
                .checked_add(bytes_read as u32)
                .ok_or_else(|| {
                    Error::invalid_input("flash length overflow during collision-safe write")
                })?;
        }

        patch_hazardous_bytes(tool, &patches)?;
        download_bar.finish(ProgressStatus::Success);

        // verify
        if verify {
            tool.check_cancelled()?;
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
    ) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        Self::write_file_full_erase_impl(tool, file, verify, packet_size, false)
    }

    pub fn write_file_full_erase_collision_safe<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        verify: bool,
        packet_size: usize,
    ) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        warn_collision_safe_transport(file.address);
        Self::write_file_full_erase_impl(tool, file, verify, packet_size, true)
    }

    fn write_file_full_erase_impl<T>(
        tool: &mut T,
        file: &WriteFlashFile,
        verify: bool,
        packet_size: usize,
        collision_safe: bool,
    ) -> Result<()>
    where
        T: SifliToolTrait + RamCommand,
    {
        tool.check_cancelled()?;
        let progress = tool.progress();
        let download_bar = progress.create_bar(
            file.file.metadata()?.len(),
            ProgressOperation::WriteFlash {
                address: file.address,
                size: file.file.metadata()?.len(),
            },
        );

        let mut buffer = vec![0u8; packet_size];
        let mut reader = BufReader::new(&file.file);
        reader.seek(SeekFrom::Start(0))?;

        let mut address = file.address;
        let mut previous_byte = None;
        let mut patches = Vec::new();
        loop {
            tool.check_cancelled()?;
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            if collision_safe {
                escape_hazardous_markers(
                    &mut buffer[..bytes_read],
                    address,
                    &mut previous_byte,
                    &mut patches,
                )?;
            }
            tool.check_cancelled()?;
            {
                let mut io = for_tool(tool);
                io.write_all(
                    Command::Write {
                        address,
                        len: bytes_read as u32,
                    }
                    .to_string()
                    .as_bytes(),
                )?;
                io.flush()?;
            }
            let res = tool.send_data(&buffer[..bytes_read])?;
            if res != Response::Ok {
                return Err(Error::protocol(format!(
                    "write flash failed during transfer at 0x{:08X}",
                    address
                )));
            }
            address += bytes_read as u32;
            download_bar.inc(bytes_read as u64);
        }

        patch_hazardous_bytes(tool, &patches)?;
        download_bar.finish(ProgressStatus::Success);

        // verify
        if verify {
            tool.check_cancelled()?;
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

fn warn_collision_safe_transport(address: u32) {
    tracing::warn!(
        address = format_args!("0x{address:08x}"),
        "firmware contains an SF32LB52 Debug IP marker; escaping hazardous bytes during flash write"
    );
}

fn escape_hazardous_markers(
    data: &mut [u8],
    address: u32,
    previous_byte: &mut Option<u8>,
    patches: &mut Vec<(u32, u8)>,
) -> Result<()> {
    for (offset, byte) in data.iter_mut().enumerate() {
        let original = *byte;
        if HAZARDOUS_MARKERS
            .iter()
            .any(|marker| *previous_byte == Some(marker[0]) && original == marker[1])
        {
            *byte = HAZARDOUS_BYTE_PLACEHOLDER;
            patches.push((
                address.checked_add(offset as u32).ok_or_else(|| {
                    Error::invalid_input("flash address overflow during collision-safe write")
                })?,
                original,
            ));
        }
        *previous_byte = Some(original);
    }
    Ok(())
}

fn patch_hazardous_bytes<T>(tool: &mut T, patches: &[(u32, u8)]) -> Result<()>
where
    T: SifliToolTrait + RamCommand,
{
    for &(address, byte) in patches {
        tool.check_cancelled()?;
        let response = tool.command(Command::WriteMultipleBytes {
            address,
            len: 1,
            data: format!("{byte:02x}"),
        })?;
        if response != Response::Ok {
            return Err(Error::protocol(format!(
                "collision-safe flash patch failed at 0x{address:08X}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{FlashWriter, escape_hazardous_markers};
    use crate::common::ram_command::{Command, RamCommand, Response};
    use crate::common::serial_io::test_support::TestSerialPort;
    use crate::progress::no_op_progress_sink;
    use crate::{BeforeOperation, Result, SifliToolBase, SifliToolTrait, WriteFlashFile};
    use serialport::SerialPort;
    use std::fs::File;
    use std::io::{Seek, SeekFrom, Write};
    use tempfile::tempfile;

    struct TestTool {
        base: SifliToolBase,
        port: Box<dyn SerialPort>,
        commands: Vec<Command>,
        sent_data: Vec<Vec<u8>>,
    }

    unsafe impl Send for TestTool {}
    unsafe impl Sync for TestTool {}

    impl SifliToolTrait for TestTool {
        fn port(&mut self) -> &mut Box<dyn SerialPort> {
            &mut self.port
        }

        fn base(&self) -> &SifliToolBase {
            &self.base
        }

        fn set_speed(&mut self, _baud: u32) -> Result<()> {
            Ok(())
        }

        fn soft_reset(&mut self) -> Result<()> {
            Ok(())
        }
    }

    impl RamCommand for TestTool {
        fn command(&mut self, command: Command) -> Result<Response> {
            let response = match command {
                Command::Verify { .. } => Response::Fail,
                Command::WriteAndErase { .. } => Response::RxWait,
                _ => Response::Ok,
            };
            self.commands.push(command);
            Ok(response)
        }

        fn send_data(&mut self, data: &[u8]) -> Result<Response> {
            self.sent_data.push(data.to_vec());
            Ok(Response::Ok)
        }
    }

    fn file_with_data(data: &[u8]) -> File {
        let mut file = tempfile().unwrap();
        file.write_all(data).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file
    }

    fn test_tool() -> TestTool {
        let (port, _) = TestSerialPort::from_bytes(&[]);
        TestTool {
            base: SifliToolBase::new_with_external_stub(
                "test-port".to_owned(),
                BeforeOperation::NoReset,
                "nor".to_owned(),
                1_000_000,
                1,
                false,
                no_op_progress_sink(),
                None,
            ),
            port: Box::new(port),
            commands: Vec::new(),
            sent_data: Vec::new(),
        }
    }

    #[test]
    fn detects_all_hazardous_markers() {
        for marker in [[0x7e, 0x79], [0x40, 0x72], [0x40, 0x77]] {
            let file = file_with_data(&[0x00, marker[0], marker[1], 0xff]);
            assert!(FlashWriter::contains_hazardous_marker(&file).unwrap());
        }
    }

    #[test]
    fn detects_hazardous_marker_across_read_boundary() {
        let mut data = vec![0; 16 * 1024 - 1];
        data.extend_from_slice(&[0x7e, 0x79]);
        let file = file_with_data(&data);
        assert!(FlashWriter::contains_hazardous_marker(&file).unwrap());
    }

    #[test]
    fn ignores_non_adjacent_marker_bytes() {
        let file = file_with_data(&[0x7e, 0x00, 0x79]);
        assert!(!FlashWriter::contains_hazardous_marker(&file).unwrap());
    }

    #[test]
    fn scan_restores_file_position() {
        let mut file = file_with_data(&[0x00, 0x7e, 0x79]);
        file.seek(SeekFrom::End(0)).unwrap();

        assert!(FlashWriter::contains_hazardous_marker(&file).unwrap());
        assert_eq!(file.stream_position().unwrap(), 0);
    }

    #[test]
    fn escapes_markers_across_chunks_and_records_patch_addresses() {
        let mut first = vec![0x00, 0x7e];
        let mut second = vec![0x79, 0x40, 0x72, 0x40, 0x77, 0x7e, 0x79];
        let mut previous = None;
        let mut addresses = Vec::new();

        escape_hazardous_markers(&mut first, 0x1202_0000, &mut previous, &mut addresses).unwrap();
        escape_hazardous_markers(&mut second, 0x1202_0002, &mut previous, &mut addresses).unwrap();

        assert_eq!(first, [0x00, 0x7e]);
        assert_eq!(second, [0xff, 0x40, 0xff, 0x40, 0xff, 0x7e, 0xff]);
        assert_eq!(
            addresses,
            [
                (0x1202_0002, 0x79),
                (0x1202_0004, 0x72),
                (0x1202_0006, 0x77),
                (0x1202_0008, 0x79)
            ]
        );
    }

    #[test]
    fn collision_safe_writer_bulk_writes_placeholders_then_patches_bytes() {
        let data = [0x7e, 0x79, 0x40, 0x72, 0x40, 0x77];
        let file = WriteFlashFile {
            address: 0x1202_0000,
            file: file_with_data(&data),
            crc32: 0x1234_5678,
        };
        let mut tool = test_tool();

        FlashWriter::write_file_incremental_collision_safe(&mut tool, &file, false).unwrap();

        assert_eq!(tool.sent_data, [vec![0x7e, 0xff, 0x40, 0xff, 0x40, 0xff]]);
        assert_eq!(tool.commands.len(), 5);
        assert!(matches!(
            tool.commands[0],
            Command::Verify {
                address: 0x1202_0000,
                len: 6,
                crc: 0x1234_5678
            }
        ));
        assert!(matches!(
            tool.commands[1],
            Command::WriteAndErase {
                address: 0x1202_0000,
                len: 6
            }
        ));
        assert!(matches!(
            &tool.commands[2],
            Command::WriteMultipleBytes {
                address: 0x1202_0001,
                len: 1,
                data
            } if data == "79"
        ));
        assert!(matches!(
            &tool.commands[3],
            Command::WriteMultipleBytes {
                address: 0x1202_0003,
                len: 1,
                data
            } if data == "72"
        ));
        assert!(matches!(
            &tool.commands[4],
            Command::WriteMultipleBytes {
                address: 0x1202_0005,
                len: 1,
                data
            } if data == "77"
        ));
    }
}
