use crate::erase_flash::EraseFlashTrait;
use crate::{EraseFlashParams, EraseRegionParams};

impl EraseFlashTrait for super::SF32LB56Tool {
    fn erase_flash(&mut self, _params: &EraseFlashParams) -> Result<(), std::io::Error> {
        todo!("SF32LB56 erase_flash implementation not yet available")
    }

    fn erase_region(&mut self, _params: &EraseRegionParams) -> Result<(), std::io::Error> {
        todo!("SF32LB56 erase_region implementation not yet available")
    }
}
