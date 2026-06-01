use super::SF32LB57Tool;
use crate::common::read_flash::FlashReader;
use crate::read_flash::ReadFlashTrait;
use crate::{ReadFlashParams, Result};

impl ReadFlashTrait for SF32LB57Tool {
    fn read_flash(&mut self, params: &ReadFlashParams) -> Result<()> {
        for file in params.files.iter() {
            FlashReader::read_flash_data(self, file.address, file.size, &file.file_path)?;
        }

        Ok(())
    }
}
