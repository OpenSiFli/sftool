use sftool_lib::{ChipType, load_stub_bytes};

#[test]
fn loads_sf32lb57_nor_nand_sd_stub() {
    let nor = load_stub_bytes(None, ChipType::SF32LB57, "nor")
        .expect("SF32LB57 NOR stub should be embedded");

    assert!(!nor.is_empty());

    for memory in ["nand", "sd"] {
        let data = load_stub_bytes(None, ChipType::SF32LB57, memory)
            .expect("SF32LB57 NAND/SD stub should be embedded");

        assert_eq!(data, nor);
    }
}

#[test]
fn sf32lb57_type1_stubs_are_not_embedded() {
    for memory in ["nor_type1", "nand_type1", "sd_type1"] {
        let err = load_stub_bytes(None, ChipType::SF32LB57, memory)
            .expect_err("SF32LB57 should not expose type1 embedded stubs");

        assert!(err.to_string().contains("sf32lb57_"));
    }
}
