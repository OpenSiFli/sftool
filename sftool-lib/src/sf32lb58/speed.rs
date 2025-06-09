use crate::speed::SpeedTrait;
use super::SF32LB58Tool;

impl SpeedTrait for SF32LB58Tool {
    fn set_speed(&mut self, _speed: u32) -> Result<(), std::io::Error> {
        todo!("SF32LB58 set_speed implementation not yet available")
    }
}
