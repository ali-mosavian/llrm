//! `GlobalMerge`: globals one function uses together share one base address.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn body(
    level: Level,
    name: &str,
) -> String {
    let asm = driver::assembly(include_str!("data/global_merge.ll"), Platform::Linux, level).expect("lowers");
    let mut lines = asm.lines().skip_while(|line| *line != format!("{name}:"));
    let first = lines.next().expect("the function is there");
    let rest = lines.take_while(|line| line.starts_with('\t') || line.starts_with(".L"));
    std::iter::once(first).chain(rest).collect::<Vec<_>>().join("\n")
}

fn pages(text: &str) -> usize {
    text.lines().filter(|line| line.contains("adrp")).count()
}

/// Three globals were three `adrp`+`add` pairs: the pin merges them, so the
/// function takes one page address and offsets from it (nbody_single -O3: 8
/// `adrp` against the pin's 5, +40 B).
#[test]
fn globals_used_together_take_one_page_address() {
    let text = body(Level::O3, "three");
    assert_eq!(pages(&text), 1, "{text}");
    assert!(text.contains("_MergedGlobals"), "{text}");
}

/// A global no other is used with stays alone.
#[test]
fn a_lone_global_is_not_merged() {
    let text = body(Level::O3, "alone");
    assert!(!text.contains("_MergedGlobals"), "{text}");
}

/// A global past the offset reach stays out of the struct.
#[test]
fn a_global_past_the_reach_is_not_merged() {
    let text = body(Level::O3, "large");
    assert_eq!(pages(&text), 2, "{text}");
}

/// Below -O3 only `minsize` functions count: `small` is the one that uses
/// `b` and `c` together, so `three` gets them merged but `a` apart.
#[test]
fn below_o3_only_minsize_functions_count() {
    assert_eq!(pages(&body(Level::O2, "three")), 2);
    assert_eq!(pages(&body(Level::O2, "small")), 1);
}

/// At -Os (`optsize`, not `minsize`) the pin merged nothing: stft_fixed -Os
/// made one `adrp` per static array, 15 against gcc's 5 anchored ones.
#[test]
fn optsize_functions_count_below_o3() {
    assert_eq!(pages(&body(Level::O2, "sized")), 1);
}
