use crate::read_flash::ReadFlashTrait;
use crate::ReadFlashParams;

impl ReadFlashTrait for super::SF32LB56Tool {
    fn read_flash(&mut self, _params: &ReadFlashParams) -> Result<(), std::io::Error> {
        todo!("SF32LB56 read_flash implementation not yet available")
    }
}
