//! DEVIATE SPILL-FOLD: grep's 32-bit trip counter was region-split at m16 -O2,
//! `inc ebx; mov [bp-8], ebx` and a reload in front of each read: 3 x (1 + 66h)
//! = 6 clk a trip against `inc dword ptr [bp-8]`'s 3 + 1. 36,989,263 clk, 506
//! B; spilled, its references fold: 34,891,879 clk, 495 B. -O1 holds bytes and
//! is unchanged.

mod common;

fn listing(level: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    let source = format!("{}/bench/grep/grep.c", env!("CARGO_MANIFEST_DIR"));
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args([level, "-m16", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

#[test]
fn at_o2_a_counter_whose_references_fold_lives_in_memory() {
    let text = listing("-O2");
    assert!(text.contains("inc dword ptr [bp-"), "the counter was split into a register:\n{text}");
}

#[test]
fn at_o1_where_bytes_are_held_the_counter_stays_split() {
    let text = listing("-O1");
    assert!(!text.contains("inc dword ptr [bp-"), "{text}");
}
