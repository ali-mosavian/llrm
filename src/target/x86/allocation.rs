//! x86 target hooks for target-independent register assignment.

use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    self, AllocationError, ConstraintError, InstructionFlags, MachineCallingConvention,
    MachineFunction, MachineInstruction, MachineInstructionId, MachineOperand, MachineOperandKind,
    MachineRegister, OperandRole, RegisterAssignment, RegisterClass, VirtualRegisterId,
};

use super::{X86Opcode, X86Register, X86RegisterClass};

/// A target-description or generic allocation refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86AllocationError {
    UnknownRegisterClass(RegisterClass),
    Constraint(ConstraintError),
    Allocation(AllocationError),
}

impl fmt::Display for X86AllocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRegisterClass(class) => {
                write!(formatter, "unknown x86 register class {class}")
            }
            Self::Constraint(error) => error.fmt(formatter),
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}

impl Error for X86AllocationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::UnknownRegisterClass(_) => None,
            Self::Constraint(error) => Some(error),
            Self::Allocation(error) => Some(error),
        }
    }
}

struct X86ConstraintTarget;

impl machine::ConstraintTarget for X86ConstraintTarget {
    fn copy(
        &self,
        id: MachineInstructionId,
        destination: VirtualRegisterId,
        source: VirtualRegisterId,
    ) -> MachineInstruction {
        MachineInstruction {
            id,
            opcode: X86Opcode::Copy.machine_opcode(),
            operands: vec![
                MachineOperand {
                    kind: MachineOperandKind::Register(MachineRegister::Virtual(destination)),
                    role: OperandRole::Def,
                    constraint: None,
                    tied_to: None,
                },
                MachineOperand {
                    kind: MachineOperandKind::Register(MachineRegister::Virtual(source)),
                    role: OperandRole::Use,
                    constraint: None,
                    tied_to: None,
                },
            ],
            flags: InstructionFlags {
                copy: true,
                ..InstructionFlags::NONE
            },
        }
    }
}

/// Gives every instruction-local fixed-register requirement a short virtual.
///
/// This is the x86 half of Python `backend.constrain.constrained`: target
/// opcodes remain target-owned, while the generic splitter owns lifetimes.
/// Run it after ABI and call-clobber construction and immediately before
/// allocation.  A fixed operand is not a whole-range ABI pin.
pub fn split_fixed_register_occurrences(
    function: &MachineFunction,
) -> Result<MachineFunction, X86AllocationError> {
    validate_register_classes(function)?;
    machine::split_fixed_occurrences(function, &X86ConstraintTarget)
        .map_err(X86AllocationError::Constraint)
}

/// Assigns x86 registers using the target's stable preference and alias data.
pub fn allocate_registers(
    function: &MachineFunction,
) -> Result<RegisterAssignment, X86AllocationError> {
    validate_register_classes(function)?;

    let reserve_bp = function.signature.calling_convention == MachineCallingConvention::FarPascal
        || !function.frame_objects.is_empty()
        || uses_basic_runtime_frame(function);
    machine::allocate(function, |class| candidates(class, reserve_bp), overlaps)
        .map_err(X86AllocationError::Allocation)
}

