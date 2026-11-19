//! A call of a function the module defines, pc-relative: `jsr (name,%pc)` is
//! 4 B and 2 clk shorter on the 68000 than the `jsr name` (absolute long) the
//! pin selects for a non-PIC call.
//!
//! # How it works
//! [`local_calls`] turns each `CALLb` of a symbol `local` accepts into `CALLq`.
//! The displacement is 16 bits, so the driver turns the option on only when
//! the whole module's code is under [`REACH`] (any two points of one section
//! are then in reach); a longer module keeps the absolute call.
//!
//! DEVIATE: the pin selects `CALLq` for PIC alone (`CallPat<CALLq, IsPIC>`).
//! Reason: best of gcc and clang is the goal; gcc calls a function of the same
//! file pc-relative (memory_expansion: 528 calls of 6 B). Owner: m68k. Exit:
//! 2026-12-12, the shared flow chooses the call form by the callee's reach.
//!
//! # See also
//! `M68kInstrControl.td` (`CallPat`), [`crate::calltarget`].

use llrm_spike_mf::{Kind, Operand};

use crate::mfunc::MFunction;

/// The largest `.text` for which every pc-relative 16-bit displacement reaches:
/// the displacement is signed, so a span under 32 KB is in reach both ways.
pub const REACH: usize = 0x7000;

/// Each `CALLb` of a symbol `local` accepts becomes a pc-relative call.
pub fn local_calls(
    function: &mut MFunction,
    local: impl Fn(&str) -> bool,
) {
    for inst in function.blocks.iter_mut().flat_map(|block| &mut block.insts) {
        if let (Kind::Op("CALLb"), Some(Operand::Symbol(name, 0))) = (inst.kind, inst.operands.first())
            && local(name)
        {
            inst.kind = Kind::Op("CALLq");
        }
    }
}
