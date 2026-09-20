//! Deliberately limited deterministic virtual-register assignment.
//!
//! This initial allocator colours the virtual-register interference graph with
//! caller-provided physical-register candidates.  It has no coalescing,
//! splitting, spilling, or operand rewriting; unsupported constraints fail
//! explicitly instead of changing program meaning.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use super::{
    InterferenceError, MachineBlockId, MachineFunction, MachineInstructionId, MachineOperandKind,
    MachineRegister, PhysicalRegister, RegisterClass, RegisterConstraint, VirtualRegisterId,
    compute_interference,
};

/// A deterministic physical-register assignment for one machine function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisterAssignment {
    assignments: BTreeMap<VirtualRegisterId, PhysicalRegister>,
}

impl RegisterAssignment {
    /// The physical register assigned to one declared virtual register.
    pub fn get(&self, register: VirtualRegisterId) -> Option<PhysicalRegister> {
        self.assignments.get(&register).copied()
    }

    /// Assignments in deterministic virtual-register ID order.
    pub fn iter(&self) -> impl Iterator<Item = (VirtualRegisterId, PhysicalRegister)> + '_ {
        self.assignments
            .iter()
            .map(|(virtual_register, physical_register)| (*virtual_register, *physical_register))
    }
}

/// A refusal or malformed Machine IR fact encountered during assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AllocationError {
    Interference {
        error: InterferenceError,
    },
    UnsupportedTie {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
    },
    MissingDeclaredClass {
        register: VirtualRegisterId,
    },
    ClassConstraintConflict {
        register: VirtualRegisterId,
        declared: RegisterClass,
        constraint: RegisterClass,
    },
    FixedConstraintConflict {
        register: VirtualRegisterId,
        first: PhysicalRegister,
        second: PhysicalRegister,
    },
    FixedRegisterUnavailable {
        register: VirtualRegisterId,
        class: RegisterClass,
        physical: PhysicalRegister,
    },
    ConflictingFixedRegisters {
        first: VirtualRegisterId,
        first_physical: PhysicalRegister,
        second: VirtualRegisterId,
        second_physical: PhysicalRegister,
    },
    NoRegister {
        register: VirtualRegisterId,
        class: RegisterClass,
    },
}

impl fmt::Display for AllocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Interference { error } => write!(formatter, "cannot allocate registers: {error}"),
            Self::UnsupportedTie {
                block,
                instruction,
                operand,
            } => write!(
                formatter,
                "machine block {block} instruction {instruction} operand {} has an unsupported tie",
                operand
            ),
            Self::MissingDeclaredClass { register } => {
                write!(
                    formatter,
                    "virtual register {register} has no declared class"
                )
            }
            Self::ClassConstraintConflict {
                register,
                declared,
                constraint,
            } => write!(
                formatter,
                "virtual register {register} declares class {declared} but is constrained to class {constraint}"
            ),
            Self::FixedConstraintConflict {
                register,
                first,
                second,
            } => write!(
                formatter,
                "virtual register {register} is constrained to both physical registers {first} and {second}"
            ),
            Self::FixedRegisterUnavailable {
                register,
                class,
                physical,
            } => write!(
                formatter,
                "virtual register {register} class {class} cannot use fixed physical register {physical}"
            ),
            Self::ConflictingFixedRegisters {
                first,
                first_physical,
                second,
                second_physical,
            } => write!(
                formatter,
                "interfering virtual registers {first} ({first_physical}) and {second} ({second_physical}) have conflicting fixed assignments"
            ),
            Self::NoRegister { register, class } => write!(
                formatter,
                "no physical register is available for virtual register {register} in class {class}"
            ),
        }
    }
}

impl Error for AllocationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Interference { error } => Some(error),
            _ => None,
        }
    }
}

/// Assigns virtual registers using deterministic greedy graph colouring.
///
/// `candidates` supplies physical registers in the target's deterministic
/// preference order for a register class.  `aliases` reports whether two
/// different physical registers overlap; identical registers always conflict.
/// The function remains target-independent by accepting those facts as hooks.
pub fn allocate<Candidates, Aliases>(
    function: &MachineFunction,
    candidates: Candidates,
    aliases: Aliases,
) -> Result<RegisterAssignment, AllocationError>
where
    Candidates: Fn(RegisterClass) -> Vec<PhysicalRegister>,
    Aliases: Fn(PhysicalRegister, PhysicalRegister) -> bool,
{
    let graph =
        compute_interference(function).map_err(|error| AllocationError::Interference { error })?;
    reject_ties(function)?;

    let classes = function
        .virtual_registers
        .iter()
        .map(|virtual_register| (virtual_register.id, virtual_register.class))
        .collect::<BTreeMap<_, _>>();
    let fixed = collect_constraints(function, &classes)?;

    for (register, physical) in &fixed {
        let class = class_for(&classes, *register)?;
        if !candidates(class).contains(physical) {
            return Err(AllocationError::FixedRegisterUnavailable {
                register: *register,
                class,
                physical: *physical,
            });
        }
    }

    reject_conflicting_fixed_assignments(&graph, &fixed, &aliases)?;

    let mut assignments = fixed;
    let mut remaining = graph
        .registers()
        .filter(|register| !assignments.contains_key(register))
        .collect::<Vec<_>>();
    remaining.sort_by(|left, right| {
        let left_degree = graph
            .neighbours(*left)
            .map_or(0, |neighbours| neighbours.len());
        let right_degree = graph
            .neighbours(*right)
            .map_or(0, |neighbours| neighbours.len());
        right_degree.cmp(&left_degree).then_with(|| left.cmp(right))
    });

    for register in remaining {
        let class = class_for(&classes, register)?;
        let neighbours = graph
            .neighbours(register)
            .ok_or(AllocationError::MissingDeclaredClass { register })?;
        let physical = candidates(class)
            .iter()
            .copied()
            .find(|candidate| {
                neighbours.iter().all(|neighbour| {
                    assignments
                        .get(neighbour)
                        .is_none_or(|assigned| !physical_conflicts(*candidate, *assigned, &aliases))
                })
            })
            .ok_or(AllocationError::NoRegister { register, class })?;
        assignments.insert(register, physical);
    }

    Ok(RegisterAssignment { assignments })
}

