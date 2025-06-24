use clap::{Parser, Subcommand, ValueEnum};
use serialport;
use sftool_lib::{ChipType, Operation, SifliToolBase, create_sifli_tool};
use std::io::ErrorKind;
use std::process;
use strum::{Display, EnumString};

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
    /// Target chip type
    #[arg(short = 'c', long = "chip", value_enum)]
    chip: ChipType,

    /// Memory type
    #[arg(short = 'm', long = "memory", value_enum, default_value = "nor")]
    memory: Memory,

    /// Serial port device
    #[arg(short = 'p', long = "port")]
    port: String,

    /// Serial port baud rate used when flashing/reading
    #[arg(short = 'b', long = "baud", default_value = "1000000")]
    baud: u32,

    /// What to do before connecting to the chip, `default_reset` uses DTR & RTS serial control lines
    /// to reset the chip; `soft_reset` uses a soft reset command; `no_reset` does nothing.
    #[arg(long = "before", value_enum, default_value = "default_reset")]
    before: Operation,

    /// What to do after siflitool is finished
    #[arg(long = "after", value_enum, default_value = "soft_reset")]
    after: Operation,

    /// Number of attempts to connect, negative or 0 for infinite. Default: 3.
    #[arg(long = "connect-attempts", default_value_t = 3)]
    connect_attempts: i8,

    /// Enable compatibility mode
    /// You should turn on this option if you get frequent Timeout errors or if the checksum fails after downloading.
    #[arg(long = "compat")]
    compat: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
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

#[derive(Parser, Debug)]
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

#[derive(Parser, Debug)]
#[command(about = "Read a binary blob from flash")]
struct ReadFlash {
    /// Binary file (format: <filename@address:size>)
    #[arg(required = true)]
    files: Vec<String>,
}

#[derive(Parser, Debug)]
#[command(about = "Erase flash")]
struct EraseFlash {
    /// Erase flash
    #[arg(required = true)]
    address: String,
}

#[derive(Parser, Debug)]
#[command(about = "Erase a region of the flash")]
struct EraseRegion {
    /// Erase region (format: <address:size>)
    #[arg(required = true)]
    region: Vec<String>,
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
    let mut args = Cli::parse();

    // On macOS, convert /dev/tty.* to /dev/cu.*
    args.port = normalize_mac_port_name(&args.port);

    // Check if the specified serial port exists, exit early if not
    if let Err(e) = check_port_available(&args.port) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }

    let chip_type = args.chip;

    let mut siflitool = create_sifli_tool(
        chip_type,
        SifliToolBase {
            port_name: args.port.clone(),
            before: args.before,
            memory_type: args.memory.to_string().to_lowercase(),
            quiet: false,
            connect_attempts: args.connect_attempts,
            baud: args.baud,
            compat: args.compat,
        },
    );

    if args.baud != 1000000 {
        siflitool.set_speed(args.baud).unwrap();
    }

    let res = match args.command {
        Some(Commands::WriteFlash(params)) => {
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
        Some(Commands::ReadFlash(params)) => {
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
        Some(Commands::EraseFlash(params)) => {
            // 在CLI中解析擦除地址
            let address = match sftool_lib::utils::Utils::parse_erase_address(&params.address) {
                Ok(addr) => addr,
                Err(e) => {
                    eprintln!("Failed to parse erase address {}: {}", params.address, e);
                    std::process::exit(1);
                }
            };

            let erase_params = sftool_lib::EraseFlashParams { address };
            siflitool.erase_flash(&erase_params)
        }
        Some(Commands::EraseRegion(params)) => {
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
        None => Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "No command specified. Use --help to see available commands.",
        )),
    };

    if let Err(e) = res {
        eprintln!("Error: {:?}", e);
    }

    if args.after != Operation::None {
        siflitool.soft_reset().unwrap();
    }
}
