use super::SF32LB56Tool;
use crate::common::write_flash::{WriteFlashFile, FlashWriter, parse_file_info};
use crate::WriteFlashParams;
use crate::write_flash::WriteFlashTrait;

impl WriteFlashTrait for SF32LB56Tool {
    fn write_flash(&mut self, params: &WriteFlashParams) -> Result<(), std::io::Error> {
        let mut step = self.step;
        let mut write_flash_files: Vec<WriteFlashFile> = Vec::new();
        let packet_size = if self.base.compat { 256 } else { 128 * 1024 };

        for file in params.file_path.iter() {
            write_flash_files.append(&mut parse_file_info(file)?);
        }

        if params.erase_all {
            FlashWriter::erase_all(self, &write_flash_files)?;
        }

        for file in write_flash_files.iter() {
            if !params.erase_all {
                FlashWriter::write_file_incremental(self, file, &mut step, params.verify)?;
            } else {
                FlashWriter::write_file_full_erase(self, file, &mut step, params.verify, packet_size)?;
            }
        }
        Ok(())
    }
}
