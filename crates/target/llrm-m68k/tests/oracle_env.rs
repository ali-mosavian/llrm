//! The oracle tests' environment: an unset `GCC_M68K` fails the test and says
//! what to set; an explicit `M68K_ISA_SKIP_ORACLE=1` skips it with a visible
//! line (it was a bare "GCC_M68K is not set" panic in 13 copies).

mod support;

use support::{Oracle, oracle_with};

#[test]
fn an_unset_gcc_m68k_fails_naming_the_variable_the_tree_and_the_opt_out() {
    let why = oracle_with(None, None).unwrap_err();
    assert!(why.contains("GCC_M68K") && why.contains("gcc-m68k") && why.contains("M68K_ISA_SKIP_ORACLE=1"), "{why}");
    assert!(oracle_with(Some(""), Some("0")).is_err(), "an empty root and a skip that is not 1 still fail");
}

#[test]
fn the_explicit_skip_prints_a_visible_line() {
    let Ok(Oracle::Skipped(line)) = oracle_with(None, Some("1")) else { panic!("skip=1 must skip") };
    assert!(line.starts_with("ORACLE SKIPPED"), "{line}");
}

#[test]
fn a_root_without_the_assembler_fails_naming_the_missing_tool() {
    // Pointing GCC_M68K one level too high gave 44 opaque `NotFound` unwraps.
    let why = oracle_with(Some("/nonexistent-gcc-m68k"), Some("1")).unwrap_err();
    assert!(why.contains("/nonexistent-gcc-m68k/usr/bin/m68k-linux-gnu-as") && why.contains("usr/"), "{why}");
}

#[test]
fn a_set_root_with_the_assembler_is_used() {
    let root = std::env::temp_dir().join(format!("llrm-m68k-oracle-root-{}", std::process::id()));
    std::fs::create_dir_all(root.join("usr/bin")).unwrap();
    std::fs::write(root.join("usr/bin/m68k-linux-gnu-as"), "").unwrap();
    let root_str = root.to_str().unwrap().to_owned();
    let got = oracle_with(Some(&root_str), Some("1"));
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(got, Ok(Oracle::Root(root_str)));
}
