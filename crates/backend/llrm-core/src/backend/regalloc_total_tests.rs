//! The register allocator is total: every refusal it once made is a program
//! it now compiles. Each fixture is a function at regalloc's input, and each
//! test first asserts the shape that caused the refusal is still there.

use iced_x86::Register;

use crate::backend::regalloc_input::{before_regalloc, before_regalloc_in, through, Calls};
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

/// The most moves one parallel copy holds.
fn longest_copy(body: &LirBody) -> usize {
    body.blocks.iter().map(|block| block.insns.iter().filter(|one| one.group.is_some()).count()).max().unwrap_or(0)
}

/// `conc7` at `--cpu Core`: a far-pointer loop spills a dword and the word
/// of it another value reads, and the loop's parallel copy exchanges them.
/// ParallelCopy refused it ("need a temporary", #106). Which cycles the
/// allocator leaves depends on how it spills, so the premise is the loop's
/// pressure; the cycle's shape is pinned in parcopy's own tests.
#[test]
fn test_a_parallel_copy_cycle_through_a_frame_slot_is_scheduled() {
    let (body, mut phases) = before_regalloc("conc7_far.ll", CONC7, "Core");
    let longest = longest_copy(&body);
    assert!(longest >= 8, "premise: a loop-carried parallel copy of {longest} moves, more than the registers hold");
    let allocated = phases.remove(0).transform(body).expect("allocates");
    let done = through(allocated, phases);
    assert!(done.is_ok(), "{:?}", done.err());
}

const CONC7: &str = "_f_conc7_s1102468_xi_bpf_index_n_st1_sum_counteraffine_permute622_dup1";

/// The frame cell `les` or `lds` loads a far pointer from, and the displacement
/// of a word access to either half of it.
fn far_pointer_slot_also_read_as_words(body: &LirBody) -> bool {
    let cell = |place: &Loc| match place {
        Loc::Mem(mem) if mem.through == Register::BP && mem.addr.is_some_and(|addr| addr.space == crate::model::ir::Space::Frame) => {
            Some((mem.addr.expect("a frame cell").disp + mem.offset, mem.width))
        }
        _ => None,
    };
    let places: Vec<(bool, i64, u32)> = body
        .insns()
        .iter()
        .filter_map(|one| one.what.as_ref().map(|what| (target::far_load(what), what)))
        .flat_map(|(far, what)| what.dests.iter().chain(&what.sources).filter_map(cell).map(move |(at, width)| (far, at, width)).collect::<Vec<_>>())
        .collect();
    places.iter().any(|(far, at, width)| {
        *far && *width == 4 && places.iter().any(|(_, other, narrow)| *narrow == 2 && (*other == *at || *other == *at + 2))
    })
}

/// `conc9` at `--cpu Core` reloads a far pointer's halves as words, which
/// the peephole fuses into `les`; LoopSlots then held the low word's slot in
/// a register and rewrote the `les` to read it ("les si, dx": no encoding,
/// llrm-nib stopped, #107).
#[test]
fn test_a_far_pointer_load_keeps_its_slot_in_memory_when_a_loop_holds_words_in_registers() {
    let (body, phases) = before_regalloc_in(Calls::Everything, "conc9_les.ll", "f_conc9_s2_xi_bgnlnpfpn_index_n_st1_sum", "Core");
    let mut body = body;
    let mut phases = phases.into_iter();
    for mut phase in phases.by_ref() {
        if phase.class_name() == "LoopSlots" {
            assert!(far_pointer_slot_also_read_as_words(&body), "premise: a far pointer's slot is read as words too");
            body = phase.transform(body).expect("loop slots");
            break;
        }
        body = phase.transform(body).expect("phase");
    }
    for one in body.insns().iter().filter_map(|one| one.what.as_ref()).filter(|what| target::far_load(what)) {
        assert!(matches!(one.sources.as_slice(), [Loc::Mem(_)]), "a far pointer load reads memory only: {one:?}");
    }
}

#[test]
fn probe104() {
    let (body, mut phases) = before_regalloc("walks10.ll", "_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum", "386");
    eprintln!("{}", crate::tools::stages::lir_stage("in", &[(body.name.clone(), body.clone())]));
    let out = phases.remove(0).transform(body).unwrap();
    eprintln!("{}", crate::tools::stages::lir_stage("out", &[(out.name.clone(), out.clone())]));
}

/// league.nib's `main` spills a value that was loaded, updated and stored
/// again in a register (`x = x * 8` on a pointer deref). Taking every spiller
/// product as unspillable refused it ("value#23 cannot be spilled and no
/// register is free", #129): only a product that lives for one use is.
#[test]
fn test_a_value_the_spiller_updated_in_a_register_can_be_spilled_again() {
    let (body, mut phases) = before_regalloc_in(Calls::Everything, "league.ll", "main", "386");
    let allocated = phases.remove(0).transform(body).expect("allocates");
    let reloads = allocated.insns().iter().filter(|one| one.spill_reload).count();
    assert!(reloads > 0, "premise: the function spills");
}
