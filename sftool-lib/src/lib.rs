mod ram_stub;
pub mod write_flash;

use std::io::Read;
use probe_rs::architecture::arm::core::registers::cortex_m::{PC, SP};
use probe_rs::architecture::arm::sequences::ArmDebugSequence;
use probe_rs::probe::list::Lister;
use probe_rs::{MemoryInterface, Permissions, RegisterId, RegisterRole};
use ram_stub::CHIP_FILE_NAME;
use serialport;
use serialport::SerialPort;
use std::time::Duration;

pub struct SifliTool {
    port: Box<dyn SerialPort>,
    chip: String,
    memory_type: String,
}

impl SifliTool {
    pub fn new(port_name: &str, chip: &str, memory_type: &str) -> Self {
        Self::download_stub(port_name, chip, memory_type).unwrap();
        // 打印现在时间，精确到ms
        let now = std::time::SystemTime::now();
        let now = now.duration_since(std::time::UNIX_EPOCH).unwrap();
        println!("now: {:?}", now.as_millis());
        let mut port = serialport::new(port_name, 1000000)
            .timeout(Duration::from_secs(1))
            .open()
            .unwrap();
        // 清空接收缓冲区
        port.clear(serialport::ClearBuffer::All).unwrap();
        Self {
            port,
            chip: chip.to_string(),
            memory_type: memory_type.to_string(),
        }
    }

    fn download_stub(port_name: &str, chip: &str, memory_type: &str) -> Result<(), std::io::Error> {
        let lister = Lister::new();

        let probes = lister.list_all();

        let index = probes.iter().enumerate().find_map(|(index, probe)| {
            probe.serial_number.as_ref().and_then(|s| {
                if s.contains(port_name) {
                    Some(index)
                } else {
                    None
                }
            })
        });
        println!("{:?}", index);
        let Some(index) = index else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No probe found with the given serial number",
            ));
        };
        let probe = probes[index]
            .open()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        println!("probe");

        let mut session = probe
            .attach(chip, Permissions::default())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        println!("session");
        let mut core = session
            .core(0)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        println!("core");

        core.reset_and_halt(std::time::Duration::from_secs(5))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        println!("reset");

        // Download the stub
        let stub = ram_stub::RamStubFile::get(
            CHIP_FILE_NAME
                .get(format!("{}_{}", chip, memory_type).as_str())
                .expect("REASON"),
        );
        let Some(stub) = stub else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No stub file found for the given chip and memory type",
            ));
        };

        let mut addr = 0x2005_A000;
        let mut data = &stub.data[..];
        while !data.is_empty() {
            let chunk = &data[..std::cmp::min(data.len(), 64*1024)];
            core.write_8(addr, chunk)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            addr += chunk.len() as u64;
            data = &data[chunk.len()..];
        }

        let sp = u32::from_le_bytes(
            stub.data[0..4]
                .try_into()
                .expect("slice with exactly 4 bytes"),
        );
        let pc = u32::from_le_bytes(
            stub.data[4..8]
                .try_into()
                .expect("slice with exactly 4 bytes"),
        );
        tracing::info!("SP: {:#010x}, PC: {:#010x}", sp, pc);
        // set SP
        core.write_core_reg(SP.id, sp)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        // set PC
        core.write_core_reg(PC.id, pc)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        
        core.run()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::thread::sleep(Duration::from_millis(500));
        
        // 打印现在时间，精确到ms
        let now = std::time::SystemTime::now();
        let now = now.duration_since(std::time::UNIX_EPOCH).unwrap();
        println!("now: {:?}", now.as_millis());
        Ok(())
    }
}
