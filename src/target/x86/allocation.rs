//! x86 target hooks for target-independent register assignment.

use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    self, AllocationError, MachineFunction, RegisterAssignment, RegisterClass,
};

use super::{X86Register, X86RegisterClass};

/// A target-description or generic allocation refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86AllocationError {
    UnknownRegisterClass(RegisterClass),
    Allocation(AllocationError),
}

impl fmt::Display for X86AllocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRegisterClass(class) => {
                write!(formatter, "unknown x86 register class {class}")
            }
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}

impl Error for X86AllocationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::UnknownRegisterClass(_) => None,
            Self::Allocation(error) => Some(error),
        }
    }
}

/// Assigns x86 registers using the target's stable preference and alias data.
pub fn allocate_registers(
    function: &MachineFunction,
) -> Result<RegisterAssignment, X86AllocationError> {
    for register in &function.virtual_registers {
        if X86RegisterClass::from_machine_class(register.class).is_none() {
            return Err(X86AllocationError::UnknownRegisterClass(register.class));
        }
    }

    machine::allocate(function, candidates, overlaps).map_err(X86AllocationError::Allocation)
}

fn candidates(class: RegisterClass) -> Vec<machine::PhysicalRegister> {
    X86RegisterClass::from_machine_class(class).map_or_else(Vec::new, |class| {
        class
            .allocation_order()
            .iter()
            .map(|register| register.physical())
            .collect()
    })
}

fn overlaps(left: machine::PhysicalRegister, right: machine::PhysicalRegister) -> bool {
    match (
        X86Register::from_physical(left),
        X86Register::from_physical(right),
    ) {
        (Some(left), Some(right)) => left.overlaps(right),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        InstructionFlags, MachineBlock, MachineBlockId, MachineFunctionId, MachineInstruction,
        MachineInstructionId, MachineOperand, MachineOperandKind, MachineRegister, OperandRole,
        TargetOpcode, VirtualRegister, VirtualRegisterId,
    };

    #[test]
    fn respects_aliases_between_word_and_dword_views() {
        let function = MachineFunction {
            id: MachineFunctionId::new(0),
            name: "aliasing".into(),
            virtual_registers: vec![
                VirtualRegister {
                    id: VirtualRegisterId::new(0),
                    class: X86RegisterClass::Word.machine_class(),
                },
                VirtualRegister {
                    id: VirtualRegisterId::new(1),
                    class: X86RegisterClass::Dword.machine_class(),
                },
            ],
            blocks: vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![MachineInstruction {
                    id: MachineInstructionId::new(0),
                    opcode: TargetOpcode::new(0),
                    operands: vec![
                        virtual_definition(0),
                        virtual_definition(1),
                    ],
                    flags: InstructionFlags::NONE,
                }],
                successors: Vec::new(),
            }],
            frame_objects: Vec::new(),
        };

        let assignment = allocate_registers(&function).unwrap();

        assert_eq!(
            assignment.get(VirtualRegisterId::new(0)),
            Some(X86Register::Ax.physical())
        );
        assert_eq!(
            assignment.get(VirtualRegisterId::new(1)),
            Some(X86Register::Ecx.physical())
        );
    }

    fn virtual_definition(id: u32) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(
                id,
            ))),
            role: OperandRole::Def,
            constraint: None,
            tied_to: None,
        }
    }
}
