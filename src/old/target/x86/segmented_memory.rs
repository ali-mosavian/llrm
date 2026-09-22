//! Final expansion of allocated segmented-memory accesses.
//!
//! Selection represents an access through a 16:16 pointer as an explicit
//! bracket: save `ES`, extract the offset and selector, install the selector,
//! access memory through `ES`, then restore `ES`.  Allocation needs those
//! separate values and their register classes.  Once every operand is
//! physical, this module recognizes only that complete bracket and replaces
//! it with the balanced stack sequence used by the x86 backend.
//!
//! The recognition is intentionally narrower than lowering `LowWord` or
//! `HighWord` in general.  Those pseudos also serve ABI results, whose meaning
//! belongs to their ABI finalizers rather than to segmented memory.

use std::error::Error;
use std::fmt;

use crate::old::codegen::machine::{
    InstructionFlags, MachineBlockId, MachineFunction, MachineInstruction, MachineInstructionId,
    MachineOperand, MachineOperandKind, MachineRegister, OperandRole,
};

use super::{X86Opcode, X86Register, X86RegisterClass};

/// A refusal while expanding an allocated segmented-memory bracket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SegmentedMemoryExpansionError {
    MalformedPattern {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        reason: &'static str,
    },
}

impl fmt::Display for SegmentedMemoryExpansionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedPattern {
                block,
                instruction,
                reason,
            } => write!(
                formatter,
                "block {block} instruction {instruction} has malformed segmented-memory bracket: {reason}"
            ),
        }
    }
}

impl Error for SegmentedMemoryExpansionError {}

/// Replaces complete allocated segmented-memory brackets with physical x86
/// stack operations.
///
/// The input is never changed.  All brackets are validated before cloning, so
/// a malformed candidate returns an error without a partial rewrite.  Every
/// replacement has the original bracket's six instruction IDs in order:
/// `Push ES`, `Push pointer`, `Pop offset`, `Pop ES`, access, `Pop ES`.
pub fn expand_allocated_segmented_memory(
    function: &MachineFunction,
) -> Result<MachineFunction, SegmentedMemoryExpansionError> {
    let patterns = preflight(function)?;
    if patterns.is_empty() {
        return Ok(function.clone());
    }

    let mut expanded = function.clone();
    for (block_index, block) in expanded.blocks.iter_mut().enumerate() {
        let mut instructions = Vec::with_capacity(block.instructions.len());
        let mut position = 0;
        while position < block.instructions.len() {
            let pattern = patterns
                .iter()
                .find(|pattern| pattern.block_index == block_index && pattern.position == position);
            let Some(pattern) = pattern else {
                instructions.push(block.instructions[position].clone());
                position += 1;
                continue;
            };

            let bracket = &block.instructions[position..position + 6];
            instructions.extend(replace_bracket(bracket, pattern));
            position += 6;
        }
        block.instructions = instructions;
    }
    Ok(expanded)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Pattern {
    block_index: usize,
    position: usize,
    pointer: X86Register,
    offset: X86Register,
}

fn preflight(function: &MachineFunction) -> Result<Vec<Pattern>, SegmentedMemoryExpansionError> {
    let mut patterns = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        let instructions = &block.instructions;
        for position in 0..instructions.len() {
            if !starts_candidate(instructions, position) {
                continue;
            }
            patterns.push(validate_pattern(
                block_index,
                block.id,
                instructions,
                position,
            )?);
        }
    }
    Ok(patterns)
}

fn starts_candidate(instructions: &[MachineInstruction], position: usize) -> bool {
    let Some(push) = instructions.get(position) else {
        return false;
    };
    let Some(low) = instructions.get(position + 1) else {
        return false;
    };
    X86Opcode::from_machine_opcode(push.opcode) == Some(X86Opcode::Push)
        && push
            .operands
            .iter()
            .any(|operand| physical_register(operand) == Some(X86Register::Es))
        && X86Opcode::from_machine_opcode(low.opcode) == Some(X86Opcode::LowWord)
}

