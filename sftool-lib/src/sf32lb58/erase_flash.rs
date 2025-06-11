use crate::erase_flash::EraseFlashTrait;
use crate::{EraseFlashParams, EraseRegionParams};

impl EraseFlashTrait for super::SF32LB58Tool {
    fn erase_flash(&mut self, _params: &EraseFlashParams) -> Result<(), std::io::Error> {
        todo!("SF32LB58 erase_flash implementation not yet available")
    }

    fn erase_region(&mut self, _params: &EraseRegionParams) -> Result<(), std::io::Error> {
        todo!("SF32LB58 erase_region implementation not yet available")
    }
}
