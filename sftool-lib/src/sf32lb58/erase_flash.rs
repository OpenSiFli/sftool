use super::SF32LB58Tool;
use crate::common::erase_flash::EraseOps;
use crate::erase_flash::EraseFlashTrait;
use crate::{EraseFlashParams, EraseRegionParams};

impl EraseFlashTrait for SF32LB58Tool {
    fn erase_flash(&mut self, params: &EraseFlashParams) -> Result<(), std::io::Error> {
        // 解析擦除地址
        let address = EraseOps::parse_address(&params.address)?;
        EraseOps::erase_all(self, address)
    }

    fn erase_region(&mut self, params: &EraseRegionParams) -> Result<(), std::io::Error> {
        // 处理每个区域
        for region_spec in params.region.iter() {
            let (address, len) = EraseOps::parse_region(region_spec)?;
            EraseOps::erase_region(self, address, len)?;
        }
        Ok(())
    }
}
