pub trait Reset {
    fn soft_reset(&mut self) -> Result<(), std::io::Error>;
}