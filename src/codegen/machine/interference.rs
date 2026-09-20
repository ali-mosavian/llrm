//! Target-independent virtual-register interference for Machine IR.
//!
//! The graph records only overlap between virtual-register live ranges.  It
//! deliberately does not decide register classes, copies, coalescing, spills,
//! or physical-register availability.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::{
    MachineBlockId, MachineFunction, MachineInstruction, MachineLivenessError, MachineOperandKind,
    MachineRegister, VirtualRegisterId, compute_liveness,
};

/// An undirected virtual-register interference graph.
///
/// Every declared virtual register appears in `adjacency`, including nodes
/// with no neighbours.  The two entries for an edge always agree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterferenceGraph {
    adjacency: BTreeMap<VirtualRegisterId, BTreeSet<VirtualRegisterId>>,
}

impl InterferenceGraph {
    /// Declared virtual registers in deterministic ID order.
    pub fn registers(&self) -> impl Iterator<Item = VirtualRegisterId> + '_ {
        self.adjacency.keys().copied()
    }

    /// Registers that interfere with `register`, if it was declared.
    pub fn neighbours(&self, register: VirtualRegisterId) -> Option<&BTreeSet<VirtualRegisterId>> {
        self.adjacency.get(&register)
    }

    /// Whether two distinct virtual registers have overlapping live ranges.
    pub fn interferes(&self, left: VirtualRegisterId, right: VirtualRegisterId) -> bool {
        left != right
            && self
                .adjacency
                .get(&left)
                .is_some_and(|neighbours| neighbours.contains(&right))
    }
}

/// A failure while deriving virtual-register interference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterferenceError {
    Liveness {
        errors: Vec<MachineLivenessError>,
    },
    MissingBlockLiveness {
        block: MachineBlockId,
    },
    UnknownBlockFact {
        block: MachineBlockId,
    },
    UnknownVirtualRegister {
        block: MachineBlockId,
        register: VirtualRegisterId,
    },
}

impl fmt::Display for InterferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Liveness { errors } => {
                write!(
                    formatter,
                    "cannot compute interference: {} liveness error(s)",
                    errors.len()
                )
            }
            Self::MissingBlockLiveness { block } => {
                write!(formatter, "machine block {block} has no liveness fact")
            }
            Self::UnknownBlockFact { block } => {
                write!(formatter, "liveness contains unknown machine block {block}")
            }
            Self::UnknownVirtualRegister { block, register } => write!(
                formatter,
                "machine block {block} liveness references unknown virtual register {register}"
            ),
        }
    }
}

impl Error for InterferenceError {}

/// Computes virtual-register interference from Machine IR liveness.
///
/// Definitions conflict with every distinct value live after the instruction.
/// Definitions produced by the same instruction also conflict with each other.
/// Operand ties remain allocation constraints: they do not erase a real
/// overlap when the tied input remains live after the instruction.
pub fn compute_interference(
    function: &MachineFunction,
) -> Result<InterferenceGraph, InterferenceError> {
    let liveness =
        compute_liveness(function).map_err(|errors| InterferenceError::Liveness { errors })?;
    let declared = function
        .virtual_registers
        .iter()
        .map(|virtual_register| virtual_register.id)
        .collect::<BTreeSet<_>>();
    let block_ids = function
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();

    for (block, fact) in &liveness.blocks {
        if !block_ids.contains(block) {
            return Err(InterferenceError::UnknownBlockFact { block: *block });
        }
        for register in fact.live_in.iter().chain(fact.live_out.iter()) {
            if !declared.contains(register) {
                return Err(InterferenceError::UnknownVirtualRegister {
                    block: *block,
                    register: *register,
                });
            }
        }
    }

    let mut graph = InterferenceGraph {
        adjacency: declared
            .iter()
            .map(|register| (*register, BTreeSet::new()))
            .collect(),
    };

    for block in &function.blocks {
        let block_liveness = liveness
            .blocks
            .get(&block.id)
            .ok_or(InterferenceError::MissingBlockLiveness { block: block.id })?;
        let mut live = block_liveness.live_out.clone();

        for instruction in block.instructions.iter().rev() {
            let uses = instruction_uses(instruction);
            let definitions = instruction_definitions(instruction);
            for definition in &definitions {
                for live_register in &live {
                    add_edge(&mut graph, *definition, *live_register, block.id)?;
                }
            }
            add_simultaneous_definition_edges(&mut graph, &definitions, block.id)?;

            live.retain(|register| !definitions.contains(register));
            live.extend(uses);
        }
    }

    Ok(graph)
}

fn instruction_uses(instruction: &MachineInstruction) -> BTreeSet<VirtualRegisterId> {
    instruction
        .operands
        .iter()
        .filter_map(|operand| match &operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register))
                if operand.role.reads() =>
            {
                Some(*register)
            }
            _ => None,
        })
        .collect()
}

fn instruction_definitions(instruction: &MachineInstruction) -> BTreeSet<VirtualRegisterId> {
    instruction
        .operands
        .iter()
        .filter_map(|operand| match &operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register))
                if operand.role.writes() =>
            {
                Some(*register)
            }
            _ => None,
        })
        .collect()
}

