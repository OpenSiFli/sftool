use std::io::Write;
use crate::SifliTool;

pub trait WriteFlashTrait {
    fn write_flash(&mut self);
}

impl WriteFlashTrait for SifliTool {
    fn write_flash(&mut self) {
        println!("write_flash");
        let cmd = "burn_verify 0x12200000 0x2fdf40 0xe7940796\r";
        self.port.write_all(cmd.as_bytes()).unwrap();
        self.port.flush().unwrap();
        let mut rec_buf = vec![0u8; 100];
        self.port.read_exact(rec_buf.as_mut_slice()).unwrap();
        println!("{:?}", rec_buf);
    }
}