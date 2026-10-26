//! The register allocator is total: every refusal it once made is a program
//! it now compiles. Each fixture is a function at regalloc's input, and each
//! test first asserts the shape that caused the refusal is still there.

use crate::backend::regalloc_input::{before_regalloc, through};

/// `horner`'s `s = (s * x + cf[i]) % 10007` divides in place: `idiv` reads
/// `s` in ax and writes the remainder to `s` in dx. The allocator refused it
/// ("value#9 is required in two registers at once"); llrm-c stopped (#99).
/// Since TwoAddress commutes as the pin's `tryInstructionCommute` this input
/// no longer reaches the allocator with the value in two registers (cause not
/// examined; the premise check was removed with it); the shape is
/// `constrain`'s own test, and the function is still allocated here.
#[test]
fn test_a_value_read_in_one_register_and_written_in_another_is_allocated() {
    let (body, phases) = before_regalloc("horner.ll", "_horner", "486");
    let done = through(body, phases);
    assert!(done.is_ok(), "{:?}", done.err());
}
