use super::SF32LB55Tool;
use crate::common::read_flash::FlashReader;
use crate::read_flash::ReadFlashTrait;
use crate::{ReadFlashParams, Result};

impl ReadFlashTrait for SF32LB55Tool {
    fn read_flash(&mut self, params: &ReadFlashParams) -> Result<()> {
        // 处理每个读取文件
        for file in params.files.iter() {
            FlashReader::read_flash_data(self, file.address, file.size, &file.file_path)?;
        }

        Ok(())
    }
}
