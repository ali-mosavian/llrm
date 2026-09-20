//! Materialization of BASIC frame indices into x86 addressing operands.
//!
//! A selected frame reference is an abstract [`MachineOperandKind::FrameIndex`].
//! x86 instruction forms spell its concrete address as an opcode-defined tuple:
//! `BP`, followed by its displacement.  This pass changes only that tuple; it
//! does not allocate registers, lower to MC, or choose a ModR/M encoding.

use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    FrameIndex, MachineBlockId, MachineFunction, MachineFunctionId, MachineInstruction,
    MachineInstructionId, MachineOperand, MachineOperandKind, MachineRegister, OperandRole,
    TargetOpcode,
};

use super::{BasicFramePlan, X86Opcode, X86Register};

/// A refusal while turning an abstract frame reference into x86 operands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameIndexMaterializationError {
    /// The immutable layout belongs to a different Machine IR function.
    MismatchedFramePlan {
        function: MachineFunctionId,
        planned: MachineFunctionId,
    },
    /// The selected frame reference has no displacement in the supplied plan.
    UnknownFrameIndex {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        frame: FrameIndex,
    },
    /// The opcode or operand position cannot carry an x86 frame address.
    UnsupportedFrameIndex {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        opcode: TargetOpcode,
        operand: usize,
    },
    /// A malformed abstract frame operand would lose an IR invariant if changed.
    InvalidFrameIndexOperand {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
    },
    /// Selected x86 frame references currently carry no secondary addend.
    UnsupportedFrameAddend {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        frame: FrameIndex,
        addend: i64,
    },
    /// The x86 BP-plus-immediate tuple is already present.
    ///
    /// There is no per-function pass marker in Machine IR.  Treating this
    /// exact tuple as already materialized keeps an accidental second run from
    /// changing a frame-address shape after another stage has consumed it.
    AlreadyMaterialized {
        block: MachineBlockId,
        instruction: MachineInstructionId,
    },
}

impl fmt::Display for FrameIndexMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MismatchedFramePlan { function, planned } => write!(
                formatter,
                "machine function {function} cannot use frame plan for function {planned}"
            ),
            Self::UnknownFrameIndex {
                block,
                instruction,
                operand,
                frame,
            } => write!(
                formatter,
                "block {block} instruction {instruction} operand {operand} names unknown frame index {frame}"
            ),
            Self::UnsupportedFrameIndex {
                block,
                instruction,
                opcode,
                operand,
            } => write!(
                formatter,
                "block {block} instruction {instruction} opcode {} operand {operand} cannot carry a frame index",
                opcode.get()
            ),
            Self::InvalidFrameIndexOperand {
                block,
                instruction,
                operand,
            } => write!(
                formatter,
                "block {block} instruction {instruction} operand {operand} has an invalid frame-index contract"
            ),
            Self::UnsupportedFrameAddend {
                block,
                instruction,
                operand,
                frame,
                addend,
            } => write!(
                formatter,
                "block {block} instruction {instruction} operand {operand} frame index {frame} has unsupported addend {addend}"
            ),
            Self::AlreadyMaterialized { block, instruction } => write!(
                formatter,
                "block {block} instruction {instruction} already has a materialized x86 frame address"
            ),
        }
    }
}

impl Error for FrameIndexMaterializationError {}

/// Returns a copy of `function` with each x86 frame operand made explicit.
///
/// The result uses `[destination, BP, displacement]` for `Load` and `Lea`,
/// and `[BP, displacement, source]` for `Store`.  `BP` is a physical use and
/// the displacement is a role-less immediate. The plan is the sole source of
/// that displacement: selected frame addends remain unsupported so the offset
/// cannot be applied twice. Choosing a 16-bit modular encoding belongs to the
/// later encoding form, not frame planning. The input is never modified; all
/// refusals happen before the returned copy is changed.
pub fn materialize_frame_indices(
    function: &MachineFunction,
    plan: &BasicFramePlan,
) -> Result<MachineFunction, FrameIndexMaterializationError> {
    validate(function, plan)?;

    let mut materialized = function.clone();
    for block in &mut materialized.blocks {
        for instruction in &mut block.instructions {
            let Some(position) = frame_address_position(instruction) else {
                continue;
            };
            let MachineOperandKind::FrameIndex { index, .. } = &instruction.operands[position].kind
            else {
                continue;
            };
            let displacement =
                frame_displacement(plan, *index, block.id, instruction.id, position)?;
            instruction.operands[position] = bp_operand();
            instruction
                .operands
                .insert(position + 1, immediate_operand(displacement));
        }
    }
    Ok(materialized)
}

