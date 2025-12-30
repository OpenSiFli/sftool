use serde::{Deserialize, Serialize};
use sftool_lib::{AfterOperation, BeforeOperation, ChipType};

use crate::stub_config_spec::StubConfigSpec;

/// 应用程序的默认配置值
pub struct Defaults;

impl Defaults {
    pub const CHIP: &'static str = "SF32LB52";
    pub const MEMORY: &'static str = "nor";
    pub const BAUD: u32 = 1000000;
    pub const BEFORE: &'static str = "default_reset";
    pub const AFTER: &'static str = "soft_reset";
    pub const CONNECT_ATTEMPTS: i8 = 3;
    pub const COMPAT: bool = false;
}

/// 十六进制字符串，例如 "0x12000000"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexString(pub String);

impl HexString {
    pub fn to_u32(&self) -> Result<u32, String> {
        if !self.0.starts_with("0x") {
            return Err(format!("Invalid hex string format: {}", self.0));
        }

        let hex_part = &self.0[2..];
        u32::from_str_radix(hex_part, 16)
            .map_err(|e| format!("Failed to parse hex string '{}': {}", self.0, e))
    }
}

/// 写入文件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteFlashFileConfig {
    pub path: String,
    pub address: Option<HexString>,
}

/// 读取文件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFlashFileConfig {
    pub path: String,
    pub address: HexString,
    pub size: HexString,
}

/// 区域配置（用于擦除区域）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionItemConfig {
    pub address: HexString,
    pub size: HexString,
}

/// 写入 Flash 命令配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteFlashCommandConfig {
    #[serde(default)]
    pub verify: bool,
    #[serde(default)]
    pub erase_all: bool,
    #[serde(default)]
    pub no_compress: bool,
    pub files: Vec<WriteFlashFileConfig>,
}

/// 读取 Flash 命令配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFlashCommandConfig {
    pub files: Vec<ReadFlashFileConfig>,
}

/// 擦除 Flash 命令配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EraseFlashCommandConfig {
    pub address: HexString,
}

/// 擦除区域命令配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EraseRegionCommandConfig {
    pub regions: Vec<RegionItemConfig>,
}

/// 写入 stub 配置命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StubWriteCommandConfig {
    pub files: Vec<String>,
    pub config: StubConfigSpec,
}

/// 清空 stub 配置命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StubClearCommandConfig {
    pub files: Vec<String>,
}

/// 读取 stub 配置命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StubReadCommandConfig {
    pub files: Vec<String>,
    #[serde(default)]
    pub output: Option<String>,
}

/// JSON 配置文件的根结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SfToolConfig {
    #[serde(default = "default_chip")]
    pub chip: String,
    #[serde(default = "default_memory")]
    pub memory: String,
    #[serde(default)]
    pub port: String,
    #[serde(default = "default_baud")]
    pub baud: u32,
    #[serde(default = "default_before")]
    pub before: String,
    #[serde(default = "default_after")]
    pub after: String,
    #[serde(default = "default_connect_attempts")]
    pub connect_attempts: i8,
    #[serde(default)]
    pub compat: bool,
    #[serde(default)]
    pub quiet: bool,
    /// 外部 stub 文件路径，如果指定则优先使用外部文件而非内嵌文件
    #[serde(default)]
    pub stub: Option<String>,

    // 命令 - 只能存在其中一个
    pub write_flash: Option<WriteFlashCommandConfig>,
    pub read_flash: Option<ReadFlashCommandConfig>,
    pub erase_flash: Option<EraseFlashCommandConfig>,
    pub erase_region: Option<EraseRegionCommandConfig>,
    pub stub_write: Option<StubWriteCommandConfig>,
    pub stub_clear: Option<StubClearCommandConfig>,
    pub stub_read: Option<StubReadCommandConfig>,
}

// 默认值函数 - 使用统一的 Defaults 常量
fn default_chip() -> String {
    Defaults::CHIP.to_string()
}
fn default_memory() -> String {
    Defaults::MEMORY.to_string()
}
fn default_baud() -> u32 {
    Defaults::BAUD
}
fn default_before() -> String {
    Defaults::BEFORE.to_string()
}
fn default_after() -> String {
    Defaults::AFTER.to_string()
}
fn default_connect_attempts() -> i8 {
    Defaults::CONNECT_ATTEMPTS
}

impl SfToolConfig {
    /// 从 JSON 文件加载配置
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: SfToolConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 创建一个具有所有默认值的配置
    pub fn with_defaults() -> Self {
        Self {
            chip: Defaults::CHIP.to_string(), // 这将被要求用户提供
            memory: Defaults::MEMORY.to_string(),
            port: String::new(), // 这将被要求用户提供
            baud: Defaults::BAUD,
            before: Defaults::BEFORE.to_string(),
            after: Defaults::AFTER.to_string(),
            connect_attempts: Defaults::CONNECT_ATTEMPTS,
            compat: Defaults::COMPAT,
            quiet: false,
            stub: None,
            write_flash: None,
            read_flash: None,
            erase_flash: None,
            erase_region: None,
            stub_write: None,
            stub_clear: None,
            stub_read: None,
        }
    }

