pub trait SpeedTrait {
    fn set_speed(&mut self, speed: u32) -> Result<(), std::io::Error>;
}