fn validate_pattern(
    block_index: usize,
    block: MachineBlockId,
    instructions: &[MachineInstruction],
    position: usize,
) -> Result<Pattern, SegmentedMemoryExpansionError> {
    let leader = &instructions[position];
    let Some(bracket) = instructions.get(position..position + 6) else {
        return malformed(block, leader, "incomplete six-instruction sequence");
    };
    let [push_es, low, high, install_es, access, restore_es] = bracket else {
        unreachable!("six instruction slice has six elements");
    };

    require_opcode(block, leader, push_es, X86Opcode::Push)?;
    require_no_flags(block, leader, push_es)?;
    require_operands(block, leader, push_es, 1)?;
    require_register(
        block,
        leader,
        &push_es.operands[0],
        OperandRole::Use,
        X86Register::Es,
    )?;

    require_opcode(block, leader, low, X86Opcode::LowWord)?;
    require_no_flags(block, leader, low)?;
    let [offset, low_pointer] = low.operands.as_slice() else {
        return malformed(
            block,
            leader,
            "low-word extraction requires [address16 def, dword use]",
        );
    };
    let offset = require_register_class(
        block,
        leader,
        offset,
        OperandRole::Def,
        X86RegisterClass::Address16,
    )?;
    let pointer = require_register_class(
        block,
        leader,
        low_pointer,
        OperandRole::Use,
        X86RegisterClass::Dword,
    )?;

    require_opcode(block, leader, high, X86Opcode::HighWord)?;
    require_no_flags(block, leader, high)?;
    let [selector, high_pointer] = high.operands.as_slice() else {
        return malformed(
            block,
            leader,
            "high-word extraction requires [word def, dword use]",
        );
    };
    let selector = require_register_class(
        block,
        leader,
        selector,
        OperandRole::Def,
        X86RegisterClass::Word,
    )?;
    if require_register_class(
        block,
        leader,
        high_pointer,
        OperandRole::Use,
        X86RegisterClass::Dword,
    )? != pointer
    {
        return malformed(
            block,
            leader,
            "word extractions use different pointer registers",
        );
    }

    require_opcode(block, leader, install_es, X86Opcode::Mov)?;
    require_no_flags(block, leader, install_es)?;
    let [destination, source] = install_es.operands.as_slice() else {
        return malformed(
            block,
            leader,
            "selector installation requires [ES def, selector use]",
        );
    };
    require_register(
        block,
        leader,
        destination,
        OperandRole::Def,
        X86Register::Es,
    )?;
    if require_register_class(
        block,
        leader,
        source,
        OperandRole::Use,
        X86RegisterClass::Word,
    )? != selector
    {
        return malformed(
            block,
            leader,
            "selector installation does not use high-word result",
        );
    }

    validate_access(block, leader, access, offset)?;

    require_opcode(block, leader, restore_es, X86Opcode::Pop)?;
    require_no_flags(block, leader, restore_es)?;
    require_operands(block, leader, restore_es, 1)?;
    require_register(
        block,
        leader,
        &restore_es.operands[0],
        OperandRole::Def,
        X86Register::Es,
    )?;

    Ok(Pattern {
        block_index,
        position,
        pointer,
        offset,
    })
}

fn validate_access(
    block: MachineBlockId,
    leader: &MachineInstruction,
    access: &MachineInstruction,
    offset: X86Register,
) -> Result<(), SegmentedMemoryExpansionError> {
    match X86Opcode::from_machine_opcode(access.opcode) {
        Some(X86Opcode::Load) => {
            let [destination, base, selector] = access.operands.as_slice() else {
                return malformed(
                    block,
                    leader,
                    "segmented load requires [data def, offset use, ES use]",
                );
            };
            require_data_register(block, leader, destination, OperandRole::Def)?;
            require_register(block, leader, base, OperandRole::Use, offset)?;
            require_register(block, leader, selector, OperandRole::Use, X86Register::Es)?;
        }
        Some(X86Opcode::Store) => {
            let [base, source, selector] = access.operands.as_slice() else {
                return malformed(
                    block,
                    leader,
                    "segmented store requires [offset use, data use, ES use]",
                );
            };
            require_register(block, leader, base, OperandRole::Use, offset)?;
            require_data_register(block, leader, source, OperandRole::Use)?;
            require_register(block, leader, selector, OperandRole::Use, X86Register::Es)?;
        }
        _ => {
            return malformed(
                block,
                leader,
                "selector installation does not feed a load or store",
            );
        }
    }
    Ok(())
}

