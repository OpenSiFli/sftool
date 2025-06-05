use crate::erase_flash::EraseFlashTrait;

impl EraseFlashTrait for super::SF32LB56Tool {
    fn erase_flash(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB56 erase_flash implementation not yet available")
    }

    fn erase_region(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB56 erase_region implementation not yet available")
    }
}
