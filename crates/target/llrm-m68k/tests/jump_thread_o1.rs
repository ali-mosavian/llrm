//! bsearch at -O1: clang's -O1 IR (the pin's pipeline has no JumpThreadingPass
//! there) leaves the latch `br i1 %15` on the compare tested one block up, so
//! the join tests `a[m] == key` again (`cmp.l %a0,%d3; bne`) and the result
//! rides a loop-carried copy: 668 clk against gcc -O1's 614 at 68000. The
//! backend's `-fthread-jumps` (llrm-cc's default at -O1 for m68k rows) threads
//! it.

use llrm_m68k::compile::CodeModel;
use llrm_m68k::driver::{Level, Policy, assembly_flagged, cpu_of};

const BSEARCH_O1: &str = include_str!("fixtures/bsearch_o1_clang.ll");

fn text(flags: &[&str]) -> String {
    let mut policy = Policy::default();
    for flag in flags {
        assert!(policy.take(flag), "{flag} is a switch");
    }
    assembly_flagged(BSEARCH_O1, cpu_of("68000").unwrap(), Level::O1, false, None, CodeModel::Small, &policy)
        .expect("compiles")
}

fn compares(asm: &str) -> usize {
    asm.lines().filter(|line| line.trim_start().starts_with("cmp.l")).count()
}

/// The premise: without the switch the IR's re-test is in the code.
#[test]
fn the_pins_o1_keeps_the_retest() {
    let asm = text(&[]);
    assert!(compares(&asm) >= 3, "the O1 shape changed:\n{asm}");
}

#[test]
fn threading_the_latch_leaves_one_test_of_the_key() {
    let plain = text(&[]);
    let threaded = text(&["-fthread-jumps"]);
    assert!(compares(&threaded) < compares(&plain), "the re-test stayed:\n{threaded}");
    assert!(threaded.len() < plain.len(), "no smaller:\n{threaded}");
}
