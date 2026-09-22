//! x86 target hooks for target-independent register assignment.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    self, AllocationError, ConstraintError, FrameIndex, FrameObject, FrameObjectKind,
    GreedyAllocationError, InstructionFlags, MachineCallingConvention, MachineFunction,
    MachineInstruction, MachineInstructionId, MachineIntervalError, MachineOperand,
    MachineOperandKind, MachineRegister, OperandRole, PhysicalRegister, RegisterAssignment,
    RegisterClass, RegisterConstraint, SpillMaterializationError, SpillTarget, VirtualRegisterId,
};

use super::{X86Opcode, X86Register, X86RegisterClass};

/// A target-description or generic allocation refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86AllocationError {
    UnknownRegisterClass(RegisterClass),
    ResidualX87Virtual(VirtualRegisterId),
    Constraint(ConstraintError),
    Allocation(AllocationError),
    Intervals(Vec<MachineIntervalError>),
    Greedy(GreedyAllocationError),
    Spill(SpillMaterializationError<X86SpillError>),
    RoundLimit { rounds: usize },
}

impl fmt::Display for X86AllocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRegisterClass(class) => {
                write!(formatter, "unknown x86 register class {class}")
            }
            Self::ResidualX87Virtual(register) => write!(
                formatter,
                "x87 virtual register {register} reached the general register allocator"
            ),
            Self::Constraint(error) => error.fmt(formatter),
            Self::Allocation(error) => error.fmt(formatter),
            Self::Intervals(errors) => write!(
                formatter,
                "cannot compute allocation intervals: {} error(s)",
                errors.len()
            ),
            Self::Greedy(error) => error.fmt(formatter),
            Self::Spill(error) => error.fmt(formatter),
            Self::RoundLimit { rounds } => write!(
                formatter,
                "x86 register allocation did not settle in {rounds} spill rounds"
            ),
        }
    }
}

impl Error for X86AllocationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::UnknownRegisterClass(_) | Self::ResidualX87Virtual(_) => None,
            Self::Constraint(error) => Some(error),
            Self::Allocation(error) => Some(error),
            Self::Intervals(_) | Self::RoundLimit { .. } => None,
            Self::Greedy(error) => Some(error),
            Self::Spill(error) => Some(error),
        }
    }
}

/// The target-specific fact that prevents a virtual class from having a stack
/// representation in this first x86 spill path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86SpillError {
    UnsupportedRegisterClass(RegisterClass),
}

impl fmt::Display for X86SpillError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedRegisterClass(class) => {
                write!(formatter, "x86 cannot spill register class {class}")
            }
        }
    }
}

impl Error for X86SpillError {}

/// The completed result of the spill/retry allocator.
///
/// `function` retains abstract frame-index operands.  Frame planning and
/// frame-index materialization deliberately remain downstream clients.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct X86AllocationResult {
    pub function: MachineFunction,
    pub assignment: RegisterAssignment,
}

/// The bounded retry count used by Python `RegAlloc.transform`.
pub const SPILL_ALLOCATION_ROUNDS: usize = 12;

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

/// Assigns x86 registers, materializing an entire spill batch and retrying
/// until every remaining virtual range has a physical register.
///
/// This is the deliberately small Machine-IR half of Python
/// `RegAlloc.transform`: fixed occurrences are split once, each attempt
/// recomputes its intervals and constraints, and a materialized reload is
/// carried into the next attempt as unspillable.  It does not choose frame
/// displacements or lower the abstract `FrameIndex` operands it creates.
pub fn allocate_registers_with_spills(
    function: &MachineFunction,
) -> Result<X86AllocationResult, X86AllocationError> {
    validate_register_classes(function)?;
    let mut function = split_fixed_register_occurrences(function)?;
    let target = X86SpillTarget;
    let mut unspillable = BTreeSet::new();

    for _round in 0..SPILL_ALLOCATION_ROUNDS {
        // A first-round spill creates a frame object, which itself makes BP
        // unavailable to every subsequent round.
        let reserve_bp = reserves_bp(&function);
        let classes = declared_classes(&function);
        let fixed = fixed_constraints(&function, &classes, reserve_bp)?;
        let intervals = machine::weighted_live_intervals(&function, |_| 0)
            .map_err(X86AllocationError::Intervals)?;
        match machine::allocate_greedy(
            &intervals,
            &classes,
            &fixed,
            &unspillable,
            &BTreeSet::new(),
            |class| candidates(class, reserve_bp),
            overlaps,
        )
        .map_err(X86AllocationError::Greedy)?
        {
            machine::GreedyAllocation::Complete(assignment) => {
                return Ok(X86AllocationResult {
                    function,
                    assignment,
                });
            }
            machine::GreedyAllocation::Spills { spills, .. } => {
                let rewritten = machine::materialize_spills(
                    &function,
                    &spills.into_iter().collect::<Vec<_>>(),
                    &target,
                )
                .map_err(X86AllocationError::Spill)?;
                function = rewritten.function;
                unspillable.extend(rewritten.unspillable);
            }
        }
    }

    Err(X86AllocationError::RoundLimit {
        rounds: SPILL_ALLOCATION_ROUNDS,
    })
}

