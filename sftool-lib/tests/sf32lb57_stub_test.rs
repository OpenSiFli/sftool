use sftool_lib::{ChipType, load_stub_bytes};

#[test]
fn loads_sf32lb57_nor_stub() {
    let data = load_stub_bytes(None, ChipType::SF32LB57, "nor")
        .expect("SF32LB57 NOR stub should be embedded");

    assert!(!data.is_empty());
}

#[test]
fn sf32lb57_only_nor_stub_is_supported() {
    for memory in ["nor_type1", "nand", "nand_type1", "sd", "sd_type1"] {
        let err = load_stub_bytes(None, ChipType::SF32LB57, memory)
            .expect_err("SF32LB57 should not expose non-NOR embedded stubs");

        assert!(err.to_string().contains("sf32lb57_"));
    }
}
