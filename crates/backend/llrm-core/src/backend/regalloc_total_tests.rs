//! The register allocator is total: every refusal it once made is a program
//! it now compiles. Each fixture is a function at regalloc's input, and each
//! test first asserts the shape that caused the refusal is still there.

use iced_x86::Register;

use crate::backend::regalloc_input::{before_regalloc, through};
use crate::backend::target;
use crate::model::ir::Loc;
use crate::model::lir::LirBody;

/// An instruction that names a value in two different required registers.
fn requires_two_registers(body: &LirBody) -> bool {
    body.insns().iter().filter_map(|one| one.what.as_ref()).any(|what| {
        let required = target::requirements(what);
        let named = |side: &str, index: usize| match (side, index) {
            ("dest", at) => what.dests.get(at),
            (_, at) => what.sources.get(at),
        };
        let places: Vec<(u32, Register)> = required
            .iter()
            .filter_map(|(place, register)| match named(&place.side, place.index) {
                Some(Loc::Held(held)) => Some((held.value, crate::model::ir::root(*register))),
                _ => None,
            })
            .collect();
        places.iter().any(|(value, register)| places.iter().any(|(other, there)| other == value && there != register))
    })
}

/// `horner`'s `s = (s * x + cf[i]) % 10007` divides in place: `idiv` reads
/// `s` in ax and writes the remainder to `s` in dx. The allocator refused it
/// ("value#9 is required in two registers at once"); llrm-c stopped (#99).
#[test]
fn test_a_value_read_in_one_register_and_written_in_another_is_allocated() {
    let (body, phases) = before_regalloc("horner.ll", "_horner", "486");
    assert!(requires_two_registers(&body), "premise: one value is required in two registers");
    let done = through(body, phases);
    assert!(done.is_ok(), "{:?}", done.err());
}
