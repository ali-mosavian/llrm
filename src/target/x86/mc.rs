//! Lowering of allocated x86 Machine IR instructions to MC instructions.
//!
//! This boundary deliberately handles one instruction at a time. Block and
//! symbol operands need section-wide symbol assignment and therefore remain
//! explicit refusals until module lowering owns that context.

use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    MachineInstruction, MachineOperandKind, MachineRegister, OperandRole,
};
use crate::mc;

use super::{X86Opcode, X86Register};

/// An unresolved operand kind which instruction-local MC lowering cannot map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnresolvedOperand {
    VirtualRegister,
    FrameIndex,
    Block,
    Global,
    ExternalSymbol,
}

/// A malformed or unresolved x86 Machine IR instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McLowerError {
    UnknownOpcode {
        raw: u32,
    },
    UnknownRegister {
        operand: usize,
        raw: u32,
    },
    ResidualConstraint {
        operand: usize,
    },
    ResidualTie {
        operand: usize,
    },
    InvalidRole {
        operand: usize,
    },
    UnresolvedOperand {
        operand: usize,
        kind: UnresolvedOperand,
    },
}

impl fmt::Display for McLowerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownOpcode { raw } => write!(formatter, "unknown x86 opcode {raw}"),
            Self::UnknownRegister { operand, raw } => {
                write!(
                    formatter,
                    "operand {operand} names unknown x86 register {raw}"
                )
            }
            Self::ResidualConstraint { operand } => {
                write!(
                    formatter,
                    "operand {operand} retains an allocation constraint"
                )
            }
            Self::ResidualTie { operand } => {
                write!(formatter, "operand {operand} retains an allocation tie")
            }
            Self::InvalidRole { operand } => {
                write!(
                    formatter,
                    "operand {operand} has an invalid Machine IR role"
                )
            }
            Self::UnresolvedOperand { operand, kind } => {
                write!(formatter, "operand {operand} remains unresolved: {kind:?}")
            }
        }
    }
}

impl Error for McLowerError {}

/// Lowers one fully allocated x86 instruction into the target-neutral MC form.
pub fn lower_instruction(
    instruction: &MachineInstruction,
) -> Result<mc::MCInstruction, McLowerError> {
    let opcode =
        X86Opcode::from_machine_opcode(instruction.opcode).ok_or(McLowerError::UnknownOpcode {
            raw: instruction.opcode.get(),
        })?;
    let operands = instruction
        .operands
        .iter()
        .enumerate()
        .map(|(index, operand)| {
            if operand.constraint.is_some() {
                return Err(McLowerError::ResidualConstraint { operand: index });
            }
            if operand.tied_to.is_some() {
                return Err(McLowerError::ResidualTie { operand: index });
            }
            match operand.kind {
                MachineOperandKind::Register(MachineRegister::Physical(register)) => {
                    if matches!(operand.role, OperandRole::None) {
                        return Err(McLowerError::InvalidRole { operand: index });
                    }
                    X86Register::from_physical(register).ok_or(McLowerError::UnknownRegister {
                        operand: index,
                        raw: register.get(),
                    })?;
                    Ok(mc::MCOperand::Register(mc::PhysicalRegister::new(
                        register.get(),
                    )))
                }
                MachineOperandKind::Register(MachineRegister::Virtual(_)) => {
                    Err(McLowerError::UnresolvedOperand {
                        operand: index,
                        kind: UnresolvedOperand::VirtualRegister,
                    })
                }
                MachineOperandKind::Immediate(value) => {
                    if !matches!(operand.role, OperandRole::None) {
                        return Err(McLowerError::InvalidRole { operand: index });
                    }
                    Ok(mc::MCOperand::Immediate(value))
                }
                MachineOperandKind::FrameIndex { .. } => {
                    unresolved(index, UnresolvedOperand::FrameIndex)
                }
                MachineOperandKind::Block(_) => unresolved(index, UnresolvedOperand::Block),
                MachineOperandKind::Global { .. } => unresolved(index, UnresolvedOperand::Global),
                MachineOperandKind::ExternalSymbol { .. } => {
                    unresolved(index, UnresolvedOperand::ExternalSymbol)
                }
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(mc::MCInstruction {
        opcode: mc::TargetOpcode::new(opcode.machine_opcode().get()),
        operands,
    })
}

fn unresolved(operand: usize, kind: UnresolvedOperand) -> Result<mc::MCOperand, McLowerError> {
    Err(McLowerError::UnresolvedOperand { operand, kind })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        self, InstructionFlags, MachineInstructionId, MachineOperand, PhysicalRegister,
        TargetOpcode,
    };

    fn operand(kind: MachineOperandKind, role: OperandRole) -> MachineOperand {
        MachineOperand {
            kind,
            role,
            constraint: None,
            tied_to: None,
        }
    }

    fn instruction(opcode: TargetOpcode, operands: Vec<MachineOperand>) -> MachineInstruction {
        MachineInstruction {
            id: MachineInstructionId::new(7),
            opcode,
            operands,
            flags: InstructionFlags::NONE,
        }
    }

    #[test]
    fn lowers_allocated_register_and_immediate_operands_in_order() {
        let instruction = instruction(
            X86Opcode::Mov.machine_opcode(),
            vec![
                operand(
                    MachineOperandKind::Register(MachineRegister::Physical(
                        X86Register::Ax.physical(),
                    )),
                    OperandRole::Def,
                ),
                operand(MachineOperandKind::Immediate(42), OperandRole::None),
            ],
        );

        let lowered = lower_instruction(&instruction).unwrap();

        assert_eq!(lowered.opcode.get(), X86Opcode::Mov as u32);
        assert_eq!(
            lowered.operands,
            vec![
                mc::MCOperand::Register(mc::PhysicalRegister::new(X86Register::Ax as u32,)),
                mc::MCOperand::Immediate(42),
            ]
        );
        assert_eq!(super::super::encode(&lowered).unwrap(), vec![0xb8, 42, 0]);
    }

    #[test]
    fn refuses_virtual_registers_and_unknown_opcodes() {
        let virtual_instruction = instruction(
            X86Opcode::Mov.machine_opcode(),
            vec![operand(
                MachineOperandKind::Register(MachineRegister::Virtual(
                    machine::VirtualRegisterId::new(0),
                )),
                OperandRole::Use,
            )],
        );
        assert_eq!(
            lower_instruction(&virtual_instruction),
            Err(McLowerError::UnresolvedOperand {
                operand: 0,
                kind: UnresolvedOperand::VirtualRegister,
            })
        );

        let unknown = instruction(TargetOpcode::new(99), Vec::new());
        assert_eq!(
            lower_instruction(&unknown),
            Err(McLowerError::UnknownOpcode { raw: 99 })
        );
    }

    #[test]
    fn refuses_stale_allocation_metadata() {
        let mut constrained = operand(
            MachineOperandKind::Register(MachineRegister::Physical(PhysicalRegister::new(
                X86Register::Ax as u32,
            ))),
            OperandRole::Use,
        );
        constrained.constraint = Some(machine::RegisterConstraint::Class(
            super::super::X86RegisterClass::Word.machine_class(),
        ));
        let instruction = instruction(X86Opcode::Mov.machine_opcode(), vec![constrained]);

        assert_eq!(
            lower_instruction(&instruction),
            Err(McLowerError::ResidualConstraint { operand: 0 })
        );
    }
}
