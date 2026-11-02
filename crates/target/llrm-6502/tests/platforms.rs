//! The platform rows: every one parses, and its sixteen imaginary registers fit
//! in zero page.

use llrm_6502::platform::{Platform, ROWS};

#[test]
fn every_row_parses_and_fits_zero_page() {
    let names: Vec<String> = ROWS
        .iter()
        .map(|text| {
            let row = Platform::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert!(
                usize::from(row.imag_base) + usize::from(row.zero_page) <= 0x100,
                "{}: the imaginary registers leave zero page",
                row.name
            );
            assert!(row.soft_stack > 0x100, "{}: the soft stack is in zero page or the hardware stack", row.name);
            row.name
        })
        .collect();
    assert_eq!(names, ["c64", "sim65", "apple2", "atari8", "nes", "bbc"]);
}

const CC65_CFG: [(&str, &str); 6] = [
    ("c64", "c64.cfg"),
    ("sim65", "sim6502.cfg"),
    ("apple2", "apple2.cfg"),
    ("atari8", "atari.cfg"),
    ("nes", "nes.cfg"),
    ("bbc", "bbc.cfg"),
];

/// The imaginary registers of every row lie inside the zero-page segment of
/// cc65's own config for that target (`CC65_CFG`), the authority for
/// what the platform leaves a program.
#[test]
fn every_rows_registers_lie_inside_the_zero_page_of_its_cc65_config() {
    for text in ROWS {
        let row = Platform::parse(text).unwrap();
        let cfg = CC65_CFG
            .iter()
            .find(|(name, _)| *name == row.name)
            .unwrap_or_else(|| panic!("{}: no cc65 config", row.name))
            .1;
        let path = format!("/usr/share/cc65/cfg/{cfg}");
        let cfg_text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let zp = cfg_text
            .lines()
            .find(|l| l.trim_start().starts_with("ZP:"))
            .unwrap_or_else(|| panic!("{path}: no ZP segment"));
        let number = |key: &str| {
            let at = zp.find(key).unwrap_or_else(|| panic!("{path}: no {key} in {zp}")) + key.len();
            let digits: String = zp[at..]
                .trim_start_matches([' ', '='])
                .trim_start_matches('$')
                .chars()
                .take_while(char::is_ascii_hexdigit)
                .collect();
            usize::from_str_radix(&digits, 16).unwrap()
        };
        let (start, size) = (number("start"), number("size"));
        let (low, high) = (usize::from(row.imag_base), usize::from(row.imag_base) + usize::from(row.zero_page));
        assert!(
            low >= start && high <= start + size,
            "{}: ${low:02X}..${high:02X} is outside ${start:02X}..${:02X} of {cfg}",
            row.name,
            start + size
        );
    }
}