fn reject_ties(function: &MachineFunction) -> Result<(), AllocationError> {
    for block in &function.blocks {
        for instruction in &block.instructions {
            for (position, operand) in instruction.operands.iter().enumerate() {
                if operand.tied_to.is_some() {
                    return Err(AllocationError::UnsupportedTie {
                        block: block.id,
                        instruction: instruction.id,
                        operand: position,
                    });
                }
            }
        }
    }
    Ok(())
}

fn collect_constraints(
    function: &MachineFunction,
    classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
) -> Result<BTreeMap<VirtualRegisterId, PhysicalRegister>, AllocationError> {
    let mut fixed = BTreeMap::new();

    for block in &function.blocks {
        for instruction in &block.instructions {
            for operand in &instruction.operands {
                let MachineOperandKind::Register(MachineRegister::Virtual(register)) =
                    &operand.kind
                else {
                    continue;
                };
                let class = class_for(classes, *register)?;
                match operand.constraint {
                    Some(RegisterConstraint::Fixed(physical)) => {
                        if let Some(previous) = fixed.insert(*register, physical) {
                            if previous != physical {
                                return Err(AllocationError::FixedConstraintConflict {
                                    register: *register,
                                    first: previous,
                                    second: physical,
                                });
                            }
                        }
                    }
                    Some(RegisterConstraint::Class(constraint)) if constraint != class => {
                        return Err(AllocationError::ClassConstraintConflict {
                            register: *register,
                            declared: class,
                            constraint,
                        });
                    }
                    Some(RegisterConstraint::Class(_)) | None => {}
                }
            }
        }
    }

    Ok(fixed)
}

fn class_for(
    classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    register: VirtualRegisterId,
) -> Result<RegisterClass, AllocationError> {
    classes
        .get(&register)
        .copied()
        .ok_or(AllocationError::MissingDeclaredClass { register })
}

fn reject_conflicting_fixed_assignments<Aliases>(
    graph: &super::InterferenceGraph,
    fixed: &BTreeMap<VirtualRegisterId, PhysicalRegister>,
    aliases: &Aliases,
) -> Result<(), AllocationError>
where
    Aliases: Fn(PhysicalRegister, PhysicalRegister) -> bool,
{
    for (register, physical) in fixed {
        let neighbours =
            graph
                .neighbours(*register)
                .ok_or(AllocationError::MissingDeclaredClass {
                    register: *register,
                })?;
        for neighbour in neighbours {
            if register >= neighbour {
                continue;
            }
            let Some(neighbour_physical) = fixed.get(neighbour) else {
                continue;
            };
            if physical_conflicts(*physical, *neighbour_physical, aliases) {
                return Err(AllocationError::ConflictingFixedRegisters {
                    first: *register,
                    first_physical: *physical,
                    second: *neighbour,
                    second_physical: *neighbour_physical,
                });
            }
        }
    }
    Ok(())
}

