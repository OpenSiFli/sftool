use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::num::ParseIntError;
use crc::Algorithm;

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
}
