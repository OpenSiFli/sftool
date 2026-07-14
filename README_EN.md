# SFTool

A command-line utility for SiFli SoC serial tool.

[中文](README.md) | English

## Introduction

SFTool is an open-source tool specifically designed for SiFli series SoCs (System on Chips) to interact with the chips via serial interface. It supports various operations including writing data to flash memory, resetting chips, and more.

## Features

- Support for SF32LB52, SF32LB55, SF32LB56, SF32LB57, SF32LB58 chips
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

## Skills

This repository ships a single `sftool` skill that can be installed with the community `skills` CLI for Claude Code, Codex, and GitHub Copilot.

```bash
npx skills add OpenSiFli/sftool

# Install the explicit skill into a specific agent
npx skills add OpenSiFli/sftool --skill sftool -a claude-code
npx skills add OpenSiFli/sftool --skill sftool -a codex
npx skills add OpenSiFli/sftool --skill sftool -a github-copilot
```

Make sure the local machine can already run `sftool`, or set `SFTOOL_BIN=/path/to/sftool` first. The skill checks `PATH` and `SFTOOL_BIN` before doing real work; if the binary is missing, it stops immediately and tells the agent that the environment is misconfigured.

The skill covers firmware flashing, readback, `config` JSON templates, region erases, and common troubleshooting. The installable content lives in `skills/sftool/`.

## Usage

### Configuring the Serial Port  

### Basic Command Format

```bash
sftool [GLOBAL OPTIONS] <COMMAND> [COMMAND OPTIONS] [ARGS]
sftool [GLOBAL OPTIONS] config <FILE>
```

Run `sftool --help` or `sftool <COMMAND> --help` to view the help for the installed version.

### Global Options

- `-c, --chip <CHIP>`: Target chip type (currently supporting SF32LB52, SF32LB55, SF32LB56, SF32LB57, SF32LB58)
- `-m, --memory <MEMORY>`: Storage type [nor, nor_type1, nand, nand_type1, nand_nobbm_type1, sd, sd_type1] (default: nor, case-insensitive; `*_type1` is for the SF32LB58 Type1 pinout)
- `-p, --port <PORT>`: Serial port device path
- `-b, --baud <BAUD>`: Baud rate used for flashing/reading (default: 1000000)
- `--before <OPERATION>`: Operation before connecting to the chip [default_reset, no_reset, no_reset_no_sync] (default: default_reset)
- `--after <OPERATION>`: Operation after the tool completes [soft_reset, no_reset] (default: soft_reset)
- `--connect-attempts <ATTEMPTS>`: Number of connection attempts, negative or 0 means infinite (default: 3)
- `--compat <true|false>`: Compatibility mode (default: false). Set it to `true` after repeated timeout errors or verification failures.
- `--stub <STUB>`: Path to an external stub file. Both absolute paths and paths relative to the current working directory are supported. It overrides the embedded stub for the selected chip and memory type.
- `--stub-config <STUB_CONFIG_JSON>`: Apply this JSON configuration to the stub before flashing, reading, or erasing. It can also be placed after the subcommand.
- `-q, --quiet`: Suppress progress bar output.
- `-h, --help`: Print help information.
- `-V, --version`: Print the sftool version.

Global options go before the subcommand, for example:

```bash
sftool -c SF32LB52 -m nor -p /dev/ttyUSB0 --baud 1000000 write_flash app.bin@0x12020000
sftool -c SF32LB52 -p /dev/ttyUSB0 --compat true read_flash dump.bin@0x12020000:0x00100000
```

### Commands and Arguments

`config <FILE>`

Execute one command from a JSON file. `<FILE>` is the configuration path; global CLI options override values with the same name in JSON.

```bash
sftool config sftool_param.json
sftool -c SF32LB52 -p /dev/ttyUSB0 config sftool_param.json
```

The root object must contain exactly one command block: `write_flash`, `read_flash`, `erase_flash`, `erase_region`, or `stub`. Common fields are `chip`, `memory`, `port`, `baud`, `before`, `after`, `connect_attempts`, `compat`, `quiet`, and `stub_path`. See [`sftool/sftool_param_schema.json`](sftool/sftool_param_schema.json) for the complete structure.

The JSON file does not need to include every field. CLI options and defaults are merged with the JSON configuration, with CLI options taking precedence; validation fails only if required values are still missing.

`write_flash [OPTIONS] <FILE@ADDRESS>...`

```bash
# Linux/Mac
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash [OPTIONS] <FILE@ADDRESS>...
# Windows
sftool -c SF32LB52 -p COM9 write_flash [OPTIONS] <FILE@ADDRESS>...
```

Write flash options:

- `--verify`: Verify flash data after writing
- `-u, --no-compress`: Disable data compression during transmission
- `-e, --erase-all`: Erase all flash sectors before programming (not just written sectors)
- `<FILE@ADDRESS>`: Binary file and target address; `@ADDRESS` is optional when the file format contains address information. This argument can be repeated.

`read_flash <FILE@ADDRESS:SIZE>...`

Read one or more binary regions from flash. The file is the output path; address and size accept decimal or `0x` hexadecimal values.

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 read_flash dump.bin@0x12020000:0x00100000
sftool -c SF32LB52 -p COM7 read_flash boot.bin@0x12010000:0x00010000 app.bin@0x12020000:0x00200000
```

`erase_flash <ADDRESS>`

Erase the entire flash associated with the specified address.

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 erase_flash 0x12000000
```

`erase_region <ADDRESS:SIZE>...`

Erase one or more selected regions.

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 erase_region 0x12020000:0x00100000
sftool -c SF32LB52 -p /dev/ttyUSB0 erase_region 0x12010000:0x00010000 0x12020000:0x00100000
```

`stub`

Manage stub configuration embedded in AXF/ELF driver files without connecting to a chip. Choose exactly one subcommand:

- `stub write --stub-config <JSON> <FILE>...`: Write a JSON configuration to one or more AXF/ELF files.
- `stub clear <FILE>...`: Clear the stub configuration in one or more files.
- `stub read [--output <JSON>] <FILE>...`: Read and print the configuration. With `--output`, write it to a JSON file; only one input file is allowed.

```bash
sftool stub write --stub-config stub_config.json driver.axf
sftool stub clear driver.axf driver.elf
sftool stub read driver.axf
sftool stub read --output extracted_stub.json driver.axf
```

See [`sftool/stub_config_schema.json`](sftool/stub_config_schema.json) for stub fields and allowed values.

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
sftool -c SF32LB52 -p COM7 write_flash bootloader.bin@0x12010000 app.bin@0x12020000 ftab.bin@0x12000000
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
