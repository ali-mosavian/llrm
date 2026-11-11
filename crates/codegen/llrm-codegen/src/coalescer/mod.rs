//! `RegisterCoalescer`: joining the live intervals of the two values of a copy
//! so the copy goes. Translated from llvm/lib/CodeGen/RegisterCoalescer.cpp at
//! d1106deb71cc, over the translated `LiveIntervals`.
//!
//! # How it works
//! `pair` is `CoalescerPair`: the two values of one copy, which one is merged
//! into which. `joinvals` is `JoinVals`: for every value number of the two
//! intervals, whether it keeps, is merged into a value of the other side
//! (a copy that becomes an identity, or a copy of a value the other side
//! already holds), or replaces the other side's value where it is redefined.
//! `driver` is `joinAllIntervals`: the order copies are tried in; `env` is the
//! target's answers and the pass over a body. `join_copy` runs
//! `RegisterCoalescer::joinCopy` for one copy: refuse, or join the intervals,
//! erase the copies that became identities and rename the absorbed value in
//! every instruction.
//!
//! DEVIATE C1 (no number): sub-registers. A value here is one lane: a copy
//! between values of different widths is not a move `CoalescerPair` accepts.
//! Fact: `liverange.rs` has no `SubRange`; x86's AL/AH are register units of
//! the register classes, not lanes of a virtual register. Consequence: the
//! lane conflicts (`CR_Unresolved`, `taintExtent`, `resolveConflicts`) cannot
//! arise, and `JoinVals` asserts it. Exit: none while values have no lanes.
//!
//! DEVIATE C2 (no number): no physical-register copies. A fixed register is a
//! constraint on an instruction (`requires`, `delivers`), not a `COPY` to a
//! physical register, so `joinReservedPhysReg`, `canJoinPhys` and
//! `CP.isPhys()` have nothing to join; the environment says whether two values
//! may share a register (`CoalescerEnv::can_share`, `getCommonSubClass`).
//!
//! DEVIATE C3 (no number): `removeImplicitDefs` (sub-range joins only) is not
//! translated, and with one lane `ErasableImplicitDef` is a single flag:
//! the `implicit_def` of a phi's predecessor is erased where a live value
//! overlaps it (`analyzeValue`, `pruneValues`, `eraseInstrs`); it is kept as
//! a value where it extends beyond its block. No EH pad successors in LIR.
//! `eliminateUndefCopy` is translated.
//!
//! DEVIATE C12 (no number): a member of a parallel copy is not touched by
//! `adjustCopiesBackFrom`, `removeCopyByCommutingDef` or
//! `removePartialRedundancy`, as `reMaterializeDef` leaves it (C11). Exit:
//! parallel copies sequenced before the coalescer (D-F).
//!
//! DEVIATE C13: `CoalescerEnv::joins_sub_registers`. The pin joins every copy
//! between widths that `CoalescerPair` accepts, vetoed only by
//! `TRI->shouldCoalesce` (RegisterCoalescer.cpp#L2080, which M68k does not
//! override); a target may switch the joins off. m68k does: they raised
//! `movem` 71 -> 89 in div_constants_short (68020 -O1) and cost 15 of 90
//! cell-CPU rows. Narrowed (owner greedy-subreg, exit 2026-11-29): a copy whose
//! narrow range lies inside the wide value's is joined anyway (it lengthens
//! nothing). Worse row: div_constants_short 68020 -O2 4954 -> 4978 B (the
//! sixth join lets a function at d0-d7 take d2: movem 96 -> 98), clocks
//! unpriced. Exit: 2026-11-15, re-evaluated when the allocator-sensitivity
//! work (pair roles) lands; no join rule explains the rows (the harmful joins
//! leave palettes unchanged and shift one dequeue order).
//!
//! DEVIATE C14: `TargetEnv::kinds`. The pin's `TRI->shouldCoalesce` reads
//! register classes and at most a few uses. A target here prices the uses a
//! join would leave a narrower class to serve (`extra_cost`); that walked
//! every use of both values at each attempt, `copies x uses` for a hot
//! value (6502 `switch`: `should_coalesce` 38% of the compile). The uses are
//! summed by kind once per value and merged on a join. An instruction that
//! names both values counts once per value, not once for the pair (checked
//! by `LLRM_CHECK_USE_SUMMARY`). Exit: none; the target's price is its own.
//!
//! DEVIATE C15: `CoalescerEnv::joins_loop_carried_first`. Among blocks of one
//! depth, split-edge state and connectivity the pin takes the lower number
//! first; a target may put first the block whose copies read a value carried
//! around the loop (live at the header, or a copy of one), so the join of a
//! loop-carried value with its update comes before a join with a value born in
//! the body. m68k: `m = max(m, p[i])` (arrmax -O2) lost a copy per trip, +160
//! rate on the 68000. Default off: the pin's order, output unchanged. Owner:
//! m68k. Exit: 2026-11-22.
//!
//! DEVIATE C16 (owner greedy-anyisa, exit 2026-11-29): `joinAllIntervals` takes
//! a split edge (a block of copies and a jump between one predecessor and one
//! successor) before the others only when `JoinSplitEdges` is set
//! (`-join-splitedges`, off at the pin: RegisterCoalescer.cpp#L4273); here
//! `driver::run` always does. Ties go by block index, the pin's by
//! `MBB->getNumber()` (creation order: a block split off an edge lies next to
//! its predecessor and is numbered last). A value read at a width above its
//! definition's names the pin's `COPY` into the wider class
//! (`undef %18.sub_32bit:gr64 = COPY %0` before an `LEA64_32r`), which makes it
//! a value with another copy for `isTerminalReg`; here it is no copy. Measured
//! (fib -Os x86-64, pin MIR vs ours): the three together give the pin's copy
//! structure (`mov ebx, edi` once, the loop-carried copy at the latch) and, on
//! the bench corpus, m64 -O2 +189 B (9 worse cells, sfm16_float* +23), -O3
//! +136, m32 -Os +91 (jpeg_single +48), against -10/-36 and -32/-49/-58 at the
//! other levels; fib -Os executes 220.5 K -> 235 K (the pin's shape: a block
//! `mov eax, ebx; jmp` on the early exit). The widening alone: m64 -20 B at
//! -O2/-O3 (11 cells, none worse), fib -Os +4 B and +6.8 K executed.
//! Zero-worse-cells holds, so none is applied; each is the pin's.
//!
//! DEVIATE C9 (no number): a parameter's value on entry is defined by no
//! instruction, so LLVM's phi rule ("any real interference would show up in a
//! predecessor") does not carry over: the entry block has none. Two parameters
//! are two values and their definitions at the entry do not merge. LLVM's
//! arguments are `COPY`s of the incoming registers, separate definitions.
//!
//! # Example
//! `v2 := mov v1; ...; use v2` with `v1` dead after the copy: the intervals do
//! not overlap past the copy, `v1` is merged into `v2` (or the reverse, the
//! larger one stays), the copy goes and every `v1` is `v2`.
//!
//! # Used by
//! `coalescer::pass`.

pub mod driver;
pub mod env;
pub mod joinvals;
pub mod pair;
pub mod pass;
pub mod register_coalescer;

#[cfg(test)]
mod tests;

pub use pair::{CoalescerEnv, CoalescerPair};
pub use register_coalescer::{JoinResult, join_copy};