struct X86SpillTarget;

impl SpillTarget for X86SpillTarget {
    type Error = X86SpillError;

    fn spill_frame_object(
        &self,
        index: FrameIndex,
        class: RegisterClass,
    ) -> Result<FrameObject, Self::Error> {
        let (size, alignment) = match X86RegisterClass::from_machine_class(class) {
            Some(X86RegisterClass::Word | X86RegisterClass::Address16) => (2, 2),
            // Both current x86 frame planners lay stack objects out on word
            // boundaries; requiring 4 here would make C-frame planning
            // reject an otherwise ordinary dword spill.
            Some(X86RegisterClass::Dword) => (4, 2),
            _ => return Err(X86SpillError::UnsupportedRegisterClass(class)),
        };
        Ok(FrameObject {
            index,
            size,
            alignment,
            kind: FrameObjectKind::Spill,
        })
    }

    fn spill_load(
        &self,
        id: MachineInstructionId,
        destination: VirtualRegisterId,
        frame: FrameIndex,
    ) -> MachineInstruction {
        MachineInstruction {
            id,
            opcode: X86Opcode::Load.machine_opcode(),
            operands: vec![
                virtual_operand(destination, OperandRole::Def),
                frame_operand(frame),
            ],
            flags: InstructionFlags {
                may_load: true,
                spill_reload: true,
                ..InstructionFlags::NONE
            },
        }
    }

    fn spill_store(
        &self,
        id: MachineInstructionId,
        source: VirtualRegisterId,
        frame: FrameIndex,
    ) -> MachineInstruction {
        MachineInstruction {
            id,
            opcode: X86Opcode::Store.machine_opcode(),
            operands: vec![
                frame_operand(frame),
                virtual_operand(source, OperandRole::Use),
            ],
            flags: InstructionFlags {
                side_effects: true,
                may_store: true,
                spill_store: true,
                ..InstructionFlags::NONE
            },
        }
    }
}

