use crate::sf32lb52::SF32LB52Tool;
use crate::sifli_debug::SifliUartCommand;
use crate::SifliTool;
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};
use strum::{Display, EnumString};

#[derive(EnumString, Display, Debug, Clone, PartialEq, Eq)]
pub enum Command {
    #[strum(to_string = "burn_erase_all 0x{address:08x}\r")]
    EraseAll { address: u32 },

    #[strum(to_string = "burn_verify 0x{address:08x} 0x{len:08x} 0x{crc:08x}\r")]
    Verify { address: u32, len: u32, crc: u32 },

    #[strum(to_string = "burn_erase 0x{address:08x} 0x{len:08x}\r")]
    Erase { address: u32, len: u32 },

    #[strum(to_string = "burn_erase_write 0x{address:08x} 0x{len:08x}\r")]
    WriteAndErase { address: u32, len: u32 },

    #[strum(to_string = "burn_write 0x{address:08x} 0x{len:08x}\r")]
    Write { address: u32, len: u32 },

    #[strum(to_string = "burn_read 0x{address:08x} 0x{len:08x}\r")]
    Read { address: u32, len: u32 },

    #[strum(to_string = "burn_reset\r")]
    SoftReset,

    #[strum(to_string = "burn_speed {baud} {delay}\r")]
    SetBaud { baud: u32, delay: u32 },
}

#[derive(EnumString, Display, Debug, Clone, PartialEq, Eq)]
pub enum Response {
    #[strum(serialize = "OK")]
    Ok,
    #[strum(serialize = "Fail")]
    Fail,
    #[strum(serialize = "RX_WAIT")]
    RxWait,
}

pub const RESPONSE_STR_TABLE: [&str; 3] = ["OK", "Fail", "RX_WAIT"];

pub trait RamCommand {
    fn command(&mut self, cmd: Command) -> Result<Response, std::io::Error>;
    fn send_data(&mut self, data: &[u8]) -> Result<Response, std::io::Error>;
}

/// Helper function to get response from serial port
fn get_response(port: &mut Box<dyn serialport::SerialPort>, _expected_responses: &[&str], timeout_ms: u128) -> Result<String, std::io::Error> {
    const DEFAULT_TIMEOUT: u128 = 4000; // ms
    let timeout = if timeout_ms > 0 { timeout_ms } else { DEFAULT_TIMEOUT };
    
    let mut buffer = Vec::new();
    let start_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    
    let mut byte = [0];
    
    loop {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        if now - start_time > timeout {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Timeout waiting for response",
            ));
        }
        
        if port.read_exact(&mut byte).is_ok() {
            buffer.push(byte[0]);
            if buffer.len() >= 2 && buffer.ends_with(b"\r\n") {
                break;
            }
        }
    }
    
    // Remove \r\n from the end
    if buffer.len() >= 2 {
        buffer.truncate(buffer.len() - 2);
    }
    
    let response_str = String::from_utf8_lossy(&buffer);
    Ok(response_str.to_string())
}

pub trait DownloadStub {
    fn download_stub(&mut self) -> Result<(), std::io::Error>;
}

impl RamCommand for SF32LB52Tool {
    fn command(&mut self, cmd: Command) -> Result<Response, std::io::Error> {
        let cmd_str = cmd.to_string();
        self.port().write_all(cmd_str.as_bytes())?;
        self.port().flush()?;
        
        let response = get_response(self.port(), &RESPONSE_STR_TABLE, 0)?;
        
        match response.as_str() {
            "OK" => Ok(Response::Ok),
            "Fail" => Ok(Response::Fail),
            "RX_WAIT" => Ok(Response::RxWait),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown response: {}", response),
            )),
        }
    }

    fn send_data(&mut self, data: &[u8]) -> Result<Response, std::io::Error> {
        self.port().write_all(data)?;
        self.port().flush()?;
        
        let response = get_response(self.port(), &RESPONSE_STR_TABLE, 0)?;
        
        match response.as_str() {
            "OK" => Ok(Response::Ok),
            "Fail" => Ok(Response::Fail),
            "RX_WAIT" => Ok(Response::RxWait),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown response: {}", response),
            )),
        }
    }
}

impl DownloadStub for SF32LB52Tool {
    fn download_stub(&mut self) -> Result<(), std::io::Error> {
        // Use SifliTool trait methods
        SifliTool::attempt_connect(self)?;
        SifliTool::download_stub_impl(self)?;

        std::thread::sleep(std::time::Duration::from_millis(100));
        self.port().clear(serialport::ClearBuffer::All)?;
        SifliTool::debug_command(self, SifliUartCommand::Exit)?;

        // 1s之内串口发b"\r\n"字符串，并等待是否有"msh >"回复，200ms发一次b"\r\n"
        let mut buffer = Vec::new();
        let mut now = std::time::SystemTime::now();
        const RETRY: u32 = 5;
        let mut retry_count = 0;
        self.port().write_all(b"\r\n")?;
        self.port().flush()?;
        loop {
            let elapsed = now.elapsed().unwrap().as_millis();
            if elapsed > 200 {
                retry_count += 1;
                now = std::time::SystemTime::now();
                self.port().write_all(b"\r\n")?;
                self.port().flush()?;
                buffer.clear();
            }
            if retry_count > RETRY {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Timeout",
                ));
            }

            let mut byte = [0];
            let ret = self.port().read_exact(&mut byte);
            if ret.is_err() {
                continue;
            }
            buffer.push(byte[0]);

            // 一旦buffer出现"msh >"字符串，就认为接收完毕
            if buffer.windows(5).any(|window| window == b"msh >") {
                break;
            }
        }
        Ok(())
    }
}