fn validate_register_classes(function: &MachineFunction) -> Result<(), X86AllocationError> {
    for register in &function.virtual_registers {
        if X86RegisterClass::from_machine_class(register.class).is_none() {
            return Err(X86AllocationError::UnknownRegisterClass(register.class));
        }
    }
    Ok(())
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
        MachineOperandKind, MachineRegister, OperandRole, RegisterConstraint, TargetOpcode,
        VirtualRegister, VirtualRegisterId,
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
                calling_convention: crate::codegen::machine::MachineCallingConvention::FarPascal,
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
                calling_convention: crate::codegen::machine::MachineCallingConvention::FarPascal,
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

    #[test]
    fn c_call_result_is_copied_out_of_its_abi_register() {
        // Ported from tests/test_constrain.py's required-destination case:
        // an ABI result belongs to AX at the call, not for its whole life.
        let function = word_function(vec![
            MachineInstruction {
                id: MachineInstructionId::new(0),
                opcode: X86Opcode::CallNear.machine_opcode(),
                operands: vec![
                    MachineOperand {
                        kind: MachineOperandKind::Function(MachineFunctionId::new(1)),
                        role: OperandRole::None,
                        constraint: None,
                        tied_to: None,
                    },
                    fixed_virtual(0, OperandRole::Def, X86Register::Ax),
                ],
                flags: InstructionFlags {
                    call: true,
                    ..InstructionFlags::NONE
                },
            },
            MachineInstruction {
                id: MachineInstructionId::new(1),
                opcode: X86Opcode::Push.machine_opcode(),
                operands: vec![virtual_use(0)],
                flags: InstructionFlags::NONE,
            },
        ]);

        let split = split_fixed_register_occurrences(&function).unwrap();
        let instructions = &split.blocks[0].instructions;

        assert_eq!(split.virtual_registers.len(), 2);
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::CallNear.machine_opcode(),
                X86Opcode::Copy.machine_opcode(),
                X86Opcode::Push.machine_opcode(),
            ]
        );
        assert!(matches!(
            instructions[0].operands[1],
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(register)),
                role: OperandRole::Def,
                constraint: Some(RegisterConstraint::Fixed(physical)),
                ..
            } if register == VirtualRegisterId::new(1) && physical == X86Register::Ax.physical()
        ));
        assert_eq!(
            instructions[1].operands,
            vec![virtual_definition(0), virtual_use(1)]
        );
    }

    #[test]
    fn basic_entry_arguments_are_copied_into_cx_and_bx() {
        // Ported from tests/test_constrain.py's B$ENRA regression: its CX/BX
        // requirements constrain the call occurrences, not the source ranges.
        let function = word_function_with_registers(
            vec![
                MachineInstruction {
                    id: MachineInstructionId::new(0),
                    opcode: X86Opcode::Mov.machine_opcode(),
                    operands: vec![virtual_definition(0), immediate(4)],
                    flags: InstructionFlags::NONE,
                },
                MachineInstruction {
                    id: MachineInstructionId::new(1),
                    opcode: X86Opcode::Mov.machine_opcode(),
                    operands: vec![virtual_definition(1), immediate(0)],
                    flags: InstructionFlags::NONE,
                },
                MachineInstruction {
                    id: MachineInstructionId::new(2),
                    opcode: X86Opcode::CallFar.machine_opcode(),
                    operands: vec![
                        MachineOperand {
                            kind: MachineOperandKind::ExternalSymbol {
                                name: "B$ENRA".into(),
                                addend: 0,
                            },
                            role: OperandRole::None,
                            constraint: None,
                            tied_to: None,
                        },
                        fixed_virtual(0, OperandRole::Use, X86Register::Cx),
                        fixed_virtual(1, OperandRole::Use, X86Register::Bx),
                    ],
                    flags: InstructionFlags {
                        call: true,
                        ..InstructionFlags::NONE
                    },
                },
            ],
            2,
        );

        let split = split_fixed_register_occurrences(&function).unwrap();
        let instructions = &split.blocks[0].instructions;

        assert_eq!(split.virtual_registers.len(), 4);
        assert_eq!(
            instructions[2].operands,
            vec![virtual_definition(2), virtual_use(0)]
        );
        assert_eq!(
            instructions[3].operands,
            vec![virtual_definition(3), virtual_use(1)]
        );
        assert!(matches!(
            instructions[4].operands.as_slice(),
            [_, MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(first)),
                constraint: Some(RegisterConstraint::Fixed(cx)),
                ..
            }, MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(second)),
                constraint: Some(RegisterConstraint::Fixed(bx)),
                ..
            }] if *first == VirtualRegisterId::new(2)
                && *second == VirtualRegisterId::new(3)
                && *cx == X86Register::Cx.physical()
                && *bx == X86Register::Bx.physical()
        ));
    }

    #[test]
    fn synthetic_call_clobbers_are_already_short() {
        let function = word_function(vec![MachineInstruction {
            id: MachineInstructionId::new(0),
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
        }]);
        let clobbered = materialize_far_call_clobbers(&function).unwrap();

        assert_eq!(
            split_fixed_register_occurrences(&clobbered).unwrap(),
            clobbered
        );
    }

    fn word_function(instructions: Vec<MachineInstruction>) -> MachineFunction {
        word_function_with_registers(instructions, 1)
    }

    fn word_function_with_registers(
        instructions: Vec<MachineInstruction>,
        register_count: u32,
    ) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(0),
            name: "function".into(),
            linkage: crate::codegen::machine::MachineLinkage::Internal,
            signature: crate::codegen::machine::MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: crate::codegen::machine::MachineCallingConvention::C,
            },
            entry: MachineBlockId::new(0),
            virtual_registers: (0..register_count)
                .map(|id| VirtualRegister {
                    id: VirtualRegisterId::new(id),
                    class: X86RegisterClass::Word.machine_class(),
                })
                .collect(),
            blocks: vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions,
                successors: Vec::new(),
            }],
            frame_objects: Vec::new(),
        }
    }

    fn fixed_virtual(id: u32, role: OperandRole, physical: X86Register) -> MachineOperand {
        let mut operand = match role {
            OperandRole::Def => virtual_definition(id),
            OperandRole::Use => virtual_use(id),
            _ => MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(
                    VirtualRegisterId::new(id),
                )),
                role,
                constraint: None,
                tied_to: None,
            },
        };
        operand.constraint = Some(RegisterConstraint::Fixed(physical.physical()));
        operand
    }

    fn virtual_use(id: u32) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(
                id,
            ))),
            role: OperandRole::Use,
            constraint: None,
            tied_to: None,
        }
    }

    fn immediate(value: i64) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Immediate(value),
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        }
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
