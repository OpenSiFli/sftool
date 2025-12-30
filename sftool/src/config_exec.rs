use anyhow::{Context, Result, bail};

use crate::config::SfToolConfig;

/// Execute command from config file
pub fn execute_config_command(
    config: &SfToolConfig,
    siflitool: &mut Box<dyn sftool_lib::SifliTool>,
) -> Result<()> {
    if let Some(ref write_flash) = config.write_flash {
        let mut parsed_files = Vec::new();
        for file in write_flash.files.iter() {
            let address = match &file.address {
                Some(addr) => Some(addr.to_u32().map_err(|e| {
                    anyhow::anyhow!("Invalid write_flash address '{}': {}", addr.0, e)
                })?),
                None => None,
            };
            let mut parsed = sftool_lib::utils::Utils::parse_write_file(&file.path, address)
                .with_context(|| format!("Failed to parse file {}", file.path))?;
            parsed_files.append(&mut parsed);
        }

        let write_params = sftool_lib::WriteFlashParams {
            files: parsed_files,
            verify: write_flash.verify,
            no_compress: write_flash.no_compress,
            erase_all: write_flash.erase_all,
        };
        siflitool
            .write_flash(&write_params)
            .context("Failed to execute write_flash command")
    } else if let Some(ref read_flash) = config.read_flash {
        let mut parsed_files = Vec::new();
        for file in read_flash.files.iter() {
            let address = file.address.to_u32().map_err(|e| {
                anyhow::anyhow!("Invalid read_flash address '{}': {}", file.address.0, e)
            })?;
            let size = file
                .size
                .to_u32()
                .map_err(|e| anyhow::anyhow!("Invalid read_flash size '{}': {}", file.size.0, e))?;
            let parsed_file = sftool_lib::ReadFlashFile {
                file_path: file.path.clone(),
                address,
                size,
            };
            parsed_files.push(parsed_file);
        }

        let read_params = sftool_lib::ReadFlashParams {
            files: parsed_files,
        };
        siflitool
            .read_flash(&read_params)
            .context("Failed to execute read_flash command")
    } else if let Some(ref erase_flash) = config.erase_flash {
        // Parse erase address using existing logic
        let address = sftool_lib::utils::Utils::parse_erase_address(&erase_flash.address.0)
            .with_context(|| format!("Failed to parse erase address {}", erase_flash.address.0))?;

        let erase_params = sftool_lib::EraseFlashParams { address };
        siflitool
            .erase_flash(&erase_params)
            .context("Failed to execute erase_flash command")
    } else if let Some(ref erase_region) = config.erase_region {
        let mut parsed_regions = Vec::new();
        for region in erase_region.regions.iter() {
            let address = region.address.to_u32().map_err(|e| {
                anyhow::anyhow!("Invalid erase_region address '{}': {}", region.address.0, e)
            })?;
            let size = region.size.to_u32().map_err(|e| {
                anyhow::anyhow!("Invalid erase_region size '{}': {}", region.size.0, e)
            })?;
            let parsed_region = sftool_lib::EraseRegionFile { address, size };
            parsed_regions.push(parsed_region);
        }

        let erase_region_params = sftool_lib::EraseRegionParams {
            regions: parsed_regions,
        };
        siflitool
            .erase_region(&erase_region_params)
            .context("Failed to execute erase_region command")
    } else {
        bail!("No valid command found in config file.")
    }
}
