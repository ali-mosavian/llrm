//! x86 target hooks for target-independent register assignment.

use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    self, AllocationError, MachineCallingConvention, MachineFunction, MachineOperandKind,
    RegisterAssignment, RegisterClass,
};

use super::{X86Opcode, X86Register, X86RegisterClass};

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

    let reserve_bp = function.signature.calling_convention == MachineCallingConvention::Basic
        || !function.frame_objects.is_empty()
        || uses_basic_runtime_frame(function);
    machine::allocate(function, |class| candidates(class, reserve_bp), overlaps)
        .map_err(X86AllocationError::Allocation)
}

fn candidates(class: RegisterClass, reserve_bp: bool) -> Vec<machine::PhysicalRegister> {
    X86RegisterClass::from_machine_class(class).map_or_else(Vec::new, |class| {
        class
            .allocation_order()
            .iter()
            .filter(|register| {
                !reserve_bp || !matches!(register, X86Register::Bp | X86Register::Ebp)
            })
            .map(|register| register.physical())
            .collect()
    })
}

fn uses_basic_runtime_frame(function: &MachineFunction) -> bool {
    function.blocks.iter().any(|block| {
        block.instructions.iter().any(|instruction| {
            instruction.opcode == X86Opcode::CallFar.machine_opcode()
                && matches!(
                    instruction.operands.first().map(|operand| &operand.kind),
                    Some(MachineOperandKind::ExternalSymbol { name, .. }) if name == "B$ENRA"
                )
        })
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
        FrameIndex, FrameObject, FrameObjectKind, InstructionFlags, MachineBlock, MachineBlockId,
        MachineFunctionId, MachineInstruction, MachineInstructionId, MachineOperand,
        MachineOperandKind, MachineRegister, OperandRole, TargetOpcode, VirtualRegister,
        VirtualRegisterId,
    };
    use crate::target::x86::materialize_far_call_clobbers;

    #[test]
    fn respects_aliases_between_word_and_dword_views() {
        let function = MachineFunction {
            id: MachineFunctionId::new(0),
            name: "aliasing".into(),
            linkage: crate::codegen::machine::MachineLinkage::Internal,
            signature: crate::codegen::machine::MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: crate::codegen::machine::MachineCallingConvention::Basic,
            },
            entry: MachineBlockId::new(0),
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
                    operands: vec![virtual_definition(0), virtual_definition(1)],
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

    #[test]
    fn runtime_frames_reserve_bp_and_far_call_clobbers_force_a_spill_refusal() {
        // COM_CHECK_ARGS once put a spill at BP-2 and corrupted FindFrame.
        // A BASIC frame owns BP, so a value live through all six caller
        // clobbers must request spilling rather than quietly taking BP.
        let mut function = MachineFunction {
            id: MachineFunctionId::new(0),
            name: "framed".into(),
            linkage: crate::codegen::machine::MachineLinkage::External,
            signature: crate::codegen::machine::MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: crate::codegen::machine::MachineCallingConvention::Basic,
            },
            entry: MachineBlockId::new(0),
            virtual_registers: vec![VirtualRegister {
                id: VirtualRegisterId::new(0),
                class: X86RegisterClass::Word.machine_class(),
            }],
            blocks: vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![
                    MachineInstruction {
                        id: MachineInstructionId::new(0),
                        opcode: X86Opcode::Mov.machine_opcode(),
                        operands: vec![
                            virtual_definition(0),
                            MachineOperand {
                                kind: MachineOperandKind::Immediate(1),
                                role: OperandRole::None,
                                constraint: None,
                                tied_to: None,
                            },
                        ],
                        flags: InstructionFlags::NONE,
                    },
                    MachineInstruction {
                        id: MachineInstructionId::new(1),
                        opcode: X86Opcode::CallFar.machine_opcode(),
                        operands: vec![MachineOperand {
                            kind: MachineOperandKind::ExternalSymbol {
                                name: "B$FOO".into(),
                                addend: 0,
                            },
                            role: OperandRole::None,
                            constraint: None,
                            tied_to: None,
                        }],
                        flags: InstructionFlags {
                            call: true,
                            ..InstructionFlags::NONE
                        },
                    },
                    MachineInstruction {
                        id: MachineInstructionId::new(2),
                        opcode: X86Opcode::Push.machine_opcode(),
                        operands: vec![MachineOperand {
                            kind: MachineOperandKind::Register(MachineRegister::Virtual(
                                VirtualRegisterId::new(0),
                            )),
                            role: OperandRole::Use,
                            constraint: None,
                            tied_to: None,
                        }],
                        flags: InstructionFlags::NONE,
                    },
                ],
                successors: Vec::new(),
            }],
            frame_objects: vec![FrameObject {
                index: FrameIndex::new(0),
                size: 2,
                alignment: 2,
                kind: FrameObjectKind::Local,
            }],
        };
        function = materialize_far_call_clobbers(&function).unwrap();

        assert!(matches!(
            allocate_registers(&function),
            Err(X86AllocationError::Allocation(AllocationError::NoRegister {
                register,
                ..
            })) if register == VirtualRegisterId::new(0)
        ));
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
