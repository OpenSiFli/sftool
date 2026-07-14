# SFTool

一个用于SiFli SoC串行工具的命令行实用程序。

[English](README_EN.md) | 中文

## 简介

SFTool是一个专为SiFli系列SoC（系统芯片）设计的开源工具，用于通过串行接口与芯片进行交互。它支持多种操作，包括向闪存写入数据、重置芯片等功能。

## 特性

- 支持SF32LB52、SF32LB55、SF32LB56、SF32LB57、SF32LB58芯片
- 支持多种存储类型：NOR闪存、NAND闪存和SD卡
- 可配置的串口参数
- 可靠的闪存写入功能，支持验证和压缩
- 灵活的重置选项
- 自定义连接尝试次数

## 安装

### 使用 Cargo 安装

```bash
cargo install --git https://github.com/OpenSiFli/sftool
```

### 从源码编译

```bash
# 克隆仓库
git clone https://github.com/OpenSiFli/sftool.git
cd sftool

# 使用Cargo编译
cargo build --release

# 编译后的二进制文件位于
# ./target/release/sftool
```

## Skills

本仓库提供一个通用 `sftool` skill，可通过社区 `skills` CLI 安装到 Claude Code、Codex 和 GitHub Copilot。

```bash
npx skills add OpenSiFli/sftool

# 指定技能和目标 agent
npx skills add OpenSiFli/sftool --skill sftool -a claude-code
npx skills add OpenSiFli/sftool --skill sftool -a codex
npx skills add OpenSiFli/sftool --skill sftool -a github-copilot
```

使用前请确保本机已经能直接调用 `sftool`，或者设置 `SFTOOL_BIN=/path/to/sftool`。该 skill 会先检查 `PATH` 和 `SFTOOL_BIN`；如果找不到命令，会立即停止并提示 LLM 环境配置有误。

该 skill 覆盖固件刷写、读回、`config` JSON 模板、区域擦除和常见排障，安装内容位于 `skills/sftool/`。

## 使用方法

### 基本命令格式

```bash
sftool [全局选项] <命令> [命令选项] [参数]
sftool [全局选项] config <FILE>
```

运行 `sftool --help` 或 `sftool <命令> --help` 可以查看当前版本的帮助。

### 全局选项

- `-c, --chip <CHIP>`: 目标芯片类型 (目前支持SF32LB52、SF32LB55、SF32LB56、SF32LB57、SF32LB58)
- `-m, --memory <MEMORY>`: 存储类型 [nor, nor_type1, nand, nand_type1, nand_nobbm_type1, sd, sd_type1] (默认: nor，不区分大小写；`*_type1` 用于 SF32LB58 Type1 pinout)
- `-p, --port <PORT>`: 串行端口设备路径
- `-b, --baud <BAUD>`: 闪存/读取时使用的串口波特率 (默认: 1000000)
- `--before <OPERATION>`: 连接芯片前的操作 [default_reset, no_reset, no_reset_no_sync] (默认: default_reset)
- `--after <OPERATION>`: 工具完成后的操作 [soft_reset, no_reset] (默认: soft_reset)
- `--connect-attempts <ATTEMPTS>`: 连接尝试次数，负数或0表示无限次 (默认: 3)
- `--compat <true|false>`: 兼容模式 (默认: false)。如果经常出现超时错误或下载后校验失败，可设置为 `true`。
- `--stub <STUB>`: 外部 stub 文件路径，支持绝对路径或相对于当前工作目录的路径。指定后覆盖对应芯片和存储类型的内嵌 stub。
- `--stub-config <STUB_CONFIG_JSON>`: 在执行刷写、读回或擦除操作前，将 JSON 配置写入待使用的 stub。该选项也可放在子命令后。
- `-q, --quiet`: 不显示进度条。
- `-h, --help`: 显示帮助信息。
- `-V, --version`: 显示版本号。

全局选项应放在子命令之前，例如：

```bash
sftool -c SF32LB52 -m nor -p /dev/ttyUSB0 --baud 1000000 write_flash app.bin@0x12020000
sftool -c SF32LB52 -p /dev/ttyUSB0 --compat true read_flash dump.bin@0x12020000:0x00100000
```

### 命令和参数

`config <FILE>`

从 JSON 文件执行一次命令。`<FILE>` 是配置文件路径；全局 CLI 参数会覆盖 JSON 中的同名字段。

```bash
sftool config sftool_param.json
sftool -c SF32LB52 -p /dev/ttyUSB0 config sftool_param.json
```

