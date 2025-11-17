# sftool

[![Crates.io](https://img.shields.io/crates/v/sftool.svg)](https://crates.io/crates/sftool)
[![Documentation](https://docs.rs/sftool/badge.svg)](https://docs.rs/sftool)
[![License](https://img.shields.io/crates/l/sftool.svg)](https://github.com/OpenSiFli/sftool/blob/main/LICENSE)

A command-line utility for SiFli SoC serial communication tool.

[English](https://github.com/OpenSiFli/sftool/blob/main/README_EN.md) | [中文](https://github.com/OpenSiFli/sftool/blob/main/README.md)

## Overview

SFTool is an open-source command-line utility specifically designed for SiFli series SoCs (System on Chip). It enables communication with chips through serial interfaces, supporting various operations including writing data to flash memory, resetting chips, and more.

## Features

- **Multi-chip Support**: SF32LB52, SF32LB56, SF32LB58
- **Multiple Storage Types**: NOR flash, NAND flash, and SD card
- **Configurable Serial Parameters**: Customizable baud rates and port settings
- **Reliable Flash Operations**: Write with verification and compression support
- **Flexible Reset Options**: Configurable before/after operations
- **Cross-platform**: Works on Linux, macOS, and Windows

## Installation

### Install from Crates.io

```bash
cargo install sftool
```

### Install from Git

```bash
cargo install --git https://github.com/OpenSiFli/sftool sftool
```

### Build from Source

```bash
# Clone the repository
git clone https://github.com/OpenSiFli/sftool.git
cd sftool

# Build with Cargo
cargo build --release

# The compiled binary will be located at
# ./target/release/sftool
```

## Usage

### Basic Command Format

```bash
sftool [OPTIONS] COMMAND [COMMAND_OPTIONS]
```

### Global Options

- `-c, --chip <CHIP>`: Target chip type (currently supports SF32LB52, SF32LB56, SF32LB58)
- `-m, --memory <MEMORY>`: Memory type [nor, nand, sd] (default: nor)
- `-p, --port <PORT>`: Serial port device path
- `-b, --baud <BAUD>`: Baud rate for flash/read operations (default: 1000000)
- `--before <OPERATION>`: Operation before connecting to the chip [default_reset, no_reset, no_reset_no_sync] (default: default_reset)
- `--after <OPERATION>`: Operation after tool completion [soft_reset, no_reset] (default: soft_reset)
- `--connect-attempts <ATTEMPTS>`: Number of connection attempts, negative or 0 for infinite (default: 7)
- `--compat`: Compatibility mode, enable if you frequently encounter timeout errors or checksum failures

### Write Flash Command

```bash
# Linux/Mac
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash [OPTIONS] <file@address>...

# Windows
sftool -c SF32LB52 -p COM9 write_flash [OPTIONS] <file@address>...
```

#### Write Flash Options

- `--verify`: Verify flash data after writing
- `-u, --no-compress`: Disable data compression during transfer
- `-e, --erase-all`: Erase all flash regions before programming (not just write regions)
- `<file@address>`: Binary file and its target address. The @address part is optional if the file format contains address information

### Read Flash Command

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 read_flash <address> <size> <output_file>
```

### Erase Flash Command

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 erase_flash <address> <size>
```

## Examples

### Linux/Mac Examples

```bash
# Write single file to flash
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash app.bin@0x12020000

# Write multiple files to different addresses
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash bootloader.bin@0x12010000 app.bin@0x12020000 ftab.bin@0x12000000

# Write and verify
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash --verify app.bin@0x12020000

# Erase all flash before writing
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash -e app.bin@0x12020000

# Read flash memory
sftool -c SF32LB52 -p /dev/ttyUSB0 read_flash 0x12020000 0x100000 firmware_backup.bin

# Erase flash region
sftool -c SF32LB52 -p /dev/ttyUSB0 erase_flash 0x12020000 0x100000
```

### Windows Examples

```bash
# Write multiple files to different addresses
sftool -c SF32LB52 -p COM9 write_flash bootloader.bin@0x12010000 app.bin@0x12020000 ftab.bin@0x12000000

# Other operations are similar to Linux/Mac
```

## Supported File Formats

- **Binary files** (`.bin`): Raw binary data
- **Intel HEX files** (`.hex`): Intel HEX format
- **ELF files**: Executable and Linkable Format (address information extracted automatically)

## Troubleshooting

### Common Issues

1. **Connection timeouts**: Try enabling compatibility mode with `--compat`
2. **Permission denied on Linux/Mac**: Add your user to the dialout group or use sudo
3. **Port not found**: Check if the device is properly connected and the port path is correct
4. **Checksum failures**: Enable compatibility mode and try lower baud rates

### Getting Help

```bash
# Show general help
sftool --help

# Show help for specific command
sftool write_flash --help
```

## Library Usage

This tool is built on top of the `sftool-lib` Rust library. You can integrate the library into your own Rust projects:

```toml
[dependencies]
sftool-lib = "0.1.7"
```

See the [sftool-lib documentation](https://docs.rs/sftool-lib) for API details.

## License

This project is licensed under the Apache-2.0 License - see the [LICENSE](https://github.com/OpenSiFli/sftool/blob/main/LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit issues and Pull Requests.

## Links

- [GitHub Repository](https://github.com/OpenSiFli/sftool)
- [Documentation](https://docs.rs/sftool)
- [Crates.io](https://crates.io/crates/sftool)
- [Library Documentation](https://docs.rs/sftool-lib)
