//! The inline trial's byte estimate: the calls in a body, their arguments and
//! the shifts by whole words.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `main` inlined `wrap`, a call and a sign extension: the body's call was
/// priced as 2 flat units against 7 B for the call it replaced, so a body of
/// 11 B (`call`, argument, `ld a,d; add a,a; sbc a,a; ld c,a; ...`) looked
/// smaller than its call and the sign extension was written twice (fib -Os:
/// 65 B, SDCC 54 B).
#[test]
fn a_wrapper_that_calls_and_extends_is_not_inlined_at_os() {
    let asm = text(include_str!("data/wrapper_sext_call.ll"), "main", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(asm.contains("call _wrap"), "{asm}");
}

/// `put(uint64_t)`, two calls of a long and a shift by 32, called from a loop:
/// the estimate left the first argument out of every call and counted the
/// callee as a word, so the three calls of `put` (29 B each) cost 9, its copies
/// 22, and the trial refused them: `put` stayed, 24 B, and the program was 21 B
/// larger (shift64_variable -Os: 877 B to 898). A constant shift by whole words
/// was four word shifts where the backend renames two registers.
#[test]
fn a_wrapper_of_a_long_argument_is_inlined_where_the_copies_are_smaller() {
    let asm =
        text(include_str!("data/long_argument_wrapper.ll"), "main", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(!asm.contains("call _put"), "{asm}");
}
