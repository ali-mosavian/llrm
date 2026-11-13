//! A call before a shared `ret`.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `if (n == 2) h(); else if (n == 1) f();` made `f` a `jp` and left `h` a
/// `call _h` falling into the shared `ret` (sibling_call_callees -O2, +1 B and
/// a `ret` run per call): the return block that follows the call was not its
/// own `ret`, so the call was not seen to be last.
#[test]
fn a_call_that_falls_into_the_shared_return_is_a_tail_call() {
    let asm = text(include_str!("data/sibling_calls_in_a_chain.ll"), "g", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(!asm.contains("call "), "{asm}");
}
