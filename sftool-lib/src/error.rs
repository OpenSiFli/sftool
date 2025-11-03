use thiserror::Error;

/// Convenient result type for `sftool-lib`.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serial(#[from] serialport::Error),

    #[error("probe-rs error: {0}")]
    ProbeRs(#[from] probe_rs::Error),

    #[error("Intel HEX parse error: {0}")]
    IntelHex(#[from] ihex::ReaderError),

    #[error("ELF parse error: {0}")]
    Elf(#[from] goblin::error::Error),

    #[error("integer parse error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("timeout while {0}")]
    Timeout(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("unsupported chip: {0}")]
    UnsupportedChip(String),

    #[error("unsupported memory: {0}")]
    UnsupportedMemory(String),

    #[error("CRC mismatch: expected {expected:#010X}, got {actual:#010X}")]
    CrcMismatch { expected: u32, actual: u32 },

    #[error("embedded asset `{0}` not found")]
    MissingEmbeddedAsset(&'static str),
}

impl Error {
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }
}
