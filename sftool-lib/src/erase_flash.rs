use crate::{EraseFlashParams, EraseRegionParams};

pub trait EraseFlashTrait {
    fn erase_flash(&mut self, params: &EraseFlashParams) -> Result<(), std::io::Error>;
    fn erase_region(&mut self, params: &EraseRegionParams) -> Result<(), std::io::Error>;
}
