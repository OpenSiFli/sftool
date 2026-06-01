use super::SF32LB57Tool;
use crate::common::speed::SpeedOps;
use crate::{Result, speed::SpeedTrait};

impl SpeedTrait for SF32LB57Tool {
    fn set_speed(&mut self, speed: u32) -> Result<()> {
        SpeedOps::set_speed(self, speed)
    }
}
