//! Register allocation for the m68k machine function: m68k's register
//! classes behind the neutral allocator of [`llrm_spike_mf`].
//!
//! The classes are `DR8`/`DR16`/`DR32` (the views of `d0` to `d7`), `AR32`
//! (`a0` to `a5`; `a6` and `sp` are the frame and stack) and `XR32` (both).

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::Target;
pub use llrm_spike_mf::regalloc::{Allocate, Assignment};
use llrm_spike_mf::{Jump, Kind, Operand};

use crate::mfunc::{Class, MFunction};
use crate::{REGISTER_INFO, reg};

const DATA: [RegId; 8] = [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7];
const ADDRESS: [RegId; 6] = [reg::A0, reg::A1, reg::A2, reg::A3, reg::A4, reg::A5];

/// The registers a call leaves unspecified (`CSR_STD` keeps the rest).
const CALLER_SAVED: [RegId; 4] = [reg::D0, reg::D1, reg::A0, reg::A1];

/// m68k's register file for the allocator.
pub struct M68kRegisters;

impl Target for M68kRegisters {
    type Class = Class;

    fn meet(
        one: Class,
        other: Class,
    ) -> Option<Class> {
        one.meet(other)
    }

    fn any() -> Class {
        Class::Xr32
    }

    fn candidates(class: Class) -> Vec<RegId> {
        match class {
            Class::Dr8 | Class::Dr16 | Class::Dr32 => DATA.to_vec(),
            Class::Ar32 => ADDRESS.to_vec(),
            Class::Xr32 => DATA.iter().chain(&ADDRESS).copied().collect(),
        }
    }

    /// `bd0` for a byte class, `wd0` for a word.
    fn view(
        root: RegId,
        class: Class,
    ) -> RegId {
        let bits = match class {
            Class::Dr8 => 8,
            Class::Dr16 => 16,
            _ => 32,
        };
        REGISTER_INFO.view(root, bits).unwrap_or(root)
    }

    fn root(register: RegId) -> RegId {
        REGISTER_INFO.root(register)
    }

    fn reserved(root: RegId) -> bool {
        root == reg::SP || root == reg::A6
    }

    fn caller_saved() -> Vec<RegId> {
        CALLER_SAVED.to_vec()
    }
}

pub type GraphColoring = llrm_spike_mf::GraphColoring<M68kRegisters>;

pub use llrm_spike_mf::regalloc::{eliminate_phis, successors, two_address};

pub fn fold_reserved_copies(function: &mut MFunction) {
    llrm_spike_mf::regalloc::fold_reserved_copies::<M68kRegisters>(function)
}

/// `MachineSinking` over the m68k function.
pub fn machine_sink(function: &mut MFunction) -> bool {
    llrm_spike_mf::machine_sink::<M68kRegisters>(function)
}

impl Jump for M68kRegisters {
    fn jump(block: u32) -> llrm_spike_mf::Inst {
        let mut inst = llrm_spike_mf::Inst::new(Kind::Op("BRA8"), vec![Operand::Block(block)]);
        inst.terminator = true;
        inst.barrier = true;
        inst
    }
}
