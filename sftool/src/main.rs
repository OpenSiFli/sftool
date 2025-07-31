use clap::{Parser, Subcommand, ValueEnum};
use serialport;
use sftool_lib::{ChipType, Operation, SifliToolBase, create_sifli_tool};
use std::io::ErrorKind;
use std::process;
use strum::{Display, EnumString};

mod config;
use config::SfToolConfig;

/// Convert config file WriteFlashFileConfig to string format expected by CLI
fn config_write_file_to_string(file: &config::WriteFlashFileConfig) -> Result<String, String> {
    match &file.address {
        Some(addr) => Ok(format!("{}@{}", file.path, addr.0)),
        None => Ok(file.path.clone()),
    }
}

/// Convert config file ReadFlashFileConfig to string format expected by CLI  
fn config_read_file_to_string(file: &config::ReadFlashFileConfig) -> String {
    format!("{}@{}:{}", file.path, file.address.0, file.size.0)
}

/// Convert config file RegionItemConfig to string format expected by CLI
fn config_region_to_string(region: &config::RegionItemConfig) -> String {
    format!("{}:{}", region.address.0, region.size.0)
}

/// Execute command from config file
fn execute_config_command(
    config: &SfToolConfig,
    siflitool: &mut Box<dyn sftool_lib::SifliTool>,
) -> Result<(), std::io::Error> {
    if let Some(ref write_flash) = config.write_flash {
        // Convert config files to CLI format
        let mut files = Vec::new();
        for file_config in &write_flash.files {
            match config_write_file_to_string(file_config) {
                Ok(file_str) => files.push(file_str),
                Err(e) => {
                    eprintln!("Failed to convert config file {}: {}", file_config.path, e);
                    std::process::exit(1);
                }
            }
        }

        // Parse files using existing logic
        let mut parsed_files = Vec::new();
        for file_str in files.iter() {
            match sftool_lib::utils::Utils::parse_file_info(file_str) {
                Ok(mut parsed) => {
                    parsed_files.append(&mut parsed);
                }
                Err(e) => {
                    eprintln!("Failed to parse file {}: {}", file_str, e);
                    std::process::exit(1);
                }
            }
        }

        let write_params = sftool_lib::WriteFlashParams {
            files: parsed_files,
            verify: write_flash.verify,
            no_compress: write_flash.no_compress,
            erase_all: write_flash.erase_all,
        };
        siflitool.write_flash(&write_params)
    } else if let Some(ref read_flash) = config.read_flash {
        // Convert config files to CLI format
        let files: Vec<String> = read_flash
            .files
            .iter()
            .map(config_read_file_to_string)
            .collect();

        // Parse files using existing logic
        let mut parsed_files = Vec::new();
        for file_str in files.iter() {
            match sftool_lib::utils::Utils::parse_read_file_info(file_str) {
                Ok(parsed_file) => {
                    parsed_files.push(parsed_file);
                }
                Err(e) => {
                    eprintln!("Failed to parse read file {}: {}", file_str, e);
                    std::process::exit(1);
                }
            }
        }

        let read_params = sftool_lib::ReadFlashParams {
            files: parsed_files,
        };
        siflitool.read_flash(&read_params)
    } else if let Some(ref erase_flash) = config.erase_flash {
        // Parse erase address using existing logic
        let address = match sftool_lib::utils::Utils::parse_erase_address(&erase_flash.address.0) {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!(
                    "Failed to parse erase address {}: {}",
                    erase_flash.address.0, e
                );
                std::process::exit(1);
            }
        };

        let erase_params = sftool_lib::EraseFlashParams { address };
        siflitool.erase_flash(&erase_params)
    } else if let Some(ref erase_region) = config.erase_region {
        // Convert config regions to CLI format
        let regions: Vec<String> = erase_region
            .regions
            .iter()
            .map(config_region_to_string)
            .collect();

        // Parse regions using existing logic
        let mut parsed_regions = Vec::new();
        for region_str in regions.iter() {
            match sftool_lib::utils::Utils::parse_erase_region(region_str) {
                Ok(parsed_region) => {
                    parsed_regions.push(parsed_region);
                }
                Err(e) => {
                    eprintln!("Failed to parse erase region {}: {}", region_str, e);
                    std::process::exit(1);
                }
            }
        }

        let erase_region_params = sftool_lib::EraseRegionParams {
            regions: parsed_regions,
        };
        siflitool.erase_region(&erase_region_params)
    } else {
        Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "No valid command found in config file.",
        ))
    }
}

#[derive(EnumString, Display, Debug, Clone, ValueEnum)]
enum Memory {
    #[clap(name = "nor")]
    Nor,
    #[clap(name = "nand")]
    Nand,
    #[clap(name = "sd")]
    Sd,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "sftool CLI", long_about = None)]
struct Cli {
    /// JSON configuration file path
    #[arg(long = "config", short = 'f')]
    config: Option<String>,

