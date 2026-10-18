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
//! `driver` is `joinAllIntervals`: the order copies are tried in; `shadow` runs
//! it beside the old coalescer. `join_copy` runs `RegisterCoalescer::joinCopy`
//! for one copy: refuse, or join the intervals, erase the copies that became
//! identities and rename the absorbed value in every instruction.
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
//! DEVIATE C4 (exit: the next coalescer PR): `reMaterializeDef`,
//! `adjustCopiesBackFrom`, `removeCopyByCommutingDef` and
//! `removePartialRedundancy` are not translated yet; a copy whose intervals
//! cannot join is left.
//!
//! # Example
//! `v2 := mov v1; ...; use v2` with `v1` dead after the copy: the intervals do
//! not overlap past the copy, `v1` is merged into `v2` (or the reverse, the
//! larger one stays), the copy goes and every `v1` is `v2`.
//!
//! # Used by
//! `backend::coalesce`, in shadow (`LLRM_SHADOW_COALESCE`, see `shadow`).

pub mod driver;
pub mod joinvals;
pub mod pair;
pub mod register_coalescer;
pub mod shadow;

#[cfg(test)]
mod tests;

pub use pair::{CoalescerEnv, CoalescerPair};
pub use register_coalescer::{JoinResult, join_copy};
