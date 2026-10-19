//! `src/instructions/dialect.toml` is read by the shared instruction reader.

use llrm_instr::Dialect;

#[test]
fn the_arm64_dialect_parses_with_its_registers_conditions_and_flags() {
    let dialect = Dialect::parse(include_str!("../src/instructions/dialect.toml")).expect("the dialect reads");
    assert_eq!(dialect.file, "arm64.instr");
    assert!(["x0", "x30", "sp", "xzr", "v0", "v31"].iter().all(|r| dialect.registers.iter().any(|d| d == r)));
    assert_eq!(dialect.conditions.len(), 14);
    let bit = |name: &str| dialect.flags.iter().find(|(n, _)| n == name).map(|(_, bit)| *bit);
    assert_eq!((bit("N"), bit("Z"), bit("C"), bit("V")), (Some(8), Some(4), Some(2), Some(1)));
}
