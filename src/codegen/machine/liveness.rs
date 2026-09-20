//! Target-independent virtual-register liveness for Machine IR.
//!
//! This analysis follows the explicit Machine IR control-flow graph and does
//! not assign locations or attach any target meaning to registers.  It is a
//! pure, deterministic prerequisite for later allocation work.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::{
    MachineBlock, MachineBlockId, MachineFunction, MachineInstructionId, MachineOperandKind,
    MachineRegister, VirtualRegisterId,
};

/// Liveness facts for every block in one machine function.
///
/// The map is keyed by block ID so callers do not depend on the function's
/// storage order.  Each set contains only declared virtual registers;
/// physical-register liveness belongs to a later target-aware phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineLiveness {
    pub blocks: BTreeMap<MachineBlockId, BlockLiveness>,
}

/// Registers live at one basic-block boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockLiveness {
    pub live_in: BTreeSet<VirtualRegisterId>,
    pub live_out: BTreeSet<VirtualRegisterId>,
}

/// A malformed Machine IR fact which prevents liveness computation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineLivenessError {
    DuplicateBlockId {
        block: MachineBlockId,
    },
    UnknownSuccessor {
        block: MachineBlockId,
        successor: MachineBlockId,
    },
    UnknownBlockId {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        target: MachineBlockId,
    },
    UndeclaredVirtualRegister {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        register: VirtualRegisterId,
    },
    DuplicateVirtualRegisterDeclaration {
        register: VirtualRegisterId,
    },
}

impl fmt::Display for MachineLivenessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateBlockId { block } => {
                write!(formatter, "duplicate machine block id {block}")
            }
            Self::UnknownSuccessor { block, successor } => {
                write!(
                    formatter,
                    "machine block {block} has unknown successor {successor}"
                )
            }
            Self::UnknownBlockId {
                block,
                instruction,
                target,
            } => write!(
                formatter,
                "machine block {block} instruction {instruction} references unknown block {target}"
            ),
            Self::UndeclaredVirtualRegister {
                block,
                instruction,
                register,
            } => write!(
                formatter,
                "machine block {block} instruction {instruction} references undeclared virtual register {register}"
            ),
            Self::DuplicateVirtualRegisterDeclaration { register } => {
                write!(
                    formatter,
                    "duplicate virtual register declaration {register}"
                )
            }
        }
    }
}

impl Error for MachineLivenessError {}

/// Computes conventional backwards liveness to a fixed point.
///
/// Within an instruction all uses are observed before its definitions.  That
/// models a read/write (`UseDef`) operand as reading the old value before
/// defining the new one, independent of operand list order.
pub fn compute_liveness(
    function: &MachineFunction,
) -> Result<MachineLiveness, Vec<MachineLivenessError>> {
    let (declared_registers, mut errors) = declared_virtual_registers(function);
    let (blocks, block_errors) = indexed_blocks(function);
    errors.extend(block_errors);
    errors.extend(validate_references(&blocks, &declared_registers));
    if !errors.is_empty() {
        return Err(errors);
    }

    let facts = blocks
        .iter()
        .map(|(id, block)| (*id, block_facts(block)))
        .collect::<BTreeMap<_, _>>();
    let mut result = MachineLiveness {
        blocks: blocks
            .keys()
            .map(|id| (*id, BlockLiveness::default()))
            .collect(),
    };

    loop {
        let mut changed = false;

        for (id, block) in blocks.iter().rev() {
            let live_out = block
                .successors
                .iter()
                .flat_map(|successor| result.blocks[successor].live_in.iter().copied())
                .collect::<BTreeSet<_>>();
            let facts = &facts[id];
            let live_in = facts
                .uses
                .iter()
                .copied()
                .chain(live_out.difference(&facts.definitions).copied())
                .collect::<BTreeSet<_>>();
            let block_result = &result.blocks[id];

            if block_result.live_in != live_in || block_result.live_out != live_out {
                result
                    .blocks
                    .insert(*id, BlockLiveness { live_in, live_out });
                changed = true;
            }
        }

        if !changed {
            return Ok(result);
        }
    }
}

