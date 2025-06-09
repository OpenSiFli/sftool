use clap::{Parser, Subcommand, ValueEnum};
use sftool_lib::{Operation, SifliToolBase, ChipType, create_sifli_tool};
use strum::{Display, EnumString};
use std::process;
use serialport;

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
                let available_ports: Vec<String> = filtered_ports.iter()
                    .map(|p| p.port_name.clone())
                    .collect();
                
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
        },
        Err(e) => Err(format!("Failed to get available ports list: {}", e)),
    }
}

fn main() {    // Initialize tracing, set log level from environment variable
    // Log level can be controlled by setting the RUST_LOG environment variable, e.g.:
    // RUST_LOG=debug, RUST_LOG=sftool_lib=trace, RUST_LOG=info
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off"));
    
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .init();    let mut args = Cli::parse();

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
        match args.command {
            Some(Commands::WriteFlash(ref write_flash)) => {
                sftool_lib::SubcommandParams::WriteFlashParams(sftool_lib::WriteFlashParams {
                    file_path: write_flash.files.clone(),
                    verify: write_flash.verify,
                    no_compress: write_flash.no_compress,
                    erase_all: write_flash.erase_all,
                })
            }
            Some(Commands::ReadFlash(ref read_flash)) => {
                sftool_lib::SubcommandParams::ReadFlashParams(sftool_lib::ReadFlashParams {
                    file_path: read_flash.files.clone(),
                })
            }
            Some(Commands::EraseFlash(ref erase_flash)) => {
                sftool_lib::SubcommandParams::EraseFlashParams(sftool_lib::EraseFlashParams {
                    address: erase_flash.address.clone(),
                })
            }
            Some(Commands::EraseRegion(ref erase_region)) => {
                sftool_lib::SubcommandParams::EraseRegionParams(sftool_lib::EraseRegionParams {
                    region: erase_region.region.clone(),
                })
            }
            None => {
                eprintln!("Error: No command specified");
                process::exit(1);
            }
        },
    );

    if let Err(e) = siflitool.download_stub() {
        eprintln!("Error: {:?}", e);
        process::exit(1);
    }
    
    if args.baud != 1000000 {
        siflitool.set_speed(args.baud).unwrap();
    }

    let res = siflitool.execute_command();

    if let Err(e) = res {
        eprintln!("Error: {:?}", e);
    }
    
    if args.after != Operation::None {
        siflitool.soft_reset().unwrap();
    }
}
