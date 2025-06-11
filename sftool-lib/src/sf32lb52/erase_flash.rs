use super::SF32LB52Tool;
use crate::erase_flash::EraseFlashTrait;
use crate::{utils, EraseFlashParams, EraseRegionParams};

impl EraseFlashTrait for SF32LB52Tool {
    fn erase_flash(&mut self, params: &EraseFlashParams) -> Result<(), std::io::Error> {

        // 解析擦除地址 (这是擦除全部flash的命令，使用EraseAll)
        let address = utils::Utils::str_to_u32(&params.address)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        self.internal_erase_all(address)
    }

    fn erase_region(&mut self, params: &EraseRegionParams) -> Result<(), std::io::Error> {
        // 处理每个区域
        for region_spec in params.region.iter() {
            // 解析格式: address:size
            let Some((addr_str, size_str)) = region_spec.split_once(':') else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Invalid region format: {}. Expected: address:size", region_spec),
                ));
            };

            let address = utils::Utils::str_to_u32(addr_str)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            let len = utils::Utils::str_to_u32(size_str)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            self.internal_erase_region(address, len)?;
        }

        Ok(())
    }
}
