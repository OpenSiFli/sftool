use super::SF32LB56Tool;
use crate::common::reset::ResetOps;
use crate::reset::Reset;

impl Reset for SF32LB56Tool {
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        ResetOps::soft_reset(self)
    }
}
