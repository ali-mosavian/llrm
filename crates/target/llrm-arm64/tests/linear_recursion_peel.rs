//! A linear recursion called from another function is peeled into the caller
//! (REC-9), as gcc does.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn body(
    level: Level,
    name: &str,
) -> String {
    let asm = driver::assembly(include_str!("data/linear_recursion_peel.ll"), Platform::Linux, level).expect("lowers");
    let mut lines = asm.lines().skip_while(|line| *line != format!("{name}:"));
    let first = lines.next().expect("the function is there");
    let rest = lines.take_while(|line| line.starts_with('\t') || line.starts_with(".L"));
    std::iter::once(first).chain(rest).collect::<Vec<_>>().join("\n")
}

fn calls(text: &str) -> usize {
    text.lines().filter(|line| line.trim_start().starts_with("bl ")).count()
}

/// recpow -O2 called `power` once per trip with no copy of it: 67,475
/// instructions against gcc's 44,084 (gcc peels the recursion into the caller).
/// The copies leave the caller with one call per peeled level of the
/// recursion's remaining chain.
#[test]
fn a_linear_recursion_is_peeled_into_its_caller_at_o2() {
    let text = body(Level::O2, "bench_recpow");
    let moduli = text.lines().filter(|line| line.contains("msub")).count();
    assert!(moduli >= 6, "peeled copies each carry the modulus: {text}");
}

/// At -Os the pin's inliner and gcc both leave the call: nothing is peeled.
#[test]
fn nothing_is_peeled_at_os() {
    let text = body(Level::Os, "bench_recpow");
    assert!(calls(&text) >= 1 && text.lines().count() < 60, "{text}");
}
