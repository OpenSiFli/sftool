use super::SF32LB56Tool;
use crate::common::write_flash::FlashWriter;
use crate::write_flash::WriteFlashTrait;
use crate::{Result, WriteFlashParams};

impl WriteFlashTrait for SF32LB56Tool {
    fn write_flash(&mut self, params: &WriteFlashParams) -> Result<()> {
        let packet_size = if self.base.compat { 256 } else { 128 * 1024 };

        if params.erase_all {
            FlashWriter::erase_all(self, &params.files)?;
        }

        for file in params.files.iter() {
            if !params.erase_all {
                FlashWriter::write_file_incremental(self, file, params.verify)?;
            } else {
                FlashWriter::write_file_full_erase(self, file, params.verify, packet_size)?;
            }
        }
        Ok(())
    }
}
