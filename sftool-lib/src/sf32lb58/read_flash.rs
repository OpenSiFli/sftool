use super::SF32LB58Tool;
use crate::{ReadFlashParams, Result};
use crate::common::read_flash::FlashReader;
use crate::read_flash::ReadFlashTrait;

impl ReadFlashTrait for SF32LB58Tool {
    fn read_flash(&mut self, params: &ReadFlashParams) -> Result<()> {
        // 处理每个读取文件
        for file in params.files.iter() {
            FlashReader::read_flash_data(self, file.address, file.size, &file.file_path)?;
        }

        Ok(())
    }
}
