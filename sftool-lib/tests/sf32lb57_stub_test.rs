use sftool_lib::{ChipType, load_stub_bytes};

#[test]
fn loads_sf32lb57_nor_nand_sd_stub() {
    let nor = load_stub_bytes(None, ChipType::SF32LB57, "nor")
        .expect("SF32LB57 NOR stub should be embedded");
    let nand = load_stub_bytes(None, ChipType::SF32LB57, "nand")
        .expect("SF32LB57 NAND stub should be embedded");
    let sd = load_stub_bytes(None, ChipType::SF32LB57, "sd")
        .expect("SF32LB57 SD stub should be embedded");

    assert!(!nor.is_empty());
    assert!(!nand.is_empty());
    assert!(!sd.is_empty());
    assert_ne!(nor, nand);
    assert_ne!(nor, sd);
    assert_ne!(nand, sd);
}

#[test]
fn sf32lb57_type1_stubs_are_not_embedded() {
    for memory in ["nor_type1", "nand_type1", "sd_type1"] {
        let err = load_stub_bytes(None, ChipType::SF32LB57, memory)
            .expect_err("SF32LB57 should not expose type1 embedded stubs");

        assert!(err.to_string().contains("sf32lb57_"));
    }
}