fn validate_register_classes(function: &MachineFunction) -> Result<(), X86AllocationError> {
    for register in &function.virtual_registers {
        match X86RegisterClass::from_machine_class(register.class) {
            None => return Err(X86AllocationError::UnknownRegisterClass(register.class)),
            Some(X86RegisterClass::X87) => {
                return Err(X86AllocationError::ResidualX87Virtual(register.id));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn declared_classes(function: &MachineFunction) -> BTreeMap<VirtualRegisterId, RegisterClass> {
    function
        .virtual_registers
        .iter()
        .map(|register| (register.id, register.class))
        .collect()
}

fn fixed_constraints(
    function: &MachineFunction,
    classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    reserve_bp: bool,
) -> Result<BTreeMap<VirtualRegisterId, PhysicalRegister>, X86AllocationError> {
    let mut fixed = BTreeMap::new();
    for block in &function.blocks {
        for instruction in &block.instructions {
            for operand in &instruction.operands {
                let MachineOperandKind::Register(MachineRegister::Virtual(register)) = operand.kind
                else {
                    continue;
                };
                let class =
                    classes
                        .get(&register)
                        .copied()
                        .ok_or(X86AllocationError::Allocation(
                            AllocationError::MissingDeclaredClass { register },
                        ))?;
                match operand.constraint {
                    Some(RegisterConstraint::Fixed(physical)) => {
                        if let Some(previous) = fixed.insert(register, physical) {
                            if previous != physical {
                                return Err(X86AllocationError::Allocation(
                                    AllocationError::FixedConstraintConflict {
                                        register,
                                        first: previous,
                                        second: physical,
                                    },
                                ));
                            }
                        }
                        if !candidates(class, reserve_bp).contains(&physical) {
                            return Err(X86AllocationError::Allocation(
                                AllocationError::FixedRegisterUnavailable {
                                    register,
                                    class,
                                    physical,
                                },
                            ));
                        }
                    }
                    Some(RegisterConstraint::Class(constraint)) if constraint != class => {
                        return Err(X86AllocationError::Allocation(
                            AllocationError::ClassConstraintConflict {
                                register,
                                declared: class,
                                constraint,
                            },
                        ));
                    }
                    Some(RegisterConstraint::Class(_)) | None => {}
                }
            }
        }
    }
    Ok(fixed)
}

fn virtual_operand(register: VirtualRegisterId, role: OperandRole) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Register(MachineRegister::Virtual(register)),
        role,
        constraint: None,
        tied_to: None,
    }
}

fn frame_operand(index: FrameIndex) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::FrameIndex { index, addend: 0 },
        role: OperandRole::None,
        constraint: None,
        tied_to: None,
    }
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

fn reserves_bp(function: &MachineFunction) -> bool {
    function.signature.calling_convention == MachineCallingConvention::FarPascal
        || !function.frame_objects.is_empty()
        || uses_basic_runtime_frame(function)
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
        MachineFunctionId, MachineInstruction, MachineInstructionId, MachineModule, MachineOperand,
        MachineOperandKind, MachineRegister, OperandRole, RegisterConstraint, TargetOpcode,
        VirtualRegister, VirtualRegisterId, apply_assignment,
    };
    use crate::target::x86::{
        materialize_far_call_clobbers, materialize_frame_indices_with_layout, plan_c_frame,
        verify_machine,
    };

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

    #[test]
    fn pressure_converges_by_spilling_a_word_to_an_abstract_frame_slot() {
        // Python RegAlloc.transform's ordinary retry: one cold value spans
        // six live words, but its own use is after that pressure.  Spilling
        // it must create a Word-sized abstract slot, a store, and a later
        // reload rather than returning the first-round refusal.
        let mut instructions = vec![MachineInstruction {
            id: MachineInstructionId::new(0),
            opcode: X86Opcode::Mov.machine_opcode(),
            operands: vec![virtual_definition(0), immediate(0)],
            flags: InstructionFlags::NONE,
        }];
        for register in 1..=6 {
            instructions.push(MachineInstruction {
                id: MachineInstructionId::new(register),
                opcode: X86Opcode::Mov.machine_opcode(),
                operands: vec![virtual_definition(register), immediate(i64::from(register))],
                flags: InstructionFlags::NONE,
            });
        }
        for (offset, register) in (1..=6).enumerate() {
            instructions.push(MachineInstruction {
                id: MachineInstructionId::new(7 + offset as u32),
                opcode: X86Opcode::Push.machine_opcode(),
                operands: vec![virtual_use(register)],
                flags: InstructionFlags::NONE,
            });
        }
        instructions.push(MachineInstruction {
            id: MachineInstructionId::new(13),
            opcode: X86Opcode::Push.machine_opcode(),
            operands: vec![virtual_use(0)],
            flags: InstructionFlags::NONE,
        });

        let mut function = word_function_with_registers(instructions, 7);
        function.signature.calling_convention = MachineCallingConvention::FarPascal;
        let allocated = allocate_registers_with_spills(&function).unwrap();

        assert_eq!(allocated.function.frame_objects.len(), 1);
        assert_eq!(
            allocated.function.frame_objects[0],
            FrameObject {
                index: FrameIndex::new(0),
                size: 2,
                alignment: 2,
                kind: FrameObjectKind::Spill,
            }
        );
        let instructions = &allocated.function.blocks[0].instructions;
        assert!(
            instructions
                .iter()
                .any(|instruction| instruction.opcode == X86Opcode::Store.machine_opcode())
        );
        let reload = instructions
            .iter()
            .find(|instruction| instruction.opcode == X86Opcode::Load.machine_opcode())
            .expect("the spilled final use reloads from its frame slot");
        let MachineOperandKind::Register(MachineRegister::Virtual(reload)) =
            reload.operands[0].kind
        else {
            panic!("spill reload must define a virtual register");
        };
        assert!(allocated.assignment.get(reload).is_some());
    }

    #[test]
    fn dword_spills_use_a_word_aligned_four_byte_slot() {
        // The 16-bit C and BASIC frames align stack objects to words even
        // when the stored value is a dword. Algebra's live LONG therefore
        // needs four bytes without requesting an unsupported 4-byte frame
        // alignment.
        assert_eq!(
            X86SpillTarget
                .spill_frame_object(FrameIndex::new(7), X86RegisterClass::Dword.machine_class(),)
                .unwrap(),
            FrameObject {
                index: FrameIndex::new(7),
                size: 4,
                alignment: 2,
                kind: FrameObjectKind::Spill,
            }
        );
    }

    #[test]
    fn address_spills_reload_into_a_legal_16_bit_memory_base() {
        // Four registers encode a 16-bit ModR/M address base.  Preserve the
        // class through a spill so the reload can immediately address memory,
        // rather than merely proving that a word-sized slot was allocated.
        let mut instructions = Vec::new();
        for register in 0..5 {
            instructions.push(MachineInstruction {
                id: MachineInstructionId::new(register),
                opcode: X86Opcode::Mov.machine_opcode(),
                operands: vec![virtual_definition(register), immediate(i64::from(register))],
                flags: InstructionFlags::NONE,
            });
        }
        for register in 0..5 {
            instructions.push(MachineInstruction {
                id: MachineInstructionId::new(5 + register),
                opcode: X86Opcode::Load.machine_opcode(),
                operands: vec![virtual_definition(5 + register), virtual_use(register)],
                flags: InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            });
            instructions.push(MachineInstruction {
                id: MachineInstructionId::new(10 + register),
                opcode: X86Opcode::Store.machine_opcode(),
                operands: vec![virtual_use(register), virtual_use(5 + register)],
                flags: InstructionFlags {
                    side_effects: true,
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            });
        }
        let mut function = word_function_with_registers(instructions, 10);
        for register in &mut function.virtual_registers[..5] {
            register.class = X86RegisterClass::Address16.machine_class();
        }

        let allocated = allocate_registers_with_spills(&function)
            .expect("address pressure spills through a word-sized home");
        assert!(allocated.function.frame_objects.iter().any(|frame| {
            frame.size == 2 && frame.alignment == 2 && frame.kind == FrameObjectKind::Spill
        }));
        let (reload_index, reload) = allocated.function.blocks[0]
            .instructions
            .iter()
            .enumerate()
            .find(|(_, instruction)| {
                instruction.opcode == X86Opcode::Load.machine_opcode()
                    && matches!(
                        instruction.operands.as_slice(),
                        [
                            _,
                            MachineOperand {
                                kind: MachineOperandKind::FrameIndex { .. },
                                ..
                            }
                        ]
                    )
            })
            .expect("the spilled address is reloaded from its frame home");
        let MachineOperandKind::Register(MachineRegister::Virtual(reload)) =
            reload.operands[0].kind
        else {
            panic!("address spill reload must define a virtual register");
        };
        assert_eq!(
            allocated
                .function
                .virtual_registers
                .iter()
                .find(|register| register.id == reload)
                .expect("reload register is declared")
                .class,
            X86RegisterClass::Address16.machine_class(),
        );

        let following = &allocated.function.blocks[0].instructions[reload_index + 1];
        assert_eq!(following.opcode, X86Opcode::Load.machine_opcode());
        assert!(matches!(
            following.operands.as_slice(),
            [_, MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(base)),
                role: OperandRole::Use,
                ..
            }] if *base == reload
        ));

        let applied = apply_assignment(&allocated.function, &allocated.assignment)
            .expect("the spill reload receives a physical register");
        let plan = plan_c_frame(&applied).expect("a two-byte spill has a C frame home");
        let materialized = materialize_frame_indices_with_layout(&applied, plan.layout())
            .expect("the spill frame index materializes through BP");
        verify_machine(&MachineModule {
            data_objects: Vec::new(),
            functions: vec![materialized.clone()],
        })
        .expect("the reload and its following memory use are legal x86 Machine IR");

        let reloaded_register = match materialized.blocks[0].instructions[reload_index]
            .operands
            .first()
            .expect("reload has a destination")
            .kind
        {
            MachineOperandKind::Register(MachineRegister::Physical(register)) => register,
            _ => panic!("materialized reload must define a physical register"),
        };
        assert!(
            X86RegisterClass::Address16
                .members()
                .iter()
                .any(|register| register.physical() == reloaded_register)
        );
        assert!(matches!(
            materialized.blocks[0].instructions[reload_index + 1].operands.as_slice(),
            [_, MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(base)),
                role: OperandRole::Use,
                ..
            }] if *base == reloaded_register
        ));
    }

    #[test]
    fn residual_x87_virtual_is_refused_before_general_allocation() {
        let mut function = word_function(Vec::new());
        function.virtual_registers[0].class = X86RegisterClass::X87.machine_class();

        assert!(matches!(
            allocate_registers(&function),
            Err(X86AllocationError::ResidualX87Virtual(register))
                if register == VirtualRegisterId::new(0)
        ));
        assert!(matches!(
            allocate_registers_with_spills(&function),
            Err(X86AllocationError::ResidualX87Virtual(register))
                if register == VirtualRegisterId::new(0)
        ));
    }

    #[test]
    fn constrained_reload_is_unspillable_on_the_retry() {
        // The final AX requirement is first split into its own short range.
        // When its source spills, the inserted reload must be protected on
        // the retry; spilling it recursively would add another slot/load
        // pair forever instead of reaching the constrained use.
        let mut instructions = vec![MachineInstruction {
            id: MachineInstructionId::new(0),
            opcode: X86Opcode::Mov.machine_opcode(),
            operands: vec![virtual_definition(0), immediate(0)],
            flags: InstructionFlags::NONE,
        }];
        for register in 1..=6 {
            instructions.push(MachineInstruction {
                id: MachineInstructionId::new(register),
                opcode: X86Opcode::Mov.machine_opcode(),
                operands: vec![virtual_definition(register), immediate(i64::from(register))],
                flags: InstructionFlags::NONE,
            });
        }
        for (offset, register) in (1..=6).enumerate() {
            instructions.push(MachineInstruction {
                id: MachineInstructionId::new(7 + offset as u32),
                opcode: X86Opcode::Push.machine_opcode(),
                operands: vec![virtual_use(register)],
                flags: InstructionFlags::NONE,
            });
        }
        instructions.push(MachineInstruction {
            id: MachineInstructionId::new(13),
            opcode: X86Opcode::Push.machine_opcode(),
            operands: vec![fixed_virtual(0, OperandRole::Use, X86Register::Ax)],
            flags: InstructionFlags::NONE,
        });

        let mut function = word_function_with_registers(instructions, 7);
        function.signature.calling_convention = MachineCallingConvention::FarPascal;
        let allocated = allocate_registers_with_spills(&function).unwrap();

        assert_eq!(allocated.function.frame_objects.len(), 1);
        let reload = allocated.function.blocks[0]
            .instructions
            .iter()
            .find(|instruction| instruction.opcode == X86Opcode::Load.machine_opcode())
            .expect("the spilled source needs one protected reload");
        let MachineOperandKind::Register(MachineRegister::Virtual(reload)) =
            reload.operands[0].kind
        else {
            panic!("spill reload must define a virtual register");
        };
        assert!(allocated.assignment.get(reload).is_some());
        assert!(allocated.function.blocks[0]
            .instructions
            .iter()
            .any(|instruction| {
                instruction.opcode == X86Opcode::Push.machine_opcode()
                    && matches!(
                        instruction.operands.as_slice(),
                        [MachineOperand {
                            kind: MachineOperandKind::Register(MachineRegister::Virtual(register)),
                            constraint: Some(RegisterConstraint::Fixed(physical)),
                            ..
                        }] if *physical == X86Register::Ax.physical()
                            && allocated.assignment.get(*register) == Some(X86Register::Ax.physical())
                    )
            }));
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
