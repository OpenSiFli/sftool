use super::SF32LB52Tool;
use crate::erase_flash::EraseFlashTrait;
use crate::{EraseFlashParams, EraseRegionParams, Result};

impl EraseFlashTrait for SF32LB52Tool {
    fn erase_flash(&mut self, params: &EraseFlashParams) -> Result<()> {
        self.internal_erase_all(params.address)
    }

    fn erase_region(&mut self, params: &EraseRegionParams) -> Result<()> {
        // 处理每个区域
        for region in params.regions.iter() {
            self.internal_erase_region(region.address, region.size)?;
        }
        Ok(())
    }
}
