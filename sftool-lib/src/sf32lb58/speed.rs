use super::SF32LB58Tool;
use crate::common::speed::SpeedOps;
use crate::{Result, speed::SpeedTrait};

impl SpeedTrait for SF32LB58Tool {
    fn set_speed(&mut self, speed: u32) -> Result<()> {
        SpeedOps::set_speed(self, speed)
    }
}
