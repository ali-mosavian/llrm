//! The platform rows: every one parses, and its sixteen imaginary registers fit in zero page.

use llrm_6502::platform::{Platform, ROWS};

#[test]
fn every_row_parses_and_fits_zero_page() {
    let names: Vec<String> = ROWS
        .iter()
        .map(|text| {
            let row = Platform::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert!(usize::from(row.imag_base) + 16 <= 0x100, "{}: rc15 leaves zero page", row.name);
            row.name
        })
        .collect();
    assert_eq!(names, ["c64", "sim65", "apple2", "atari8", "nes", "bbc"]);
}
