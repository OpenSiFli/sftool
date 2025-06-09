use crate::reset::Reset;
use super::SF32LB56Tool;

impl Reset for SF32LB56Tool {
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB56 soft_reset implementation not yet available")
    }
}
