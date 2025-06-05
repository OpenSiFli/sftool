pub trait EraseFlashTrait {
    fn erase_flash(&mut self) -> Result<(), std::io::Error>;
    fn erase_region(&mut self) -> Result<(), std::io::Error>;
}
