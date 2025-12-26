use phf::phf_map;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "stub/"]
pub(crate) struct RamStubFile;

pub static CHIP_FILE_NAME: phf::Map<&'static str, &'static str> = phf_map! {
    "sf32lb52_nor" => "ram_patch_52X.bin",
    "sf32lb52_nand" => "ram_patch_52X_NAND.bin",
    "sf32lb52_sd" => "ram_patch_52X_SD.bin",
    "sf32lb55_nor" => "ram_patch_55X.bin",
    "sf32lb55_sd" => "ram_patch_55X_SD.bin",
    "sf32lb56_nor" => "ram_patch_56X.bin",
    "sf32lb56_nand" => "ram_patch_56X_NAND.bin",
    "sf32lb56_sd" => "ram_patch_56X_SD.bin",
    "sf32lb58_nor" => "ram_patch_58x.bin",
    "sf32lb58_nand" => "ram_patch_58X_NAND.bin",
    "sf32lb58_sd" => "ram_patch_SD.bin",
};

// 签名公钥文件常量
pub static SIG_PUB_FILE: &str = "58X_sig_pub.der";
