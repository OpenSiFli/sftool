# sftool-lib

[![Crates.io](https://img.shields.io/crates/v/sftool-lib.svg)](https://crates.io/crates/sftool-lib)
[![Documentation](https://docs.rs/sftool-lib/badge.svg)](https://docs.rs/sftool-lib)
[![License](https://img.shields.io/crates/l/sftool-lib.svg)](https://github.com/OpenSiFli/sftool/blob/main/LICENSE)

A Rust library for communicating with SiFli SoC (System on Chip) devices through serial interfaces.

[English](https://github.com/OpenSiFli/sftool/blob/main/README_EN.md) | [中文](https://github.com/OpenSiFli/sftool/blob/main/README.md)

## Features

- **Multi-chip Support**: SF32LB52, SF32LB56, SF32LB58
- **Multiple Storage Types**: NOR flash, NAND flash, and SD card
- **Configurable Serial Parameters**: Customizable baud rates and connection settings
- **Reliable Flash Operations**: Write, read, and erase with verification and compression support
- **Flexible Reset Options**: Configurable before/after operations
- **Retry Mechanism**: Configurable connection attempts with timeout handling

## Supported Chips

- **SF32LB52**: Full support for NOR/NAND flash and SD card operations
- **SF32LB56**: Support for flash operations and memory management
- **SF32LB58**: Extended support for advanced flash operations

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
sftool-lib = "0.1.7"
```

## Core Functionality

### Flash Operations

- **Write Flash**: Program binary data, Intel HEX, or ELF files to flash memory with optional verification and compression
- **Read Flash**: Read data from flash memory to backup or verify content
- **Erase Flash**: Erase specific regions or entire flash memory

### Communication Features

- **Serial Interface**: Robust serial communication with configurable baud rates and retry mechanisms
- **Protocol Support**: Implementation of SiFli-specific communication protocols for each chip type
- **Error Recovery**: Automatic retry and error handling for reliable operations

## Configuration Options

### Base Configuration

- **Port Settings**: Serial port path and communication parameters
- **Chip Selection**: Target chip type (SF32LB52, SF32LB56, SF32LB58)
- **Memory Type**: Storage type selection (NOR, NAND, SD)
- **Baud Rate**: Configurable communication speed
- **Reset Operations**: Before/after operation reset control
- **Retry Logic**: Connection attempt configuration with timeout handling

### Operation Parameters

- **Write Operations**: File paths, target addresses, verification, compression settings
- **Read Operations**: Address ranges, output file configuration
- **Erase Operations**: Address ranges and erase scope control

## File Format Support

The library automatically detects and supports various file formats:

- **Binary files** (`.bin`): Raw binary data
- **Intel HEX files** (`.hex`): Intel HEX format
- **ELF files**: Executable and Linkable Format

## Error Handling

The library provides comprehensive error handling with specific error types for different failure scenarios:

- **Connection Errors**: Serial port and communication failures
- **Flash Operation Errors**: Write, read, and erase operation failures  
- **Protocol Errors**: Communication protocol and chip response errors
- **File Format Errors**: Invalid file format or address parsing errors

## Integration

### Basic Integration

Add the library to your Rust project and configure it with your target chip and serial port settings. The library provides a simple API for all flash operations.

### CLI Integration

Enable the `cli` feature for command-line argument parsing integration:

```toml
[dependencies]
sftool-lib = { version = "0.1.7", features = ["cli"] }
```

This enables integration with `clap` for command-line tools.

## API Documentation

For detailed API documentation and usage examples, see the [online documentation](https://docs.rs/sftool-lib).

## License

This project is licensed under the Apache-2.0 License - see the [LICENSE](https://github.com/OpenSiFli/sftool/blob/main/LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Links

- [GitHub Repository](https://github.com/OpenSiFli/sftool)
- [Documentation](https://docs.rs/sftool-lib)
- [Crates.io](https://crates.io/crates/sftool-lib)
- [Command Line Tool](https://crates.io/crates/sftool)
