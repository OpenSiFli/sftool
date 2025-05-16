use std::time::Duration;
use indicatif::{ProgressBar, ProgressStyle};
use crate::ram_command::{Command, RamCommand, Response};
use crate::{SifliTool, SubcommandParams, utils};

pub trait EraseFlashTrait {
    fn erase_flash(&mut self) -> Result<(), std::io::Error>;
    fn erase_region(&mut self) -> Result<(), std::io::Error>;
}

impl EraseFlashTrait for SifliTool {
    fn erase_flash(&mut self) -> Result<(), std::io::Error> {
        let SubcommandParams::EraseFlashParams(params) = self.subcommand_params.clone() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid params for erase flash",
            ));
        };

        // Convert address to u32
        let address = utils::Utils::str_to_u32(&params.address)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        let spinner = ProgressBar::new_spinner();
        if !self.base.quiet {
            spinner.enable_steady_tick(Duration::from_millis(100));
            spinner
                .set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
            spinner.set_prefix(format!("0x{:02X}", self.step));
            self.step = self.step.wrapping_add(1);
            spinner.set_message(format!("Erasing flash at 0x{:08X} ...", address));
        }

        let response = self.command(Command::EraseAll {
            address: address & 0xFF00_0000,
        })?;

        if response != Response::Ok {
            if !self.base.quiet {
                spinner.finish_with_message("Failed to erase flash");
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to erase flash",
            ));
        }
        if !self.base.quiet {
            spinner.finish_with_message("Flash erased successfully");
        }
        Ok(())
    }

    fn erase_region(&mut self) -> Result<(), std::io::Error> {
        let SubcommandParams::EraseRegionParams(params) = self.subcommand_params.clone() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid params for erase region",
            ));
        };

        let mut regions: Vec<(u32, u32)> = Vec::new();

        for region in params.region {
            // address:size
            let Some((address, len)) = region.split_once(':') else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Invalid region format: {}", region),
                ));
            };

            let address = utils::Utils::str_to_u32(address)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            let len = utils::Utils::str_to_u32(len)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            regions.push((address, len));
        }

        for (address, len) in regions {
            let spinner = ProgressBar::new_spinner();
            if !self.base.quiet {
                spinner.enable_steady_tick(Duration::from_millis(100));
                spinner
                    .set_style(ProgressStyle::with_template("[{prefix}] {spinner} {msg}").unwrap());
                spinner.set_prefix(format!("0x{:02X}", self.step));
                self.step = self.step.wrapping_add(1);
                spinner.set_message(format!("Erasing 0x{:08X} region at address 0x{:08X} ...", len, address));
            }

            let response = self.command(Command::Erase {
                address,
                len,
            })?;

            if response != Response::Ok {
                if !self.base.quiet {
                    spinner.finish_with_message(format!("Failed to erase 0x{:08X} region at address 0x{:08X}", len, address));
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to erase region: {}:{}", address, len),
                ));
            }
            if !self.base.quiet {
                spinner.finish_with_message(format!("Erasing region successfully: 0x{:08X}", address));
            }
        }

        Ok(())
    }
}
