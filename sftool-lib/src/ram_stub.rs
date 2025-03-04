use phf::phf_map;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "stub/"]
pub(crate) struct RamStubFile;

pub static CHIP_FILE_NAME: phf::Map<&'static str, &'static str> = phf_map! {
    "sf32lb52_nor" => "ram_patch_52X.bin",
    "sf32lb52_nand" => "ram_patch_52X_NAND.bin",
    "sf32lb52_sd" => "ram_patch_52X_SD.bin",
};