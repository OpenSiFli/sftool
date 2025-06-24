pub mod erase_flash;
mod ram_stub;
pub mod read_flash;
pub mod reset;
pub mod speed;
pub mod utils;
pub mod write_flash;

// 公共模块，包含可复用的逻辑
pub mod common;

// 芯片特定的实现模块
pub mod sf32lb52;
pub mod sf32lb56;
pub mod sf32lb58;

use crate::erase_flash::EraseFlashTrait;
use crate::read_flash::ReadFlashTrait;
use crate::write_flash::WriteFlashTrait;
use serialport::SerialPort;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum Operation {
    #[cfg_attr(feature = "cli", clap(name = "no_reset"))]
    None,
    #[cfg_attr(feature = "cli", clap(name = "soft_reset"))]
    SoftReset,
    #[cfg_attr(feature = "cli", clap(name = "default_reset"))]
    DefaultReset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum ChipType {
    #[cfg_attr(feature = "cli", clap(name = "SF32LB52"))]
    SF32LB52,
    #[cfg_attr(feature = "cli", clap(name = "SF32LB56"))]
    SF32LB56,
    #[cfg_attr(feature = "cli", clap(name = "SF32LB58"))]
    SF32LB58,
}

#[derive(Clone)]
pub struct SifliToolBase {
    pub port_name: String,
    pub before: Operation,
    pub memory_type: String,
    pub baud: u32,
    pub connect_attempts: i8,
    pub compat: bool,
    pub quiet: bool,
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

pub trait SifliToolTrait {
    /// 获取串口的可变引用
    fn port(&mut self) -> &mut Box<dyn SerialPort>;

    /// 获取基础配置的引用
    fn base(&self) -> &SifliToolBase;

    /// 获取当前步骤
    fn step(&self) -> i32;

    /// 获取当前步骤的可变引用
    fn step_mut(&mut self) -> &mut i32;

    fn set_speed(&mut self, baud: u32) -> Result<(), std::io::Error>;
    fn soft_reset(&mut self) -> Result<(), std::io::Error>;
}

pub trait SifliTool: SifliToolTrait + WriteFlashTrait + ReadFlashTrait + EraseFlashTrait {
    /// 工厂函数，根据芯片类型创建对应的 SifliTool 实现
    fn create_tool(base_param: SifliToolBase) -> Box<dyn SifliTool>
    where
        Self: Sized;
}

/// 工厂函数，根据芯片类型创建对应的 SifliTool 实现
pub fn create_sifli_tool(chip_type: ChipType, base_param: SifliToolBase) -> Box<dyn SifliTool> {
    match chip_type {
        ChipType::SF32LB52 => sf32lb52::SF32LB52Tool::create_tool(base_param),
        ChipType::SF32LB56 => sf32lb56::SF32LB56Tool::create_tool(base_param),
        ChipType::SF32LB58 => sf32lb58::SF32LB58Tool::create_tool(base_param),
    }
}
