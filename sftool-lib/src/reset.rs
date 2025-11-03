use crate::Result;

pub trait Reset {
    fn soft_reset(&mut self) -> Result<()>;
}
