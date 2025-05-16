use std::num::ParseIntError;

pub struct Utils;
impl Utils {
    pub fn str_to_u32(s: &str) -> Result<u32, ParseIntError> {
        let s = s.trim();

        let (num_str, multiplier) = match s.chars().last() {
            Some('k') | Some('K') => (&s[..s.len()-1], 1_000u32),
            Some('m') | Some('M') => (&s[..s.len()-1], 1_000_000u32),
            Some('g') | Some('G') => (&s[..s.len()-1], 1_000_000_000u32),
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
}
