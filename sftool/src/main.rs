use std::path::PathBuf;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
enum Chip {
    #[clap(name = "SF32LB52")]
    SF32LB52,
}

#[derive(Debug, Clone, ValueEnum)]
enum Operation {
    #[clap(name = "none")]
    None,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "sftool CLI", long_about = None)]
struct Cli {
    /// Target chip type
    #[arg(short = 'c', long = "chip", value_enum)]
    chip: Chip,

    /// Serial port device
    #[arg(short = 'p', long = "port")]
    port: String,

    /// Serial port baud rate used when flashing/reading
    #[arg(short = 'b', long = "baud")]
    baud: u32,

    /// What to do before connecting to the chip
    #[arg(long = "before", value_enum, default_value = "none")]
    before: Operation,

    /// What to do after siflitool is finished
    #[arg(long = "after", value_enum, default_value = "none")]
    after: Operation,

    /// Number of attempts to connect, negative or 0 for infinite. Default: 7.
    #[arg(long = "connect-attempts", default_value_t = 7)]
    connect_attempts: i8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Write a binary blob to flash
    #[command(name = "write_flash")]
    WriteFlash(WriteFlash),
}

#[derive(Parser, Debug)]
#[command(about = "Write a binary blob to flash")]
struct WriteFlash {
    /// Verify just-written data on flash (mostly superfluous, data is read back during flashing)
    #[arg(long = "verify")]
    verify: bool,

    /// Disable data compression during transfer
    #[arg(short = 'u', long = "no-compress")]
    no_compress: bool,

    /// Erase all regions of flash (not just write areas) before programming
    #[arg(short = 'e', long = "erase-all")]
    erase_all: bool,

    /// Binary file (format: <filename@address>, if file format includes address info, @address is optional)
    #[arg(required = true)]
    files: Vec<String>,
}

fn main() {
    let args = Cli::parse();
}