    /// 将字符串转换为 ChipType 枚举
    pub fn parse_chip_type(&self) -> Result<ChipType, String> {
        match self.chip.as_str() {
            "SF32LB52" => Ok(ChipType::SF32LB52),
            "SF32LB55" => Ok(ChipType::SF32LB55),
            "SF32LB56" => Ok(ChipType::SF32LB56),
            "SF32LB58" => Ok(ChipType::SF32LB58),
            _ => Err(format!("Invalid chip type: {}", self.chip)),
        }
    }

    /// 将字符串转换为 before 操作枚举
    pub fn parse_before(&self) -> Result<BeforeOperation, String> {
        match self.before.as_str() {
            "default_reset" => Ok(BeforeOperation::DefaultReset),
            "no_reset" => Ok(BeforeOperation::NoReset),
            "no_reset_no_sync" => Ok(BeforeOperation::NoResetNoSync),
            _ => Err(format!("Invalid before operation: {}", self.before)),
        }
    }

    /// 将字符串转换为 after 操作枚举
    pub fn parse_after(&self) -> Result<AfterOperation, String> {
        match self.after.as_str() {
            "no_reset" => Ok(AfterOperation::NoReset),
            "soft_reset" => Ok(AfterOperation::SoftReset),
            _ => Err(format!("Invalid after operation: {}", self.after)),
        }
    }

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<(), String> {
        // 检查是否恰好有一个命令
        let command_count = [
            self.write_flash.is_some(),
            self.read_flash.is_some(),
            self.erase_flash.is_some(),
            self.erase_region.is_some(),
            self.stub_write.is_some(),
            self.stub_clear.is_some(),
            self.stub_read.is_some(),
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        if command_count != 1 {
            return Err("Configuration must contain exactly one command (write_flash, read_flash, erase_flash, erase_region, stub_write, stub_clear, or stub_read)".to_string());
        }

        if self.stub_write.is_some() || self.stub_clear.is_some() || self.stub_read.is_some() {
            if let Some(ref stub_write) = self.stub_write {
                if stub_write.files.is_empty() {
                    return Err("stub_write.files must not be empty".to_string());
                }
            }
            if let Some(ref stub_clear) = self.stub_clear {
                if stub_clear.files.is_empty() {
                    return Err("stub_clear.files must not be empty".to_string());
                }
            }
            if let Some(ref stub_read) = self.stub_read {
                if stub_read.files.is_empty() {
                    return Err("stub_read.files must not be empty".to_string());
                }
                if stub_read.output.is_some() && stub_read.files.len() != 1 {
                    return Err("stub_read.output requires exactly one input file".to_string());
                }
            }
            return Ok(());
        }

        // 验证芯片类型
        self.parse_chip_type()?;

        // 验证操作类型
        self.parse_before()?;
        self.parse_after()?;

        // 验证内存类型
        if !["nor", "nand", "sd"].contains(&self.memory.as_str()) {
            return Err(format!(
                "Invalid memory type '{}'. Must be one of: nor, nand, sd",
                self.memory
            ));
        }

        // 验证文件路径格式中的十六进制字符串
        if let Some(ref write_flash) = self.write_flash {
            for file in &write_flash.files {
                if let Some(ref addr) = file.address {
                    addr.to_u32().map_err(|e| {
                        format!("Invalid address in write_flash file '{}': {}", file.path, e)
                    })?;
                }
            }
        }

        if let Some(ref read_flash) = self.read_flash {
            for file in &read_flash.files {
                file.address.to_u32().map_err(|e| {
                    format!("Invalid address in read_flash file '{}': {}", file.path, e)
                })?;
                file.size.to_u32().map_err(|e| {
                    format!("Invalid size in read_flash file '{}': {}", file.path, e)
                })?;
            }
        }

        if let Some(ref erase_flash) = self.erase_flash {
            erase_flash
                .address
                .to_u32()
                .map_err(|e| format!("Invalid erase_flash address: {}", e))?;
        }

        if let Some(ref erase_region) = self.erase_region {
            for region in &erase_region.regions {
                region
                    .address
                    .to_u32()
                    .map_err(|e| format!("Invalid erase_region address: {}", e))?;
                region
                    .size
                    .to_u32()
                    .map_err(|e| format!("Invalid erase_region size: {}", e))?;
            }
        }

        Ok(())
    }
}
