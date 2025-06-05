use crate::erase_flash::EraseFlashTrait;

impl EraseFlashTrait for super::SF32LB58Tool {
    fn erase_flash(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB58 erase_flash implementation not yet available")
    }

    fn erase_region(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB58 erase_region implementation not yet available")
    }
}
