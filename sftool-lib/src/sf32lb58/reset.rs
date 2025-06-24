use super::SF32LB58Tool;
use crate::common::reset::ResetOps;
use crate::reset::Reset;

impl Reset for SF32LB58Tool {
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        ResetOps::soft_reset(self)
    }
}