fn add_simultaneous_definition_edges(
    graph: &mut InterferenceGraph,
    definitions: &BTreeSet<VirtualRegisterId>,
    block: MachineBlockId,
) -> Result<(), InterferenceError> {
    for left in definitions {
        for right in definitions.range(*left..) {
            if left == right {
                continue;
            }
            add_edge(graph, *left, *right, block)?;
        }
    }
    Ok(())
}

fn add_edge(
    graph: &mut InterferenceGraph,
    left: VirtualRegisterId,
    right: VirtualRegisterId,
    block: MachineBlockId,
) -> Result<(), InterferenceError> {
    if left == right {
        return Ok(());
    }
    if !graph.adjacency.contains_key(&left) {
        return Err(InterferenceError::UnknownVirtualRegister {
            block,
            register: left,
        });
    }
    if !graph.adjacency.contains_key(&right) {
        return Err(InterferenceError::UnknownVirtualRegister {
            block,
            register: right,
        });
    }

    graph.adjacency.entry(left).or_default().insert(right);
    graph.adjacency.entry(right).or_default().insert(left);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        InstructionFlags, MachineBlock, MachineFunctionId, MachineInstruction,
        MachineInstructionId, MachineOperand, OperandIndex, OperandRole, RegisterClass,
        TargetOpcode, VirtualRegister,
    };

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

    fn function(blocks: Vec<MachineBlock>, registers: &[u32]) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(0),
            name: "interference".to_owned(),
            linkage: crate::codegen::machine::MachineLinkage::Internal,
            signature: crate::codegen::machine::MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: crate::codegen::machine::MachineCallingConvention::FarPascal,
            },
            entry: blocks
                .first()
                .map(|block| block.id)
                .unwrap_or(MachineBlockId::new(0)),
            virtual_registers: registers
                .iter()
                .map(|id| VirtualRegister {
                    id: VirtualRegisterId::new(*id),
                    class: RegisterClass::new(0),
                })
                .collect(),
            blocks,
            frame_objects: Vec::new(),
        }
    }

    #[test]
    fn separates_overlapping_and_non_overlapping_straight_line_ranges() {
        let function = function(
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
            &[0, 1, 2],
        );

        let graph = compute_interference(&function).expect("machine function is well-formed");
        assert!(graph.interferes(VirtualRegisterId::new(0), VirtualRegisterId::new(1)));
        assert!(!graph.interferes(VirtualRegisterId::new(0), VirtualRegisterId::new(2)));
        assert!(!graph.interferes(VirtualRegisterId::new(1), VirtualRegisterId::new(2)));
    }

    #[test]
    fn follows_live_ranges_across_blocks() {
        let function = function(
            vec![
                MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: vec![
                        instruction(0, vec![virtual_register(0, OperandRole::Def)]),
                        instruction(1, vec![virtual_register(1, OperandRole::Def)]),
                    ],
                    successors: vec![MachineBlockId::new(1)],
                },
                MachineBlock {
                    id: MachineBlockId::new(1),
                    instructions: vec![instruction(2, vec![virtual_register(0, OperandRole::Use)])],
                    successors: Vec::new(),
                },
            ],
            &[0, 1],
        );

        let graph = compute_interference(&function).expect("machine function is well-formed");
        assert!(graph.interferes(VirtualRegisterId::new(0), VirtualRegisterId::new(1)));
    }

    #[test]
    fn models_use_def_and_a_dying_tied_input() {
        let simultaneous = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(
                    0,
                    vec![
                        virtual_register(0, OperandRole::UseDef),
                        virtual_register(1, OperandRole::UseDef),
                    ],
                )],
                successors: Vec::new(),
            }],
            &[0, 1],
        );
        let simultaneous_graph =
            compute_interference(&simultaneous).expect("machine function is well-formed");
        assert!(
            simultaneous_graph.interferes(VirtualRegisterId::new(0), VirtualRegisterId::new(1))
        );

        let mut output = virtual_register(0, OperandRole::Def);
        output.tied_to = Some(OperandIndex::new(1));
        let tied = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(
                    0,
                    vec![output, virtual_register(1, OperandRole::Use)],
                )],
                successors: Vec::new(),
            }],
            &[0, 1],
        );
        let tied_graph = compute_interference(&tied).expect("machine function is well-formed");
        assert!(!tied_graph.interferes(VirtualRegisterId::new(0), VirtualRegisterId::new(1)));
    }

    #[test]
    fn retains_isolated_declared_virtual_registers() {
        let function = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: Vec::new(),
                successors: Vec::new(),
            }],
            &[3],
        );

        let graph = compute_interference(&function).expect("machine function is well-formed");
        assert_eq!(
            graph.neighbours(VirtualRegisterId::new(3)),
            Some(&BTreeSet::new())
        );
    }

    #[test]
    fn propagates_malformed_liveness_input() {
        let function = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(0, vec![virtual_register(7, OperandRole::Use)])],
                successors: Vec::new(),
            }],
            &[],
        );

        let error =
            compute_interference(&function).expect_err("liveness must reject undeclared registers");
        assert!(matches!(
            error,
            InterferenceError::Liveness { errors }
                if errors.iter().any(|error| matches!(
                    error,
                    MachineLivenessError::UndeclaredVirtualRegister {
                        register,
                        ..
                    } if *register == VirtualRegisterId::new(7)
                ))
        ));
    }
}
