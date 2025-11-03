use crate::{ReadFlashParams, Result};

pub trait ReadFlashTrait {
    fn read_flash(&mut self, params: &ReadFlashParams) -> Result<()>;
}
