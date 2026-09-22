//! Non-mutating application of a completed virtual-register assignment.
//!
//! Assignment is intentionally separate from rewriting: allocation decides
//! locations, while this module only replaces virtual operands in an owned
//! Machine IR copy after checking that the assignment still fits the function.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use super::{
    MachineBlockId, MachineFunction, MachineInstructionId, MachineOperandKind, MachineRegister,
    PhysicalRegister, RegisterAssignment, RegisterConstraint, VirtualRegisterId,
};

/// A failure while applying a virtual-register assignment to Machine IR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AllocationRewriteError {
    MissingDeclaredAssignment {
        register: VirtualRegisterId,
    },
    MissingAssignment {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        register: VirtualRegisterId,
    },
    UndeclaredAssignment {
        register: VirtualRegisterId,
    },
    ResidualTie {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
    },
    FixedConstraintMismatch {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        register: VirtualRegisterId,
        expected: PhysicalRegister,
        assigned: PhysicalRegister,
    },
}

impl fmt::Display for AllocationRewriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDeclaredAssignment { register } => {
                write!(
                    formatter,
                    "assignment omits declared virtual register {register}"
                )
            }
            Self::MissingAssignment {
                block,
                instruction,
                operand,
                register,
            } => write!(
                formatter,
                "machine block {block} instruction {instruction} operand {operand} has no assignment for virtual register {register}"
            ),
            Self::UndeclaredAssignment { register } => {
                write!(
                    formatter,
                    "assignment contains undeclared virtual register {register}"
                )
            }
            Self::ResidualTie {
                block,
                instruction,
                operand,
            } => write!(
                formatter,
                "machine block {block} instruction {instruction} operand {operand} retains an unsupported tie"
            ),
            Self::FixedConstraintMismatch {
                block,
                instruction,
                operand,
                register,
                expected,
                assigned,
            } => write!(
                formatter,
                "machine block {block} instruction {instruction} operand {operand} assigns virtual register {register} to physical register {assigned}, not fixed register {expected}"
            ),
        }
    }
}

impl Error for AllocationRewriteError {}

/// Returns an owned Machine IR function with virtual operands made physical.
///
/// Fixed constraints are checked against the completed assignment before they
/// are cleared.  Class constraints are also cleared: their equality with the
/// declared virtual-register class was validated by allocation, and cannot be
/// rechecked here without target-specific class membership knowledge.
pub fn apply_assignment(
    function: &MachineFunction,
    assignment: &RegisterAssignment,
) -> Result<MachineFunction, AllocationRewriteError> {
    validate_assignment_entries(function, assignment)?;
    validate_rewrite(function, assignment)?;

    let mut rewritten = function.clone();
    let mut anchor_registers = BTreeSet::new();
    for block in &mut rewritten.blocks {
        for instruction in &mut block.instructions {
            for (position, operand) in instruction.operands.iter_mut().enumerate() {
                let MachineOperandKind::Register(MachineRegister::Virtual(register)) =
                    &operand.kind
                else {
                    continue;
                };
                if instruction.flags.anchor {
                    // An anchor is deliberately run after allocation has
                    // chosen its lanes but before virtual def/use lineage is
                    // erased.  Its virtuals remain as opaque logical facts;
                    // MC lowers its retained instruction position directly
                    // to a zero-byte fragment.
                    anchor_registers.insert(*register);
                    continue;
                }
                let physical =
                    assignment
                        .get(*register)
                        .ok_or(AllocationRewriteError::MissingAssignment {
                            block: block.id,
                            instruction: instruction.id,
                            operand: position,
                            register: *register,
                        })?;
                operand.kind = MachineOperandKind::Register(MachineRegister::Physical(physical));
                operand.constraint = None;
            }
        }
    }
    rewritten
        .virtual_registers
        .retain(|register| anchor_registers.contains(&register.id));

    Ok(rewritten)
}

fn validate_assignment_entries(
    function: &MachineFunction,
    assignment: &RegisterAssignment,
) -> Result<(), AllocationRewriteError> {
    let declared = function
        .virtual_registers
        .iter()
        .map(|virtual_register| virtual_register.id)
        .collect::<BTreeSet<_>>();

    for (register, _) in assignment.iter() {
        if !declared.contains(&register) {
            return Err(AllocationRewriteError::UndeclaredAssignment { register });
        }
    }
    for register in declared {
        if assignment.get(register).is_none() {
            return Err(AllocationRewriteError::MissingDeclaredAssignment { register });
        }
    }

    Ok(())
}