    /// Target chip type
    #[arg(short = 'c', long = "chip", value_enum)]
    chip: Option<ChipType>,

    /// Memory type (default: nor)
    #[arg(short = 'm', long = "memory", value_enum)]
    memory: Option<Memory>,

    /// Serial port device
    #[arg(short = 'p', long = "port")]
    port: Option<String>,

    /// Serial port baud rate used when flashing/reading (default: 1000000)
    #[arg(short = 'b', long = "baud")]
    baud: Option<u32>,

    /// What to do before connecting to the chip (default: default_reset)
    #[arg(long = "before", value_enum)]
    before: Option<Operation>,

    /// What to do after siflitool is finished (default: soft_reset)
    #[arg(long = "after", value_enum)]
    after: Option<Operation>,

    /// Number of attempts to connect, negative or 0 for infinite (default: 3)
    #[arg(long = "connect-attempts")]
    connect_attempts: Option<i8>,

    /// Enable compatibility mode (default: false)
    #[arg(long = "compat")]
    compat: Option<bool>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
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
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Write a binary blob to flash")]
struct WriteFlash {
    /// Verify just-written data on flash (mostly superfluous, data is read back during flashing)
    #[arg(long = "verify", default_value = "true")]
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

#[derive(Parser, Debug, Clone)]
#[command(about = "Read a binary blob from flash")]
struct ReadFlash {
    /// Binary file (format: <filename@address:size>)
    #[arg(required = true)]
    files: Vec<String>,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Erase flash")]
struct EraseFlash {
    /// Erase flash
    #[arg(required = true)]
    address: String,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Erase a region of the flash")]
struct EraseRegion {
    /// Erase region (format: <address:size>)
    #[arg(required = true)]
    region: Vec<String>,
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
fn merge_config(
    args: &Cli,
    config: Option<SfToolConfig>,
) -> Result<
    (
        ChipType,
        String,
        String,
        u32,
        Operation,
        Operation,
        i8,
        bool,
    ),
    String,
> {
    // 使用配置文件或默认配置
    let base_config = config.unwrap_or_else(|| SfToolConfig::with_defaults());

    let chip = match &args.chip {
        Some(c) => c.clone(),
        None => base_config
            .parse_chip_type()
            .map_err(|e| format!("Invalid chip type in config: {}", e))?,
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
            .map_err(|e| format!("Invalid before operation in config: {}", e))?,
    };

    let after = match &args.after {
        Some(a) => a.clone(),
        None => base_config
            .parse_after()
            .map_err(|e| format!("Invalid after operation in config: {}", e))?,
    };

    let connect_attempts = args
        .connect_attempts
        .unwrap_or(base_config.connect_attempts);
    let compat = args.compat.unwrap_or(base_config.compat);

    // 验证必需字段
    if port.is_empty() {
        return Err("Port must be specified either via --port or in config file".to_string());
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
    ))
}

/// Determine which command to execute from CLI args or config file
#[derive(Debug)]
enum CommandSource {
    Cli(Commands),
    Config(SfToolConfig),
}

fn get_command_source(args: &Cli, config: Option<SfToolConfig>) -> Result<CommandSource, String> {
    match (&args.command, &config) {
        (Some(cmd), _) => Ok(CommandSource::Cli(cmd.clone())),
        (None, Some(cfg)) => Ok(CommandSource::Config(cfg.clone())),
        (None, None) => Err(
            "No command specified. Use a subcommand or provide a config file with a command."
                .to_string(),
        ),
    }
}

/// Convert macOS /dev/tty.* ports to /dev/cu.* ports
///
/// On macOS, /dev/tty.* ports should be avoided in favor of /dev/cu.* ports
/// This function automatically converts any /dev/tty.* path to its /dev/cu.* equivalent
///
/// # Parameters
/// * `port_name` - The original port name
///
/// # Returns
/// * The corrected port name
fn normalize_mac_port_name(port_name: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        if port_name.starts_with("/dev/tty.") {
            return port_name.replace("/dev/tty.", "/dev/cu.");
        }
    }
    port_name.to_string()
}

/// Check if the specified serial port is available
///
/// # Parameters
/// * `port_name` - The name of the serial port to check
///
/// # Returns
/// * `Result<(), String>` - Returns Ok(()) if the port is available; otherwise returns an Err with error message
fn check_port_available(port_name: &str) -> Result<(), String> {
    match serialport::available_ports() {
        Ok(ports) => {
            // On macOS, only use /dev/cu.* ports, not /dev/tty.* ports
            let filtered_ports: Vec<_> = ports
                .into_iter()
                .filter(|p| {
                    #[cfg(target_os = "macos")]
                    {
                        !p.port_name.starts_with("/dev/tty.")
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        true
                    }
                })
                .collect();

            // Check if the specified port is in the available list
            if filtered_ports.iter().any(|p| p.port_name == port_name) {
                Ok(())
            } else {
                // If the port doesn't exist, return an error and list all available ports
                let available_ports: Vec<String> =
                    filtered_ports.iter().map(|p| p.port_name.clone()).collect();

                Err(format!(
                    "The specified port '{}' does not exist. Available ports: {}",
                    port_name,
                    if available_ports.is_empty() {
                        "No available ports".to_string()
                    } else {
                        available_ports.join(", ")
                    }
                ))
            }
        }
        Err(e) => Err(format!("Failed to get available ports list: {}", e)),
    }
}

fn main() {
    // Initialize tracing, set log level from environment variable
    // Log level can be controlled by setting the RUST_LOG environment variable, e.g.:
    // RUST_LOG=debug, RUST_LOG=sftool_lib=trace, RUST_LOG=info
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off"));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();
    let args = Cli::parse();

    // Load config file if specified
    let config = if let Some(ref config_path) = args.config {
        match SfToolConfig::from_file(config_path) {
            Ok(cfg) => {
                if let Err(e) = cfg.validate() {
                    eprintln!("Configuration validation failed: {}", e);
                    process::exit(1);
                }
                Some(cfg)
            }
            Err(e) => {
                eprintln!("Failed to load config file '{}': {}", config_path, e);
                process::exit(1);
            }
        }
    } else {
        None
    };

    // Merge CLI args with config file, CLI args take precedence
    let (chip_type, memory_type, port, baud, before, after, connect_attempts, compat) =
        match merge_config(&args, config.clone()) {
            Ok(merged) => merged,
            Err(e) => {
                eprintln!("Configuration error: {}", e);
                process::exit(1);
            }
        };

    // On macOS, convert /dev/tty.* to /dev/cu.*
    let port = normalize_mac_port_name(&port);

    // Check if the specified serial port exists, exit early if not
    if let Err(e) = check_port_available(&port) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }

