use sftool_lib::stub_config as lib;

#[derive(Debug, serde::Deserialize)]
struct StubConfigSpec {
    #[serde(default)]
    pins: Vec<PinSpec>,
    #[serde(default)]
    flash: Vec<FlashSpec>,
    pmic: Option<PmicSpec>,
    sd0: Option<Sd0Spec>,
}

#[derive(Debug, serde::Deserialize)]
struct PinSpec {
    port: PinPortSpec,
    number: u8,
    level: PinLevelSpec,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
enum PinPortSpec {
    #[serde(rename = "PA")]
    Pa,
    #[serde(rename = "PB")]
    Pb,
    #[serde(rename = "PBR")]
    Pbr,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum PinLevelSpec {
    Low,
    High,
}

#[derive(Debug, serde::Deserialize)]
struct FlashSpec {
    media: FlashMediaSpec,
    driver_index: u8,
    manufacturer_id: u8,
    device_type: u8,
    density_id: u8,
    flags: u8,
    capacity_bytes: u64,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum FlashMediaSpec {
    Nor,
    Nand,
}

#[derive(Debug, serde::Deserialize)]
struct PmicSpec {
    disabled: bool,
    scl_port: PinPortSpec,
    scl_pin: u8,
    sda_port: PinPortSpec,
    sda_pin: u8,
    channels: Vec<PmicChannelSpec>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
enum PmicChannelSpec {
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

#[derive(Debug, serde::Deserialize)]
struct Sd0Spec {
    base_address: u64,
    pinmux: Sd0PinmuxSpec,
    init_sequence: Sd0InitSequenceSpec,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
enum Sd0PinmuxSpec {
    #[serde(rename = "clk_pa34_or_pa09")]
    ClkPa34OrPa09,
    #[serde(rename = "clk_pa60_or_pa39")]
    ClkPa60OrPa39,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
enum Sd0InitSequenceSpec {
    #[serde(rename = "emmc_then_sd")]
    EmmcThenSd,
    #[serde(rename = "sd_then_emmc")]
    SdThenEmmc,
}

impl StubConfigSpec {
    fn to_lib(&self) -> lib::StubConfig {
        let pins = self
            .pins
            .iter()
            .map(|pin| lib::PinConfig {
                port: pin.port.into(),
                number: pin.number,
                level: pin.level.into(),
            })
            .collect();

        let flash = self
            .flash
            .iter()
            .map(|entry| lib::FlashConfig {
                media: entry.media.into(),
                driver_index: entry.driver_index,
                manufacturer_id: entry.manufacturer_id,
                device_type: entry.device_type,
                density_id: entry.density_id,
                flags: entry.flags,
                capacity_bytes: entry.capacity_bytes as u32,
            })
            .collect();

        let pmic = self.pmic.as_ref().map(|pmic| lib::PmicConfig {
            disabled: pmic.disabled,
            scl_port: pmic.scl_port.into(),
            scl_pin: pmic.scl_pin,
            sda_port: pmic.sda_port.into(),
            sda_pin: pmic.sda_pin,
            channels: pmic.channels.iter().map(|c| (*c).into()).collect(),
        });

        let sd0 = self.sd0.as_ref().map(|sd0| lib::Sd0Config {
            base_address: sd0.base_address as u32,
            pinmux: sd0.pinmux.into(),
            init_sequence: sd0.init_sequence.into(),
        });

        lib::StubConfig {
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

impl From<PinLevelSpec> for lib::PinLevel {
    fn from(value: PinLevelSpec) -> Self {
        match value {
            PinLevelSpec::Low => lib::PinLevel::Low,
            PinLevelSpec::High => lib::PinLevel::High,
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

impl From<Sd0PinmuxSpec> for lib::Sd0Pinmux {
    fn from(value: Sd0PinmuxSpec) -> Self {
        match value {
            Sd0PinmuxSpec::ClkPa34OrPa09 => lib::Sd0Pinmux::ClkPa34OrPa09,
            Sd0PinmuxSpec::ClkPa60OrPa39 => lib::Sd0Pinmux::ClkPa60OrPa39,
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

fn fixture_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn load_fixture_spec() -> StubConfigSpec {
    let path = fixture_path("tests/stub_config_test.json");
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("failed to read {}", path.display()));
    serde_json::from_str(&json).expect("failed to parse stub config JSON")
}

#[test]
fn stub_config_matches_fixture() {
    let spec = load_fixture_spec();
    let expected = spec.to_lib();

    let path = fixture_path("tests/ram_patch_52X_test.bin");
    let actual = lib::read_stub_config_from_file(&path)
        .unwrap_or_else(|_| panic!("failed to read stub config from {}", path.display()));

    assert_eq!(actual, expected);
}

#[test]
fn stub_config_write_matches_fixture() {
    let spec = load_fixture_spec();
    let config = spec.to_lib();

    let source_path = fixture_path("stub/ram_patch_52X.bin");
    let mut data = std::fs::read(&source_path)
        .unwrap_or_else(|_| panic!("failed to read {}", source_path.display()));

    lib::write_stub_config_to_bytes(&mut data, &config)
        .expect("failed to write stub config into source binary");

    let expected_path = fixture_path("tests/ram_patch_52X_test.bin");
    let expected = std::fs::read(&expected_path)
        .unwrap_or_else(|_| panic!("failed to read {}", expected_path.display()));

    assert_eq!(data, expected);
}
