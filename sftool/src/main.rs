use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use sftool_lib::{AfterOperation, BeforeOperation, ChipType, SifliToolBase, create_sifli_tool};
use strum::{Display, EnumString};

mod config;
mod progress;
mod stub_config_spec;

use config::SfToolConfig;
use progress::create_indicatif_progress_callback;
use stub_config_spec::StubConfigSpec;

type MergedConfig = (
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

/// Convert config file WriteFlashFileConfig to string format expected by CLI
fn config_write_file_to_string(file: &config::WriteFlashFileConfig) -> String {
    match &file.address {
        Some(addr) => format!("{}@{}", file.path, addr.0),
        None => file.path.clone(),
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
) -> Result<()> {
    if let Some(ref write_flash) = config.write_flash {
        // Convert config files to CLI format
        let files: Vec<String> = write_flash
            .files
            .iter()
            .map(config_write_file_to_string)
            .collect();

        // Parse files using existing logic
        let mut parsed_files = Vec::new();
        for file_str in files.iter() {
            let mut parsed = sftool_lib::utils::Utils::parse_file_info(file_str)
                .with_context(|| format!("Failed to parse file {}", file_str))?;
            parsed_files.append(&mut parsed);
        }

        let write_params = sftool_lib::WriteFlashParams {
            files: parsed_files,
            verify: write_flash.verify,
            no_compress: write_flash.no_compress,
            erase_all: write_flash.erase_all,
        };
        siflitool
            .write_flash(&write_params)
            .context("Failed to execute write_flash command")
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
            let parsed_file = sftool_lib::utils::Utils::parse_read_file_info(file_str)
                .with_context(|| format!("Failed to parse read file {}", file_str))?;
            parsed_files.push(parsed_file);
        }

        let read_params = sftool_lib::ReadFlashParams {
            files: parsed_files,
        };
        siflitool
            .read_flash(&read_params)
            .context("Failed to execute read_flash command")
    } else if let Some(ref erase_flash) = config.erase_flash {
        // Parse erase address using existing logic
        let address = sftool_lib::utils::Utils::parse_erase_address(&erase_flash.address.0)
            .with_context(|| format!("Failed to parse erase address {}", erase_flash.address.0))?;

        let erase_params = sftool_lib::EraseFlashParams { address };
        siflitool
            .erase_flash(&erase_params)
            .context("Failed to execute erase_flash command")
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
            let parsed_region = sftool_lib::utils::Utils::parse_erase_region(region_str)
                .with_context(|| format!("Failed to parse erase region {}", region_str))?;
            parsed_regions.push(parsed_region);
        }

        let erase_region_params = sftool_lib::EraseRegionParams {
            regions: parsed_regions,
        };
        siflitool
            .erase_region(&erase_region_params)
            .context("Failed to execute erase_region command")
    } else {
        bail!("No valid command found in config file.")
    }
}

fn load_stub_config_spec(path: &str) -> Result<StubConfigSpec> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read stub config file '{}'", path))?;
    let spec: StubConfigSpec =
        serde_json::from_str(&content).with_context(|| "Failed to parse stub config JSON")?;
    Ok(spec)
}

fn execute_stub_write(files: &[String], spec: &StubConfigSpec) -> Result<()> {
    let config = spec.to_stub_config().context("Invalid stub config")?;
    for file in files {
        sftool_lib::stub_config::write_stub_config_to_file(file, &config)
            .with_context(|| format!("Failed to write stub config to '{}'", file))?;
    }
    Ok(())
}

fn execute_stub_clear(files: &[String]) -> Result<()> {
    for file in files {
        sftool_lib::stub_config::clear_stub_config_in_file(file)
            .with_context(|| format!("Failed to clear stub config in '{}'", file))?;
    }
    Ok(())
}

fn execute_stub_read(files: &[String], output: Option<&str>) -> Result<()> {
    if let Some(output_path) = output {
        if files.len() != 1 {
            bail!("--output requires exactly one input file");
        }
        let config = sftool_lib::stub_config::read_stub_config_from_file(&files[0])
            .with_context(|| format!("Failed to read stub config from '{}'", files[0]))?;
        let spec = StubConfigSpec::from_stub_config(&config);
        let json = serde_json::to_string_pretty(&spec)?;
        std::fs::write(output_path, json)
            .with_context(|| format!("Failed to write stub config to '{}'", output_path))?;
        return Ok(());
    }

    if files.len() == 1 {
        let config = sftool_lib::stub_config::read_stub_config_from_file(&files[0])
            .with_context(|| format!("Failed to read stub config from '{}'", files[0]))?;
        let spec = StubConfigSpec::from_stub_config(&config);
        println!("{}", serde_json::to_string_pretty(&spec)?);
        return Ok(());
    }

    #[derive(serde::Serialize)]
    struct StubReadOutput<'a> {
        file: &'a str,
        config: StubConfigSpec,
    }

    let mut output_items = Vec::new();
    for file in files {
        let config = sftool_lib::stub_config::read_stub_config_from_file(file)
            .with_context(|| format!("Failed to read stub config from '{}'", file))?;
        let spec = StubConfigSpec::from_stub_config(&config);
        output_items.push(StubReadOutput { file, config: spec });
    }

    println!("{}", serde_json::to_string_pretty(&output_items)?);
    Ok(())
}

