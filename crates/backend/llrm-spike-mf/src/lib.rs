//! A target-neutral machine function and register allocator for the m68k and
//! 6502 spikes: instructions on virtual registers, phi elimination, two-address
//! rewriting and a graph-colouring allocator, over a [`Target`]'s register
//! classes.
//!
//! DEVIATE: spike-local, not `llrm-lir`'s function (its `RegId` is numbered by
//! iced's x86 registers; here an id is an opaque register of the target's own
//! `registers.regs`). The allocator is not the pin's greedy one and does not
//! spill. Reason: there is no neutral machine function yet
//! (neutral-lir-need.md). Exit: the neutral-LIR task.

pub mod cfg;
pub mod fold;
pub mod mfunc;
pub mod regalloc;
pub mod sink;

pub use fold::{branch_fold, remove_identity_copies};
pub use mfunc::{Block, Inst, Kind, MFunction, Operand, Phi, Reg};
pub use regalloc::{Allocate, Assignment, GraphColoring, Target};
pub use sink::{Jump, machine_sink, split_critical_edge};
