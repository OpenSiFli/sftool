use super::SF32LB52Tool;
use crate::common::speed::SpeedOps;
use crate::speed::SpeedTrait;

impl SpeedTrait for SF32LB52Tool {
    fn set_speed(&mut self, speed: u32) -> Result<(), std::io::Error> {
        SpeedOps::set_speed(self, speed)
    }
}
