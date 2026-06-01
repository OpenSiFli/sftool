use super::SF32LB57Tool;
use crate::erase_flash::EraseFlashTrait;
use crate::{EraseFlashParams, EraseRegionParams, Result};

impl EraseFlashTrait for SF32LB57Tool {
    fn erase_flash(&mut self, params: &EraseFlashParams) -> Result<()> {
        self.internal_erase_all(params.address)
    }

    fn erase_region(&mut self, params: &EraseRegionParams) -> Result<()> {
        for region in params.regions.iter() {
            self.internal_erase_region(region.address, region.size)?;
        }
        Ok(())
    }
}
