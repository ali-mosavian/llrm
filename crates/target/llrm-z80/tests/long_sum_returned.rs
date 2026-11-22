//! A long sum returned in `ah:bl` made the legalizer's user order differ from
//! the pin's, and a `x + x` (one user, two slots of one operand) left behind
//! by `ReplaceAllUsesWith` was released once: it never became ready, the long
//! adds stayed unexpanded and selection refused `@f` (`CannotSelect(Srl)`).

use llrm_z80::driver::{Level, Request, program};

const IR: &str = include_str!("data/long_sum_returned.ll");

#[test]
fn a_long_sum_with_a_doubled_term_is_selected() {
    let result = program(IR, &Request::new(Level::O1));
    assert!(result.is_ok(), "{}", result.err().unwrap_or_default());
}
