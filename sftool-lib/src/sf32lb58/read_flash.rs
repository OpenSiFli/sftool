use crate::ReadFlashParams;
use crate::read_flash::ReadFlashTrait;

impl ReadFlashTrait for super::SF32LB58Tool {
    fn read_flash(&mut self, _params: &ReadFlashParams) -> Result<(), std::io::Error> {
        todo!("SF32LB58 read_flash implementation not yet available")
    }
}
