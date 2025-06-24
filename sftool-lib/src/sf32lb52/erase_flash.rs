use super::SF32LB52Tool;
use crate::common::erase_flash::EraseOps;
use crate::erase_flash::EraseFlashTrait;
use crate::{EraseFlashParams, EraseRegionParams};

impl EraseFlashTrait for SF32LB52Tool {
    fn erase_flash(&mut self, params: &EraseFlashParams) -> Result<(), std::io::Error> {
        // 解析擦除地址 (这是擦除全部flash的命令，使用EraseAll)
        let address = EraseOps::parse_address(&params.address)?;
        self.internal_erase_all(address)
    }

    fn erase_region(&mut self, params: &EraseRegionParams) -> Result<(), std::io::Error> {
        // 处理每个区域
        for region_spec in params.region.iter() {
            let (address, len) = EraseOps::parse_region(region_spec)?;
            self.internal_erase_region(address, len)?;
        }
        Ok(())
    }
}
