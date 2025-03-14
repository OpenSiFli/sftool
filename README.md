# SFTool

一个用于SiFli SoC串行工具的命令行实用程序。

[English](README_EN.md) | 中文

## 简介

SFTool是一个专为SiFli系列SoC（系统芯片）设计的开源工具，用于通过串行接口与芯片进行交互。它支持多种操作，包括向闪存写入数据、重置芯片等功能。

## 特性

- 支持SF32LB52芯片
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

## 使用方法

### 基本命令格式

```bash
sftool [选项] 命令 [命令选项]
```

### 全局选项

- `-c, --chip <CHIP>`: 目标芯片类型 (目前支持SF32LB52)
- `-m, --memory <MEMORY>`: 存储类型 [nor, nand, sd] (默认: nor)
- `-p, --port <PORT>`: 串行端口设备路径
- `-b, --baud <BAUD>`: 闪存/读取时使用的串口波特率 (默认: 1000000)
- `--before <OPERATION>`: 连接芯片前的操作 [no_reset, soft_reset] (默认: no_reset)
- `--after <OPERATION>`: 工具完成后的操作 [no_reset, soft_reset] (默认: soft_reset)
- `--connect-attempts <ATTEMPTS>`: 连接尝试次数，负数或0表示无限次 (默认: 7)
- `--compat` : 兼容模式，如果经常出现超时错误或下载后校验失败，则应打开此选项。

### 写入闪存命令

```bash
# Linux/Mac
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash [选项] <文件@地址>...
# Windows
sftool -c SF32LB52 -p COM9 write_flash [选项] <文件@地址>...
```

#### 写入闪存选项

- `--verify`: 验证刚写入的闪存数据
- `-u, --no-compress`: 传输期间禁用数据压缩
- `-e, --erase-all`: 在编程前擦除所有闪存区域（不仅仅是写入区域）
- `<文件@地址>`: 二进制文件及其目标地址，如果文件格式包含地址信息，@地址部分是可选的

### 示例

Linux/Mac:

```bash
# 写入单个文件到闪存
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash app.bin@0x12020000

# 写入多个文件到不同地址
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash bootloader.bin@0x12010000 app.bin@0x12020000 ftab.bin@0x12000000

# 写入并验证
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash --verify app.bin@0x12020000

# 写入前擦除所有闪存
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash -e app.bin@0x12020000
```

Windows:

```bash
# 写入多个文件到不同地址
sftool -c SF32LB52 -p /dev/ttyUSB0 write_flash bootloader.bin@0x1000 app.bin@0x12010000 ftab.bin@0x12000000
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