fn validate(
    function: &MachineFunction,
    plan: &BasicFramePlan,
) -> Result<(), FrameIndexMaterializationError> {
    if plan.function() != function.id {
        return Err(FrameIndexMaterializationError::MismatchedFramePlan {
            function: function.id,
            planned: plan.function(),
        });
    }
    for block in &function.blocks {
        for instruction in &block.instructions {
            if is_materialized_frame_address(instruction) {
                return Err(FrameIndexMaterializationError::AlreadyMaterialized {
                    block: block.id,
                    instruction: instruction.id,
                });
            }
            for (position, operand) in instruction.operands.iter().enumerate() {
                let MachineOperandKind::FrameIndex { index, addend } = &operand.kind else {
                    continue;
                };
                if frame_address_position(instruction) != Some(position) {
                    return Err(FrameIndexMaterializationError::UnsupportedFrameIndex {
                        block: block.id,
                        instruction: instruction.id,
                        opcode: instruction.opcode,
                        operand: position,
                    });
                }
                if !matches!(operand.role, OperandRole::None)
                    || operand.constraint.is_some()
                    || operand.tied_to.is_some()
                {
                    return Err(FrameIndexMaterializationError::InvalidFrameIndexOperand {
                        block: block.id,
                        instruction: instruction.id,
                        operand: position,
                    });
                }
                if *addend != 0 {
                    return Err(FrameIndexMaterializationError::UnsupportedFrameAddend {
                        block: block.id,
                        instruction: instruction.id,
                        operand: position,
                        frame: *index,
                        addend: *addend,
                    });
                }
                frame_displacement(plan, *index, block.id, instruction.id, position)?;
            }
        }
    }
    Ok(())
}

fn frame_address_position(instruction: &MachineInstruction) -> Option<usize> {
    match X86Opcode::from_machine_opcode(instruction.opcode) {
        Some(X86Opcode::Load | X86Opcode::Lea) if instruction.operands.len() == 2 => Some(1),
        Some(X86Opcode::Store) if instruction.operands.len() == 2 => Some(0),
        _ => None,
    }
}

fn is_materialized_frame_address(instruction: &MachineInstruction) -> bool {
    let position = match X86Opcode::from_machine_opcode(instruction.opcode) {
        Some(X86Opcode::Load | X86Opcode::Lea) if instruction.operands.len() == 3 => 1,
        Some(X86Opcode::Store) if instruction.operands.len() == 3 => 0,
        _ => return false,
    };
    matches!(
        instruction.operands.get(position),
        Some(MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
            role: OperandRole::Use,
            constraint: None,
            tied_to: None,
        }) if *register == X86Register::Bp.physical()
    ) && matches!(
        instruction.operands.get(position + 1),
        Some(MachineOperand {
            kind: MachineOperandKind::Immediate(_),
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        })
    )
}

fn frame_displacement(
    plan: &BasicFramePlan,
    frame: FrameIndex,
    block: MachineBlockId,
    instruction: MachineInstructionId,
    operand: usize,
) -> Result<i64, FrameIndexMaterializationError> {
    plan.offset(frame)
        .map(i64::from)
        .ok_or(FrameIndexMaterializationError::UnknownFrameIndex {
            block,
            instruction,
            operand,
            frame,
        })
}

fn bp_operand() -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Register(MachineRegister::Physical(X86Register::Bp.physical())),
        role: OperandRole::Use,
        constraint: None,
        tied_to: None,
    }
}

