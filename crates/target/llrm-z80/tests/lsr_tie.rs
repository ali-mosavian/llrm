//! A loop LSR rates no better than it stands.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// shellsort's gap loops were rewritten to three stepping pointers and a
/// step held in a register (`-O1`: 664 B, 136 frame accesses against 517 B and
/// 88 as written; SDCC 445 B). LSR rated the rewrite equal and the pin
/// rewrites on a tie.
#[test]
fn a_loop_lsr_rates_equal_is_left_as_written() {
    for level in [Level::O1, Level::O2] {
        let asm = text(include_str!("data/shellsort_inner_loop.ll"), "bench_shellsort", 0, &Request::new(level))
            .expect("compiles");
        let frame = asm.lines().filter(|line| line.contains("(ix)")).count();
        assert!(frame < 110, "{level:?}: {frame} frame accesses\n{asm}");
    }
}
