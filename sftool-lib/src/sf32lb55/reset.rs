use super::SF32LB55Tool;
use crate::common::reset::ResetOps;
use crate::{Result, reset::Reset};

impl Reset for SF32LB55Tool {
    fn soft_reset(&mut self) -> Result<()> {
        ResetOps::soft_reset(self)
    }
}
