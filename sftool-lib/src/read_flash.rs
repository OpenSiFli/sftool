pub trait ReadFlashTrait {
    fn read_flash(&mut self) -> Result<(), std::io::Error>;
}
