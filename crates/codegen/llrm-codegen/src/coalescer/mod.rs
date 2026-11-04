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
//! DEVIATE C3 (no number): no IMPLICIT_DEF, so `ErasableImplicitDef`,
//! `removeImplicitDefs` and `eliminateUndefCopy` are not translated. Fact:
//! phi elimination here leaves no `IMPLICIT_DEF`.
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
//! cell-CPU rows. Exit: 2026-11-15, re-evaluated when the allocator-sensitivity
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
