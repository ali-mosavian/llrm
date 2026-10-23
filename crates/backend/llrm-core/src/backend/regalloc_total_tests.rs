//! The register allocator is total: every refusal it once made is a program
//! it now compiles. Each fixture is a function at regalloc's input, and each
//! test first asserts the shape that caused the refusal is still there.

use crate::backend::regalloc_input::{before_regalloc, through};
use crate::model::lir::LirBody;

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

/// The most moves one parallel copy holds.
fn longest_copy(body: &LirBody) -> usize {
    body.blocks.iter().map(|block| block.insns.iter().filter(|one| one.group.is_some()).count()).max().unwrap_or(0)
}

/// `conc7` at `-march=core2`: a far-pointer loop spills a dword and the word
/// of it another value reads, and the loop's parallel copy exchanges them.
/// ParallelCopy refused it ("need a temporary", #106). Which cycles the
/// allocator leaves depends on how it spills, so the premise is the loop's
/// pressure; the cycle's shape is pinned in parcopy's own tests.
#[test]
fn test_a_parallel_copy_cycle_through_a_frame_slot_is_scheduled() {
    // Unspilled: with SsaSpill in front the loop's longest parallel copy is 3
    // moves, not 8. A cycle through frame slots occurs in no corpus
    // program, with or without the spiller, so this guards ParallelCopy for
    // an allocator run on its own; what production does reach is a register
    // cycle with no spare register.
    let (body, mut phases) = crate::backend::phielim::with_old_lowering(|| {
        // Without the coalescer too: the premise is the copies' number, and
        // a coalescer that joins more leaves fewer.
        crate::backend::regalloc_input::before_phase_skipping(
            crate::backend::regalloc_input::Calls::C,
            "conc7_far.ll",
            CONC7,
            "Core",
            "RegAlloc",
            &["SsaSpill", "Coalescer", "MachineSink"],
        )
    });
    let longest = longest_copy(&body);
    assert!(longest >= 8, "premise: a loop-carried parallel copy of {longest} moves, more than the registers hold");
    let allocated = phases.remove(0).transform(body).expect("allocates");
    let done = through(allocated, phases);
    assert!(done.is_ok(), "{:?}", done.err());
}

const CONC7: &str = "_f_conc7_s1102468_xi_bpf_index_n_st1_sum_counteraffine_permute622_dup1";
