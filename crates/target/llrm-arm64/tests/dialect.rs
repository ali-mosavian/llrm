//! `src/instructions/dialect.toml` is read by the shared instruction reader.

use llrm_target::instr::Dialect;

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
    let forms =
        llrm_target::instr::parse(include_str!("../src/instructions/arm64.instr"), &dialect).expect("the forms read");
    let has = |name: &str| forms.iter().any(|form| form.name == name);
    // Every pattern the gap list names has a form to select.
    for name in
        ["madd", "msub", "ccmp", "csinc", "cinc", "fnmul", "fmadd", "ubfx", "ldp", "stp", "b.eq", "b.le", "cbz", "tbz"]
    {
        assert!(has(name), "no form `{name}`");
    }
    // `b.{cc}` expands to one form per condition, each reading the flags it
    // tests.
    assert_eq!(forms.iter().filter(|form| form.name.starts_with("b.")).count(), 14);
}

#[test]
fn the_arm64_patterns_generate_against_the_arm64_forms() {
    let dialect = Dialect::parse(include_str!("../src/instructions/dialect.toml")).expect("the dialect reads");
    let generated = llrm_iselgen::generate(
        include_str!("../src/instructions/arm64.instr"),
        include_str!("../src/isel/patterns.isel"),
        "arm64",
        "llrm_codegen",
        &dialect,
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert!(generated.code.contains("fn arm64_holds("), "{}", &generated.code[..200.min(generated.code.len())]);
}

#[test]
fn the_calling_description_parses_and_names_aapcs64() {
    let calling = llrm_target::calling::Calling::parse(include_str!("../src/machines/calling.toml"))
        .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(calling.names(), ["aapcs64"]);
    assert_eq!(calling.native().slot_bytes, 8);
    assert_eq!(calling.native().stack_alignment, Some(16));
}

#[test]
fn the_platform_descriptions_parse_and_agree_with_the_platform() {
    use llrm_arm64::machine::Platform;
    for platform in [Platform::Linux, Platform::Darwin] {
        let (layout, object) = (platform.layout_description(), platform.object_description());
        let layout = llrm_target::layout::Layout::parse(layout).unwrap_or_else(|why| panic!("{why}"));
        assert_eq!((layout.mode, layout.datalayout.as_str()), (64, platform.datalayout()), "{platform:?}");
        llrm_target::object::ObjectFormat::parse(object).unwrap_or_else(|why| panic!("{why}"));
    }
}
