pub trait WriteFlashTrait {
    fn write_flash(&mut self) -> Result<(), std::io::Error>;
}