fn declared_virtual_registers(
    function: &MachineFunction,
) -> (BTreeSet<VirtualRegisterId>, Vec<MachineLivenessError>) {
    let mut declared = BTreeSet::new();
    let mut errors = Vec::new();

    for virtual_register in &function.virtual_registers {
        if !declared.insert(virtual_register.id) {
            errors.push(MachineLivenessError::DuplicateVirtualRegisterDeclaration {
                register: virtual_register.id,
            });
        }
    }

    (declared, errors)
}

fn indexed_blocks(
    function: &MachineFunction,
) -> (
    BTreeMap<MachineBlockId, &MachineBlock>,
    Vec<MachineLivenessError>,
) {
    let mut blocks = BTreeMap::new();
    let mut errors = Vec::new();

    for block in &function.blocks {
        if blocks.contains_key(&block.id) {
            errors.push(MachineLivenessError::DuplicateBlockId { block: block.id });
        } else {
            blocks.insert(block.id, block);
        }
    }

    (blocks, errors)
}

fn validate_references(
    blocks: &BTreeMap<MachineBlockId, &MachineBlock>,
    declared_registers: &BTreeSet<VirtualRegisterId>,
) -> Vec<MachineLivenessError> {
    let mut errors = Vec::new();

    for (block_id, block) in blocks {
        for successor in &block.successors {
            if !blocks.contains_key(successor) {
                errors.push(MachineLivenessError::UnknownSuccessor {
                    block: *block_id,
                    successor: *successor,
                });
            }
        }

        for instruction in &block.instructions {
            for operand in &instruction.operands {
                match &operand.kind {
                    MachineOperandKind::Register(MachineRegister::Virtual(register))
                        if !declared_registers.contains(register) =>
                    {
                        errors.push(MachineLivenessError::UndeclaredVirtualRegister {
                            block: *block_id,
                            instruction: instruction.id,
                            register: *register,
                        });
                    }
                    MachineOperandKind::Block(target) if !blocks.contains_key(target) => {
                        errors.push(MachineLivenessError::UnknownBlockId {
                            block: *block_id,
                            instruction: instruction.id,
                            target: *target,
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    errors
}

struct BlockFacts {
    uses: BTreeSet<VirtualRegisterId>,
    definitions: BTreeSet<VirtualRegisterId>,
}

fn block_facts(block: &MachineBlock) -> BlockFacts {
    let mut uses = BTreeSet::new();
    let mut definitions = BTreeSet::new();

    for instruction in &block.instructions {
        for operand in &instruction.operands {
            let MachineOperandKind::Register(MachineRegister::Virtual(register)) = &operand.kind
            else {
                continue;
            };
            if operand.role.reads() && !definitions.contains(register) {
                uses.insert(*register);
            }
        }
        for operand in &instruction.operands {
            let MachineOperandKind::Register(MachineRegister::Virtual(register)) = &operand.kind
            else {
                continue;
            };
            if operand.role.writes() {
                definitions.insert(*register);
            }
        }
    }

    BlockFacts { uses, definitions }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        InstructionFlags, MachineFunctionId, MachineInstruction, MachineOperand, OperandRole,
        RegisterClass, TargetOpcode, VirtualRegister,
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
            name: "liveness".to_owned(),
            linkage: crate::codegen::machine::MachineLinkage::Internal,
            signature: crate::codegen::machine::MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: crate::codegen::machine::MachineCallingConvention::Basic,
            },
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

    fn registers(ids: &[u32]) -> BTreeSet<VirtualRegisterId> {
        ids.iter().copied().map(VirtualRegisterId::new).collect()
    }

    #[test]
    fn preserves_cross_block_live_through_values() {
        let function = function(
            vec![
                MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: Vec::new(),
                    successors: vec![MachineBlockId::new(1)],
                },
                MachineBlock {
                    id: MachineBlockId::new(1),
                    instructions: vec![instruction(0, vec![virtual_register(0, OperandRole::Use)])],
                    successors: Vec::new(),
                },
            ],
            &[0],
        );

        let liveness = compute_liveness(&function).expect("machine function is well-formed");
        assert_eq!(
            liveness.blocks[&MachineBlockId::new(0)].live_in,
            registers(&[0])
        );
        assert_eq!(
            liveness.blocks[&MachineBlockId::new(0)].live_out,
            registers(&[0])
        );
        assert_eq!(
            liveness.blocks[&MachineBlockId::new(1)].live_in,
            registers(&[0])
        );
    }

    #[test]
    fn definition_kills_incoming_liveness() {
        let function = function(
            vec![
                MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: vec![instruction(0, vec![virtual_register(0, OperandRole::Def)])],
                    successors: vec![MachineBlockId::new(1)],
                },
                MachineBlock {
                    id: MachineBlockId::new(1),
                    instructions: vec![instruction(1, vec![virtual_register(0, OperandRole::Use)])],
                    successors: Vec::new(),
                },
            ],
            &[0],
        );

        let liveness = compute_liveness(&function).expect("machine function is well-formed");
        assert_eq!(
            liveness.blocks[&MachineBlockId::new(0)].live_in,
            BTreeSet::new()
        );
        assert_eq!(
            liveness.blocks[&MachineBlockId::new(0)].live_out,
            registers(&[0])
        );
    }

    #[test]
    fn use_def_reads_before_defining() {
        let function = function(
            vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions: vec![instruction(
                    0,
                    vec![virtual_register(0, OperandRole::UseDef)],
                )],
                successors: Vec::new(),
            }],
            &[0],
        );

        let liveness = compute_liveness(&function).expect("machine function is well-formed");
        assert_eq!(
            liveness.blocks[&MachineBlockId::new(0)].live_in,
            registers(&[0])
        );
    }

    #[test]
    fn converges_through_a_loop_with_a_use_before_definition() {
        let function = function(
            vec![
                MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: vec![instruction(0, vec![virtual_register(0, OperandRole::Use)])],
                    successors: vec![MachineBlockId::new(1)],
                },
                MachineBlock {
                    id: MachineBlockId::new(1),
                    instructions: vec![instruction(1, vec![virtual_register(0, OperandRole::Def)])],
                    successors: vec![MachineBlockId::new(0)],
                },
            ],
            &[0],
        );

        let liveness = compute_liveness(&function).expect("machine function is well-formed");
        assert_eq!(
            liveness.blocks[&MachineBlockId::new(0)].live_in,
            registers(&[0])
        );
        assert_eq!(
            liveness.blocks[&MachineBlockId::new(1)].live_in,
            BTreeSet::new()
        );
        assert_eq!(
            liveness.blocks[&MachineBlockId::new(1)].live_out,
            registers(&[0])
        );
    }

    #[test]
    fn rejects_malformed_block_and_virtual_register_references() {
        let function = function(
            vec![
                MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: vec![instruction(
                        0,
                        vec![
                            virtual_register(9, OperandRole::Use),
                            MachineOperand {
                                kind: MachineOperandKind::Block(MachineBlockId::new(8)),
                                role: OperandRole::None,
                                constraint: None,
                                tied_to: None,
                            },
                        ],
                    )],
                    successors: vec![MachineBlockId::new(7)],
                },
                MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: Vec::new(),
                    successors: Vec::new(),
                },
            ],
            &[0, 0],
        );

        let errors = compute_liveness(&function).expect_err("references are malformed");
        assert!(
            errors.contains(&MachineLivenessError::DuplicateVirtualRegisterDeclaration {
                register: VirtualRegisterId::new(0),
            })
        );
        assert!(errors.contains(&MachineLivenessError::DuplicateBlockId {
            block: MachineBlockId::new(0),
        }));
        assert!(errors.contains(&MachineLivenessError::UnknownSuccessor {
            block: MachineBlockId::new(0),
            successor: MachineBlockId::new(7),
        }));
        assert!(errors.contains(&MachineLivenessError::UnknownBlockId {
            block: MachineBlockId::new(0),
            instruction: MachineInstructionId::new(0),
            target: MachineBlockId::new(8),
        }));
        assert!(
            errors.contains(&MachineLivenessError::UndeclaredVirtualRegister {
                block: MachineBlockId::new(0),
                instruction: MachineInstructionId::new(0),
                register: VirtualRegisterId::new(9),
            })
        );
    }
}
