//! A spilled word that an `and`/`or`/`xor` of words reads is its operand.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `x & mask` on longs with the mask spilled reloaded it into a pair
/// (`ld l, -6(ix)`, `ld h, -5(ix)`) to `and a, l` and `and a, h`: 2 B and 8 T
/// more per word than `and a, -6(ix)` (shift_until_idioms `until_bit`, -O2).
#[test]
fn a_spilled_mask_is_the_operand_of_the_and() {
    let asm = text(include_str!("kernels/ir/and_long_with_spilled_mask.ll"), "g", 0, &Request::new(Level::O2))
        .expect("compiles");
    assert!(asm.lines().any(|line| line.trim().starts_with("and a, ") && line.ends_with("(ix)")), "{asm}");
}
