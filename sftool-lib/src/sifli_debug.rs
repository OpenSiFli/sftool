use crate::SifliTool;
use probe_rs::architecture::arm::armv8m::Dcrdr;
use probe_rs::{MemoryMappedRegister, RegisterId, memory_mapped_bitfield_register};
use std::cmp::{max, min};
use std::fmt;
use std::io::{BufReader, BufWriter, Read, Write};
use std::time::{Duration, Instant};
use probe_rs::architecture::arm::ArmError;

const START_WORD: [u8; 2] = [0x7E, 0x79];
const DEFUALT_RECV_TIMEOUT: Duration = Duration::from_secs(3);
const DEFUALT_UART_BAUD: u32 = 1000000;

#[derive(Debug)]
pub(crate) enum SifliUartCommand<'a> {
    Enter,
    Exit,
    MEMRead { addr: u32, len: u16 },
    MEMWrite { addr: u32, data: &'a [u32] },
}

#[derive(Debug)]
pub(crate) enum SifliUartResponse {
    Enter,
    Exit,
    MEMRead { data: Vec<u8> },
    MEMWrite,
}

impl fmt::Display for SifliUartCommand<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SifliUartCommand::Enter => write!(f, "Enter"),
            SifliUartCommand::Exit => write!(f, "Exit"),
            SifliUartCommand::MEMRead { addr, len } => {
                write!(f, "MEMRead {{ addr: {:#X}, len: {:#X} }}", addr, len)
            }
            SifliUartCommand::MEMWrite { addr, data } => {
                write!(f, "MEMWrite {{ addr: {:#X}, data: [", addr)?;
                for (i, d) in data.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:#X}", d)?;
                }
                write!(f, "] }}")
            }
        }
    }
}

impl fmt::Display for SifliUartResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SifliUartResponse::Enter => write!(f, "Enter"),
            SifliUartResponse::Exit => write!(f, "Exit"),
            SifliUartResponse::MEMRead { data } => {
                write!(f, "MEMRead {{ data: [")?;
                for (i, byte) in data.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:#04X}", byte)?;
                }
                write!(f, "] }}")
            }
            SifliUartResponse::MEMWrite => write!(f, "MEMWrite"),
        }
    }
}

memory_mapped_bitfield_register! {
    pub struct Dcrsr(u32);
    0xE000_EDF4, "DCRSR",
    impl From;
    pub _, set_regwnr: 16;
    // If the processor does not implement the FP extension the REGSEL field is bits `[4:0]`, and bits `[6:5]` are Reserved, SBZ.
    pub _, set_regsel: 6,0;
}

memory_mapped_bitfield_register! {
    pub struct Dhcsr(u32);
    0xE000_EDF0, "DHCSR",
    impl From;
    pub s_reset_st, _: 25;
    pub s_retire_st, _: 24;
    pub s_lockup, _: 19;
    pub s_sleep, _: 18;
    pub s_halt, _: 17;
    pub s_regrdy, _: 16;
    pub c_maskints, set_c_maskints: 3;
    pub c_step, set_c_step: 2;
    pub c_halt, set_c_halt: 1;
    pub c_debugen, set_c_debugen: 0;
}

impl Dhcsr {
    /// This function sets the bit to enable writes to this register.
    ///
    /// C1.6.3 Debug Halting Control and Status Register, DHCSR:
    /// Debug key:
    /// Software must write 0xA05F to this field to enable write accesses to bits
    /// `[15:0]`, otherwise the processor ignores the write access.
    pub fn enable_write(&mut self) {
        self.0 &= !(0xffff << 16);
        self.0 |= 0xa05f << 16;
    }
}

impl SifliTool {
    fn create_header(len: u16) -> Vec<u8> {
        let mut header = vec![];
        header.extend_from_slice(&START_WORD);
        header.extend_from_slice(&len.to_le_bytes());
        header.push(0x10);
        header.push(0x00);
        header
    }

    fn send(
        writer: &mut BufWriter<Box<dyn Write + Send>>,
        command: &SifliUartCommand,
    ) -> Result<(), std::io::Error> {
        let mut send_data = vec![];
        match command {
            SifliUartCommand::Enter => {
                let temp = [0x41, 0x54, 0x53, 0x46, 0x33, 0x32, 0x05, 0x21];
                send_data.extend_from_slice(&temp);
            }
            SifliUartCommand::Exit => {
                let temp = [0x41, 0x54, 0x53, 0x46, 0x33, 0x32, 0x18, 0x21];
                send_data.extend_from_slice(&temp);
            }
            SifliUartCommand::MEMRead { addr, len } => {
                send_data.push(0x40);
                send_data.push(0x72);
                send_data.extend_from_slice(&addr.to_le_bytes());
                send_data.extend_from_slice(&len.to_le_bytes());
            }
            SifliUartCommand::MEMWrite { addr, data } => {
                send_data.push(0x40);
                send_data.push(0x77);
                send_data.extend_from_slice(&addr.to_le_bytes());
                send_data.extend_from_slice(&(data.len() as u16).to_le_bytes());
                for d in data.iter() {
                    send_data.extend_from_slice(&d.to_le_bytes());
                }
            }
        }

        let header = Self::create_header(send_data.len() as u16);
        writer.write_all(&header)?;
        writer.write_all(&send_data)?;
        writer.flush()?;

        Ok(())
    }

