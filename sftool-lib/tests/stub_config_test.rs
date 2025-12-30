use sftool_lib::stub_config::*;

#[test]
fn roundtrip_stub_config() {
    let config = StubConfig {
        pins: vec![
            PinConfig {
                port: PinPort::Pa,
                number: 5,
                level: PinLevel::High,
            },
            PinConfig {
                port: PinPort::Pb,
                number: 12,
                level: PinLevel::Low,
            },
        ],
        flash: vec![FlashConfig {
            media: FlashMedia::Nor,
            driver_index: 2,
            manufacturer_id: 0xEF,
            device_type: 0x40,
            density_id: 0x18,
            flags: 0x00,
            capacity_bytes: 0x0080_0000,
        }],
        pmic: Some(PmicConfig {
            disabled: false,
            scl_port: PinPort::Pa,
            scl_pin: 10,
            sda_port: PinPort::Pa,
            sda_pin: 11,
            channels: vec![
                PmicChannel::LvSw1001,
                PmicChannel::HvSw1501,
                PmicChannel::Ldo33,
            ],
        }),
        sd0: Some(Sd0Config {
            base_address: 0x6800_0000,
            pinmux: Sd0Pinmux::ClkPa60OrPa39,
            init_sequence: Sd0InitSequence::EmmcThenSd,
        }),
    };

    let offset = 7;
    let mut data = vec![0u8; offset + DRIVER_CONFIG_SIZE + 9];

    write_stub_config_at(&mut data, offset, &config).expect("write config");

    let found = find_stub_config_offset(&data).expect("find config");
    assert_eq!(found, offset);

    let decoded = read_stub_config_from_bytes(&data).expect("read config");
    assert_eq!(decoded, config);
}
