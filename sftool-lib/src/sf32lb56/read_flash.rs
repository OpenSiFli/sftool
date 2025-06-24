use super::SF32LB56Tool;
use crate::common::read_flash::{FlashReader, ReadFlashFile};
use crate::read_flash::ReadFlashTrait;
use crate::ReadFlashParams;

impl ReadFlashTrait for SF32LB56Tool {
    fn read_flash(&mut self, params: &ReadFlashParams) -> Result<(), std::io::Error> {
        let mut read_flash_files: Vec<ReadFlashFile> = Vec::new();

        // 解析所有文件读取
        for file_spec in params.file_path.iter() {
            read_flash_files.push(FlashReader::parse_file_info(file_spec)?);
        }

        // 处理每个读取
        for file in read_flash_files {
            FlashReader::read_flash_data(self, file.address, file.size, &file.file_path)?;
        }

        Ok(())
    }
}
