use super::SF32LB58Tool;
use crate::common::reset::ResetOps;
use crate::{Result, reset::Reset};

impl Reset for SF32LB58Tool {
    fn soft_reset(&mut self) -> Result<()> {
        ResetOps::soft_reset(self)
    }
}
