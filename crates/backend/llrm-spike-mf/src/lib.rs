//! A target-neutral machine function and register allocator for the m68k and
//! 6502 spikes: instructions on virtual registers, phi elimination, two-address
//! rewriting, a graph-colouring allocator and a spill layer, over a
//! [`Target`]'s register classes.
//!
//! DEVIATE: spike-local, not `llrm-lir`'s function (its `RegId` is numbered by
//! iced's x86 registers; here an id is an opaque register of the target's own
//! `registers.regs`). The allocator is not the pin's greedy one. Reason: there
//! is no neutral machine function yet (neutral-lir-need.md). Exit: the
//! neutral-LIR task.

pub mod cfg;
pub mod commute;
pub mod fold;
pub mod mfunc;
pub mod placement;
pub mod regalloc;
pub mod reschedule;
pub mod sink;
pub mod spill;

pub use commute::commute_tied;
pub use fold::{branch_fold, remove_identity_copies, remove_redundant_copies, remove_redundant_copies_between};
pub use mfunc::{Block, Inst, Kind, MFunction, Operand, Phi, Reg};
pub use regalloc::{Allocate, Assignment, GraphColoring, Target};
pub use reschedule::reschedule_kills;
pub use sink::{Jump, machine_sink, machine_sink_in_place, split_critical_edge};
