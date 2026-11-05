//! gcc copies a tree recursion into itself and eliminates its tail call first
//! (REC-2, owner arm64, exit 2026-11-30). rectwo (`paths(n-1,m) + paths(n,m-1)`)
//! at -O3 ran 777,916 instructions in 96 B on arm64, the pin's order; gcc's
//! copies run 266,373 in 972 B.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn calls(source: &str, level: Level) -> usize {
    let asm = driver::assembly(source, Platform::Linux, level).expect("lowers");
    asm.lines().filter(|line| line.trim_start().starts_with("bl ")).count()
}

#[test]
fn a_tree_recursion_is_copied_into_itself_at_o3() {
    let calls = calls(include_str!("data/bench_rectwo.ll"), Level::O3);
    assert!(calls > 2, "{calls} calls");
}

#[test]
fn a_tree_recursion_is_not_copied_at_os() {
    let calls = calls(include_str!("data/bench_rectwo.ll"), Level::Os);
    assert!(calls <= 2, "{calls} calls");
}
