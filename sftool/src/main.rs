use anyhow::{Context, Result, anyhow};
use clap::Parser;
use sftool_lib::{SifliToolBase, create_sifli_tool};

mod cli;
mod config;
mod config_exec;
mod progress;
mod serial;
mod stub_config_spec;
mod stub_ops;

use cli::{Cli, CommandSource, Commands, StubAction, get_command_source, merge_config};
use config::SfToolConfig;
use config_exec::execute_config_command;
use progress::create_indicatif_progress_callback;
use serial::{check_port_available, normalize_port_name};
use stub_ops::{
    chip_key, execute_stub_clear, execute_stub_config_command, execute_stub_read,
    execute_stub_write, load_stub_config_spec, prepare_stub_path,
};

fn main() -> Result<()> {
    // Initialize tracing, set log level from environment variable
    // Log level can be controlled by setting the RUST_LOG environment variable, e.g.:
    // RUST_LOG=debug, RUST_LOG=sftool_lib=trace, RUST_LOG=info
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off"));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();
    let args = Cli::parse();

    // Load config file when using the config subcommand
    let config = match &args.command {
        Some(Commands::Config(params)) => {
            let cfg = SfToolConfig::from_file(&params.path)
                .map_err(|e| anyhow!("Failed to load config file '{}': {}", params.path, e))?;
            cfg.validate().map_err(|e| {
                anyhow!(
                    "Configuration validation failed for '{}': {}",
                    params.path,
                    e
                )
            })?;
            Some(cfg)
        }
        _ => None,
    };

    // Determine which command to execute
    let command_source = get_command_source(&args, config.clone())?;

    match &command_source {
        CommandSource::Cli(Commands::Stub(stub)) => {
            match &stub.action {
                StubAction::Write(params) => {
                    let stub_spec = load_stub_config_spec(&params.stub_config)?;
                    execute_stub_write(&params.files, &stub_spec)?;
                }
                StubAction::Clear(params) => {
                    execute_stub_clear(&params.files)?;
                }
                StubAction::Read(params) => {
                    execute_stub_read(&params.files, params.output.as_deref())?;
                }
            }
            return Ok(());
        }
        CommandSource::Config(cfg) => {
            if let Some(stub) = &cfg.stub
                && (stub.write.is_some() || stub.clear.is_some() || stub.read.is_some())
            {
                execute_stub_config_command(cfg)?;
                return Ok(());
            }
        }
        _ => {}
    }

    // Merge CLI args with config file, CLI args take precedence
    let (
        chip_type,
        memory_type,
        port,
        baud,
        before,
        after,
        connect_attempts,
        compat,
        quiet,
        stub_path,
    ) = merge_config(&args, config.clone()).context("Configuration error")?;

    let (stub_path, _stub_temp) = prepare_stub_path(
        args.stub_config_json.as_deref(),
        &chip_type,
        &memory_type,
        stub_path,
    )
    .with_context(|| {
        format!(
            "Failed to prepare stub with config for chip {}",
            chip_key(&chip_type)
        )
    })?;

    // On macOS, convert /dev/tty.* to /dev/cu.* ports
    let port = normalize_port_name(&port);

    // Check if the specified serial port exists, exit early if not
    check_port_available(&port)?;

    let mut siflitool = create_sifli_tool(
        chip_type,
        SifliToolBase::new_with_external_stub(
            port.clone(),
            before,
            memory_type.to_lowercase(),
            baud,
            connect_attempts,
            compat,
            if quiet {
                sftool_lib::progress::no_op_progress_callback()
            } else {
                create_indicatif_progress_callback()
            },
            stub_path,
        ),
    );

    if baud != 1000000 {
        siflitool
            .set_speed(baud)
            .with_context(|| format!("Failed to set baud rate to {}", baud))?;
    }

    match command_source {
        CommandSource::Cli(command) => match command {
            Commands::Stub(_) | Commands::Config(_) => {
                // handled earlier
            }
            Commands::WriteFlash(params) => {
                let mut files = Vec::new();
                for file_str in params.files.iter() {
                    let mut parsed_files = sftool_lib::utils::Utils::parse_file_info(file_str)
                        .with_context(|| format!("Failed to parse file {}", file_str))?;
                    files.append(&mut parsed_files);
                }

                let write_params = sftool_lib::WriteFlashParams {
                    files,
                    verify: params.verify,
                    no_compress: params.no_compress,
                    erase_all: params.erase_all,
                };
                siflitool
                    .write_flash(&write_params)
                    .context("Failed to execute write_flash command")?;
            }
            Commands::ReadFlash(params) => {
                let mut files = Vec::new();
                for file_str in params.files.iter() {
                    let parsed_file = sftool_lib::utils::Utils::parse_read_file_info(file_str)
                        .with_context(|| format!("Failed to parse read file {}", file_str))?;
                    files.push(parsed_file);
                }

                let read_params = sftool_lib::ReadFlashParams { files };
                siflitool
                    .read_flash(&read_params)
                    .context("Failed to execute read_flash command")?;
            }
            Commands::EraseFlash(params) => {
                let address = sftool_lib::utils::Utils::parse_erase_address(&params.address)
                    .with_context(|| format!("Failed to parse erase address {}", params.address))?;

                let erase_params = sftool_lib::EraseFlashParams { address };
                siflitool
                    .erase_flash(&erase_params)
                    .context("Failed to execute erase_flash command")?;
            }
            Commands::EraseRegion(params) => {
                let mut regions = Vec::new();
                for region_str in params.region.iter() {
                    let parsed_region = sftool_lib::utils::Utils::parse_erase_region(region_str)
                        .with_context(|| format!("Failed to parse erase region {}", region_str))?;
                    regions.push(parsed_region);
                }

                let erase_region_params = sftool_lib::EraseRegionParams { regions };
                siflitool
                    .erase_region(&erase_region_params)
                    .context("Failed to execute erase_region command")?;
            }
        },
        CommandSource::Config(config) => {
            execute_config_command(&config, &mut siflitool)?;
        }
    }

    if after.requires_soft_reset() {
        siflitool
            .soft_reset()
            .context("Failed to perform post-operation soft reset")?;
    }

    Ok(())
}
