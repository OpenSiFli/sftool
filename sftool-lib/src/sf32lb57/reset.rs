use super::SF32LB57Tool;
use crate::common::reset::ResetOps;
use crate::{Result, reset::Reset};

impl Reset for SF32LB57Tool {
    fn soft_reset(&mut self) -> Result<()> {
        ResetOps::soft_reset(self)
    }
}
