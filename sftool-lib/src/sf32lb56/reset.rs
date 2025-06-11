use super::SF32LB56Tool;
use crate::reset::Reset;

impl Reset for SF32LB56Tool {
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB56 soft_reset implementation not yet available")
    }
}
