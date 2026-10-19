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

#[test]
fn the_arm64_forms_read_and_cover_the_selection_gaps() {
    let dialect = Dialect::parse(include_str!("../src/instructions/dialect.toml")).expect("the dialect reads");
    let forms = llrm_instr::parse(include_str!("../src/instructions/arm64.instr"), &dialect).expect("the forms read");
    let has = |name: &str| forms.iter().any(|form| form.name == name);
    // Every pattern the gap list names has a form to select.
    for name in ["madd", "msub", "ccmp", "csinc", "cinc", "fnmul", "fmadd", "ubfx", "ldp", "stp", "b.eq", "b.le", "cbz", "tbz"] {
        assert!(has(name), "no form `{name}`");
    }
    // `b.{cc}` expands to one form per condition, each reading the flags it tests.
    assert_eq!(forms.iter().filter(|form| form.name.starts_with("b.")).count(), 14);
}
