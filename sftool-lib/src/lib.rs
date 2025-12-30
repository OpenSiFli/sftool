pub mod erase_flash;
mod ram_stub;
pub mod read_flash;
pub mod reset;
pub mod speed;
pub mod stub_config;
pub mod utils;
pub mod write_flash;

pub mod error;

// 进度条回调系统
pub mod progress;

// 公共模块，包含可复用的逻辑
pub mod common;

// 芯片特定的实现模块
pub mod sf32lb52;
pub mod sf32lb55;
pub mod sf32lb56;
pub mod sf32lb58;

// 重新导出 trait，使其在 crate 外部可用
pub use crate::erase_flash::EraseFlashTrait;
pub use crate::read_flash::ReadFlashTrait;
pub use crate::write_flash::WriteFlashTrait;
pub use error::{Error, Result};

use crate::progress::{ProgressCallbackArc, ProgressHelper, no_op_progress_callback};
use serialport::SerialPort;
use std::sync::Arc;

/// Load stub image bytes for the given chip and memory type.
pub fn load_stub_bytes(
    external_path: Option<&str>,
    chip_type: ChipType,
    memory_type: &str,
) -> Result<Vec<u8>> {
    let chip_key = match chip_type {
        ChipType::SF32LB52 => "sf32lb52",
        ChipType::SF32LB55 => "sf32lb55",
        ChipType::SF32LB56 => "sf32lb56",
        ChipType::SF32LB58 => "sf32lb58",
    };
    let key = format!("{}_{}", chip_key, memory_type.to_lowercase());
    let stub = ram_stub::load_stub_file(external_path, &key)?;
    Ok(stub.data.into_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum BeforeOperation {
    #[cfg_attr(feature = "cli", clap(name = "default_reset"))]
    DefaultReset,
    #[cfg_attr(feature = "cli", clap(name = "no_reset"))]
    NoReset,
    #[cfg_attr(feature = "cli", clap(name = "no_reset_no_sync"))]
    NoResetNoSync,
}

impl BeforeOperation {
    pub fn requires_reset(&self) -> bool {
        matches!(self, Self::DefaultReset)
    }

    pub fn should_download_stub(&self) -> bool {
        !matches!(self, Self::NoResetNoSync)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum AfterOperation {
    #[cfg_attr(feature = "cli", clap(name = "no_reset"))]
    NoReset,
    #[cfg_attr(feature = "cli", clap(name = "soft_reset"))]
    SoftReset,
}

impl AfterOperation {
    pub fn requires_soft_reset(&self) -> bool {
        matches!(self, Self::SoftReset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum ChipType {
    #[cfg_attr(feature = "cli", clap(name = "SF32LB52"))]
    SF32LB52,
    #[cfg_attr(feature = "cli", clap(name = "SF32LB55"))]
    SF32LB55,
    #[cfg_attr(feature = "cli", clap(name = "SF32LB56"))]
    SF32LB56,
    #[cfg_attr(feature = "cli", clap(name = "SF32LB58"))]
    SF32LB58,
}

#[derive(Clone)]
pub struct SifliToolBase {
    pub port_name: String,
    pub before: BeforeOperation,
    pub memory_type: String,
    pub baud: u32,
    pub connect_attempts: i8,
    pub compat: bool,
    pub progress_callback: ProgressCallbackArc,
    pub progress_helper: Arc<ProgressHelper>,
    /// 外部 stub 文件路径，如果指定则优先使用外部文件而非内嵌文件
    pub external_stub_path: Option<String>,
}

impl SifliToolBase {
    /// 创建一个使用默认空进度回调的 SifliToolBase
    pub fn new_with_no_progress(
        port_name: String,
        before: BeforeOperation,
        memory_type: String,
        baud: u32,
        connect_attempts: i8,
        compat: bool,
    ) -> Self {
        let progress_callback = no_op_progress_callback();
        let progress_helper = Arc::new(ProgressHelper::new(progress_callback.clone(), 0));
        Self {
            port_name,
            before,
            memory_type,
            baud,
            connect_attempts,
            compat,
            progress_callback,
            progress_helper,
            external_stub_path: None,
        }
    }

    /// 创建一个使用自定义进度回调的 SifliToolBase
    pub fn new_with_progress(
        port_name: String,
        before: BeforeOperation,
        memory_type: String,
        baud: u32,
        connect_attempts: i8,
        compat: bool,
        progress_callback: ProgressCallbackArc,
    ) -> Self {
        let progress_helper = Arc::new(ProgressHelper::new(progress_callback.clone(), 0));
        Self {
            port_name,
            before,
            memory_type,
            baud,
            connect_attempts,
            compat,
            progress_callback,
            progress_helper,
            external_stub_path: None,
        }
    }

    /// 创建一个使用自定义进度回调和外部 stub 文件的 SifliToolBase
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_external_stub(
        port_name: String,
        before: BeforeOperation,
        memory_type: String,
        baud: u32,
        connect_attempts: i8,
        compat: bool,
        progress_callback: ProgressCallbackArc,
        external_stub_path: Option<String>,
    ) -> Self {
        let progress_helper = Arc::new(ProgressHelper::new(progress_callback.clone(), 0));
        Self {
            port_name,
            before,
            memory_type,
            baud,
            connect_attempts,
            compat,
            progress_callback,
            progress_helper,
            external_stub_path,
        }
    }
}

pub struct WriteFlashParams {
    pub files: Vec<WriteFlashFile>,
    pub verify: bool,
    pub no_compress: bool,
    pub erase_all: bool,
}

#[derive(Debug)]
pub struct WriteFlashFile {
    pub address: u32,
    pub file: std::fs::File,
    pub crc32: u32,
}

pub struct ReadFlashParams {
    pub files: Vec<ReadFlashFile>,
}

#[derive(Debug)]
pub struct ReadFlashFile {
    pub file_path: String,
    pub address: u32,
    pub size: u32,
}

#[derive(Clone)]
pub struct EraseFlashParams {
    pub address: u32,
}

pub struct EraseRegionParams {
    pub regions: Vec<EraseRegionFile>,
}

#[derive(Debug)]
pub struct EraseRegionFile {
    pub address: u32,
    pub size: u32,
}

pub trait SifliToolTrait: Send + Sync {
    /// 获取串口的可变引用
    fn port(&mut self) -> &mut Box<dyn SerialPort>;

    /// 获取基础配置的引用
    fn base(&self) -> &SifliToolBase;

    /// 获取进度助手
    fn progress(&mut self) -> Arc<ProgressHelper> {
        // 使用共享的进度助手，它会自动处理步骤计数
        self.base().progress_helper.clone()
    }

    fn set_speed(&mut self, baud: u32) -> Result<()>;
    fn soft_reset(&mut self) -> Result<()>;
}

pub trait SifliTool:
    SifliToolTrait + WriteFlashTrait + ReadFlashTrait + EraseFlashTrait + Send + Sync
{
    /// 工厂函数，根据芯片类型创建对应的 SifliTool 实现
    fn create_tool(base_param: SifliToolBase) -> Box<dyn SifliTool>
    where
        Self: Sized;
}

/// 工厂函数，根据芯片类型创建对应的 SifliTool 实现
pub fn create_sifli_tool(chip_type: ChipType, base_param: SifliToolBase) -> Box<dyn SifliTool> {
    match chip_type {
        ChipType::SF32LB52 => sf32lb52::SF32LB52Tool::create_tool(base_param),
        ChipType::SF32LB55 => sf32lb55::SF32LB55Tool::create_tool(base_param),
        ChipType::SF32LB56 => sf32lb56::SF32LB56Tool::create_tool(base_param),
        ChipType::SF32LB58 => sf32lb58::SF32LB58Tool::create_tool(base_param),
    }
}
