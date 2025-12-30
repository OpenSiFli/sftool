use anyhow::{Context, Result, bail};

use crate::config;
use crate::config::SfToolConfig;

/// Convert config file WriteFlashFileConfig to string format expected by CLI
fn config_write_file_to_string(file: &config::WriteFlashFileConfig) -> String {
    match &file.address {
        Some(addr) => format!("{}@{}", file.path, addr.0),
        None => file.path.clone(),
    }
}

/// Convert config file ReadFlashFileConfig to string format expected by CLI  
fn config_read_file_to_string(file: &config::ReadFlashFileConfig) -> String {
    format!("{}@{}:{}", file.path, file.address.0, file.size.0)
}

/// Convert config file RegionItemConfig to string format expected by CLI
fn config_region_to_string(region: &config::RegionItemConfig) -> String {
    format!("{}:{}", region.address.0, region.size.0)
}

/// Execute command from config file
pub fn execute_config_command(
    config: &SfToolConfig,
    siflitool: &mut Box<dyn sftool_lib::SifliTool>,
) -> Result<()> {
    if let Some(ref write_flash) = config.write_flash {
        // Convert config files to CLI format
        let files: Vec<String> = write_flash
            .files
            .iter()
            .map(config_write_file_to_string)
            .collect();

        // Parse files using existing logic
        let mut parsed_files = Vec::new();
        for file_str in files.iter() {
            let mut parsed = sftool_lib::utils::Utils::parse_file_info(file_str)
                .with_context(|| format!("Failed to parse file {}", file_str))?;
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
        // Convert config files to CLI format
        let files: Vec<String> = read_flash
            .files
            .iter()
            .map(config_read_file_to_string)
            .collect();

        // Parse files using existing logic
        let mut parsed_files = Vec::new();
        for file_str in files.iter() {
            let parsed_file = sftool_lib::utils::Utils::parse_read_file_info(file_str)
                .with_context(|| format!("Failed to parse read file {}", file_str))?;
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
        // Convert config regions to CLI format
        let regions: Vec<String> = erase_region
            .regions
            .iter()
            .map(config_region_to_string)
            .collect();

        // Parse regions using existing logic
        let mut parsed_regions = Vec::new();
        for region_str in regions.iter() {
            let parsed_region = sftool_lib::utils::Utils::parse_erase_region(region_str)
                .with_context(|| format!("Failed to parse erase region {}", region_str))?;
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