    fn recv(
        reader: &mut BufReader<Box<dyn Read + Send>>,
    ) -> Result<SifliUartResponse, std::io::Error> {
        let start_time = Instant::now();
        let mut buffer = vec![];
        let mut recv_data = vec![];

        loop {
            if start_time.elapsed() >= DEFUALT_RECV_TIMEOUT {
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout"));
            }

            let mut byte = [0; 1];
            if reader.read_exact(&mut byte).is_err() {
                continue;
            }

            if (byte[0] == START_WORD[0]) || (buffer.len() == 1 && byte[0] == START_WORD[1]) {
                buffer.push(byte[0]);
            } else {
                buffer.clear();
            }
            tracing::info!("Recv buffer: {:?}", buffer);

            if buffer.ends_with(&START_WORD) {
                let err = Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid frame start",
                ));
                recv_data.clear();
                // Header Length
                let mut temp = [0; 2];
                if reader.read_exact(&mut temp).is_err() {
                    return err;
                }
                let size = u16::from_le_bytes(temp);
                tracing::info!("Recv size: {}", size);

                // Header channel and crc
                if reader.read_exact(&mut temp).is_err() {
                    return err;
                }

                while recv_data.len() < size as usize {
                    if reader.read_exact(&mut byte).is_err() {
                        return err;
                    }
                    recv_data.push(byte[0]);
                    tracing::info!("Recv data: {:?}", recv_data);
                }
                break;
            } else if buffer.len() == 2 {
                buffer.clear();
            }
        }

        if recv_data[recv_data.len() - 1] != 0x06 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid frame end",
            ));
        }

        match recv_data[0] {
            0xD1 => Ok(SifliUartResponse::Enter),
            0xD0 => Ok(SifliUartResponse::Exit),
            0xD2 => {
                let data = recv_data[1..recv_data.len() - 1].to_vec();
                Ok(SifliUartResponse::MEMRead { data })
            }
            0xD3 => Ok(SifliUartResponse::MEMWrite),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid response",
            )),
        }
    }

    pub(crate) fn debug_command(
        &mut self,
        command: SifliUartCommand,
    ) -> Result<SifliUartResponse, std::io::Error> {
        tracing::info!("Command: {}", command);
        let writer: Box<dyn Write + Send> = self.port.try_clone()?;
        let mut buf_writer = BufWriter::new(writer);

        let reader: Box<dyn Read + Send> = self.port.try_clone()?;
        let mut buf_reader = BufReader::new(reader);

        let ret = Self::send(&mut buf_writer, &command);
        if let Err(e) = ret {
            tracing::error!("Command send error: {:?}", e);
            return Err(e);
        }

        match command {
            SifliUartCommand::Exit => Ok(SifliUartResponse::Exit),
            _ => Self::recv(&mut buf_reader),
        }
    }

    pub(crate) fn debug_read_word32(&mut self, addr: u32) -> Result<u32, std::io::Error> {
        let command = SifliUartCommand::MEMRead { addr, len: 1 };
        match self.debug_command(command) {
            Ok(SifliUartResponse::MEMRead { data }) => {
                if data.len() != 4 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid response length",
                    ));
                }
                let value = u32::from_le_bytes(data.try_into().unwrap());
                Ok(value)
            }
            Ok(_) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid response",
            )),
            Err(e) => Err(e),
        }
    }

    pub(crate) fn debug_write_word32(&mut self, addr: u32, data: u32) -> Result<(), std::io::Error> {
        let command = SifliUartCommand::MEMWrite {
            addr,
            data: &[data],
        };
        match self.debug_command(command) {
            Ok(SifliUartResponse::MEMWrite) => Ok(()),
            Ok(_) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid response",
            )),
            Err(e) => Err(e),
        }
    }

    pub(crate) fn debug_write_memory(&mut self, address: u32, data: &[u8]) -> Result<(), std::io::Error> {
        if data.is_empty() {
            return Ok(());
        }

        let address = if (address & 0xff000000) == 0x12000000 {
            (address & 0x00ffffff) | 0x62000000
        } else {
            address
        };

        let addr_usize = address as usize;
        // Calculate the start address and end address after alignment
        let start_aligned = addr_usize - (addr_usize % 4);
        let end_aligned = (addr_usize + data.len()).div_ceil(4) * 4;
        let total_bytes = end_aligned - start_aligned;
        let total_words = total_bytes / 4;

        let mut buffer = vec![0u8; total_bytes];

        for i in 0..total_words {
            let block_addr = start_aligned + i * 4;
            let block_end = block_addr + 4;

            // Determine if the current 4-byte block is ‘completely overwritten’ by the new data written to it
            // If the block is completely in the new data area, then copy the new data directly
            if block_addr >= addr_usize && block_end <= addr_usize + data.len() {
                let offset_in_data = block_addr - addr_usize;
                buffer[i * 4..i * 4 + 4].copy_from_slice(&data[offset_in_data..offset_in_data + 4]);
            } else {
                // For the rest of the cases (header or tail incomplete overwrite):
                // Call MEMRead first to read out the original 4-byte block.
                let resp = self.debug_command(SifliUartCommand::MEMRead {
                    addr: block_addr as u32,
                    len: 1,
                })?;
                let mut block: [u8; 4] = match resp {
                    SifliUartResponse::MEMRead { data: d } if d.len() == 4 => {
                        [d[0], d[1], d[2], d[3]]
                    }
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Invalid response length",
                        ));
                    }
                };
                // Calculate the overlap of the block with the new data area
                let overlap_start = max(block_addr, addr_usize);
                let overlap_end = min(block_end, addr_usize + data.len());
                if overlap_start < overlap_end {
                    let in_block_offset = overlap_start - block_addr;
                    let in_data_offset = overlap_start - addr_usize;
                    let overlap_len = overlap_end - overlap_start;
                    block[in_block_offset..in_block_offset + overlap_len]
                        .copy_from_slice(&data[in_data_offset..in_data_offset + overlap_len]);
                }
                buffer[i * 4..i * 4 + 4].copy_from_slice(&block);
            }
        }

        let words: Vec<u32> = buffer
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("chunk length is 4")))
            .collect();

        // Write the entire alignment area at once
        self.debug_command(SifliUartCommand::MEMWrite {
            addr: start_aligned as u32,
            data: &words,
        })?;

        Ok(())
    }
    
    fn wait_for_core_register_transfer(
        &mut self,
        timeout: Duration
    ) -> Result<(), std::io::Error> {
        // now we have to poll the dhcsr register, until the dhcsr.s_regrdy bit is set
        // (see C1-292, cortex m0 arm)
        let start = Instant::now();

        while start.elapsed() < timeout {
            let dhcsr_val = Dhcsr(self.debug_read_word32(Dhcsr::get_mmio_address() as u32)?);

            if dhcsr_val.s_regrdy() {
                return Ok(());
            }
        }
        Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout"))
    }

    pub(crate) fn debug_write_core_reg(
        &mut self,
        addr: RegisterId,
        value: u32,
    ) -> Result<(), std::io::Error> {
        self.debug_write_word32(Dcrdr::get_mmio_address() as u32, value)?;

        let mut dcrsr_val = Dcrsr(0);
        dcrsr_val.set_regwnr(true); // Perform a write.
        dcrsr_val.set_regsel(addr.into()); // The address of the register to write.

        self.debug_write_word32(Dcrsr::get_mmio_address() as u32, dcrsr_val.into())?;

        // self.wait_for_core_register_transfer(Duration::from_millis(100))?;
        std::thread::sleep(Duration::from_millis(10));
        Ok(())
    }

    fn debug_read_core_reg(&mut self, addr: RegisterId) -> Result<u32, std::io::Error> {
        // Write the DCRSR value to select the register we want to read.
        let mut dcrsr_val = Dcrsr(0);
        dcrsr_val.set_regwnr(false); // Perform a read.
        dcrsr_val.set_regsel(addr.into()); // The address of the register to read.

        self.debug_write_word32(Dcrsr::get_mmio_address() as u32, dcrsr_val.into())?;

        self.wait_for_core_register_transfer(Duration::from_millis(100))?;

        let value = self.debug_read_word32(Dcrdr::get_mmio_address() as u32)?;

        Ok(value)
    }
    
    fn debug_step(&mut self)-> Result<(), std::io::Error> {
        // 这里我们忽略了很多必要的检查，请参考probe-rs源码
        let mut value = Dhcsr(0);
        // Leave halted state.
        // Step one instruction.
        value.set_c_step(true);
        value.set_c_halt(false);
        value.set_c_debugen(true);
        value.set_c_maskints(true);
        value.enable_write();

        self.debug_write_word32(Dhcsr::get_mmio_address() as u32, value.into())?;

        std::thread::sleep(Duration::from_millis(10));
        Ok(())
    }
    
    pub(crate) fn debug_run(&mut self) -> Result<(), std::io::Error> {
        self.debug_step()?;
        let mut value = Dhcsr(0);
        value.set_c_halt(false);
        value.set_c_debugen(true);
        value.enable_write();

        self.debug_write_word32(Dhcsr::get_mmio_address() as u32, value.into())?;
        
        std::thread::sleep(Duration::from_millis(10));
        Ok(())
    }
    
    pub fn debug_halt(&mut self) -> Result<(), std::io::Error> {
        let mut value = Dhcsr(0);
        value.set_c_halt(true);
        value.set_c_debugen(true);
        value.enable_write();

        self.debug_write_word32(Dhcsr::get_mmio_address() as u32, value.into())?;
        std::thread::sleep(Duration::from_millis(10));
        Ok(())
    }
}
