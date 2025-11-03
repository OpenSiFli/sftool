use crate::{EraseFlashParams, EraseRegionParams, Result};

pub trait EraseFlashTrait {
    fn erase_flash(&mut self, params: &EraseFlashParams) -> Result<()>;
    fn erase_region(&mut self, params: &EraseRegionParams) -> Result<()>;
}
