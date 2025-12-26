use phf::phf_map;
use rust_embed::Embed;
use std::borrow::Cow;

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

/// Stub 文件数据的包装结构
pub struct StubData {
    pub data: Cow<'static, [u8]>,
}

/// 加载 stub 文件，优先使用外部文件，否则使用内嵌文件
///
/// # Arguments
/// * `external_path` - 可选的外部 stub 文件路径
/// * `chip_memory_key` - 芯片和内存类型的键，如 "sf32lb52_nor"
///
/// # Returns
/// * `Ok(StubData)` - 成功加载的 stub 数据
/// * `Err` - 加载失败
pub fn load_stub_file(
    external_path: Option<&str>,
    chip_memory_key: &str,
) -> Result<StubData, std::io::Error> {
    // 如果指定了外部文件路径，优先使用外部文件
    if let Some(path) = external_path {
        tracing::info!("Loading external stub file: {}", path);
        let data = std::fs::read(path).map_err(|e| {
            tracing::error!("Failed to read external stub file '{}': {}", path, e);
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Failed to read external stub file '{}': {}", path, e),
            )
        })?;
        tracing::debug!(
            "External stub file loaded successfully, size: {} bytes",
            data.len()
        );
        return Ok(StubData {
            data: Cow::Owned(data),
        });
    }

    // 使用内嵌文件
    tracing::debug!(
        "Looking for embedded stub file with key: {}",
        chip_memory_key
    );
    let stub_file_name = CHIP_FILE_NAME.get(chip_memory_key).ok_or_else(|| {
        tracing::error!("No stub file found for chip type: {}", chip_memory_key);
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "No stub file found for the given chip and memory type: {}",
                chip_memory_key
            ),
        )
    })?;

    tracing::debug!("Loading embedded RAM stub file: {}", stub_file_name);
    let stub = RamStubFile::get(stub_file_name).ok_or_else(|| {
        tracing::error!("Embedded stub file not found: {}", stub_file_name);
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Embedded stub file not found: {}", stub_file_name),
        )
    })?;

    tracing::debug!(
        "Embedded stub file loaded successfully, size: {} bytes",
        stub.data.len()
    );
    Ok(StubData {
        data: Cow::Owned(stub.data.to_vec()),
    })
}
