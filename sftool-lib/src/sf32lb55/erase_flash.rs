use super::SF32LB55Tool;
use crate::common::erase_flash::EraseOps;
use crate::erase_flash::EraseFlashTrait;
use crate::{EraseFlashParams, EraseRegionParams, Result};

impl EraseFlashTrait for SF32LB55Tool {
    fn erase_flash(&mut self, params: &EraseFlashParams) -> Result<()> {
        EraseOps::erase_all(self, params.address)
    }

    fn erase_region(&mut self, params: &EraseRegionParams) -> Result<()> {
        // 处理每个区域
        for region in params.regions.iter() {
            EraseOps::erase_region(self, region.address, region.size)?;
        }
        Ok(())
    }
}