fn physical_conflicts<Aliases>(
    left: PhysicalRegister,
    right: PhysicalRegister,
    aliases: &Aliases,
) -> bool
where
    Aliases: Fn(PhysicalRegister, PhysicalRegister) -> bool,
{
    left == right || aliases(left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        InstructionFlags, MachineBlock, MachineFunctionId, MachineInstruction, MachineOperand,
        OperandIndex, OperandRole, TargetOpcode, VirtualRegister,
    };

    const GENERAL: RegisterClass = RegisterClass::new(0);
    const OTHER: RegisterClass = RegisterClass::new(1);
    const FIRST: PhysicalRegister = PhysicalRegister::new(0);
    const SECOND: PhysicalRegister = PhysicalRegister::new(1);
    const GENERAL_CANDIDATES: [PhysicalRegister; 2] = [FIRST, SECOND];
    const ONE_CANDIDATE: [PhysicalRegister; 1] = [FIRST];

    fn candidates(class: RegisterClass) -> Vec<PhysicalRegister> {
        match class {
            GENERAL => GENERAL_CANDIDATES.to_vec(),
            _ => Vec::new(),
        }
    }

    fn one_candidate(_: RegisterClass) -> Vec<PhysicalRegister> {
        ONE_CANDIDATE.to_vec()
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
            opcode: TargetOpcode::new(0),
            operands,
            flags: InstructionFlags::NONE,
        }
    }

    fn function(blocks: Vec<MachineBlock>, registers: &[(u32, RegisterClass)]) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(0),
            name: "allocation".to_owned(),
            virtual_registers: registers
                .iter()
                .map(|(id, class)| VirtualRegister {
                    id: VirtualRegisterId::new(*id),
                    class: *class,
                })
                .collect(),
            blocks,
            frame_objects: Vec::new(),
        }
    }

    fn overlapping_function() -> MachineFunction {
        function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![
                    instruction(0, vec![virtual_register(0, OperandRole::Def)]),
                    instruction(1, vec![virtual_register(1, OperandRole::Def)]),
                    instruction(
                        2,
                        vec![
                            virtual_register(0, OperandRole::Use),
                            virtual_register(1, OperandRole::Use),
                        ],
                    ),
                    instruction(3, vec![virtual_register(2, OperandRole::Def)]),
                    instruction(4, vec![virtual_register(2, OperandRole::Use)]),
                ],
                successors: Vec::new(),
            }],
            &[(0, GENERAL), (1, GENERAL), (2, GENERAL)],
        )
    }

    #[test]
    fn colours_deterministically_and_reuses_noninterfering_registers() {
        let assignment = allocate(&overlapping_function(), candidates, does_not_alias)
            .expect("two candidates colour the graph");

        assert_eq!(assignment.get(VirtualRegisterId::new(0)), Some(FIRST));
        assert_eq!(assignment.get(VirtualRegisterId::new(1)), Some(SECOND));
        assert_eq!(assignment.get(VirtualRegisterId::new(2)), Some(FIRST));
    }

    #[test]
    fn rejects_interfering_fixed_register_assignments() {
        let mut function = overlapping_function();
        function.blocks[0].instructions[0].operands[0].constraint =
            Some(RegisterConstraint::Fixed(FIRST));
        function.blocks[0].instructions[1].operands[0].constraint =
            Some(RegisterConstraint::Fixed(FIRST));

        let error = allocate(&function, candidates, does_not_alias)
            .expect_err("interfering fixed assignments cannot share a physical register");
        assert!(matches!(
            error,
            AllocationError::ConflictingFixedRegisters { .. }
        ));
    }

    #[test]
    fn reports_an_exhausted_register_class_without_spilling() {
        let error = allocate(&overlapping_function(), one_candidate, does_not_alias)
            .expect_err("one candidate cannot colour an interfering pair");
        assert!(matches!(
            error,
            AllocationError::NoRegister {
                register,
                class: GENERAL,
            } if register == VirtualRegisterId::new(1)
        ));
    }

    #[test]
    fn rejects_a_class_constraint_that_disagrees_with_the_declaration() {
        let mut operand = virtual_register(0, OperandRole::Use);
        operand.constraint = Some(RegisterConstraint::Class(OTHER));
        let function = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(0, vec![operand])],
                successors: Vec::new(),
            }],
            &[(0, GENERAL)],
        );

        let error = allocate(&function, candidates, does_not_alias)
            .expect_err("class constraints must agree with declarations");
        assert!(matches!(
            error,
            AllocationError::ClassConstraintConflict {
                register,
                declared: GENERAL,
                constraint: OTHER,
            } if register == VirtualRegisterId::new(0)
        ));
    }

    #[test]
    fn refuses_operand_ties_before_assignment() {
        let mut destination = virtual_register(0, OperandRole::Def);
        destination.tied_to = Some(OperandIndex::new(1));
        let function = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(
                    0,
                    vec![destination, virtual_register(1, OperandRole::Use)],
                )],
                successors: Vec::new(),
            }],
            &[(0, GENERAL), (1, GENERAL)],
        );

        let error = allocate(&function, candidates, does_not_alias)
            .expect_err("ties are not yet supported by this allocator");
        assert!(matches!(
            error,
            AllocationError::UnsupportedTie {
                block,
                instruction,
                operand,
            } if block == MachineBlockId::new(0)
                && instruction == MachineInstructionId::new(0)
                && operand == 0
        ));
    }

    #[test]
    fn rejects_fixed_registers_outside_their_class_candidates() {
        let mut operand = virtual_register(0, OperandRole::Use);
        operand.constraint = Some(RegisterConstraint::Fixed(SECOND));
        let function = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(0, vec![operand])],
                successors: Vec::new(),
            }],
            &[(0, GENERAL)],
        );

        let error = allocate(&function, one_candidate, does_not_alias)
            .expect_err("fixed registers must belong to their declared class");
        assert!(matches!(
            error,
            AllocationError::FixedRegisterUnavailable {
                register,
                class: GENERAL,
                physical: SECOND,
            } if register == VirtualRegisterId::new(0)
        ));
    }
}
