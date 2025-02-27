mod write_flash;

use probe_rs::Permissions;
use probe_rs::config::Chip;
use probe_rs::probe::{Probe, list::Lister};
use serialport;
use serialport::SerialPort;
use std::time::Duration;

pub struct SifliTool {
    port: Box<dyn SerialPort>,
    chip: String,
}

impl SifliTool {
    fn new(port_name: &str, chip: &str) -> Self {
        Self::download_stub(port_name, chip).unwrap();
        let port = serialport::new(port_name, 1000000)
            .timeout(Duration::from_secs(1))
            .open()
            .unwrap();
        Self {
            port,
            chip: chip.to_string(),
        }
    }

    fn download_stub(port_name: &str, chip: &str) -> Result<(), std::io::Error> {
        let lister = Lister::new();

        let probes = lister.list_all();

        let index = probes.iter().enumerate().find_map(|(index, probe)| {
            // Assuming the serial number is an Option<String>
            probe.serial_number.as_ref().and_then(|s| {
                if s.contains(port_name) {
                    Some(index)
                } else {
                    None
                }
            })
        });
        let Some(index) = index else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No probe found with the given serial number",
            ));
        };
        let probe = probes[index]
            .open()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let mut session = probe
            .attach(chip, Permissions::default())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let mut core = session
            .core(0)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        core.halt(std::time::Duration::from_millis(10))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(())
    }
    
}
