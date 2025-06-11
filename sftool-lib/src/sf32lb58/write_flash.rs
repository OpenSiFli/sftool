use crate::WriteFlashParams;
use crate::write_flash::WriteFlashTrait;

impl WriteFlashTrait for super::SF32LB58Tool {
    fn write_flash(&mut self, _params: &WriteFlashParams) -> Result<(), std::io::Error> {
        todo!("SF32LB58 write_flash implementation not yet available")
    }
}
