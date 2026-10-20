//! The contract of an inline routine the compiler places where an operation
//! has no instruction (`llrm_x86::helpers`): what it reads and what it changes.
//!
//! # How it works
//! `_helper` makes the [`crate::abi::runtime::Contract`] of such a routine: it
//! returns, pops nothing, reads and writes no memory, and clobbers the
//! registers it is given. Its operands and results are in registers it
//! states, and the clobber list describes the inline bytes exactly (it is not
//! a separately called routine). `runtime_registers` maps register views
//! (`AX`, `BL`, ...) to the runtime registers they belong to, and adds the
//! flags where the routine changes them.
//!
//! # Example
//! No honest example: this builds a routine's contract and rewrites no LIR.
//! Read `isel/wide.rs` for the instructions that name it.
//!
//! # Used by
//! `isel/wide` (64-bit arithmetic helpers) and `isel/unwind`.

use std::collections::BTreeSet;

use llrm_lir::registers::{RegId, Regs};

use crate::abi::runtime::{self, Reg};

pub(crate) fn _helper(
    name: &str,
    inputs: BTreeSet<Reg>,
    clobbers: BTreeSet<Reg>,
) -> runtime::Contract {
    runtime::Contract {
        name: name.to_owned(),
        cleanup: Some(0),
        control: runtime::Control::Returns,
        enters_user_code: false,
        raises_error: false,
        error_handling: false,
        writes: runtime::Memory::None,
        reads: runtime::Memory::None,
        clobbers,
        established: true,
        evidence: "an inline routine of the compiler's; its operands and results are in the registers it states"
            .to_owned(),
        documented: None,
        inputs: Some(inputs),
        direct_inputs: None,
        clobbers_reached: true,
        caller_cleanup: 0,
        // Not a separately called 386 routine: `clobbers` describes the inline
        // bytes exactly.
        i386: false,
        direct_writes: None,
        flags_result: false,
        direct_reads: None,
    }
}

/// The runtime registers `registers` are views of, and the flags too where
/// `flags` says so. A register is the runtime's by the name of its word view
/// in the target's description.
pub(crate) fn runtime_registers(
    regs: &Regs,
    registers: &[RegId],
    flags: bool,
) -> BTreeSet<Reg> {
    let mut out: BTreeSet<Reg> = registers
        .iter()
        .map(|&register| {
            let root = regs.root(register);
            let word = regs.view(root, 16).unwrap_or(root);
            let name = regs.name(word).unwrap_or_default();
            Reg::ALL
                .into_iter()
                .find(|one| one.value() == name)
                .unwrap_or_else(|| panic!("an inline routine names {register:?}"))
        })
        .collect();
    if flags {
        out.insert(Reg::Flags);
    }
    out
}
