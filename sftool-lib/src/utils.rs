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

    pub fn verify_crc32(data: &[u8], expected_crc: u32) -> bool {
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

        let crc = crc::Crc::<u32>::new(&CRC_32_ALGO);
        let mut digest = crc.digest();
        digest.update(data);
        let checksum = digest.finalize();

        checksum == expected_crc
    }
}
