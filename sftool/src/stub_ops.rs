use anyhow::{Context, Result, anyhow, bail};
use std::io::Write;

use sftool_lib::ChipType;

use crate::config::SfToolConfig;
use crate::stub_config_spec::StubConfigSpec;

pub fn load_stub_config_spec(path: &str) -> Result<StubConfigSpec> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read stub config file '{}'", path))?;
    let spec: StubConfigSpec =
        serde_json::from_str(&content).with_context(|| "Failed to parse stub config JSON")?;
    Ok(spec)
}

pub fn execute_stub_write(files: &[String], spec: &StubConfigSpec) -> Result<()> {
    let config = spec.to_stub_config().context("Invalid stub config")?;
    for file in files {
        sftool_lib::stub_config::write_stub_config_to_file(file, &config)
            .with_context(|| format!("Failed to write stub config to '{}'", file))?;
    }
    Ok(())
}

pub fn execute_stub_clear(files: &[String]) -> Result<()> {
    for file in files {
        sftool_lib::stub_config::clear_stub_config_in_file(file)
            .with_context(|| format!("Failed to clear stub config in '{}'", file))?;
    }
    Ok(())
}

pub fn execute_stub_read(files: &[String], output: Option<&str>) -> Result<()> {
    if let Some(output_path) = output {
        if files.len() != 1 {
            bail!("--output requires exactly one input file");
        }
        let config = sftool_lib::stub_config::read_stub_config_from_file(&files[0])
            .with_context(|| format!("Failed to read stub config from '{}'", files[0]))?;
        let spec = StubConfigSpec::from_stub_config(&config);
        let json = serde_json::to_string_pretty(&spec)?;
        std::fs::write(output_path, json)
            .with_context(|| format!("Failed to write stub config to '{}'", output_path))?;
        return Ok(());
    }

    if files.len() == 1 {
        let config = sftool_lib::stub_config::read_stub_config_from_file(&files[0])
            .with_context(|| format!("Failed to read stub config from '{}'", files[0]))?;
        let spec = StubConfigSpec::from_stub_config(&config);
        println!("{}", serde_json::to_string_pretty(&spec)?);
        return Ok(());
    }

    #[derive(serde::Serialize)]
    struct StubReadOutput<'a> {
        file: &'a str,
        config: StubConfigSpec,
    }

    let mut output_items = Vec::new();
    for file in files {
        let config = sftool_lib::stub_config::read_stub_config_from_file(file)
            .with_context(|| format!("Failed to read stub config from '{}'", file))?;
        let spec = StubConfigSpec::from_stub_config(&config);
        output_items.push(StubReadOutput { file, config: spec });
    }

    println!("{}", serde_json::to_string_pretty(&output_items)?);
    Ok(())
}

pub fn execute_stub_config_command(config: &SfToolConfig) -> Result<()> {
    let stub = config
        .stub
        .as_ref()
        .ok_or_else(|| anyhow!("No stub command found in config file"))?;

    if let Some(ref stub_write) = stub.write {
        execute_stub_write(&stub_write.files, &stub_write.config)
    } else if let Some(ref stub_clear) = stub.clear {
        execute_stub_clear(&stub_clear.files)
    } else if let Some(ref stub_read) = stub.read {
        execute_stub_read(&stub_read.files, stub_read.output.as_deref())
    } else {
        bail!("Stub command must contain exactly one of write, clear, or read")
    }
}

pub fn chip_key(chip_type: &ChipType) -> &'static str {
    match chip_type {
        ChipType::SF32LB52 => "sf32lb52",
        ChipType::SF32LB55 => "sf32lb55",
        ChipType::SF32LB56 => "sf32lb56",
        ChipType::SF32LB58 => "sf32lb58",
    }
}

pub fn prepare_stub_path(
    stub_config_json: Option<&str>,
    chip_type: &ChipType,
    memory_type: &str,
    stub_path: Option<String>,
) -> Result<(Option<String>, Option<tempfile::NamedTempFile>)> {
    let config_path = match stub_config_json {
        Some(path) => path,
        None => return Ok((stub_path, None)),
    };

    let spec = load_stub_config_spec(config_path)?;
    let config = spec.to_stub_config().context("Invalid stub config")?;

    let mut data =
        sftool_lib::load_stub_bytes(stub_path.as_deref(), chip_type.clone(), memory_type)
            .context("Failed to load base stub image")?;

    sftool_lib::stub_config::write_stub_config_to_bytes(&mut data, &config)
        .context("Failed to apply stub config")?;

    let mut temp_file = tempfile::Builder::new()
        .prefix(&format!(
            "sftool_stub_{}_{}_",
            chip_key(chip_type),
            memory_type
        ))
        .suffix(".bin")
        .tempfile()
        .context("Failed to create temp stub file")?;
    temp_file
        .write_all(&data)
        .context("Failed to write stub into temp file")?;

    let path = temp_file.path().to_string_lossy().to_string();
    Ok((Some(path), Some(temp_file)))
}
