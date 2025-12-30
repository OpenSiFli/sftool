use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sftool_lib::stub_config as lib;
use sftool_lib::utils::Utils;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StubConfigSpec {
    #[serde(default)]
    pub pins: Vec<PinSpec>,
    #[serde(default)]
    pub flash: Vec<FlashSpec>,
    pub pmic: Option<PmicSpec>,
    pub sd0: Option<Sd0Spec>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PinPortSpec {
    #[serde(rename = "PA")]
    Pa,
    #[serde(rename = "PB")]
    Pb,
    #[serde(rename = "PBR")]
    Pbr,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinLevelSpec {
    Low,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinSpec {
    pub port: PinPortSpec,
    pub number: u8,
    pub level: PinLevelSpec,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlashMediaSpec {
    Nor,
    Nand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashSpec {
    pub media: FlashMediaSpec,
    pub driver_index: u8,
    pub manufacturer_id: u8,
    pub device_type: u8,
    pub density_id: u8,
    pub flags: u8,
    pub capacity_bytes: SizeValue,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PmicChannelSpec {
    #[serde(rename = "1v8_lvsw100_1")]
    LvSw1001,
    #[serde(rename = "1v8_lvsw100_2")]
    LvSw1002,
    #[serde(rename = "1v8_lvsw100_3")]
    LvSw1003,
    #[serde(rename = "1v8_lvsw100_4")]
    LvSw1004,
    #[serde(rename = "1v8_lvsw100_5")]
    LvSw1005,
    #[serde(rename = "vbat_hvsw150_1")]
    HvSw1501,
    #[serde(rename = "vbat_hvsw150_2")]
    HvSw1502,
    #[serde(rename = "ldo33")]
    Ldo33,
    #[serde(rename = "ldo30")]
    Ldo30,
    #[serde(rename = "ldo28")]
    Ldo28,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmicSpec {
    pub disabled: bool,
    pub scl_port: PinPortSpec,
    pub scl_pin: u8,
    pub sda_port: PinPortSpec,
    pub sda_pin: u8,
    pub channels: Vec<PmicChannelSpec>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Sd0PinmuxSpec {
    #[serde(rename = "clk_pa34_or_pa09")]
    ClkPa34OrPa09,
    #[serde(rename = "clk_pa60_or_pa39")]
    ClkPa60OrPa39,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Sd0InitSequenceSpec {
    #[serde(rename = "emmc_then_sd")]
    EmmcThenSd,
    #[serde(rename = "sd_then_emmc")]
    SdThenEmmc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sd0Spec {
    pub base_address: AddressValue,
    pub pinmux: Sd0PinmuxSpec,
    pub init_sequence: Sd0InitSequenceSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AddressValue {
    Number(u64),
    Text(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SizeValue {
    Number(u64),
    Text(String),
}

impl AddressValue {
    fn to_u32(&self, label: &str) -> Result<u32> {
        match self {
            AddressValue::Number(value) => {
                u32::try_from(*value).context(format!("{label} exceeds u32 range"))
            }
            AddressValue::Text(value) => {
                if value
                    .chars()
                    .last()
                    .map(|c| matches!(c, 'k' | 'K' | 'm' | 'M' | 'g' | 'G'))
                    .unwrap_or(false)
                {
                    anyhow::bail!("{label} does not accept k/M/G suffix");
                }
                Utils::str_to_u32(value).map_err(|e| anyhow::anyhow!("{label} parse error: {e}"))
            }
        }
    }
}

impl SizeValue {
    fn to_u32(&self, label: &str) -> Result<u32> {
        match self {
            SizeValue::Number(value) => {
                u32::try_from(*value).context(format!("{label} exceeds u32 range"))
            }
            SizeValue::Text(value) => {
                Utils::str_to_u32(value).map_err(|e| anyhow::anyhow!("{label} parse error: {e}"))
            }
        }
    }
}

impl StubConfigSpec {
    pub fn to_stub_config(&self) -> Result<lib::StubConfig> {
        if self.pins.len() > 12 {
            anyhow::bail!("pins length exceeds 12");
        }
        if self.flash.len() > 12 {
            anyhow::bail!("flash length exceeds 12");
        }

        let pins = self
            .pins
            .iter()
            .map(|pin| {
                Ok(lib::PinConfig {
                    port: pin.port.into(),
                    number: pin.number,
                    level: pin.level.into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let flash = self
            .flash
            .iter()
            .map(|entry| {
                Ok(lib::FlashConfig {
                    media: entry.media.into(),
                    driver_index: entry.driver_index,
                    manufacturer_id: entry.manufacturer_id,
                    device_type: entry.device_type,
                    density_id: entry.density_id,
                    flags: entry.flags,
                    capacity_bytes: entry.capacity_bytes.to_u32("capacity_bytes")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let pmic = match &self.pmic {
            Some(pmic) => Some(lib::PmicConfig {
                disabled: pmic.disabled,
                scl_port: pmic.scl_port.into(),
                scl_pin: pmic.scl_pin,
                sda_port: pmic.sda_port.into(),
                sda_pin: pmic.sda_pin,
                channels: pmic.channels.iter().map(|c| (*c).into()).collect(),
            }),
            None => None,
        };

        let sd0 = match &self.sd0 {
            Some(sd0) => Some(lib::Sd0Config {
                base_address: sd0.base_address.to_u32("base_address")?,
                pinmux: sd0.pinmux.into(),
                init_sequence: sd0.init_sequence.into(),
            }),
            None => None,
        };

        Ok(lib::StubConfig {
            pins,
            flash,
            pmic,
            sd0,
        })
    }

    pub fn from_stub_config(config: &lib::StubConfig) -> Self {
        let pins = config
            .pins
            .iter()
            .map(|pin| PinSpec {
                port: pin.port.into(),
                number: pin.number,
                level: pin.level.into(),
            })
            .collect();

        let flash = config
            .flash
            .iter()
            .map(|entry| FlashSpec {
                media: entry.media.into(),
                driver_index: entry.driver_index,
                manufacturer_id: entry.manufacturer_id,
                device_type: entry.device_type,
                density_id: entry.density_id,
                flags: entry.flags,
                capacity_bytes: SizeValue::Number(entry.capacity_bytes as u64),
            })
            .collect();

        let pmic = config.pmic.as_ref().map(|pmic| PmicSpec {
            disabled: pmic.disabled,
            scl_port: pmic.scl_port.into(),
            scl_pin: pmic.scl_pin,
            sda_port: pmic.sda_port.into(),
            sda_pin: pmic.sda_pin,
            channels: pmic.channels.iter().map(|c| (*c).into()).collect(),
        });

        let sd0 = config.sd0.as_ref().map(|sd0| Sd0Spec {
            base_address: AddressValue::Number(sd0.base_address as u64),
            pinmux: sd0.pinmux.into(),
            init_sequence: sd0.init_sequence.into(),
        });

        Self {
            pins,
            flash,
            pmic,
            sd0,
        }
    }
}

impl From<PinPortSpec> for lib::PinPort {
    fn from(value: PinPortSpec) -> Self {
        match value {
            PinPortSpec::Pa => lib::PinPort::Pa,
            PinPortSpec::Pb => lib::PinPort::Pb,
            PinPortSpec::Pbr => lib::PinPort::Pbr,
        }
    }
}

impl From<lib::PinPort> for PinPortSpec {
    fn from(value: lib::PinPort) -> Self {
        match value {
            lib::PinPort::Pa => PinPortSpec::Pa,
            lib::PinPort::Pb => PinPortSpec::Pb,
            lib::PinPort::Pbr => PinPortSpec::Pbr,
        }
    }
}

impl From<PinLevelSpec> for lib::PinLevel {
    fn from(value: PinLevelSpec) -> Self {
        match value {
            PinLevelSpec::Low => lib::PinLevel::Low,
            PinLevelSpec::High => lib::PinLevel::High,
        }
    }
}

impl From<lib::PinLevel> for PinLevelSpec {
    fn from(value: lib::PinLevel) -> Self {
        match value {
            lib::PinLevel::Low => PinLevelSpec::Low,
            lib::PinLevel::High => PinLevelSpec::High,
        }
    }
}

impl From<FlashMediaSpec> for lib::FlashMedia {
    fn from(value: FlashMediaSpec) -> Self {
        match value {
            FlashMediaSpec::Nor => lib::FlashMedia::Nor,
            FlashMediaSpec::Nand => lib::FlashMedia::Nand,
        }
    }
}

impl From<lib::FlashMedia> for FlashMediaSpec {
    fn from(value: lib::FlashMedia) -> Self {
        match value {
            lib::FlashMedia::Nor => FlashMediaSpec::Nor,
            lib::FlashMedia::Nand => FlashMediaSpec::Nand,
        }
    }
}

impl From<PmicChannelSpec> for lib::PmicChannel {
    fn from(value: PmicChannelSpec) -> Self {
        match value {
            PmicChannelSpec::LvSw1001 => lib::PmicChannel::LvSw1001,
            PmicChannelSpec::LvSw1002 => lib::PmicChannel::LvSw1002,
            PmicChannelSpec::LvSw1003 => lib::PmicChannel::LvSw1003,
            PmicChannelSpec::LvSw1004 => lib::PmicChannel::LvSw1004,
            PmicChannelSpec::LvSw1005 => lib::PmicChannel::LvSw1005,
            PmicChannelSpec::HvSw1501 => lib::PmicChannel::HvSw1501,
            PmicChannelSpec::HvSw1502 => lib::PmicChannel::HvSw1502,
            PmicChannelSpec::Ldo33 => lib::PmicChannel::Ldo33,
            PmicChannelSpec::Ldo30 => lib::PmicChannel::Ldo30,
            PmicChannelSpec::Ldo28 => lib::PmicChannel::Ldo28,
        }
    }
}

impl From<lib::PmicChannel> for PmicChannelSpec {
    fn from(value: lib::PmicChannel) -> Self {
        match value {
            lib::PmicChannel::LvSw1001 => PmicChannelSpec::LvSw1001,
            lib::PmicChannel::LvSw1002 => PmicChannelSpec::LvSw1002,
            lib::PmicChannel::LvSw1003 => PmicChannelSpec::LvSw1003,
            lib::PmicChannel::LvSw1004 => PmicChannelSpec::LvSw1004,
            lib::PmicChannel::LvSw1005 => PmicChannelSpec::LvSw1005,
            lib::PmicChannel::HvSw1501 => PmicChannelSpec::HvSw1501,
            lib::PmicChannel::HvSw1502 => PmicChannelSpec::HvSw1502,
            lib::PmicChannel::Ldo33 => PmicChannelSpec::Ldo33,
            lib::PmicChannel::Ldo30 => PmicChannelSpec::Ldo30,
            lib::PmicChannel::Ldo28 => PmicChannelSpec::Ldo28,
        }
    }
}

impl From<Sd0PinmuxSpec> for lib::Sd0Pinmux {
    fn from(value: Sd0PinmuxSpec) -> Self {
        match value {
            Sd0PinmuxSpec::ClkPa34OrPa09 => lib::Sd0Pinmux::ClkPa34OrPa09,
            Sd0PinmuxSpec::ClkPa60OrPa39 => lib::Sd0Pinmux::ClkPa60OrPa39,
        }
    }
}

impl From<lib::Sd0Pinmux> for Sd0PinmuxSpec {
    fn from(value: lib::Sd0Pinmux) -> Self {
        match value {
            lib::Sd0Pinmux::ClkPa34OrPa09 => Sd0PinmuxSpec::ClkPa34OrPa09,
            lib::Sd0Pinmux::ClkPa60OrPa39 => Sd0PinmuxSpec::ClkPa60OrPa39,
        }
    }
}

impl From<Sd0InitSequenceSpec> for lib::Sd0InitSequence {
    fn from(value: Sd0InitSequenceSpec) -> Self {
        match value {
            Sd0InitSequenceSpec::EmmcThenSd => lib::Sd0InitSequence::EmmcThenSd,
            Sd0InitSequenceSpec::SdThenEmmc => lib::Sd0InitSequence::SdThenEmmc,
        }
    }
}

impl From<lib::Sd0InitSequence> for Sd0InitSequenceSpec {
    fn from(value: lib::Sd0InitSequence) -> Self {
        match value {
            lib::Sd0InitSequence::EmmcThenSd => Sd0InitSequenceSpec::EmmcThenSd,
            lib::Sd0InitSequence::SdThenEmmc => Sd0InitSequenceSpec::SdThenEmmc,
        }
    }
}
