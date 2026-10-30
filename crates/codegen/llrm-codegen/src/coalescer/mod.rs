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
