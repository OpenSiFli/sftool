use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use sftool_lib::{AfterOperation, BeforeOperation, ChipType};
use strum::{Display, EnumString};

use crate::config::SfToolConfig;

pub type MergedConfig = (
    ChipType,
    String,
    String,
    u32,
    BeforeOperation,
    AfterOperation,
    i8,
    bool,
    bool,
    Option<String>, // stub path
);

#[derive(EnumString, Display, Debug, Clone, ValueEnum)]
pub enum Memory {
    #[clap(name = "nor")]
    Nor,
    #[clap(name = "nand")]
    Nand,
    #[clap(name = "sd")]
    Sd,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "sftool CLI", long_about = None)]
pub struct Cli {
    /// JSON configuration file path
    #[arg(long = "config", short = 'f')]
    pub config: Option<String>,

    /// Target chip type
    #[arg(short = 'c', long = "chip", value_enum)]
    pub chip: Option<ChipType>,

    /// Memory type (default: nor)
    #[arg(short = 'm', long = "memory", value_enum)]
    pub memory: Option<Memory>,

    /// Serial port device
    #[arg(short = 'p', long = "port")]
    pub port: Option<String>,

    /// Serial port baud rate used when flashing/reading (default: 1000000)
    #[arg(short = 'b', long = "baud")]
    pub baud: Option<u32>,

    /// What to do before connecting to the chip (default: default_reset)
    #[arg(long = "before", value_enum)]
    pub before: Option<BeforeOperation>,

    /// What to do after siflitool is finished (default: soft_reset)
    #[arg(long = "after", value_enum)]
    pub after: Option<AfterOperation>,

    /// Number of attempts to connect, negative or 0 for infinite (default: 3)
    #[arg(long = "connect-attempts")]
    pub connect_attempts: Option<i8>,

    /// Enable compatibility mode (default: false)
    #[arg(long = "compat")]
    pub compat: Option<bool>,

    /// External stub file path (overrides embedded stub)
    #[arg(long = "stub")]
    pub stub: Option<String>,

    /// Stub config JSON to apply to the stub before operations
    #[arg(long = "stub-config", global = true)]
    pub stub_config_json: Option<String>,

    /// Suppress progress bar output (default: false)
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Write a binary blob to flash
    #[command(name = "write_flash")]
    WriteFlash(WriteFlash),

    /// Read a binary blob from flash
    #[command(name = "read_flash")]
    ReadFlash(ReadFlash),

    /// Erase the entire flash
    #[command(name = "erase_flash")]
    EraseFlash(EraseFlash),

    /// Erase a region of the flash
    #[command(name = "erase_region")]
    EraseRegion(EraseRegion),

    /// Manage stub config in AXF/ELF driver files
    #[command(name = "stub")]
    Stub(StubCommand),
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Write a binary blob to flash")]
pub struct WriteFlash {
    /// Verify just-written data on flash (mostly superfluous, data is read back during flashing)
    #[arg(long = "verify", default_value = "true")]
    pub verify: bool,

    /// Disable data compression during transfer
    #[arg(short = 'u', long = "no-compress")]
    pub no_compress: bool,

    /// Erase all regions of flash (not just write areas) before programming
    #[arg(short = 'e', long = "erase-all")]
    pub erase_all: bool,

    /// Binary file (format: <filename@address>, if file format includes address info, @address is optional)
    #[arg(required = true)]
    pub files: Vec<String>,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Read a binary blob from flash")]
pub struct ReadFlash {
    /// Binary file (format: <filename@address:size>)
    #[arg(required = true)]
    pub files: Vec<String>,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Erase flash")]
pub struct EraseFlash {
    /// Erase flash
    #[arg(required = true)]
    pub address: String,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Erase a region of the flash")]
pub struct EraseRegion {
    /// Erase region (format: <address:size>)
    #[arg(required = true)]
    pub region: Vec<String>,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Manage stub config in AXF/ELF driver files")]
pub struct StubCommand {
    #[command(subcommand)]
    pub action: StubAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum StubAction {
    /// Write stub config into AXF/ELF driver files
    #[command(name = "write")]
    Write(StubWrite),

    /// Clear stub config in AXF/ELF driver files
    #[command(name = "clear")]
    Clear(StubClear),

    /// Read stub config from AXF/ELF driver files
    #[command(name = "read")]
    Read(StubRead),
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Write stub config into AXF/ELF driver files")]
pub struct StubWrite {
    /// Target driver files
    #[arg(required = true)]
    pub files: Vec<String>,

    /// Stub config JSON file path
    #[arg(long = "stub-config")]
    pub stub_config: String,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Clear stub config in AXF/ELF driver files")]
pub struct StubClear {
    /// Target driver files
    #[arg(required = true)]
    pub files: Vec<String>,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Read stub config from AXF/ELF driver files")]
pub struct StubRead {
    /// Target driver files
    #[arg(required = true)]
    pub files: Vec<String>,

    /// Optional output JSON file (single input only)
    #[arg(long = "output")]
    pub output: Option<String>,
}

/// Convert Memory enum to string
fn memory_to_string(memory: &Memory) -> String {
    match memory {
        Memory::Nor => "nor".to_string(),
        Memory::Nand => "nand".to_string(),
        Memory::Sd => "sd".to_string(),
    }
}

/// Merge CLI arguments with configuration file, CLI args take precedence
pub fn merge_config(args: &Cli, config: Option<SfToolConfig>) -> Result<MergedConfig> {
    // 使用配置文件或默认配置
    let base_config = config.unwrap_or_else(SfToolConfig::with_defaults);

    let chip = match &args.chip {
        Some(c) => c.clone(),
        None => base_config
            .parse_chip_type()
            .map_err(|e| anyhow!("Invalid chip type in config: {}", e))?,
    };

    let memory = args
        .memory
        .as_ref()
        .map(memory_to_string)
        .unwrap_or_else(|| base_config.memory.clone());

    let port = args
        .port
        .clone()
        .unwrap_or_else(|| base_config.port.clone());
    let baud = args.baud.unwrap_or(base_config.baud);

    let before = match &args.before {
        Some(b) => b.clone(),
        None => base_config
            .parse_before()
            .map_err(|e| anyhow!("Invalid before operation in config: {}", e))?,
    };

    let after = match &args.after {
        Some(a) => a.clone(),
        None => base_config
            .parse_after()
            .map_err(|e| anyhow!("Invalid after operation in config: {}", e))?,
    };

    let connect_attempts = args
        .connect_attempts
        .unwrap_or(base_config.connect_attempts);
    let compat = args.compat.unwrap_or(base_config.compat);
    let quiet = args.quiet;
    let stub_path = args.stub.clone().or_else(|| base_config.stub_path.clone());
    // 验证必需字段
    if port.is_empty() {
        bail!("Port must be specified either via --port or in config file");
    }

    Ok((
        chip,
        memory,
        port,
        baud,
        before,
        after,
        connect_attempts,
        compat,
        quiet,
        stub_path,
    ))
}

/// Determine which command to execute from CLI args or config file
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum CommandSource {
    Cli(Commands),
    Config(SfToolConfig),
}

pub fn get_command_source(args: &Cli, config: Option<SfToolConfig>) -> Result<CommandSource> {
    match (&args.command, &config) {
        (Some(cmd), _) => Ok(CommandSource::Cli(cmd.clone())),
        (None, Some(cfg)) => Ok(CommandSource::Config(cfg.clone())),
        (None, None) => {
            bail!("No command specified. Use a subcommand or provide a config file with a command.")
        }
    }
}
