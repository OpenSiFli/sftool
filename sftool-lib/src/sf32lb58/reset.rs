use crate::reset::Reset;
use super::SF32LB58Tool;

impl Reset for SF32LB58Tool {
    fn soft_reset(&mut self) -> Result<(), std::io::Error> {
        todo!("SF32LB58 soft_reset implementation not yet available")
    }
}
