mod ram_stub;
pub mod reset;
pub mod speed;
pub mod write_flash;
pub mod read_flash;
pub mod erase_flash;
pub mod utils;

// 芯片特定的实现模块
pub mod sf32lb52;
pub mod sf32lb56;
pub mod sf32lb58;

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

#[derive(Clone)]
pub struct WriteFlashParams {
    pub file_path: Vec<String>,
    pub verify: bool,
    pub no_compress: bool,
    pub erase_all: bool,
}

#[derive(Clone)]
pub struct ReadFlashParams {
    pub file_path: Vec<String>,
}

#[derive(Clone)]
pub struct EraseFlashParams {
    pub address: String,
}

#[derive(Clone)]
pub struct EraseRegionParams {
    pub region: Vec<String>,
}

#[derive(Clone)]
pub enum SubcommandParams {
    WriteFlashParams(WriteFlashParams),
    ReadFlashParams(ReadFlashParams),
    EraseFlashParams(EraseFlashParams),
    EraseRegionParams(EraseRegionParams),
}

/// 核心的 SifliTool trait，定义了所有芯片实现必须提供的接口
pub trait SifliTool {
    /// 获取串口的可变引用
    fn port(&mut self) -> &mut Box<dyn SerialPort>;
    
    /// 获取基础配置的引用
    fn base(&self) -> &SifliToolBase;
    
    /// 获取当前步骤
    fn step(&self) -> i32;
    
    /// 获取当前步骤的可变引用
    fn step_mut(&mut self) -> &mut i32;
    
    /// 获取子命令参数的引用
    fn subcommand_params(&self) -> &SubcommandParams;
    
    /// 执行命令
    fn execute_command(&mut self) -> Result<(), std::io::Error>;
    
    /// High-level operations
    fn attempt_connect(&mut self) -> Result<(), std::io::Error>;
    fn download_stub_impl(&mut self) -> Result<(), std::io::Error>;
    
    /// Additional operation methods that need to be trait object safe
    fn download_stub(&mut self) -> Result<(), std::io::Error>;
    fn set_speed(&mut self, baud: u32) -> Result<(), std::io::Error>;
    fn soft_reset(&mut self) -> Result<(), std::io::Error>;
}

/// 工厂函数，根据芯片类型创建对应的 SifliTool 实现
pub fn create_sifli_tool(
    chip_type: ChipType,
    base_param: SifliToolBase,
    subcommand_params: SubcommandParams,
) -> Box<dyn SifliTool> {
    match chip_type {
        ChipType::SF32LB52 => sf32lb52::SF32LB52Tool::new(base_param, subcommand_params),
        ChipType::SF32LB56 => sf32lb56::SF32LB56Tool::new(base_param, subcommand_params),
        ChipType::SF32LB58 => sf32lb58::SF32LB58Tool::new(base_param, subcommand_params),
    }
}

    
