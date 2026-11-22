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

/// The surviving objects keep the pin's order (a group sits where its largest
/// member did), the order they have without sharing: a byte local declared
/// first keeps the highest offset. Numbering groups largest first moved it to
/// the bottom of the frame and shifted the offsets `using.c`'s byte slots use
/// (+2 bytes a use where `(%sp)` became `(4,%sp)`).
#[test]
fn the_objects_that_remain_keep_their_declaration_order() {
    let source = include_str!("../data/stack_coloring.ll");
    let text = assembly(source, cpu_of("68000").unwrap(), Level::O2).unwrap();
    let body = text.split("hot_first:").nth(1).expect("hot_first");
    assert!(body.contains("move.b\t#1, (263,%sp)"), "the byte local is not above the shared array:\n{body}");
}
