//! Register allocation for the m68k machine function: m68k's register
//! classes behind the neutral allocator of [`llrm_spike_mf`].
//!
//! The classes are `DR8`/`DR16`/`DR32` (the views of `d0` to `d7`), `AR32`
//! (`a0` to `a5`; `a6` and `sp` are the frame and stack) and `XR32` (both).

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::Target;
pub use llrm_spike_mf::regalloc::{Allocate, Assignment};
use llrm_spike_mf::{Jump, Kind, Operand, Reg};

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
            // the registers a call clobbers anyway first, then the saved ones:
            // a saved register costs a push and a pop (`RegisterClassInfo`
            // orders the callee-saved last)
            Class::Xr32 => CALLER_SAVED
                .iter()
                .chain(DATA.iter().filter(|one| !CALLER_SAVED.contains(one)))
                .chain(ADDRESS.iter().filter(|one| !CALLER_SAVED.contains(one)))
                .copied()
                .collect(),
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

    /// `getStoreRegOpcode`: a `MOVEM` pseudo, since a `move` would overwrite
    /// the condition codes a branch may still read.
    fn spill_store(
        _class: Class,
        register: Reg,
        slot: i32,
    ) -> Option<llrm_spike_mf::Inst> {
        Some(llrm_spike_mf::Inst {
            may_store: true,
            ..llrm_spike_mf::Inst::new(
                Kind::Op("MOVM32pm_P"),
                vec![Operand::Imm(0), Operand::Frame(slot), Operand::Use(register)],
            )
        })
    }

    /// `getLoadRegOpcode`.
    fn spill_load(
        _class: Class,
        register: Reg,
        slot: i32,
    ) -> Option<llrm_spike_mf::Inst> {
        Some(llrm_spike_mf::Inst {
            may_load: true,
            ..llrm_spike_mf::Inst::new(
                Kind::Op("MOVM32mp_P"),
                vec![Operand::Def(register), Operand::Imm(0), Operand::Frame(slot)],
            )
        })
    }
}

pub type GraphColoring = llrm_spike_mf::GraphColoring<M68kRegisters>;

pub use llrm_spike_mf::regalloc::{eliminate_phis, eliminate_phis_with, successors, two_address};

pub fn fold_reserved_copies(function: &mut MFunction) {
    llrm_spike_mf::regalloc::fold_reserved_copies::<M68kRegisters>(function)
}

/// Allocation that spills what does not fit; the slots it made, with their
/// classes.
pub fn allocate_spilling(
    function: &mut MFunction,
    first_slot: i32,
) -> Result<(Assignment, Vec<(i32, Class)>), String> {
    llrm_spike_mf::spill::allocate_spilling::<M68kRegisters>(function, first_slot)
}

/// `MachineSinking` over the m68k function.
pub fn machine_sink(function: &mut MFunction) -> bool {
    llrm_spike_mf::machine_sink::<M68kRegisters>(function)
}

/// `reverseBranchCondition` for `Bcc8`: the opposite condition is the
/// condition code xor 1.
const CONDITIONAL: [(&str, &str); 7] = [
    ("Bhi8", "Bls8"),
    ("Bcc8", "Bcs8"),
    ("Bne8", "Beq8"),
    ("Bvc8", "Bvs8"),
    ("Bpl8", "Bmi8"),
    ("Bge8", "Blt8"),
    ("Bgt8", "Ble8"),
];

/// `BranchFolder` over the m68k function.
pub fn branch_fold(function: &mut MFunction) -> bool {
    llrm_spike_mf::branch_fold::<M68kRegisters>(function)
}

pub fn remove_identity_copies(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    llrm_spike_mf::remove_identity_copies::<M68kRegisters>(function, assignment)
}

/// `TwoAddressInstructionPass`'s choice of the tied source.
pub fn commute_tied(function: &mut MFunction) {
    llrm_spike_mf::commute_tied::<M68kRegisters>(function)
}

/// `MachineCopyPropagation`'s spurious copies; the 8 and 16-bit classes copy
/// part of a register and do not count.
pub fn remove_redundant_copies(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    llrm_spike_mf::remove_redundant_copies::<M68kRegisters>(function, assignment, |class| {
        matches!(class, Class::Dr32 | Class::Ar32 | Class::Xr32)
    })
}

impl Jump for M68kRegisters {
    fn reverse(branch: &llrm_spike_mf::Inst) -> Option<llrm_spike_mf::Inst> {
        let Kind::Op(name) = branch.kind else { return None };
        let opposite = CONDITIONAL
            .iter()
            .find_map(
                |&(one, two)| match () {
                    _ if one == name => Some(two),
                    _ if two == name => Some(one),
                    _ => None,
                },
            )?;
        Some(llrm_spike_mf::Inst { kind: Kind::Op(opposite), ..branch.clone() })
    }

    fn jump(block: u32) -> llrm_spike_mf::Inst {
        let mut inst = llrm_spike_mf::Inst::new(Kind::Op("BRA8"), vec![Operand::Block(block)]);
        inst.terminator = true;
        inst.barrier = true;
        inst
    }
}
