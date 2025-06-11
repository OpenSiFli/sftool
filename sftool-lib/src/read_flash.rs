use crate::ReadFlashParams;

pub trait ReadFlashTrait {
    fn read_flash(&mut self, params: &ReadFlashParams) -> Result<(), std::io::Error>;
}