    let mut siflitool = create_sifli_tool(
        chip_type,
        SifliToolBase {
            port_name: port.clone(),
            before,
            memory_type: memory_type.to_lowercase(),
            quiet: false,
            connect_attempts,
            baud,
            compat,
        },
    );

    if baud != 1000000 {
        siflitool.set_speed(baud).unwrap();
    }

    // Determine which command to execute
    let command_source = match get_command_source(&args, config) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    let res = match command_source {
        CommandSource::Cli(command) => {
            match command {
                Commands::WriteFlash(params) => {
                    // 在CLI中解析文件信息
                    let mut files = Vec::new();
                    for file_str in params.files.iter() {
                        match sftool_lib::utils::Utils::parse_file_info(file_str) {
                            Ok(mut parsed_files) => {
                                files.append(&mut parsed_files);
                            }
                            Err(e) => {
                                eprintln!("Failed to parse file {}: {}", file_str, e);
                                std::process::exit(1);
                            }
                        }
                    }

                    let write_params = sftool_lib::WriteFlashParams {
                        files,
                        verify: params.verify,
                        no_compress: params.no_compress,
                        erase_all: params.erase_all,
                    };
                    siflitool.write_flash(&write_params)
                }
                Commands::ReadFlash(params) => {
                    // 在CLI中解析读取文件信息
                    let mut files = Vec::new();
                    for file_str in params.files.iter() {
                        match sftool_lib::utils::Utils::parse_read_file_info(file_str) {
                            Ok(parsed_file) => {
                                files.push(parsed_file);
                            }
                            Err(e) => {
                                eprintln!("Failed to parse read file {}: {}", file_str, e);
                                std::process::exit(1);
                            }
                        }
                    }

                    let read_params = sftool_lib::ReadFlashParams { files };
                    siflitool.read_flash(&read_params)
                }
                Commands::EraseFlash(params) => {
                    // 在CLI中解析擦除地址
                    let address =
                        match sftool_lib::utils::Utils::parse_erase_address(&params.address) {
                            Ok(addr) => addr,
                            Err(e) => {
                                eprintln!(
                                    "Failed to parse erase address {}: {}",
                                    params.address, e
                                );
                                std::process::exit(1);
                            }
                        };

                    let erase_params = sftool_lib::EraseFlashParams { address };
                    siflitool.erase_flash(&erase_params)
                }
                Commands::EraseRegion(params) => {
                    // 在CLI中解析擦除区域信息
                    let mut regions = Vec::new();
                    for region_str in params.region.iter() {
                        match sftool_lib::utils::Utils::parse_erase_region(region_str) {
                            Ok(parsed_region) => {
                                regions.push(parsed_region);
                            }
                            Err(e) => {
                                eprintln!("Failed to parse erase region {}: {}", region_str, e);
                                std::process::exit(1);
                            }
                        }
                    }

                    let erase_region_params = sftool_lib::EraseRegionParams { regions };
                    siflitool.erase_region(&erase_region_params)
                }
            }
        }
        CommandSource::Config(config) => execute_config_command(&config, &mut siflitool),
    };

    if let Err(e) = res {
        eprintln!("Error: {:?}", e);
    }

    if after != Operation::None {
        siflitool.soft_reset().unwrap();
    }
}
