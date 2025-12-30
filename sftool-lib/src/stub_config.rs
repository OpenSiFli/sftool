//! Stub/driver config block handling for AXF/ELF files.
//!
//! This module focuses only on locating, reading, and overwriting the
//! fixed-size T_EXT_DRIVER_CFG block inside a file. It does not handle
//! encrypted ram_patch images (imgtool) or any CLI parsing concerns.

use crate::{Error, Result};
use std::path::Path;

const MAGIC_FLAG: u32 = 0xABCDDBCA;
const VERSION_FLAG: u32 = 0xFFFF0003;
const PIN_CFG_COUNT: usize = 12;
const FLASH_CFG_COUNT: usize = 12;
const PMIC_CHANNEL_COUNT: usize = 10;

pub const DRIVER_CONFIG_SIZE: usize = 236;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubConfig {
    pub pins: Vec<PinConfig>,
    pub flash: Vec<FlashConfig>,
    pub pmic: Option<PmicConfig>,
    pub sd0: Option<Sd0Config>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinPort {
    Pa,
    Pb,
    Pbr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinLevel {
    Low,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinConfig {
    pub port: PinPort,
    pub number: u8,
    pub level: PinLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashMedia {
    Nor,
    Nand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashConfig {
    pub media: FlashMedia,
    pub driver_index: u8,
    pub manufacturer_id: u8,
    pub device_type: u8,
    pub density_id: u8,
    pub flags: u8,
    pub capacity_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmicChannel {
    LvSw1001,
    LvSw1002,
    LvSw1003,
    LvSw1004,
    LvSw1005,
    HvSw1501,
    HvSw1502,
    Ldo33,
    Ldo30,
    Ldo28,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmicConfig {
    pub disabled: bool,
    pub scl_port: PinPort,
    pub scl_pin: u8,
    pub sda_port: PinPort,
    pub sda_pin: u8,
    pub channels: Vec<PmicChannel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sd0Pinmux {
    ClkPa34OrPa09,
    ClkPa60OrPa39,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sd0InitSequence {
    EmmcThenSd,
    SdThenEmmc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sd0Config {
    pub base_address: u32,
    pub pinmux: Sd0Pinmux,
    pub init_sequence: Sd0InitSequence,
}

/// Scan for the first valid driver config block and return its offset.
pub fn find_stub_config_offset(data: &[u8]) -> Option<usize> {
    if data.len() < DRIVER_CONFIG_SIZE {
        return None;
    }
    let last = data.len() - DRIVER_CONFIG_SIZE;
    for offset in 0..=last {
        let header = read_u32_le(data, offset)?;
        if header != MAGIC_FLAG {
            continue;
        }
        let version = read_u32_le(data, offset + 4)?;
        if version != VERSION_FLAG {
            continue;
        }
        let tail = read_u32_le(data, offset + DRIVER_CONFIG_SIZE - 4)?;
        if tail == MAGIC_FLAG {
            return Some(offset);
        }
    }
    None
}

/// Locate and parse a driver config block from raw bytes.
pub fn read_stub_config_from_bytes(data: &[u8]) -> Result<StubConfig> {
    let offset = find_stub_config_offset(data)
        .ok_or_else(|| Error::invalid_input("driver config block not found"))?;
    read_stub_config_at(data, offset)
}

/// Locate and overwrite a driver config block inside a byte buffer.
pub fn write_stub_config_to_bytes(data: &mut [u8], config: &StubConfig) -> Result<()> {
    let offset = find_stub_config_offset(data)
        .ok_or_else(|| Error::invalid_input("driver config block not found"))?;
    write_stub_config_at(data, offset, config)
}

/// Clear the config by writing an empty block (masks set to zero).
pub fn clear_stub_config_in_bytes(data: &mut [u8]) -> Result<()> {
    let empty = StubConfig {
        pins: Vec::new(),
        flash: Vec::new(),
        pmic: None,
        sd0: None,
    };
    write_stub_config_to_bytes(data, &empty)
}

/// Read and parse a driver config block from a file.
pub fn read_stub_config_from_file<P: AsRef<Path>>(path: P) -> Result<StubConfig> {
    let data = std::fs::read(path)?;
    read_stub_config_from_bytes(&data)
}

/// Overwrite a driver config block in a file.
pub fn write_stub_config_to_file<P: AsRef<Path>>(path: P, config: &StubConfig) -> Result<()> {
    let path = path.as_ref();
    let mut data = std::fs::read(path)?;
    write_stub_config_to_bytes(&mut data, config)?;
    std::fs::write(path, data)?;
    Ok(())
}

/// Clear the config block in a file by writing an empty block.
pub fn clear_stub_config_in_file<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    let mut data = std::fs::read(path)?;
    clear_stub_config_in_bytes(&mut data)?;
    std::fs::write(path, data)?;
    Ok(())
}

/// Parse a driver config block at the given offset.
pub fn read_stub_config_at(data: &[u8], offset: usize) -> Result<StubConfig> {
    if data.len() < offset + DRIVER_CONFIG_SIZE {
        return Err(Error::invalid_input("buffer too small for driver config"));
    }

    let pin_mask = read_u16_le(data, offset + 8)
        .ok_or_else(|| Error::invalid_input("failed to read pin_mask from driver config"))?;
    let flash_mask = read_u16_le(data, offset + 10)
        .ok_or_else(|| Error::invalid_input("failed to read flash_mask from driver config"))?;
    let pmic_mask = read_u8_required(data, offset + 12, "pmic_mask from driver config")?;
    let sd0_mask = read_u8_required(data, offset + 13, "sd0_mask from driver config")?;

    let mut pins = Vec::new();
    let pin_base = offset + 16;
    for index in 0..PIN_CFG_COUNT {
        if (pin_mask & (1 << index)) == 0 {
            continue;
        }
        let entry_offset = pin_base + index * 4;
        let port = PinPort::try_from(read_u8_required(data, entry_offset, "pin port")?)?;
        let number = read_u8_required(data, entry_offset + 1, "pin number")?;
        let level = PinLevel::try_from(read_u8_required(data, entry_offset + 2, "pin level")?)?;
        pins.push(PinConfig {
            port,
            number,
            level,
        });
    }

    let mut flash = Vec::new();
    let flash_base = pin_base + PIN_CFG_COUNT * 4;
    for index in 0..FLASH_CFG_COUNT {
        if (flash_mask & (1 << index)) == 0 {
            continue;
        }
        let entry_offset = flash_base + index * 12;
        let media = FlashMedia::try_from(read_u8_required(data, entry_offset, "flash media")?)?;
        let driver_index = read_u8_required(data, entry_offset + 1, "flash driver index")?;
        let manufacturer_id = read_u8_required(data, entry_offset + 4, "flash manufacturer_id")?;
        let device_type = read_u8_required(data, entry_offset + 5, "flash device_type")?;
        let density_id = read_u8_required(data, entry_offset + 6, "flash density_id")?;
        let flags = read_u8_required(data, entry_offset + 7, "flash flags")?;
        let capacity_bytes = read_u32_le(data, entry_offset + 8).ok_or_else(|| {
            Error::invalid_input("failed to read flash capacity from driver config")
        })?;
        flash.push(FlashConfig {
            media,
            driver_index,
            manufacturer_id,
            device_type,
            density_id,
            flags,
            capacity_bytes,
        });
    }

    let pmic_offset = flash_base + FLASH_CFG_COUNT * 12;
    let pmic = if pmic_mask != 0 {
        let disabled = read_u8_required(data, pmic_offset, "pmic disabled")? != 0;
        let scl_port =
            PinPort::try_from(read_u8_required(data, pmic_offset + 1, "pmic scl_port")?)?;
        let scl_pin = read_u8_required(data, pmic_offset + 2, "pmic scl_pin")?;
        let sda_port =
            PinPort::try_from(read_u8_required(data, pmic_offset + 3, "pmic sda_port")?)?;
        let sda_pin = read_u8_required(data, pmic_offset + 4, "pmic sda_pin")?;
        let mut channels = Vec::new();
        for idx in 0..PMIC_CHANNEL_COUNT {
            let value = read_u8_required(data, pmic_offset + 6 + idx, "pmic channel")?;
            if value != 0 {
                channels.push(PmicChannel::try_from(idx as u8)?);
            }
        }
        Some(PmicConfig {
            disabled,
            scl_port,
            scl_pin,
            sda_port,
            sda_pin,
            channels,
        })
    } else {
        None
    };

    let sd0_offset = pmic_offset + 16;
    let sd0 = if sd0_mask != 0 {
        let base_address = read_u32_le(data, sd0_offset).ok_or_else(|| {
            Error::invalid_input("failed to read sd0 base address from driver config")
        })?;
        let pinmux = Sd0Pinmux::try_from(read_u8_required(data, sd0_offset + 4, "sd0 pinmux")?)?;
        let init_sequence = Sd0InitSequence::try_from(read_u8_required(
            data,
            sd0_offset + 5,
            "sd0 init sequence",
        )?)?;
        Some(Sd0Config {
            base_address,
            pinmux,
            init_sequence,
        })
    } else {
        None
    };

    Ok(StubConfig {
        pins,
        flash,
        pmic,
        sd0,
    })
}

/// Overwrite a driver config block at the given offset.
pub fn write_stub_config_at(data: &mut [u8], offset: usize, config: &StubConfig) -> Result<()> {
    if data.len() < offset + DRIVER_CONFIG_SIZE {
        return Err(Error::invalid_input("buffer too small for driver config"));
    }

    if config.pins.len() > PIN_CFG_COUNT {
        return Err(Error::invalid_input("pin entries exceed 12"));
    }
    if config.flash.len() > FLASH_CFG_COUNT {
        return Err(Error::invalid_input("flash entries exceed 12"));
    }

    let block = build_stub_config_block(config)?;
    let target = &mut data[offset..offset + DRIVER_CONFIG_SIZE];
    target.copy_from_slice(&block);
    Ok(())
}

// Build a serialized driver config block with fixed size and masks.
fn build_stub_config_block(config: &StubConfig) -> Result<Vec<u8>> {
    let pin_mask: u16 = if config.pins.is_empty() {
        0
    } else {
        (1u16 << config.pins.len()) - 1
    };
    let flash_mask: u16 = if config.flash.is_empty() {
        0
    } else {
        (1u16 << config.flash.len()) - 1
    };
    let pmic_mask: u8 = if config.pmic.is_some() { 1 } else { 0 };
    let sd0_mask: u8 = if config.sd0.is_some() { 1 } else { 0 };

    let mut buf = Vec::with_capacity(DRIVER_CONFIG_SIZE);
    push_u32(&mut buf, MAGIC_FLAG);
    push_u32(&mut buf, VERSION_FLAG);
    push_u16(&mut buf, pin_mask);
    push_u16(&mut buf, flash_mask);
    push_u8(&mut buf, pmic_mask);
    push_u8(&mut buf, sd0_mask);
    push_u8(&mut buf, 0);
    push_u8(&mut buf, 0);

    for i in 0..PIN_CFG_COUNT {
        if let Some(entry) = config.pins.get(i) {
            push_u8(&mut buf, u8::from(entry.port));
            push_u8(&mut buf, entry.number);
            push_u8(&mut buf, u8::from(entry.level));
            push_u8(&mut buf, 0);
        } else {
            buf.extend_from_slice(&[0u8; 4]);
        }
    }

    for i in 0..FLASH_CFG_COUNT {
        if let Some(entry) = config.flash.get(i) {
            push_u8(&mut buf, u8::from(entry.media));
            push_u8(&mut buf, entry.driver_index);
            push_u8(&mut buf, 0);
            push_u8(&mut buf, 0);
            push_u8(&mut buf, entry.manufacturer_id);
            push_u8(&mut buf, entry.device_type);
            push_u8(&mut buf, entry.density_id);
            push_u8(&mut buf, entry.flags);
            push_u32(&mut buf, entry.capacity_bytes);
        } else {
            buf.extend_from_slice(&[0u8; 12]);
        }
    }

    if let Some(pmic) = &config.pmic {
        push_u8(&mut buf, if pmic.disabled { 1 } else { 0 });
        push_u8(&mut buf, u8::from(pmic.scl_port));
        push_u8(&mut buf, pmic.scl_pin);
        push_u8(&mut buf, u8::from(pmic.sda_port));
        push_u8(&mut buf, pmic.sda_pin);
        push_u8(&mut buf, 0);

        let mut channel_bytes = [0u8; PMIC_CHANNEL_COUNT];
        for channel in &pmic.channels {
            let index = channel.index();
            if channel_bytes[index] != 0 {
                return Err(Error::invalid_input("duplicate PMIC channel"));
            }
            channel_bytes[index] = 1;
        }
        buf.extend_from_slice(&channel_bytes);
    } else {
        buf.extend_from_slice(&[0u8; 16]);
    }

    if let Some(sd0) = &config.sd0 {
        push_u32(&mut buf, sd0.base_address);
        push_u8(&mut buf, u8::from(sd0.pinmux));
        push_u8(&mut buf, u8::from(sd0.init_sequence));
        push_u8(&mut buf, 0);
        push_u8(&mut buf, 0);
    } else {
        buf.extend_from_slice(&[0u8; 8]);
    }

    push_u32(&mut buf, MAGIC_FLAG);

    if buf.len() != DRIVER_CONFIG_SIZE {
        return Err(Error::invalid_input("driver config block size mismatch"));
    }
    Ok(buf)
}

// Append a u8 to the buffer.
fn push_u8(buf: &mut Vec<u8>, value: u8) {
    buf.push(value);
}

// Append a little-endian u16 to the buffer.
fn push_u16(buf: &mut Vec<u8>, value: u16) {
    buf.extend_from_slice(&value.to_le_bytes());
}

// Append a little-endian u32 to the buffer.
fn push_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_le_bytes());
}

// Read a u8 at the given offset.
fn read_u8(data: &[u8], offset: usize) -> Option<u8> {
    data.get(offset).copied()
}

// Read a u8 and return a labeled error if missing.
fn read_u8_required(data: &[u8], offset: usize, label: &str) -> Result<u8> {
    read_u8(data, offset).ok_or_else(|| Error::invalid_input(format!("failed to read {}", label)))
}

// Read a little-endian u16 at the given offset.
fn read_u16_le(data: &[u8], offset: usize) -> Option<u16> {
    let bytes = data.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

// Read a little-endian u32 at the given offset.
fn read_u32_le(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

impl PinPort {
    // Convert the port enum to its on-wire value.
    fn to_u8(self) -> u8 {
        match self {
            PinPort::Pa => 0,
            PinPort::Pb => 1,
            PinPort::Pbr => 2,
        }
    }
}

impl From<PinPort> for u8 {
    // Convert the port enum to u8 for serialization.
    fn from(value: PinPort) -> Self {
        value.to_u8()
    }
}

impl TryFrom<u8> for PinPort {
    type Error = Error;

    // Parse the port enum from its on-wire value.
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(PinPort::Pa),
            1 => Ok(PinPort::Pb),
            2 => Ok(PinPort::Pbr),
            _ => Err(Error::invalid_input("invalid pin port value")),
        }
    }
}

impl PinLevel {
    // Convert the level enum to its on-wire value.
    fn to_u8(self) -> u8 {
        match self {
            PinLevel::Low => 0,
            PinLevel::High => 1,
        }
    }
}

impl From<PinLevel> for u8 {
    // Convert the level enum to u8 for serialization.
    fn from(value: PinLevel) -> Self {
        value.to_u8()
    }
}

impl TryFrom<u8> for PinLevel {
    type Error = Error;

    // Parse the level enum from its on-wire value.
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(PinLevel::Low),
            1 => Ok(PinLevel::High),
            _ => Err(Error::invalid_input("invalid pin level value")),
        }
    }
}

impl FlashMedia {
    // Convert the media enum to its on-wire value.
    fn to_u8(self) -> u8 {
        match self {
            FlashMedia::Nor => 0,
            FlashMedia::Nand => 1,
        }
    }
}

impl From<FlashMedia> for u8 {
    // Convert the media enum to u8 for serialization.
    fn from(value: FlashMedia) -> Self {
        value.to_u8()
    }
}

impl TryFrom<u8> for FlashMedia {
    type Error = Error;

    // Parse the media enum from its on-wire value.
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(FlashMedia::Nor),
            1 => Ok(FlashMedia::Nand),
            _ => Err(Error::invalid_input("invalid flash media value")),
        }
    }
}

impl PmicChannel {
    // Map a channel to its fixed array index.
    fn index(self) -> usize {
        match self {
            PmicChannel::LvSw1001 => 0,
            PmicChannel::LvSw1002 => 1,
            PmicChannel::LvSw1003 => 2,
            PmicChannel::LvSw1004 => 3,
            PmicChannel::LvSw1005 => 4,
            PmicChannel::HvSw1501 => 5,
            PmicChannel::HvSw1502 => 6,
            PmicChannel::Ldo33 => 7,
            PmicChannel::Ldo30 => 8,
            PmicChannel::Ldo28 => 9,
        }
    }
}

impl TryFrom<u8> for PmicChannel {
    type Error = Error;

    // Parse the channel from its index.
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(PmicChannel::LvSw1001),
            1 => Ok(PmicChannel::LvSw1002),
            2 => Ok(PmicChannel::LvSw1003),
            3 => Ok(PmicChannel::LvSw1004),
            4 => Ok(PmicChannel::LvSw1005),
            5 => Ok(PmicChannel::HvSw1501),
            6 => Ok(PmicChannel::HvSw1502),
            7 => Ok(PmicChannel::Ldo33),
            8 => Ok(PmicChannel::Ldo30),
            9 => Ok(PmicChannel::Ldo28),
            _ => Err(Error::invalid_input("invalid PMIC channel index")),
        }
    }
}

impl Sd0Pinmux {
    // Convert the pinmux enum to its on-wire value.
    fn to_u8(self) -> u8 {
        match self {
            Sd0Pinmux::ClkPa34OrPa09 => 0,
            Sd0Pinmux::ClkPa60OrPa39 => 1,
        }
    }
}

impl From<Sd0Pinmux> for u8 {
    // Convert the pinmux enum to u8 for serialization.
    fn from(value: Sd0Pinmux) -> Self {
        value.to_u8()
    }
}

impl TryFrom<u8> for Sd0Pinmux {
    type Error = Error;

    // Parse the pinmux enum from its on-wire value.
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Sd0Pinmux::ClkPa34OrPa09),
            1 => Ok(Sd0Pinmux::ClkPa60OrPa39),
            _ => Err(Error::invalid_input("invalid SD0 pinmux value")),
        }
    }
}

impl Sd0InitSequence {
    // Convert the init sequence enum to its on-wire value.
    fn to_u8(self) -> u8 {
        match self {
            Sd0InitSequence::EmmcThenSd => 0,
            Sd0InitSequence::SdThenEmmc => 1,
        }
    }
}

impl From<Sd0InitSequence> for u8 {
    // Convert the init sequence enum to u8 for serialization.
    fn from(value: Sd0InitSequence) -> Self {
        value.to_u8()
    }
}

impl TryFrom<u8> for Sd0InitSequence {
    type Error = Error;

    // Parse the init sequence enum from its on-wire value.
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Sd0InitSequence::EmmcThenSd),
            1 => Ok(Sd0InitSequence::SdThenEmmc),
            _ => Err(Error::invalid_input("invalid SD0 init sequence value")),
        }
    }
}
