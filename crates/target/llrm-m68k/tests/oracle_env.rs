//! The oracle tests' environment: an unset `GCC_M68K` fails the test and says what to set; an explicit
//! `M68K_ISA_SKIP_ORACLE=1` skips it with a visible line (it was a bare "GCC_M68K is not set" panic in 13 copies).

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
fn a_set_root_is_used() {
    assert_eq!(oracle_with(Some("/x"), Some("1")), Ok(Oracle::Root("/x".to_owned())));
}
