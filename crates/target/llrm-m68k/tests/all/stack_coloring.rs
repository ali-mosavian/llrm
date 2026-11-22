//! Two locals whose lifetimes never overlap share one frame object
//! (StackColoring): a function of two such `[64 x i32]` arrays had a 512-byte
//! frame, the pin's is 256 (plus the call's slot).

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn locals_with_disjoint_lifetimes_share_a_frame_object() {
    let source = include_str!("../data/stack_coloring.ll");
    for cpu in ["68000", "68020"] {
        let text = assembly(source, cpu_of(cpu).unwrap(), Level::O2).unwrap_or_else(|why| panic!("{cpu}: {why}"));
        assert!(!text.contains("512"), "{cpu}: the two arrays have a frame object each:\n{text}");
    }
}
