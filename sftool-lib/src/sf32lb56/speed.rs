use super::SF32LB56Tool;
use crate::speed::SpeedTrait;

impl SpeedTrait for SF32LB56Tool {
    fn set_speed(&mut self, _speed: u32) -> Result<(), std::io::Error> {
        todo!("SF32LB56 set_speed implementation not yet available")
    }
}
