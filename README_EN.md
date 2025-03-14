# SFTool

A command-line utility for SiFli SoC serial tool.

[中文](README.md) | English

## Introduction

SFTool is an open-source tool specifically designed for SiFli series SoCs (System on Chips) to interact with the chips via serial interface. It supports various operations including writing data to flash memory, resetting chips, and more.

## Features

- Support for SF32LB52 chip
- Support for multiple storage types: NOR flash, NAND flash, and SD card
- Configurable serial port parameters
- Reliable flash writing functionality with verification and compression support
- Flexible reset options
- Custom connection attempt count

## Installation

### Install with Cargo

```bash
cargo install --git https://github.com/OpenSiFli/sftool
```

### Compile from Source

```bash
# Clone the repository
git clone https://github.com/OpenSiFli/sftool.git
cd sftool

# Build with Cargo
cargo build --release

# The compiled binary will be at
# ./target/release/sftool
```

## Usage

### Configuring the Serial Port  

### Basic Command Format

```bash
sftool [OPTIONS] COMMAND [COMMAND OPTIONS]
```

### Global Options

- `-c, --chip <CHIP>`: Target chip type (currently supporting SF32LB52)
- `-m, --memory <MEMORY>`: Storage type [nor, nand, sd] (default: nor)
- `-p, --port <PORT>`: Serial port device path
- `-b, --baud <BAUD>`: Baud rate used for flashing/reading (default: 1000000)
- `--before <OPERATION>`: Operation before connecting to the chip [no_reset, soft_reset] (default: no_reset)
- `--after <OPERATION>`: Operation after the tool completes [no_reset, soft_reset] (default: soft_reset)
- `--connect-attempts <ATTEMPTS>`: Number of connection attempts, negative or 0 means infinite (default: 7)
- `--compat` : Compatibility mode, should be turned on if timeout errors or verification failures occur frequently after downloading.

### Write Flash Command

```bash
# Linux/Mac
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash [OPTIONS] <FILE@ADDRESS>...
# Windows
sftool -c SF32LB52 -p COM9 write_flash [选项] <文件@地址>...
```

#### Write Flash Options

- `--verify`: Verify flash data after writing
- `-u, --no-compress`: Disable data compression during transmission
- `-e, --erase-all`: Erase all flash sectors before programming (not just written sectors)
- `<FILE@ADDRESS>`: Binary file and its target address, @ADDRESS is optional if the file format contains address information

### Examples

Linux/Mac:

```bash
# Write a single file to flash
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash app.bin@0x12020000

# Write multiple files to different addresses
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash bootloader.bin@0x12010000 app.bin@0x12020000 ftab.bin@0x12000000

# Write and verify
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash --verify app.bin@0x12020000

# Erase all flash before writing
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash -e app.bin@0x12020000
```

Windows:

```bash
# Write multiple files to different addresses
sftool -c SF32LB52 -p COM7 write_flash bootloader.bin@0x1000 app.bin@0x12010000 ftab.bin@0x12000000
# Other as above
```

## Library Usage

SFTool also provides a reusable Rust library `sftool-lib` that can be integrated into other Rust projects:

```rust
use sftool_lib::{SifliTool, SifliToolBase, WriteFlashParams};

fn main() {
    let mut tool = SifliTool::new(
        SifliToolBase {
            port_name: "/dev/ttyUSB0".to_string(),
            chip: "sf32lb52".to_string(),
            memory_type: "nor".to_string(),
            quiet: false,
        },
        Some(WriteFlashParams {
            file_path: vec!["app.bin@0x10000".to_string()],
            verify: true,
            no_compress: false,
            erase_all: false,
        }),
    );
    
    if let Err(e) = tool.write_flash() {
        eprintln!("Error: {:?}", e);
    }
}
```

## Contributing

Issues and Pull Requests are welcome!

## License

This project is licensed under the Apache-2.0 License - see the [LICENSE](LICENSE) file for details.

## Project Links

- [GitHub Repository](https://github.com/OpenSiFli/sftool)
- [Documentation](https://docs.rs/sftool)
