//! arm64 prefers a post-indexed walk (`ldrsh x, [p], #2`) where gcc's ivopts
//! does, in a loop that makes enough trips. tile -O1 kept an index beside a
//! compare (5 instructions a trip, gcc 4); the same preference over nbody's
//! triangular inner loop (4 streams, at most 3 trips) started every pointer
//! again on each entry: 49,441 instructions against 44,241.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn post_indexed(
    ir: &str,
    level: Level,
) -> usize {
    let asm = driver::assembly(ir, Platform::Linux, level).expect("lowers");
    asm.lines().filter(|line| line.trim_start().starts_with("ldr") && line.contains("], #")).count()
}

#[test]
fn a_loop_of_forty_trips_walks_a_pointer() {
    assert_eq!(post_indexed(include_str!("data/postinc_walk.ll"), Level::O1), 1);
}

#[test]
fn a_triangular_loop_of_few_trips_keeps_its_index() {
    assert_eq!(post_indexed(include_str!("data/postinc_triangle.ll"), Level::O1), 0);
}

#[test]
fn a_size_build_keeps_its_index() {
    assert_eq!(post_indexed(include_str!("data/postinc_walk.ll"), Level::Os), 0);
}