fn immediate_operand(value: i64) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Immediate(value),
        role: OperandRole::None,
        constraint: None,
        tied_to: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        FrameObject, FrameObjectKind, InstructionFlags, MachineCallingConvention, MachineLinkage,
        MachineSignature, MachineValueType, VirtualRegister, VirtualRegisterId,
    };

    fn function(
        parameters: Vec<MachineValueType>,
        frames: Vec<FrameObject>,
        instructions: Vec<MachineInstruction>,
    ) -> MachineFunction {
        MachineFunction {
            id: crate::codegen::machine::MachineFunctionId::new(1),
            name: "frame_address".to_owned(),
            linkage: MachineLinkage::Internal,
            signature: MachineSignature {
                result: None,
                parameters,
                variadic: false,
                calling_convention: MachineCallingConvention::Basic,
            },
            virtual_registers: vec![VirtualRegister {
                id: VirtualRegisterId::new(0),
                class: super::super::X86RegisterClass::Word.machine_class(),
            }],
            blocks: vec![crate::codegen::machine::MachineBlock {
                id: MachineBlockId::new(2),
                instructions,
                successors: Vec::new(),
            }],
            frame_objects: frames,
        }
    }

    fn incoming(index: u32, parameter: u32) -> FrameObject {
        FrameObject {
            index: FrameIndex::new(index),
            size: 2,
            alignment: 2,
            kind: FrameObjectKind::IncomingArgument { parameter },
        }
    }

    fn local(index: u32, size: u32) -> FrameObject {
        FrameObject {
            index: FrameIndex::new(index),
            size,
            alignment: 2,
            kind: FrameObjectKind::Local,
        }
    }

    fn virtual_operand(role: OperandRole) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(0))),
            role,
            constraint: None,
            tied_to: None,
        }
    }

    fn physical_operand(register: X86Register, role: OperandRole) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Physical(register.physical())),
            role,
            constraint: None,
            tied_to: None,
        }
    }

    fn frame_operand(index: u32, addend: i64) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::FrameIndex {
                index: FrameIndex::new(index),
                addend,
            },
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        }
    }

    fn instruction(opcode: X86Opcode, operands: Vec<MachineOperand>) -> MachineInstruction {
        let flags = match opcode {
            X86Opcode::Load => InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            },
            X86Opcode::Store => InstructionFlags {
                side_effects: true,
                may_store: true,
                ..InstructionFlags::NONE
            },
            _ => InstructionFlags::NONE,
        };
        MachineInstruction {
            id: MachineInstructionId::new(3),
            opcode: opcode.machine_opcode(),
            operands,
            flags,
        }
    }

    fn plan(function: &MachineFunction, runtime: super::super::BasicRuntime) -> BasicFramePlan {
        super::super::plan_basic_frame(function, runtime, 0).unwrap()
    }

    fn assert_bp_displacement(operands: &[MachineOperand], bp: usize, displacement: i64) {
        assert!(matches!(
            operands.get(bp),
            Some(MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
                role: OperandRole::Use,
                constraint: None,
                tied_to: None,
            }) if *register == X86Register::Bp.physical()
        ));
        assert!(matches!(
            operands.get(bp + 1),
            Some(MachineOperand {
                kind: MachineOperandKind::Immediate(value),
                role: OperandRole::None,
                constraint: None,
                tied_to: None,
            }) if *value == displacement
        ));
    }

    #[test]
    fn materializes_incoming_argument_at_bp_plus_six() {
        // The only incoming INTEGER begins at BP+6 under the far-Pascal ABI.
        let function = function(
            vec![MachineValueType::Integer { bits: 16 }],
            vec![incoming(0, 0)],
            vec![instruction(
                X86Opcode::Load,
                vec![virtual_operand(OperandRole::Def), frame_operand(0, 0)],
            )],
        );
        let materialized = materialize_frame_indices(
            &function,
            &plan(&function, super::super::BasicRuntime::Qb45),
        )
        .unwrap();

        let operands = &materialized.blocks[0].instructions[0].operands;
        assert_bp_displacement(operands, 1, 6);
        assert_eq!(operands.len(), 3);
    }

    #[test]
    fn materialized_frame_displacement_reaches_exact_mc_bytes_without_a_relocation() {
        // A frame slot's displacement is literal. It must survive Machine IR
        // to MC exactly once and must not become a symbolic object fixup.
        let function = function(
            vec![MachineValueType::Integer { bits: 16 }],
            vec![incoming(0, 0)],
            vec![instruction(
                X86Opcode::Load,
                vec![
                    physical_operand(X86Register::Ax, OperandRole::Def),
                    frame_operand(0, 0),
                ],
            )],
        );
        let materialized = materialize_frame_indices(
            &function,
            &plan(&function, super::super::BasicRuntime::Qb45),
        )
        .unwrap();
        let mc = super::super::lower_instruction(&materialized.blocks[0].instructions[0]).unwrap();

        assert_eq!(super::super::encode(&mc).unwrap(), vec![0x8b, 0x46, 0x06]);
    }

    #[test]
    fn materializes_vbdos_local_at_bp_minus_twenty_four() {
        // VBDOS's 20-byte header precedes this four-byte local.
        let function = function(
            Vec::new(),
            vec![local(0, 4)],
            vec![instruction(
                X86Opcode::Store,
                vec![frame_operand(0, 0), virtual_operand(OperandRole::Use)],
            )],
        );
        let materialized = materialize_frame_indices(
            &function,
            &plan(&function, super::super::BasicRuntime::Vbdos),
        )
        .unwrap();

        let operands = &materialized.blocks[0].instructions[0].operands;
        assert_bp_displacement(operands, 0, -24);
        assert!(matches!(operands[2].kind, MachineOperandKind::Register(_)));
    }

    #[test]
    fn materializes_pascal_long_formals_in_reverse_stack_order() {
        // SUBTRACTPAIR once read 8-50 because source-order formals were given
        // ascending stack offsets. The first LONG is farther from the return.
        let first = instruction(
            X86Opcode::Load,
            vec![virtual_operand(OperandRole::Def), frame_operand(0, 0)],
        );
        let mut second = instruction(
            X86Opcode::Load,
            vec![virtual_operand(OperandRole::Def), frame_operand(1, 0)],
        );
        second.id = MachineInstructionId::new(4);
        let function = function(
            vec![
                MachineValueType::Integer { bits: 32 },
                MachineValueType::Integer { bits: 32 },
            ],
            vec![
                FrameObject {
                    index: FrameIndex::new(0),
                    size: 4,
                    alignment: 2,
                    kind: FrameObjectKind::IncomingArgument { parameter: 0 },
                },
                FrameObject {
                    index: FrameIndex::new(1),
                    size: 4,
                    alignment: 2,
                    kind: FrameObjectKind::IncomingArgument { parameter: 1 },
                },
            ],
            vec![first, second],
        );
        let materialized = materialize_frame_indices(
            &function,
            &plan(&function, super::super::BasicRuntime::Qb45),
        )
        .unwrap();

        assert_bp_displacement(&materialized.blocks[0].instructions[0].operands, 1, 10);
        assert_bp_displacement(&materialized.blocks[0].instructions[1].operands, 1, 6);
    }

    #[test]
    fn rejects_unknown_frame_index() {
        let function = function(
            vec![MachineValueType::Integer { bits: 16 }],
            vec![incoming(0, 0)],
            vec![instruction(
                X86Opcode::Load,
                vec![virtual_operand(OperandRole::Def), frame_operand(9, 0)],
            )],
        );
        let error = materialize_frame_indices(
            &function,
            &plan(&function, super::super::BasicRuntime::Qb45),
        );

        assert!(matches!(
            error,
            Err(FrameIndexMaterializationError::UnknownFrameIndex {
                frame,
                operand: 1,
                ..
            }) if frame == FrameIndex::new(9)
        ));
    }

    #[test]
    fn refuses_a_frame_plan_owned_by_another_function() {
        let original = function(
            vec![MachineValueType::Integer { bits: 16 }],
            vec![incoming(0, 0)],
            vec![instruction(
                X86Opcode::Load,
                vec![virtual_operand(OperandRole::Def), frame_operand(0, 0)],
            )],
        );
        let frame = plan(&original, super::super::BasicRuntime::Qb45);
        let mut other = original.clone();
        other.id = MachineFunctionId::new(2);

        assert_eq!(
            materialize_frame_indices(&other, &frame),
            Err(FrameIndexMaterializationError::MismatchedFramePlan {
                function: MachineFunctionId::new(2),
                planned: MachineFunctionId::new(1),
            })
        );
    }

    #[test]
    fn rejects_frame_index_outside_an_address_operand() {
        let function = function(
            Vec::new(),
            vec![local(0, 2)],
            vec![instruction(
                X86Opcode::Copy,
                vec![virtual_operand(OperandRole::Def), frame_operand(0, 0)],
            )],
        );
        let error = materialize_frame_indices(
            &function,
            &plan(&function, super::super::BasicRuntime::Qb45),
        );

        assert!(matches!(
            error,
            Err(FrameIndexMaterializationError::UnsupportedFrameIndex { operand: 1, .. })
        ));
    }

    #[test]
    fn materializes_the_complete_vbdos_local_extent_once() {
        // A 4096-byte source local begins below VBDOS's complete 20-byte
        // runtime header. Applying either component twice produces a wrong
        // address even though the resulting instruction remains encodable.
        let function = function(
            Vec::new(),
            vec![local(0, 4096)],
            vec![instruction(
                X86Opcode::Lea,
                vec![virtual_operand(OperandRole::Def), frame_operand(0, 0)],
            )],
        );
        let materialized = materialize_frame_indices(
            &function,
            &plan(&function, super::super::BasicRuntime::Vbdos),
        )
        .unwrap();

        assert_bp_displacement(&materialized.blocks[0].instructions[0].operands, 1, -4116);
    }

    #[test]
    fn rejects_a_secondary_frame_addend() {
        let function = function(
            vec![MachineValueType::Integer { bits: 16 }],
            vec![incoming(0, 0)],
            vec![instruction(
                X86Opcode::Lea,
                vec![virtual_operand(OperandRole::Def), frame_operand(0, 4)],
            )],
        );
        let error = materialize_frame_indices(
            &function,
            &plan(&function, super::super::BasicRuntime::Qb45),
        );

        assert!(matches!(
            error,
            Err(FrameIndexMaterializationError::UnsupportedFrameAddend {
                frame,
                addend: 4,
                ..
            }) if frame == FrameIndex::new(0)
        ));
    }

    #[test]
    fn leaves_input_unchanged() {
        let function = function(
            vec![MachineValueType::Integer { bits: 16 }],
            vec![incoming(0, 0)],
            vec![instruction(
                X86Opcode::Load,
                vec![virtual_operand(OperandRole::Def), frame_operand(0, 0)],
            )],
        );
        let original = function.clone();
        let _ = materialize_frame_indices(
            &function,
            &plan(&function, super::super::BasicRuntime::Qb45),
        )
        .unwrap();

        assert_eq!(function, original);
    }

    #[test]
    fn explicitly_refuses_a_second_materialization() {
        let function = function(
            vec![MachineValueType::Integer { bits: 16 }],
            vec![incoming(0, 0)],
            vec![instruction(
                X86Opcode::Load,
                vec![virtual_operand(OperandRole::Def), frame_operand(0, 0)],
            )],
        );
        let frame = plan(&function, super::super::BasicRuntime::Qb45);
        let materialized = materialize_frame_indices(&function, &frame).unwrap();

        assert!(matches!(
            materialize_frame_indices(&materialized, &frame),
            Err(FrameIndexMaterializationError::AlreadyMaterialized { .. })
        ));
    }
}
