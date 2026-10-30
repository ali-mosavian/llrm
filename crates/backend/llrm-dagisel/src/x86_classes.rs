//! Which register class each part of a memory reference takes (SD4).
//!
//! # How it works
//! A memory reference is five operands the pin tags `OPERAND_MEMORY`: base,
//! scale, index, displacement, segment. [`memory_reference`] finds where they
//! start in an [`InstrDesc`] and [`role_of`] names an operand. [`class_tag`]
//! gives the register tag (`registers.regs`) a value in that role is
//! constrained to: in real mode a base is `base` (BX, BP), an index is `index`
//! (SI, DI); at 32 bits any general register serves (the pin's `ptr_rc_nosp`
//! for an index is the allocator's business, ESP being reserved). The emitter
//! calls the same way the pin's `InstrEmitter::AddRegisterOperand` calls
//! `constrainRegClass`.
//!
//! # Used by
//! The emitter (sdag-d), when it turns a machine node's operands into LIR
//! values.

use crate::instr::{InstrDesc, operand_type};
use crate::x86_addr::AddrMode;

/// A part of a memory reference, in operand order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Base,
    Scale,
    Index,
    Disp,
    Segment,
}

const ROLES: [Role; 5] = [Role::Base, Role::Scale, Role::Index, Role::Disp, Role::Segment];

/// The first operand of the instruction's memory reference, if it has one: a
/// run of five memory operands whose first is a pointer class.
pub fn memory_reference(desc: &InstrDesc) -> Option<usize> {
    (0..desc.operands.len().saturating_sub(4)).find(|&at| {
        desc.operands[at].pointer_class && desc.operands[at..at + 5].iter().all(|o| o.kind == operand_type::MEMORY)
    })
}

/// What operand `operand` of the instruction is within its memory reference.
pub fn role_of(
    desc: &InstrDesc,
    operand: usize,
) -> Option<Role> {
    let start = memory_reference(desc)?;
    ROLES.get(operand.checked_sub(start)?).copied()
}

/// The register tag a value in `role` is constrained to, `None` when any
/// register will do.
pub fn class_tag(
    mode: AddrMode,
    role: Role,
) -> Option<&'static str> {
    match (mode, role) {
        (AddrMode::Real16, Role::Base) => Some("base"),
        (AddrMode::Real16, Role::Index) => Some("index"),
        _ => None,
    }
}
