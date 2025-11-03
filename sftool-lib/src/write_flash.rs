use crate::{Result, WriteFlashParams};

pub trait WriteFlashTrait {
    fn write_flash(&mut self, params: &WriteFlashParams) -> Result<()>;
}
