//! The contract of an inline routine the compiler places where an operation has
//! no instruction (`llrm_x86::helpers`): what it reads and what it changes.

use std::collections::BTreeSet;

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
/// `flags` says so.
pub(crate) fn runtime_registers(
    registers: &[iced_x86::Register],
    flags: bool,
) -> BTreeSet<Reg> {
    let mut out: BTreeSet<Reg> = registers
        .iter()
        .map(|register| match register.full_register32() {
            iced_x86::Register::EAX => Reg::Ax,
            iced_x86::Register::EBX => Reg::Bx,
            iced_x86::Register::ECX => Reg::Cx,
            iced_x86::Register::EDX => Reg::Dx,
            other => panic!("an inline routine names {other:?}"),
        })
        .collect();
    if flags {
        out.insert(Reg::Flags);
    }
    out
}
