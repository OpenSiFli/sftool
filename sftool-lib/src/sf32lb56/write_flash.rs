use crate::write_flash::WriteFlashTrait;
use crate::WriteFlashParams;

impl WriteFlashTrait for super::SF32LB56Tool {
    fn write_flash(&mut self, _params: &WriteFlashParams) -> Result<(), std::io::Error> {
        todo!("SF32LB56 write_flash implementation not yet available")
    }
}
