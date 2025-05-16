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

        println!("Erase address: {}", address);

        let response = self.command(Command::EraseAll {
            address: address & 0xFF00_0000,
        })?;

        if response != Response::Ok {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to erase flash",
            ));
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
            println!("Erase region: {}:{}", address, len);

            let response = self.command(Command::Erase {
                address,
                len,
            })?;

            if response != Response::Ok {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to erase region: {}:{}", address, len),
                ));
            }
        }

        Ok(())
    }
}