fn replace_bracket(bracket: &[MachineInstruction], pattern: &Pattern) -> [MachineInstruction; 6] {
    let [push_es, low, high, install_es, access, restore_es] = bracket else {
        unreachable!("preflight supplied a complete bracket");
    };
    [
        push_es.clone(),
        instruction(
            low.id,
            X86Opcode::Push,
            vec![physical_operand(pattern.pointer, OperandRole::Use)],
        ),
        instruction(
            high.id,
            X86Opcode::Pop,
            vec![physical_operand(pattern.offset, OperandRole::Def)],
        ),
        instruction(
            install_es.id,
            X86Opcode::Pop,
            vec![physical_operand(X86Register::Es, OperandRole::Def)],
        ),
        access.clone(),
        restore_es.clone(),
    ]
}

fn require_opcode(
    block: MachineBlockId,
    leader: &MachineInstruction,
    instruction: &MachineInstruction,
    expected: X86Opcode,
) -> Result<(), SegmentedMemoryExpansionError> {
    if X86Opcode::from_machine_opcode(instruction.opcode) != Some(expected) {
        return malformed(
            block,
            leader,
            "bracket opcode order is not push/low/high/mov/access/pop",
        );
    }
    Ok(())
}

fn require_no_flags(
    block: MachineBlockId,
    leader: &MachineInstruction,
    instruction: &MachineInstruction,
) -> Result<(), SegmentedMemoryExpansionError> {
    if instruction.flags != InstructionFlags::NONE {
        return malformed(
            block,
            leader,
            "bracket setup and restore instructions must not have flags",
        );
    }
    Ok(())
}

fn require_operands(
    block: MachineBlockId,
    leader: &MachineInstruction,
    instruction: &MachineInstruction,
    count: usize,
) -> Result<(), SegmentedMemoryExpansionError> {
    if instruction.operands.len() != count {
        return malformed(
            block,
            leader,
            "bracket instruction has an unexpected operand count",
        );
    }
    Ok(())
}

fn require_register(
    block: MachineBlockId,
    leader: &MachineInstruction,
    operand: &MachineOperand,
    role: OperandRole,
    expected: X86Register,
) -> Result<X86Register, SegmentedMemoryExpansionError> {
    let actual = require_physical(block, leader, operand, role)?;
    if actual != expected {
        return malformed(
            block,
            leader,
            "bracket register flow does not match its access",
        );
    }
    Ok(actual)
}

fn require_register_class(
    block: MachineBlockId,
    leader: &MachineInstruction,
    operand: &MachineOperand,
    role: OperandRole,
    class: X86RegisterClass,
) -> Result<X86Register, SegmentedMemoryExpansionError> {
    let register = require_physical(block, leader, operand, role)?;
    if !class.members().contains(&register) {
        return malformed(block, leader, "bracket register has the wrong x86 class");
    }
    Ok(register)
}

fn require_data_register(
    block: MachineBlockId,
    leader: &MachineInstruction,
    operand: &MachineOperand,
    role: OperandRole,
) -> Result<X86Register, SegmentedMemoryExpansionError> {
    let register = require_physical(block, leader, operand, role)?;
    if ![
        X86RegisterClass::Byte,
        X86RegisterClass::Word,
        X86RegisterClass::Dword,
    ]
    .iter()
    .any(|class| class.members().contains(&register))
    {
        return malformed(block, leader, "memory value must use a data register");
    }
    Ok(register)
}

fn require_physical(
    block: MachineBlockId,
    leader: &MachineInstruction,
    operand: &MachineOperand,
    role: OperandRole,
) -> Result<X86Register, SegmentedMemoryExpansionError> {
    if operand.role != role || operand.constraint.is_some() || operand.tied_to.is_some() {
        return malformed(
            block,
            leader,
            "bracket operand is not an unconstrained allocated register",
        );
    }
    let Some(register) = physical_register(operand) else {
        return malformed(
            block,
            leader,
            "bracket operand is not a known physical x86 register",
        );
    };
    Ok(register)
}

fn physical_register(operand: &MachineOperand) -> Option<X86Register> {
    let MachineOperandKind::Register(MachineRegister::Physical(register)) = operand.kind else {
        return None;
    };
    X86Register::from_physical(register)
}

