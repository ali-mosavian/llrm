//! The register allocator is total: every refusal it once made is a program
//! it now compiles. Each fixture is a function at regalloc's input, and each
//! test first asserts the shape that caused the refusal is still there.

use iced_x86::Register;

use crate::backend::regalloc_input::{before_regalloc, through};
use crate::backend::target;
use crate::model::ir::Loc;
use crate::model::lir::LirBody;
use crate::model::passes::LIRTransform;

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

/// Whether a parallel copy has moves that all read each other's
/// destinations, one of them through a frame slot.
fn has_a_cycle_through_a_slot(body: &LirBody) -> bool {
    use crate::backend::parcopy::{_into, _outof};
    body.blocks.iter().any(|block| {
        let mut groups: std::collections::BTreeMap<i64, Vec<(String, String, bool)>> = Default::default();
        for one in block.insns.iter().filter(|one| one.group.is_some()) {
            let what = one.what.as_ref().expect("a move");
            let slot = matches!(what.dests[0], Loc::Mem(_)) || matches!(what.sources[0], Loc::Mem(_));
            groups.entry(one.group.expect("filtered")).or_default().push((_into(one).unwrap(), _outof(one).unwrap(), slot));
        }
        groups.values().any(|group| {
            let mut left: Vec<&(String, String, bool)> = group.iter().filter(|(into, outof, _)| into != outof).collect();
            while let Some(at) = left.iter().position(|(into, _, _)| !left.iter().any(|(_, outof, _)| outof == into)) {
                left.remove(at);
            }
            left.iter().any(|(_, _, slot)| *slot)
        })
    })
}

/// `conc7` at `--cpu Core`: a far-pointer loop spills a dword and the word
/// of it another value reads, and the loop's parallel copy exchanges them.
/// ParallelCopy refused it ("need a temporary", #106).
#[test]
fn test_a_parallel_copy_cycle_through_a_frame_slot_is_scheduled() {
    let (body, mut phases) = before_regalloc("conc7_far.ll", CONC7, "Core");
    let allocated = phases.remove(0).transform(body).expect("allocates");
    assert!(has_a_cycle_through_a_slot(&allocated), "premise: a copy cycle through a slot");
    let done = through(allocated, phases);
    assert!(done.is_ok(), "{:?}", done.err());
}

const CONC7: &str = "_f_conc7_s1102468_xi_bpf_index_n_st1_sum_counteraffine_permute622_dup1";
