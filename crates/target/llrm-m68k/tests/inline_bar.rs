//! A 30-operation body called from six places was inlined six times (about
//! 100 B a copy) where a call is 20 B; tests/run/c memory_expansion -O2:
//! 61684 B against gcc's 24204, its `check` and `reset` helpers inlined at
//! over 200 sites.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const SIX_SITES: &str = include_str!("fixtures/inline_bar.ll");

#[test]
fn a_body_of_thirty_operations_is_called_not_inlined() {
    let out = assembly(SIX_SITES, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    let calls = out.lines().filter(|line| line.trim_start().starts_with("jsr")).count();
    assert_eq!(calls, 6, "{out}");
}