fn instruction(
    id: MachineInstructionId,
    opcode: X86Opcode,
    operands: Vec<MachineOperand>,
) -> MachineInstruction {
    MachineInstruction {
        id,
        opcode: opcode.machine_opcode(),
        operands,
        flags: InstructionFlags::NONE,
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

fn malformed<T>(
    block: MachineBlockId,
    instruction: &MachineInstruction,
    reason: &'static str,
) -> Result<T, SegmentedMemoryExpansionError> {
    Err(SegmentedMemoryExpansionError::MalformedPattern {
        block,
        instruction: instruction.id,
        reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::old::codegen::machine::{
        MachineBlock, MachineCallingConvention, MachineFunctionId, MachineLinkage, MachineSignature,
    };

    fn function(instructions: Vec<MachineInstruction>) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(1),
            name: "segmented_access".to_owned(),
            linkage: MachineLinkage::Internal,
            signature: MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::C,
            },
            entry: MachineBlockId::new(0),
            virtual_registers: Vec::new(),
            blocks: vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions,
                successors: Vec::new(),
            }],
            frame_objects: Vec::new(),
        }
    }

    fn access_flags(store: bool) -> InstructionFlags {
        InstructionFlags {
            side_effects: store,
            may_load: !store,
            may_store: store,
            volatile: store,
            ..InstructionFlags::NONE
        }
    }

    fn segmented_access(store: bool) -> MachineFunction {
        let access = if store {
            MachineInstruction {
                id: MachineInstructionId::new(14),
                opcode: X86Opcode::Store.machine_opcode(),
                operands: vec![
                    physical_operand(X86Register::Bx, OperandRole::Use),
                    physical_operand(X86Register::Dx, OperandRole::Use),
                    physical_operand(X86Register::Es, OperandRole::Use),
                ],
                flags: access_flags(true),
            }
        } else {
            MachineInstruction {
                id: MachineInstructionId::new(14),
                opcode: X86Opcode::Load.machine_opcode(),
                operands: vec![
                    physical_operand(X86Register::Dx, OperandRole::Def),
                    physical_operand(X86Register::Bx, OperandRole::Use),
                    physical_operand(X86Register::Es, OperandRole::Use),
                ],
                flags: access_flags(false),
            }
        };
        function(vec![
            instruction(
                MachineInstructionId::new(10),
                X86Opcode::Push,
                vec![physical_operand(X86Register::Es, OperandRole::Use)],
            ),
            instruction(
                MachineInstructionId::new(11),
                X86Opcode::LowWord,
                vec![
                    physical_operand(X86Register::Bx, OperandRole::Def),
                    physical_operand(X86Register::Eax, OperandRole::Use),
                ],
            ),
            instruction(
                MachineInstructionId::new(12),
                X86Opcode::HighWord,
                vec![
                    physical_operand(X86Register::Cx, OperandRole::Def),
                    physical_operand(X86Register::Eax, OperandRole::Use),
                ],
            ),
            instruction(
                MachineInstructionId::new(13),
                X86Opcode::Mov,
                vec![
                    physical_operand(X86Register::Es, OperandRole::Def),
                    physical_operand(X86Register::Cx, OperandRole::Use),
                ],
            ),
            access,
            instruction(
                MachineInstructionId::new(15),
                X86Opcode::Pop,
                vec![physical_operand(X86Register::Es, OperandRole::Def)],
            ),
        ])
    }

    fn opcodes(function: &MachineFunction) -> Vec<Option<X86Opcode>> {
        function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| X86Opcode::from_machine_opcode(instruction.opcode))
            .collect()
    }

    #[test]
    fn expands_a_segmented_load_with_balanced_es_and_stack() {
        let input = segmented_access(false);
        let expanded = expand_allocated_segmented_memory(&input).unwrap();
        let instructions = &expanded.blocks[0].instructions;

        assert_eq!(
            opcodes(&expanded),
            vec![
                Some(X86Opcode::Push),
                Some(X86Opcode::Push),
                Some(X86Opcode::Pop),
                Some(X86Opcode::Pop),
                Some(X86Opcode::Load),
                Some(X86Opcode::Pop),
            ]
        );
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            (10..=15).map(MachineInstructionId::new).collect::<Vec<_>>(),
        );
        assert_eq!(
            instructions[0].operands,
            vec![physical_operand(X86Register::Es, OperandRole::Use)]
        );
        assert_eq!(
            instructions[1].operands,
            vec![physical_operand(X86Register::Eax, OperandRole::Use)]
        );
        assert_eq!(
            instructions[2].operands,
            vec![physical_operand(X86Register::Bx, OperandRole::Def)]
        );
        assert_eq!(
            instructions[3].operands,
            vec![physical_operand(X86Register::Es, OperandRole::Def)]
        );
        assert_eq!(
            instructions[4].operands,
            vec![
                physical_operand(X86Register::Dx, OperandRole::Def),
                physical_operand(X86Register::Bx, OperandRole::Use),
                physical_operand(X86Register::Es, OperandRole::Use),
            ]
        );
        assert_eq!(instructions[4].flags, access_flags(false));
        assert_eq!(
            instructions[5].operands,
            vec![physical_operand(X86Register::Es, OperandRole::Def)]
        );
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| matches!(
                    X86Opcode::from_machine_opcode(instruction.opcode),
                    Some(X86Opcode::LowWord | X86Opcode::HighWord | X86Opcode::Mov)
                ))
                .count(),
            0
        );
    }

    #[test]
    fn expands_a_segmented_store_without_changing_access_flags() {
        let input = segmented_access(true);
        let expanded = expand_allocated_segmented_memory(&input).unwrap();
        let instructions = &expanded.blocks[0].instructions;

        assert_eq!(
            opcodes(&expanded),
            vec![
                Some(X86Opcode::Push),
                Some(X86Opcode::Push),
                Some(X86Opcode::Pop),
                Some(X86Opcode::Pop),
                Some(X86Opcode::Store),
                Some(X86Opcode::Pop),
            ]
        );
        assert_eq!(
            instructions[4].operands,
            vec![
                physical_operand(X86Register::Bx, OperandRole::Use),
                physical_operand(X86Register::Dx, OperandRole::Use),
                physical_operand(X86Register::Es, OperandRole::Use),
            ]
        );
        assert_eq!(instructions[4].flags, access_flags(true));
        let pushes = instructions
            .iter()
            .filter(|instruction| {
                X86Opcode::from_machine_opcode(instruction.opcode) == Some(X86Opcode::Push)
            })
            .count();
        let pops = instructions
            .iter()
            .filter(|instruction| {
                X86Opcode::from_machine_opcode(instruction.opcode) == Some(X86Opcode::Pop)
            })
            .count();
        assert_eq!((pushes, pops), (2, 3));
        assert_eq!(
            instructions[0].operands,
            vec![physical_operand(X86Register::Es, OperandRole::Use)]
        );
        assert_eq!(
            instructions[3].operands,
            vec![physical_operand(X86Register::Es, OperandRole::Def)]
        );
        assert_eq!(
            instructions[5].operands,
            vec![physical_operand(X86Register::Es, OperandRole::Def)]
        );
    }

    #[test]
    fn rejects_an_incomplete_candidate_without_changing_the_input() {
        let mut input = segmented_access(false);
        input.blocks[0].instructions.truncate(2);
        let original = input.clone();

        let error = expand_allocated_segmented_memory(&input).unwrap_err();

        assert_eq!(input, original);
        assert_eq!(
            error,
            SegmentedMemoryExpansionError::MalformedPattern {
                block: MachineBlockId::new(0),
                instruction: MachineInstructionId::new(10),
                reason: "incomplete six-instruction sequence",
            }
        );
    }

    #[test]
    fn leaves_unrelated_word_pseudos_for_their_abi_finalizer() {
        let input = function(vec![
            instruction(
                MachineInstructionId::new(20),
                X86Opcode::LowWord,
                vec![
                    physical_operand(X86Register::Ax, OperandRole::Def),
                    physical_operand(X86Register::Eax, OperandRole::Use),
                ],
            ),
            instruction(
                MachineInstructionId::new(21),
                X86Opcode::HighWord,
                vec![
                    physical_operand(X86Register::Dx, OperandRole::Def),
                    physical_operand(X86Register::Eax, OperandRole::Use),
                ],
            ),
        ]);

        assert_eq!(expand_allocated_segmented_memory(&input).unwrap(), input);
    }
}
