pub struct Utils;
impl Utils {
    pub fn str_to_u32(s: &str) -> Result<u32, std::num::ParseIntError> {
        if let Some(hex_digits) = s.strip_prefix("0x") {
            u32::from_str_radix(hex_digits, 16)
        } else if let Some(bin_digits) = s.strip_prefix("0b") {
            u32::from_str_radix(bin_digits, 2)
        } else if let Some(oct_digits) = s.strip_prefix("0o") {
            u32::from_str_radix(oct_digits, 8)
        } else {
            s.parse::<u32>()
        }
    }
}