fn execute_stub_config_command(config: &SfToolConfig) -> Result<()> {
    if let Some(ref stub_write) = config.stub_write {
        execute_stub_write(&stub_write.files, &stub_write.config)
    } else if let Some(ref stub_clear) = config.stub_clear {
        execute_stub_clear(&stub_clear.files)
    } else if let Some(ref stub_read) = config.stub_read {
        execute_stub_read(&stub_read.files, stub_read.output.as_deref())
    } else {
        bail!("No stub command found in config file")
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
    before: Option<BeforeOperation>,

    /// What to do after siflitool is finished (default: soft_reset)
    #[arg(long = "after", value_enum)]
    after: Option<AfterOperation>,

    /// Number of attempts to connect, negative or 0 for infinite (default: 3)
    #[arg(long = "connect-attempts")]
    connect_attempts: Option<i8>,

    /// Enable compatibility mode (default: false)
    #[arg(long = "compat")]
    compat: Option<bool>,

    /// External stub file path (overrides embedded stub)
    #[arg(long = "stub")]
    stub: Option<String>,

    /// Suppress progress bar output (default: false)
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

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
    /// Manage stub config in AXF/ELF driver files
    #[command(name = "stub")]
    Stub(StubCommand),
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

#[derive(Parser, Debug, Clone)]
#[command(about = "Manage stub config in AXF/ELF driver files")]
struct StubCommand {
    #[command(subcommand)]
    action: StubAction,
}

#[derive(Subcommand, Debug, Clone)]
enum StubAction {
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
struct StubWrite {
    /// Target driver files
    #[arg(required = true)]
    files: Vec<String>,

    /// Stub config JSON file path
    #[arg(long = "stub-config")]
    stub_config: String,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Clear stub config in AXF/ELF driver files")]
struct StubClear {
    /// Target driver files
    #[arg(required = true)]
    files: Vec<String>,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Read stub config from AXF/ELF driver files")]
struct StubRead {
    /// Target driver files
    #[arg(required = true)]
    files: Vec<String>,

    /// Optional output JSON file (single input only)
    #[arg(long = "output")]
    output: Option<String>,
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
fn merge_config(args: &Cli, config: Option<SfToolConfig>) -> Result<MergedConfig> {
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
    let stub = args.stub.clone().or_else(|| base_config.stub.clone());
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
        stub,
    ))
}

/// Determine which command to execute from CLI args or config file
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum CommandSource {
    Cli(Commands),
    Config(SfToolConfig),
}

fn get_command_source(args: &Cli, config: Option<SfToolConfig>) -> Result<CommandSource> {
    match (&args.command, &config) {
        (Some(cmd), _) => Ok(CommandSource::Cli(cmd.clone())),
        (None, Some(cfg)) => Ok(CommandSource::Config(cfg.clone())),
        (None, None) => {
            bail!("No command specified. Use a subcommand or provide a config file with a command.")
        }
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
fn check_port_available(port_name: &str) -> Result<()> {
    match serialport::available_ports() {
        Ok(ports) => {
            // On macOS, only use /dev/cu.* ports, not /dev/tty.* ports
            #[cfg(target_os = "macos")]
            let filtered_ports: Vec<_> = ports
                .into_iter()
                .filter(|port| !port.port_name.starts_with("/dev/tty."))
                .collect();

            #[cfg(not(target_os = "macos"))]
            let filtered_ports: Vec<_> = ports.into_iter().collect();

            // Check if the specified port is in the available list
            if filtered_ports.iter().any(|p| p.port_name == port_name) {
                return Ok(());
            }

            // If the port doesn't exist, return an error and list all available ports
            let available_ports: Vec<String> =
                filtered_ports.iter().map(|p| p.port_name.clone()).collect();

            bail!(
                "The specified port '{}' does not exist. Available ports: {}",
                port_name,
                if available_ports.is_empty() {
                    "No available ports".to_string()
                } else {
                    available_ports.join(", ")
                }
            )
        }
        Err(e) => Err(anyhow!("Failed to get available ports list: {}", e)),
    }
}

fn main() -> Result<()> {
    // Initialize tracing, set log level from environment variable
    // Log level can be controlled by setting the RUST_LOG environment variable, e.g.:
    // RUST_LOG=debug, RUST_LOG=sftool_lib=trace, RUST_LOG=info
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off"));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();
    let args = Cli::parse();

    // Load config file if specified
    let config = if let Some(ref config_path) = args.config {
        let cfg = SfToolConfig::from_file(config_path)
            .map_err(|e| anyhow!("Failed to load config file '{}': {}", config_path, e))?;
        cfg.validate().map_err(|e| {
            anyhow!(
                "Configuration validation failed for '{}': {}",
                config_path,
                e
            )
        })?;
        Some(cfg)
    } else {
        None
    };

    // Determine which command to execute
    let command_source = get_command_source(&args, config.clone())?;

    match &command_source {
        CommandSource::Cli(Commands::Stub(stub)) => {
            match &stub.action {
                StubAction::Write(params) => {
                    let stub_spec = load_stub_config_spec(&params.stub_config)?;
                    execute_stub_write(&params.files, &stub_spec)?;
                }
                StubAction::Clear(params) => {
                    execute_stub_clear(&params.files)?;
                }
                StubAction::Read(params) => {
                    execute_stub_read(&params.files, params.output.as_deref())?;
                }
            }
            return Ok(());
        }
        CommandSource::Config(cfg) => {
            if cfg.stub_write.is_some() || cfg.stub_clear.is_some() || cfg.stub_read.is_some() {
                execute_stub_config_command(cfg)?;
                return Ok(());
            }
        }
        _ => {}
    }

    // Merge CLI args with config file, CLI args take precedence
    let (chip_type, memory_type, port, baud, before, after, connect_attempts, compat, quiet, stub) =
        merge_config(&args, config.clone()).context("Configuration error")?;

    // On macOS, convert /dev/tty.* to /dev/cu.*
    let port = normalize_mac_port_name(&port);

    // Check if the specified serial port exists, exit early if not
    check_port_available(&port)?;

    let mut siflitool = create_sifli_tool(
        chip_type,
        SifliToolBase::new_with_external_stub(
            port.clone(),
            before,
            memory_type.to_lowercase(),
            baud,
            connect_attempts,
            compat,
            if quiet {
                sftool_lib::progress::no_op_progress_callback()
            } else {
                create_indicatif_progress_callback()
            },
            stub,
        ),
    );

    if baud != 1000000 {
        siflitool
            .set_speed(baud)
            .with_context(|| format!("Failed to set baud rate to {}", baud))?;
    }

    match command_source {
        CommandSource::Cli(command) => match command {
            Commands::Stub(_) => {
                // handled earlier
            }
            Commands::WriteFlash(params) => {
                let mut files = Vec::new();
                for file_str in params.files.iter() {
                    let mut parsed_files = sftool_lib::utils::Utils::parse_file_info(file_str)
                        .with_context(|| format!("Failed to parse file {}", file_str))?;
                    files.append(&mut parsed_files);
                }

                let write_params = sftool_lib::WriteFlashParams {
                    files,
                    verify: params.verify,
                    no_compress: params.no_compress,
                    erase_all: params.erase_all,
                };
                siflitool
                    .write_flash(&write_params)
                    .context("Failed to execute write_flash command")?;
            }
            Commands::ReadFlash(params) => {
                let mut files = Vec::new();
                for file_str in params.files.iter() {
                    let parsed_file = sftool_lib::utils::Utils::parse_read_file_info(file_str)
                        .with_context(|| format!("Failed to parse read file {}", file_str))?;
                    files.push(parsed_file);
                }

                let read_params = sftool_lib::ReadFlashParams { files };
                siflitool
                    .read_flash(&read_params)
                    .context("Failed to execute read_flash command")?;
            }
            Commands::EraseFlash(params) => {
                let address = sftool_lib::utils::Utils::parse_erase_address(&params.address)
                    .with_context(|| format!("Failed to parse erase address {}", params.address))?;

                let erase_params = sftool_lib::EraseFlashParams { address };
                siflitool
                    .erase_flash(&erase_params)
                    .context("Failed to execute erase_flash command")?;
            }
            Commands::EraseRegion(params) => {
                let mut regions = Vec::new();
                for region_str in params.region.iter() {
                    let parsed_region = sftool_lib::utils::Utils::parse_erase_region(region_str)
                        .with_context(|| format!("Failed to parse erase region {}", region_str))?;
                    regions.push(parsed_region);
                }

                let erase_region_params = sftool_lib::EraseRegionParams { regions };
                siflitool
                    .erase_region(&erase_region_params)
                    .context("Failed to execute erase_region command")?;
            }
        },
        CommandSource::Config(config) => {
            execute_config_command(&config, &mut siflitool)?;
        }
    }

    if after.requires_soft_reset() {
        siflitool
            .soft_reset()
            .context("Failed to perform post-operation soft reset")?;
    }

    Ok(())
}
