//! `optimizeCRCLoop`: the pin's `LoopIdiom/cyclic-redundancy-check.ll`, run
//! with `-loop-idiom-crc-strategy=table`. Its `TABLE` lines say which
//! functions become a Sarwate lookup (a `@.crctable.N` load) and which stay
//! as written.

use std::rc::Rc;

use super::CrcStrategy;
use super::pass::LoopIdiom;

/// The names of the functions whose `TABLE` lines read a `.crctable`.
fn tabled(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(rest) =
            line.strip_prefix("; TABLE-LABEL: define ").or_else(|| line.strip_prefix("; ALL-LABEL: define "))
        {
            current = Some(rest.split('@').nth(1).unwrap().split('(').next().unwrap().to_owned());
        } else if line.starts_with("; CLMUL-LABEL") {
            current = None;
        } else if line.contains(".crctable")
            && line.starts_with("; TABLE")
            && let Some(name) = current.take()
        {
            names.push(name);
        }
    }
    names
}

fn after(
    text: &str,
    strategy: CrcStrategy,
) -> String {
    after_sized(text, strategy, false)
}

fn after_sized(
    text: &str,
    strategy: CrcStrategy,
    for_size: bool,
) -> String {
    let mut module = llrm_mir::parse::module(text).expect("the fixture");
    let mut pass = LoopIdiom::new();
    pass.options.crc_strategy = strategy;
    pass.for_size = for_size;
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(pass);
    manager.run_module(&mut module, Rc::new(crate::cgpaddr::tests::Flat)).unwrap();
    assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
    llrm_mir::print::module(&module)
}

fn body<'a>(
    module: &'a str,
    name: &str,
) -> &'a str {
    let start = module.find(&format!("@{name}(")).expect("the function");
    let rest = &module[start..];
    &rest[..rest.find("\n}").unwrap_or(rest.len())]
}

fn fixture() -> String {
    let path = format!("{}/tests/hashrecognize/loopidiom-cyclic-redundancy-check.ll", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).expect("the fixture")
}

/// x_crc32 kept its bit-at-a-time loop, 8 shifts, 8 selects and 8 xors per
/// byte, where the pin looks the byte up in a 256-entry table (283 B against
/// 139 B at -O2). Every function the pin tables is tabled, and every other
/// one is left alone.
#[test]
fn test_the_loops_the_pin_tables_are_tabled_and_the_rest_are_not() {
    let text = fixture();
    let out = after(&text, CrcStrategy::Table);
    let want = tabled(&text);
    assert!(want.len() > 10, "{want:?}");
    for name in &want {
        assert!(body(&out, name).contains(".crctable"), "{name} is not tabled:\n{}", body(&out, name));
    }
    for line in out.lines().filter(|line| line.starts_with("define ")) {
        let name = line.split('@').nth(1).unwrap().split('(').next().unwrap();
        assert_eq!(body(&out, name).contains(".crctable"), want.iter().any(|one| one == name), "{name}");
    }
}

/// With the idiom off the loops stay: the same input comes out without a table.
#[test]
fn test_with_the_strategy_off_no_table_is_made() {
    let out = after(&fixture(), CrcStrategy::Disable);
    assert!(!out.contains(".crctable"), "{out}");
}

/// At -Os the table is 1024 B of data for a loop of a few instructions (m16
/// -Os crc.c grew by 1,024 B in `tools/sizes.py`): the pin's
/// `ApplyCodeSizeHeuristics` skips it under size, and the -Os pipeline has no
/// `optsize` attribute to read, so the pass is told. -O2 still tables.
#[test]
fn test_the_auto_strategy_makes_no_table_for_size() {
    let sized = after_sized(&fixture(), CrcStrategy::Auto, true);
    assert!(!sized.contains(".crctable"), "{sized}");
    let fast = after_sized(&fixture(), CrcStrategy::Auto, false);
    assert!(fast.contains(".crctable"), "{fast}");
}
