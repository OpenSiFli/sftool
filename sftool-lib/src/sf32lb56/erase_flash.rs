use super::SF32LB56Tool;
use crate::common::erase_flash::EraseOps;
use crate::erase_flash::EraseFlashTrait;
use crate::{EraseFlashParams, EraseRegionParams};

impl EraseFlashTrait for SF32LB56Tool {
    fn erase_flash(&mut self, params: &EraseFlashParams) -> Result<(), std::io::Error> {
        EraseOps::erase_all(self, params.address)
    }

    fn erase_region(&mut self, params: &EraseRegionParams) -> Result<(), std::io::Error> {
        // 处理每个区域
        for region in params.regions.iter() {
            EraseOps::erase_region(self, region.address, region.size)?;
        }
        Ok(())
    }
}
