//! The argument registers of a tail call.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `report((long)count * 1000 + s0 + s1)` as a tail call gave `report` a zero
/// low word (tail_call_long_argument -O1/-O2/-Os, got 0 want 12640): the
/// `ld l, c; ld h, b` before `jp _report` was dropped as a dead load, the jump
/// being taken for one to a block that reads nothing.
#[test]
fn a_tail_call_reads_the_registers_it_is_given() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let asm =
            text(include_str!("data/tail_call_long_argument.ll"), "kernel", 0, &Request::new(level)).expect("compiles");
        let before: Vec<&str> = asm.lines().take_while(|line| line.trim() != "jp _report").collect();
        let tail = &before[before.len().saturating_sub(4)..];
        assert!(tail.iter().any(|line| line.trim() == "ld l, c"), "{level:?}: {asm}");
        assert!(tail.iter().any(|line| line.trim() == "ld h, b"), "{level:?}: {asm}");
    }
}
