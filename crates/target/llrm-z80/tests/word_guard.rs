//! A signed word compared with 0 or 1.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `x > 0` and `x <= 0` were a subtract with the sign biased (`xor a, #128 ...
/// sbc a, #128`): 13 B and `b` clobbered, in a branch, a select and a bool
/// alike; the zero test and the sign joined in the carry are 7 B (queens'
/// `place` -Os 360 B, with three such guards).
#[test]
fn a_word_against_zero_is_one_carry_test() {
    for level in [Level::Os, Level::O1] {
        for function in ["gt", "le", "sel", "flag"] {
            let asm = text(include_str!("data/word_guard.ll"), function, 0, &Request::new(level)).expect("compiles");
            assert!(!asm.contains("sbc"), "{function} {level:?}: a subtract\n{asm}");
            assert!(asm.contains("scf"), "{function} {level:?}: no word test\n{asm}");
        }
    }
}

/// queens' `place` ended its `x > 0` guard on two blocks that both start `ld
/// de, #0`: the load was not hoisted into the guard (the spill ahead of the
/// load in the other block hid the twin), so the exit block lost its share of
/// the epilogue (-Os 362 B against 354 B, three `pop ix` against two).
#[test]
fn a_constant_both_sides_of_a_word_guard_start_with_is_loaded_once() {
    for level in [Level::Os, Level::O1] {
        let asm =
            text(include_str!("data/queens_inline_frame.ll"), "place", 0, &Request::new(level)).expect("compiles");
        let epilogues = asm.matches("pop ix").count();
        assert!(epilogues <= 2, "{level:?}: {epilogues} epilogues\n{asm}");
    }
}