配置根对象只能包含一个命令块：`write_flash`、`read_flash`、`erase_flash`、`erase_region` 或 `stub`。公共字段为 `chip`、`memory`、`port`、`baud`、`before`、`after`、`connect_attempts`、`compat`、`quiet` 和 `stub_path`。完整结构见 [`sftool/sftool_param_schema.json`](sftool/sftool_param_schema.json)。

JSON 文件不需要包含所有字段，CLI 参数和默认值会先与 JSON 配置合并，CLI 参数优先；合并后仍缺少必须参数时才会报错。

`write_flash [选项] <文件@地址>...`

```bash
# Linux/Mac
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash [选项] <文件@地址>...
# Windows
sftool -c SF32LB52 -p COM9 write_flash [选项] <文件@地址>...
```

写入闪存选项：

- `--verify`: 验证刚写入的闪存数据
- `-u, --no-compress`: 传输期间禁用数据压缩
- `-e, --erase-all`: 在编程前擦除所有闪存区域（不仅仅是写入区域）
- `<文件@地址>`: 二进制文件及其目标地址；如果文件格式包含地址信息，`@地址` 部分可以省略。可重复传入多个文件。

`read_flash <文件@地址:大小>...`

从闪存读出一个或多个二进制区域。`<文件@地址:大小>` 中的文件是输出路径，地址和大小支持十进制或 `0x` 十六进制格式。

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 read_flash dump.bin@0x12020000:0x00100000
sftool -c SF32LB52 -p COM7 read_flash boot.bin@0x12010000:0x00010000 app.bin@0x12020000:0x00200000
```

`erase_flash <地址>`

擦除指定地址对应的整个闪存，地址支持十进制或 `0x` 十六进制格式。

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 erase_flash 0x12000000
```

`erase_region <地址:大小>...`

擦除一个或多个指定区域，区域格式为 `<地址:大小>`。

```bash
sftool -c SF32LB52 -p /dev/ttyUSB0 erase_region 0x12020000:0x00100000
sftool -c SF32LB52 -p /dev/ttyUSB0 erase_region 0x12010000:0x00010000 0x12020000:0x00100000
```

`stub` 命令

用于修改 AXF/ELF 驱动文件中的 stub 配置，不连接芯片。三个子命令只能选择一个：

- `stub write --stub-config <JSON> <文件>...`: 将 JSON 配置写入一个或多个 AXF/ELF 文件。
- `stub clear <文件>...`: 清空一个或多个文件中的 stub 配置。
- `stub read [--output <JSON>] <文件>...`: 读取并打印配置；使用 `--output` 时将配置写入 JSON 文件，且只能提供一个输入文件。

```bash
sftool stub write --stub-config stub_config.json driver.axf
sftool stub clear driver.axf driver.elf
sftool stub read driver.axf
sftool stub read --output extracted_stub.json driver.axf
```

stub 配置的字段和可选值见 [`sftool/stub_config_schema.json`](sftool/stub_config_schema.json)。例如：

```json
{
  "pins": [{"port": "PA", "number": 10, "level": "high"}],
  "flash": [{
    "media": "nor",
    "driver_index": 0,
    "manufacturer_id": "0xef",
    "device_type": "0x40",
    "density_id": "0x18",
    "flags": 0,
    "capacity_bytes": "8M"
  }]
}
```

### 示例

Linux/Mac:

```bash
# 写入单个文件到闪存
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash app.bin@0x12020000

# 写入多个文件到不同地址
sftool -c SF32LB52 -p COM7 write_flash bootloader.bin@0x12010000 app.bin@0x12020000 ftab.bin@0x12000000

# 写入并验证
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash --verify app.bin@0x12020000

# 写入前擦除所有闪存
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash -e app.bin@0x12020000
```

Windows:

```bash
# 写入多个文件到不同地址
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash bootloader.bin@0x12010000 app.bin@0x12020000 ftab.bin@0x12000000
# 其它同上
```

## 库使用

SFTool也提供了一个可重用的Rust库 `sftool-lib`，可以集成到其他Rust项目中：

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

## 贡献

欢迎提交问题和Pull Request！

## 许可证

本项目采用Apache-2.0许可证授权 - 详情请查看[LICENSE](LICENSE)文件。

## 项目链接

- [GitHub仓库](https://github.com/OpenSiFli/sftool)
- [文档](https://docs.rs/sftool)