fn validate_rewrite(
    function: &MachineFunction,
    assignment: &RegisterAssignment,
) -> Result<(), AllocationRewriteError> {
    for block in &function.blocks {
        for instruction in &block.instructions {
            for (position, operand) in instruction.operands.iter().enumerate() {
                if operand.tied_to.is_some() {
                    return Err(AllocationRewriteError::ResidualTie {
                        block: block.id,
                        instruction: instruction.id,
                        operand: position,
                    });
                }
                let MachineOperandKind::Register(MachineRegister::Virtual(register)) =
                    &operand.kind
                else {
                    continue;
                };
                let Some(physical) = assignment.get(*register) else {
                    return Err(AllocationRewriteError::MissingAssignment {
                        block: block.id,
                        instruction: instruction.id,
                        operand: position,
                        register: *register,
                    });
                };
                if let Some(RegisterConstraint::Fixed(expected)) = operand.constraint {
                    if physical != expected {
                        return Err(AllocationRewriteError::FixedConstraintMismatch {
                            block: block.id,
                            instruction: instruction.id,
                            operand: position,
                            register: *register,
                            expected,
                            assigned: physical,
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::old::codegen::machine::{
        InstructionFlags, MachineBlock, MachineFunctionId, MachineInstruction, MachineOperand,
        OperandRole, PhysicalRegister, RegisterClass, TargetOpcode, VirtualRegister, allocate,
    };

    const GENERAL: RegisterClass = RegisterClass::new(0);
    const FIRST: PhysicalRegister = PhysicalRegister::new(0);
    const SECOND: PhysicalRegister = PhysicalRegister::new(1);
    const CANDIDATES: [PhysicalRegister; 2] = [FIRST, SECOND];

    fn candidates(_: RegisterClass) -> Vec<PhysicalRegister> {
        CANDIDATES.to_vec()
    }

    fn does_not_alias(_: PhysicalRegister, _: PhysicalRegister) -> bool {
        false
    }

    fn virtual_register(id: u32, role: OperandRole) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(
                id,
            ))),
            role,
            constraint: None,
            tied_to: None,
        }
    }

    fn instruction(id: u32, operands: Vec<MachineOperand>) -> MachineInstruction {
        MachineInstruction {
            id: MachineInstructionId::new(id),
            opcode: TargetOpcode::new(3),
            operands,
            flags: InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            },
        }
    }

    fn function(blocks: Vec<MachineBlock>, registers: &[u32]) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(2),
            name: "rewrite".to_owned(),
            linkage: crate::old::codegen::machine::MachineLinkage::Internal,
            signature: crate::old::codegen::machine::MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention:
                    crate::old::codegen::machine::MachineCallingConvention::FarPascal,
            },
            entry: blocks
                .first()
                .map(|block| block.id)
                .unwrap_or(MachineBlockId::new(0)),
            virtual_registers: registers
                .iter()
                .map(|id| VirtualRegister {
                    id: VirtualRegisterId::new(*id),
                    class: GENERAL,
                })
                .collect(),
            blocks,
            frame_objects: Vec::new(),
        }
    }

    fn empty_function() -> MachineFunction {
        function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: Vec::new(),
                successors: Vec::new(),
            }],
            &[],
        )
    }

    fn allocated(function: &MachineFunction) -> RegisterAssignment {
        allocate(function, candidates, does_not_alias).expect("test function is allocatable")
    }

    #[test]
    fn rewrites_virtual_operands_without_changing_other_machine_facts() {
        let mut destination = virtual_register(0, OperandRole::Def);
        destination.constraint = Some(RegisterConstraint::Fixed(FIRST));
        let source = virtual_register(0, OperandRole::Use);
        let physical = MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Physical(SECOND)),
            role: OperandRole::Use,
            constraint: None,
            tied_to: None,
        };
        let original = function(
            vec![
                MachineBlock {
                    id: MachineBlockId::new(5),
                    instructions: vec![instruction(7, vec![destination, source, physical])],
                    successors: vec![MachineBlockId::new(6)],
                },
                MachineBlock {
                    id: MachineBlockId::new(6),
                    instructions: Vec::new(),
                    successors: Vec::new(),
                },
            ],
            &[0],
        );
        let assignment = allocated(&original);

        let rewritten = apply_assignment(&original, &assignment).expect("assignment fits function");

        assert_eq!(rewritten.id, original.id);
        assert_eq!(rewritten.name, original.name);
        assert!(rewritten.virtual_registers.is_empty());
        assert_eq!(rewritten.blocks[0].id, original.blocks[0].id);
        assert_eq!(
            rewritten.blocks[0].successors,
            original.blocks[0].successors
        );
        assert_eq!(
            rewritten.blocks[0].instructions[0].id,
            original.blocks[0].instructions[0].id
        );
        assert_eq!(
            rewritten.blocks[0].instructions[0].flags,
            original.blocks[0].instructions[0].flags
        );
        assert!(matches!(
            rewritten.blocks[0].instructions[0].operands[0].kind,
            MachineOperandKind::Register(MachineRegister::Physical(FIRST))
        ));
        assert_eq!(
            rewritten.blocks[0].instructions[0].operands[0].constraint,
            None
        );
        assert!(matches!(
            original.blocks[0].instructions[0].operands[0].kind,
            MachineOperandKind::Register(MachineRegister::Virtual(_))
        ));
        assert_eq!(
            rewritten.blocks[0].instructions[0].operands[2],
            original.blocks[0].instructions[0].operands[2]
        );
    }

    #[test]
    fn anchor_keeps_its_logical_virtual_definition_after_assignment() {
        // Python test_peephole.py::test_forwarded_spill_reload_retains_its_virtual_definition:
        // a deleted allocator reload still owns the virtual value its later
        // consumer names.  The actual x86 NOTHING opcode is introduced with
        // the anchor representation; use its reserved stable opcode here so
        // this regression fails against the old unconditional rewrite.
        let anchor = instruction(0, vec![virtual_register(0, OperandRole::Def)])
            .anchor(TargetOpcode::new(73));
        let original = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![anchor],
                successors: Vec::new(),
            }],
            &[0],
        );
        let assignment = allocated(&original);

        let rewritten = apply_assignment(&original, &assignment)
            .expect("the completed assignment still covers an anchor's virtual value");

        assert!(matches!(
            rewritten.blocks[0].instructions[0].operands[0].kind,
            MachineOperandKind::Register(MachineRegister::Virtual(register))
                if register == VirtualRegisterId::new(0)
        ));
        assert_eq!(rewritten.virtual_registers.len(), 1);
    }

    #[test]
    fn rejects_missing_assignment_entries() {
        let operand = virtual_register(0, OperandRole::Use);
        let function = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(0, vec![operand])],
                successors: Vec::new(),
            }],
            &[0],
        );
        let empty_assignment = allocated(&empty_function());

        let error = apply_assignment(&function, &empty_assignment)
            .expect_err("virtual operands require assignments");
        assert!(matches!(
            error,
            AllocationRewriteError::MissingAssignment {
                block,
                instruction,
                operand: 0,
                register,
            } if block == MachineBlockId::new(0)
                && instruction == MachineInstructionId::new(0)
                && register == VirtualRegisterId::new(0)
        ));
    }

    #[test]
    fn rejects_assignment_entries_for_undeclared_registers() {
        let source = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: Vec::new(),
                successors: Vec::new(),
            }],
            &[3],
        );
        let assignment = allocated(&source);

        let error = apply_assignment(&empty_function(), &assignment)
            .expect_err("extra assignment entries must be rejected");
        assert!(matches!(
            error,
            AllocationRewriteError::UndeclaredAssignment { register }
                if register == VirtualRegisterId::new(3)
        ));
    }

    #[test]
    fn rejects_assignment_that_omits_an_unused_declared_register() {
        let assignment = allocated(&empty_function());
        let function = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: Vec::new(),
                successors: Vec::new(),
            }],
            &[4],
        );

        let error = apply_assignment(&function, &assignment)
            .expect_err("a completed assignment must cover every declaration");
        assert_eq!(
            error,
            AllocationRewriteError::MissingDeclaredAssignment {
                register: VirtualRegisterId::new(4),
            }
        );
    }

    #[test]
    fn rejects_assignment_that_violates_a_fixed_constraint() {
        let unconstrained = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(0, vec![virtual_register(0, OperandRole::Use)])],
                successors: Vec::new(),
            }],
            &[0],
        );
        let assignment = allocated(&unconstrained);
        let mut fixed = virtual_register(0, OperandRole::Use);
        fixed.constraint = Some(RegisterConstraint::Fixed(SECOND));
        let constrained = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(0, vec![fixed])],
                successors: Vec::new(),
            }],
            &[0],
        );

        let error = apply_assignment(&constrained, &assignment)
            .expect_err("fixed constraints must still hold during rewrite");
        assert!(matches!(
            error,
            AllocationRewriteError::FixedConstraintMismatch {
                expected: SECOND,
                assigned: FIRST,
                ..
            }
        ));
    }

    #[test]
    fn leaves_physical_operands_unchanged() {
        let physical = MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Physical(SECOND)),
            role: OperandRole::Use,
            constraint: None,
            tied_to: None,
        };
        let function = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(0, vec![physical.clone()])],
                successors: Vec::new(),
            }],
            &[],
        );
        let assignment = allocated(&function);

        let rewritten =
            apply_assignment(&function, &assignment).expect("no virtual operands remain");
        assert_eq!(rewritten.blocks[0].instructions[0].operands[0], physical);
    }
}
