//! Final lowering of allocated Microsoft BASIC ABI pseudos.
//!
//! Selection keeps a LONG as one dword virtual value and exposes the measured
//! `DX:AX` calling convention with word pseudos.  Those pseudos must remain
//! visible until allocation: their fixed uses, definitions, and artificial
//! far-call clobbers are allocation facts, not encoded operands.  This module
//! consumes the ABI-specific word extraction and call/return facts afterwards.
//! General word composition belongs to `word_merge`.
//!
//! The expansion is deliberately mechanical and flag-preserving.  In
//! particular, extracting a high word uses a balanced push/pop sequence
//! instead of a shift: a shift would change flags which the pseudo promises
//! not to write.

use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    InstructionFlags, MachineBlockId, MachineFunction, MachineInstruction, MachineInstructionId,
    MachineOperand, MachineOperandKind, MachineRegister, OperandRole, PhysicalRegister,
    VirtualRegisterId,
};

use super::{X86Opcode, X86Register, X86RegisterClass};

/// A refusal while lowering the allocated BASIC ABI boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BasicAbiExpansionError {
    DeclaredVirtualRegister {
        register: VirtualRegisterId,
    },
    VirtualRegister {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        register: VirtualRegisterId,
    },
    ResidualConstraint {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
    },
    ResidualTie {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
    },
    UnknownPhysicalRegister {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        register: PhysicalRegister,
    },
    InstructionIdExhausted,
    MalformedPseudo {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        opcode: X86Opcode,
        reason: &'static str,
    },
    WrongRegisterWidth {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        expected: &'static str,
        actual: X86Register,
    },
    StackPointerPseudo {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        register: X86Register,
    },
    FarCallUsesAfterDefinitions {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
    },
    FarCallAliasingDefinitions {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
    },
    BadReturnCleanup {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
    },
    LongReturnOrder {
        block: MachineBlockId,
        instruction: MachineInstructionId,
    },
    ReturnValueContract {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
    },
}

impl fmt::Display for BasicAbiExpansionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeclaredVirtualRegister { register } => {
                write!(
                    formatter,
                    "BASIC ABI expansion retains virtual register declaration {register}"
                )
            }
            Self::VirtualRegister {
                block,
                instruction,
                operand,
                register,
            } => write!(
                formatter,
                "block {block} instruction {instruction} operand {operand} retains virtual register {register}"
            ),
            Self::ResidualConstraint {
                block,
                instruction,
                operand,
            } => write!(
                formatter,
                "block {block} instruction {instruction} operand {operand} retains an allocation constraint"
            ),
            Self::ResidualTie {
                block,
                instruction,
                operand,
            } => write!(
                formatter,
                "block {block} instruction {instruction} operand {operand} retains an allocation tie"
            ),
            Self::UnknownPhysicalRegister {
                block,
                instruction,
                operand,
                register,
            } => write!(
                formatter,
                "block {block} instruction {instruction} operand {operand} names unknown x86 register {}",
                register.get()
            ),
            Self::InstructionIdExhausted => {
                write!(formatter, "BASIC ABI expansion exhausted instruction IDs")
            }
            Self::MalformedPseudo {
                block,
                instruction,
                opcode,
                reason,
            } => write!(
                formatter,
                "block {block} instruction {instruction} has malformed {opcode:?}: {reason}"
            ),
            Self::WrongRegisterWidth {
                block,
                instruction,
                operand,
                expected,
                actual,
            } => write!(
                formatter,
                "block {block} instruction {instruction} operand {operand} must be a {expected} register, found {actual:?}"
            ),
            Self::StackPointerPseudo {
                block,
                instruction,
                operand,
                register,
            } => write!(
                formatter,
                "block {block} instruction {instruction} operand {operand} cannot use stack pointer {register:?} in a BASIC ABI pseudo"
            ),
            Self::FarCallUsesAfterDefinitions {
                block,
                instruction,
                operand,
            } => write!(
                formatter,
                "block {block} instruction {instruction} far-call use operand {operand} follows a definition"
            ),
            Self::FarCallAliasingDefinitions {
                block,
                instruction,
                operand,
            } => write!(
                formatter,
                "block {block} instruction {instruction} far-call definition operand {operand} aliases an earlier definition"
            ),
            Self::BadReturnCleanup {
                block,
                instruction,
                operand,
            } => write!(
                formatter,
                "block {block} instruction {instruction} return cleanup operand {operand} is not a u16 immediate"
            ),
            Self::LongReturnOrder { block, instruction } => write!(
                formatter,
                "block {block} instruction {instruction} LONG return must use AX low then DX high"
            ),
            Self::ReturnValueContract {
                block,
                instruction,
                operand,
            } => write!(
                formatter,
                "block {block} instruction {instruction} return operand {operand} must be an AX word use"
            ),
        }
    }
}

impl Error for BasicAbiExpansionError {}

/// Expands allocated Microsoft BASIC ABI pseudos into physical x86 operations.
///
/// The input is never changed.  Replacement leaders retain their original
/// instruction ID; each extra instruction gets the next unused ID after the
/// maximum input ID, in stored block/instruction order.  Planning reserves all
/// those IDs before cloning, so exhaustion is explicit and leaves the input
/// untouched.
pub fn expand_allocated_basic_abi(
    function: &MachineFunction,
) -> Result<MachineFunction, BasicAbiExpansionError> {
    let extra = preflight(function)?;
    let mut fresh_ids = reserve_extra_ids(function, extra)?.into_iter();
    let mut expanded = function.clone();

    for block in &mut expanded.blocks {
        let mut instructions = Vec::with_capacity(block.instructions.len() + extra);
        for original in std::mem::take(&mut block.instructions) {
            let opcode = X86Opcode::from_machine_opcode(original.opcode);
            match opcode {
                Some(X86Opcode::LowWord) => {
                    let [destination, source] = original.operands.as_slice() else {
                        unreachable!("preflight validated lowword arity");
                    };
                    let destination = register(destination, block.id, &original, 0)?;
                    let source = register(source, block.id, &original, 1)?;
                    let low = low_word(source).expect("preflight validated dword source");
                    if destination != low {
                        instructions.push(instruction(
                            original.id,
                            X86Opcode::Mov,
                            vec![
                                physical_operand(destination, OperandRole::Def),
                                physical_operand(low, OperandRole::Use),
                            ],
                            InstructionFlags::NONE,
                        ));
                    }
                }
                Some(X86Opcode::HighWord) => {
                    let [destination, source] = original.operands.as_slice() else {
                        unreachable!("preflight validated highword arity");
                    };
                    let destination = register(destination, block.id, &original, 0)?;
                    let source = register(source, block.id, &original, 1)?;
                    instructions.push(instruction(
                        original.id,
                        X86Opcode::Push,
                        vec![physical_operand(source, OperandRole::Use)],
                        InstructionFlags::NONE,
                    ));
                    // The first word pop deliberately discards the low half.
                    // Reusing the destination needs no scratch register and
                    // leaves every arithmetic flag intact.
                    instructions.push(instruction(
                        next_id(&mut fresh_ids),
                        X86Opcode::Pop,
                        vec![physical_operand(destination, OperandRole::Def)],
                        InstructionFlags::NONE,
                    ));
                    instructions.push(instruction(
                        next_id(&mut fresh_ids),
                        X86Opcode::Pop,
                        vec![physical_operand(destination, OperandRole::Def)],
                        InstructionFlags::NONE,
                    ));
                }
                Some(X86Opcode::CallFar) => {
                    let target = original.operands[0].clone();
                    instructions.push(instruction(
                        original.id,
                        X86Opcode::CallFar,
                        vec![target],
                        original.flags,
                    ));
                }
                Some(X86Opcode::ReturnFar) => {
                    let cleanup = return_cleanup(&original, block.id)?;
                    let operands = cleanup.map_or_else(Vec::new, |value| vec![immediate(value)]);
                    instructions.push(instruction(
                        original.id,
                        X86Opcode::ReturnFar,
                        operands,
                        original.flags,
                    ));
                }
                _ => instructions.push(original),
            }
        }
        block.instructions = instructions;
    }

    Ok(expanded)
}

fn preflight(function: &MachineFunction) -> Result<usize, BasicAbiExpansionError> {
    let anchor_registers = function.anchor_virtual_registers(X86Opcode::Nothing.machine_opcode());
    if let Some(register) = function
        .virtual_registers
        .iter()
        .find(|register| !anchor_registers.contains(&register.id))
    {
        return Err(BasicAbiExpansionError::DeclaredVirtualRegister {
            register: register.id,
        });
    }
    let mut extra = 0_usize;
    for block in &function.blocks {
        for instruction in &block.instructions {
            validate_allocated_operands(block.id, instruction)?;
            match X86Opcode::from_machine_opcode(instruction.opcode) {
                Some(X86Opcode::LowWord) => validate_low(block.id, instruction)?,
                Some(X86Opcode::HighWord) => {
                    validate_high(block.id, instruction)?;
                    extra = extra
                        .checked_add(2)
                        .ok_or(BasicAbiExpansionError::InstructionIdExhausted)?;
                }
                Some(X86Opcode::CallFar) => validate_call(block.id, instruction)?,
                Some(X86Opcode::ReturnFar) => {
                    return_cleanup(instruction, block.id)?;
                }
                _ => {}
            }
        }
    }
    Ok(extra)
}

fn validate_allocated_operands(
    block: MachineBlockId,
    instruction: &MachineInstruction,
) -> Result<(), BasicAbiExpansionError> {
    for (position, operand) in instruction.operands.iter().enumerate() {
        if operand.constraint.is_some() {
            return Err(BasicAbiExpansionError::ResidualConstraint {
                block,
                instruction: instruction.id,
                operand: position,
            });
        }
        if operand.tied_to.is_some() {
            return Err(BasicAbiExpansionError::ResidualTie {
                block,
                instruction: instruction.id,
                operand: position,
            });
        }
        match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register))
                if !instruction.is_logical_anchor(X86Opcode::Nothing.machine_opcode()) =>
            {
                return Err(BasicAbiExpansionError::VirtualRegister {
                    block,
                    instruction: instruction.id,
                    operand: position,
                    register,
                });
            }
            MachineOperandKind::Register(MachineRegister::Physical(register))
                if X86Register::from_physical(register).is_none() =>
            {
                return Err(BasicAbiExpansionError::UnknownPhysicalRegister {
                    block,
                    instruction: instruction.id,
                    operand: position,
                    register,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_low(
    block: MachineBlockId,
    instruction: &MachineInstruction,
) -> Result<(), BasicAbiExpansionError> {
    let [destination, source] = instruction.operands.as_slice() else {
        return malformed(block, instruction, "expected [word def, dword use]");
    };
    require_flags_none(block, instruction)?;
    require_register(
        block,
        instruction,
        0,
        destination,
        OperandRole::Def,
        X86RegisterClass::Word,
    )?;
    require_register(
        block,
        instruction,
        1,
        source,
        OperandRole::Use,
        X86RegisterClass::Dword,
    )
}

fn validate_high(
    block: MachineBlockId,
    instruction: &MachineInstruction,
) -> Result<(), BasicAbiExpansionError> {
    validate_low(block, instruction)
}

fn validate_call(
    block: MachineBlockId,
    instruction: &MachineInstruction,
) -> Result<(), BasicAbiExpansionError> {
    let Some((callee, metadata)) = instruction.operands.split_first() else {
        return malformed(
            block,
            instruction,
            "expected function or external-symbol callee",
        );
    };
    if !matches!(
        callee.kind,
        MachineOperandKind::Function(_) | MachineOperandKind::ExternalSymbol { .. }
    ) || callee.role != OperandRole::None
    {
        return malformed(
            block,
            instruction,
            "callee must be a role-none function or external symbol",
        );
    }
    if !is_call_flags(instruction.flags) {
        return malformed(
            block,
            instruction,
            "must have call flags without terminator or copy",
        );
    }

    let mut saw_definition = false;
    let mut definitions = Vec::new();
    for (index, operand) in metadata.iter().enumerate() {
        let position = index + 1;
        let register = register(operand, block, instruction, position)?;
        reject_stack_pointer(block, instruction.id, position, register)?;
        if !X86RegisterClass::Word.members().contains(&register) {
            return Err(BasicAbiExpansionError::WrongRegisterWidth {
                block,
                instruction: instruction.id,
                operand: position,
                expected: "word ABI",
                actual: register,
            });
        }
        match operand.role {
            OperandRole::Use => {
                if saw_definition {
                    return Err(BasicAbiExpansionError::FarCallUsesAfterDefinitions {
                        block,
                        instruction: instruction.id,
                        operand: position,
                    });
                }
            }
            OperandRole::Def => {
                saw_definition = true;
                if definitions
                    .iter()
                    .any(|previous: &X86Register| previous.overlaps(register))
                {
                    return Err(BasicAbiExpansionError::FarCallAliasingDefinitions {
                        block,
                        instruction: instruction.id,
                        operand: position,
                    });
                }
                definitions.push(register);
            }
            OperandRole::None | OperandRole::UseDef => {
                return malformed(
                    block,
                    instruction,
                    "ABI metadata must be a register use or definition",
                );
            }
        }
    }
    Ok(())
}

fn return_cleanup(
    instruction: &MachineInstruction,
    block: MachineBlockId,
) -> Result<Option<i64>, BasicAbiExpansionError> {
    if instruction.flags
        != (InstructionFlags {
            terminator: true,
            ..InstructionFlags::NONE
        })
    {
        return malformed(block, instruction, "must have only the terminator flag");
    }

    let operands = instruction.operands.as_slice();
    let (values, cleanup) = match operands.last() {
        Some(MachineOperand {
            kind: MachineOperandKind::Immediate(value),
            role: OperandRole::None,
            ..
        }) => {
            if !(0..=i64::from(u16::MAX)).contains(value) {
                return Err(BasicAbiExpansionError::BadReturnCleanup {
                    block,
                    instruction: instruction.id,
                    operand: operands.len() - 1,
                });
            }
            (&operands[..operands.len() - 1], Some(*value))
        }
        Some(MachineOperand {
            kind: MachineOperandKind::Immediate(_),
            ..
        }) => {
            return Err(BasicAbiExpansionError::BadReturnCleanup {
                block,
                instruction: instruction.id,
                operand: operands.len() - 1,
            });
        }
        _ => (operands, None),
    };

    match values {
        [] => Ok(cleanup),
        [word] => {
            let word_register = register(word, block, instruction, 0)?;
            if !X86RegisterClass::Word.members().contains(&word_register) {
                return Err(BasicAbiExpansionError::WrongRegisterWidth {
                    block,
                    instruction: instruction.id,
                    operand: 0,
                    expected: "word",
                    actual: word_register,
                });
            }
            if word.role != OperandRole::Use || word_register != X86Register::Ax {
                return Err(BasicAbiExpansionError::ReturnValueContract {
                    block,
                    instruction: instruction.id,
                    operand: 0,
                });
            }
            Ok(cleanup)
        }
        [low, high] => {
            let low_register = register(low, block, instruction, 0)?;
            let high_register = register(high, block, instruction, 1)?;
            if low.role != OperandRole::Use
                || high.role != OperandRole::Use
                || low_register != X86Register::Ax
                || high_register != X86Register::Dx
            {
                return Err(BasicAbiExpansionError::LongReturnOrder {
                    block,
                    instruction: instruction.id,
                });
            }
            Ok(cleanup)
        }
        _ => malformed(
            block,
            instruction,
            "return values must be absent or LONG AX/DX uses",
        ),
    }
}

fn require_flags_none(
    block: MachineBlockId,
    instruction: &MachineInstruction,
) -> Result<(), BasicAbiExpansionError> {
    if instruction.flags == InstructionFlags::NONE {
        Ok(())
    } else {
        malformed(block, instruction, "must have no flags")
    }
}

fn require_register(
    block: MachineBlockId,
    instruction: &MachineInstruction,
    position: usize,
    operand: &MachineOperand,
    role: OperandRole,
    class: X86RegisterClass,
) -> Result<(), BasicAbiExpansionError> {
    if operand.role != role {
        return malformed(
            block,
            instruction,
            "operand role does not match pseudo contract",
        );
    }
    let register = register(operand, block, instruction, position)?;
    reject_stack_pointer(block, instruction.id, position, register)?;
    if class.members().contains(&register) {
        Ok(())
    } else {
        Err(BasicAbiExpansionError::WrongRegisterWidth {
            block,
            instruction: instruction.id,
            operand: position,
            expected: match class {
                X86RegisterClass::Word => "word",
                X86RegisterClass::Dword => "dword",
                _ => unreachable!("only word and dword ABI classes are requested"),
            },
            actual: register,
        })
    }
}

fn register(
    operand: &MachineOperand,
    block: MachineBlockId,
    instruction: &MachineInstruction,
    position: usize,
) -> Result<X86Register, BasicAbiExpansionError> {
    let MachineOperandKind::Register(MachineRegister::Physical(physical)) = operand.kind else {
        return Err(BasicAbiExpansionError::MalformedPseudo {
            block,
            instruction: instruction.id,
            opcode: X86Opcode::from_machine_opcode(instruction.opcode).unwrap_or(X86Opcode::Mov),
            reason: "ABI operand must be a physical register",
        });
    };
    X86Register::from_physical(physical).ok_or(BasicAbiExpansionError::UnknownPhysicalRegister {
        block,
        instruction: instruction.id,
        operand: position,
        register: physical,
    })
}

fn reject_stack_pointer(
    block: MachineBlockId,
    instruction: MachineInstructionId,
    operand: usize,
    register: X86Register,
) -> Result<(), BasicAbiExpansionError> {
    if matches!(register, X86Register::Sp | X86Register::Esp) {
        Err(BasicAbiExpansionError::StackPointerPseudo {
            block,
            instruction,
            operand,
            register,
        })
    } else {
        Ok(())
    }
}

fn low_word(register: X86Register) -> Option<X86Register> {
    let [word] = register.sub_registers() else {
        return None;
    };
    X86RegisterClass::Word
        .members()
        .contains(word)
        .then_some(*word)
}

fn malformed<T>(
    block: MachineBlockId,
    instruction: &MachineInstruction,
    reason: &'static str,
) -> Result<T, BasicAbiExpansionError> {
    Err(BasicAbiExpansionError::MalformedPseudo {
        block,
        instruction: instruction.id,
        opcode: X86Opcode::from_machine_opcode(instruction.opcode).unwrap_or(X86Opcode::Mov),
        reason,
    })
}

fn is_call_flags(flags: InstructionFlags) -> bool {
    flags.call
        && !flags.terminator
        && !flags.copy
        && (!flags.volatile || flags.may_load || flags.may_store)
}

fn reserve_extra_ids(
    function: &MachineFunction,
    count: usize,
) -> Result<Vec<MachineInstructionId>, BasicAbiExpansionError> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let count = u32::try_from(count).map_err(|_| BasicAbiExpansionError::InstructionIdExhausted)?;
    let first = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .map(|instruction| instruction.id.get())
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1)
                .ok_or(BasicAbiExpansionError::InstructionIdExhausted)
        })?;
    let last = first
        .checked_add(count - 1)
        .ok_or(BasicAbiExpansionError::InstructionIdExhausted)?;
    Ok((first..=last).map(MachineInstructionId::new).collect())
}

fn next_id(ids: &mut impl Iterator<Item = MachineInstructionId>) -> MachineInstructionId {
    ids.next()
        .expect("preflight reserved one fresh instruction ID for every inserted instruction")
}

fn instruction(
    id: MachineInstructionId,
    opcode: X86Opcode,
    operands: Vec<MachineOperand>,
    flags: InstructionFlags,
) -> MachineInstruction {
    MachineInstruction {
        id,
        opcode: opcode.machine_opcode(),
        operands,
        flags,
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

fn immediate(value: i64) -> MachineOperand {
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
        MachineBlock, MachineCallingConvention, MachineFunctionId, MachineLinkage, MachineSignature,
    };
    use crate::target::x86::{encode, expand_allocated_word_merges, lower_instruction};

    fn function(instructions: Vec<MachineInstruction>) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(1),
            name: "long_abi".into(),
            linkage: MachineLinkage::External,
            signature: MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::FarPascal,
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

    fn pseudo(id: u32, opcode: X86Opcode, operands: Vec<MachineOperand>) -> MachineInstruction {
        let flags = match opcode {
            X86Opcode::CallFar => InstructionFlags {
                call: true,
                may_load: true,
                may_store: true,
                side_effects: true,
                ..InstructionFlags::NONE
            },
            X86Opcode::ReturnFar => InstructionFlags {
                terminator: true,
                ..InstructionFlags::NONE
            },
            _ => InstructionFlags::NONE,
        };
        instruction(MachineInstructionId::new(id), opcode, operands, flags)
    }

    fn register_operand(register: X86Register, role: OperandRole) -> MachineOperand {
        physical_operand(register, role)
    }

    fn callee() -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::ExternalSymbol {
                name: "B$TEST".into(),
                addend: 0,
            },
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        }
    }

    fn opcodes(function: &MachineFunction) -> Vec<X86Opcode> {
        function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| X86Opcode::from_machine_opcode(instruction.opcode).unwrap())
            .collect()
    }

    fn encoded_bytes(function: &MachineFunction) -> Vec<u8> {
        function.blocks[0]
            .instructions
            .iter()
            .flat_map(|instruction| {
                let lowered = lower_instruction(instruction).expect("expanded instruction lowers");
                encode(&lowered).expect("expanded instruction encodes")
            })
            .collect()
    }

    #[test]
    fn low_word_is_a_direct_move_or_an_exact_view_deletion() {
        let moved = function(vec![pseudo(
            2,
            X86Opcode::LowWord,
            vec![
                register_operand(X86Register::Cx, OperandRole::Def),
                register_operand(X86Register::Eax, OperandRole::Use),
            ],
        )]);
        let expanded = expand_allocated_basic_abi(&moved).unwrap();
        assert_eq!(opcodes(&expanded), vec![X86Opcode::Mov]);
        assert_eq!(
            expanded.blocks[0].instructions[0].operands[1],
            register_operand(X86Register::Ax, OperandRole::Use)
        );

        let same_view = function(vec![pseudo(
            2,
            X86Opcode::LowWord,
            vec![
                register_operand(X86Register::Ax, OperandRole::Def),
                register_operand(X86Register::Eax, OperandRole::Use),
            ],
        )]);
        assert!(
            expand_allocated_basic_abi(&same_view).unwrap().blocks[0]
                .instructions
                .is_empty()
        );
    }

    #[test]
    fn high_word_uses_balanced_stack_operations_without_flags_or_scratch() {
        let input = function(vec![pseudo(
            4,
            X86Opcode::HighWord,
            vec![
                register_operand(X86Register::Dx, OperandRole::Def),
                register_operand(X86Register::Eax, OperandRole::Use),
            ],
        )]);
        let expanded = expand_allocated_basic_abi(&input).unwrap();
        assert_eq!(
            opcodes(&expanded),
            vec![X86Opcode::Push, X86Opcode::Pop, X86Opcode::Pop]
        );
        assert!(
            expanded.blocks[0]
                .instructions
                .iter()
                .all(|one| one.flags == InstructionFlags::NONE)
        );
        assert_eq!(
            expanded.blocks[0].instructions[1].operands,
            vec![register_operand(X86Register::Dx, OperandRole::Def)]
        );
        assert_eq!(
            expanded.blocks[0].instructions[2].operands,
            vec![register_operand(X86Register::Dx, OperandRole::Def)]
        );
        assert_eq!(encoded_bytes(&expanded), vec![0x66, 0x50, 0x5a, 0x5a]);

        let alias = function(vec![pseudo(
            0,
            X86Opcode::HighWord,
            vec![
                register_operand(X86Register::Ax, OperandRole::Def),
                register_operand(X86Register::Eax, OperandRole::Use),
            ],
        )]);
        assert_eq!(
            opcodes(&expand_allocated_basic_abi(&alias).unwrap()),
            vec![X86Opcode::Push, X86Opcode::Pop, X86Opcode::Pop]
        );
    }

    #[test]
    fn strips_word_return_metadata_but_keeps_callee_cleanup() {
        let input = function(vec![pseudo(
            4,
            X86Opcode::ReturnFar,
            vec![
                register_operand(X86Register::Ax, OperandRole::Use),
                immediate(2),
            ],
        )]);
        let expanded = expand_allocated_basic_abi(&input).unwrap();
        assert_eq!(
            expanded.blocks[0].instructions[0].operands,
            vec![immediate(2)]
        );

        let without_cleanup = function(vec![pseudo(
            5,
            X86Opcode::ReturnFar,
            vec![register_operand(X86Register::Ax, OperandRole::Use)],
        )]);
        assert!(
            expand_allocated_basic_abi(&without_cleanup).unwrap().blocks[0].instructions[0]
                .operands
                .is_empty()
        );
    }

    #[test]
    fn strips_long_return_metadata_but_keeps_callee_cleanup() {
        let input = function(vec![pseudo(
            4,
            X86Opcode::ReturnFar,
            vec![
                register_operand(X86Register::Ax, OperandRole::Use),
                register_operand(X86Register::Dx, OperandRole::Use),
                immediate(4),
            ],
        )]);
        let expanded = expand_allocated_basic_abi(&input).unwrap();
        assert_eq!(
            expanded.blocks[0].instructions[0].operands,
            vec![immediate(4)]
        );
    }

    #[test]
    fn strips_far_call_allocator_metadata_only_after_validating_it() {
        let input = function(vec![pseudo(
            4,
            X86Opcode::CallFar,
            vec![
                callee(),
                register_operand(X86Register::Ax, OperandRole::Use),
                register_operand(X86Register::Dx, OperandRole::Def),
                register_operand(X86Register::Cx, OperandRole::Def),
            ],
        )]);
        let expanded = expand_allocated_basic_abi(&input).unwrap();
        assert_eq!(expanded.blocks[0].instructions[0].operands, vec![callee()]);
    }

    #[test]
    fn applies_the_generic_word_merge_phase_before_basic_word_extraction() {
        let input = function(vec![
            pseudo(
                2,
                X86Opcode::MergeWords,
                vec![
                    register_operand(X86Register::Ecx, OperandRole::Def),
                    register_operand(X86Register::Ax, OperandRole::Use),
                    register_operand(X86Register::Dx, OperandRole::Use),
                ],
            ),
            pseudo(
                9,
                X86Opcode::HighWord,
                vec![
                    register_operand(X86Register::Dx, OperandRole::Def),
                    register_operand(X86Register::Ecx, OperandRole::Use),
                ],
            ),
        ]);
        let baseline = input.clone();
        let first =
            expand_allocated_basic_abi(&expand_allocated_word_merges(&input).unwrap()).unwrap();
        let second =
            expand_allocated_basic_abi(&expand_allocated_word_merges(&input).unwrap()).unwrap();
        assert_eq!(input, baseline);
        assert_eq!(first, second);
        assert_eq!(
            first.blocks[0]
                .instructions
                .iter()
                .map(|one| one.id.get())
                .collect::<Vec<_>>(),
            vec![2, 10, 11, 9, 12, 13]
        );
    }

    #[test]
    fn preserves_unrelated_allocated_instructions_exactly() {
        let unchanged = instruction(
            MachineInstructionId::new(12),
            X86Opcode::Mov,
            vec![
                register_operand(X86Register::Ax, OperandRole::Def),
                register_operand(X86Register::Bx, OperandRole::Use),
            ],
            InstructionFlags::NONE,
        );
        let input = function(vec![unchanged.clone()]);
        assert_eq!(
            expand_allocated_basic_abi(&input).unwrap().blocks[0].instructions,
            vec![unchanged]
        );
    }

    #[test]
    fn refuses_bad_abi_boundary_facts() {
        let bad_return = function(vec![pseudo(
            0,
            X86Opcode::ReturnFar,
            vec![
                register_operand(X86Register::Dx, OperandRole::Use),
                register_operand(X86Register::Ax, OperandRole::Use),
            ],
        )]);
        assert!(matches!(
            expand_allocated_basic_abi(&bad_return),
            Err(BasicAbiExpansionError::LongReturnOrder { .. })
        ));

        let wrong_word_return_register = function(vec![pseudo(
            0,
            X86Opcode::ReturnFar,
            vec![register_operand(X86Register::Bx, OperandRole::Use)],
        )]);
        assert!(matches!(
            expand_allocated_basic_abi(&wrong_word_return_register),
            Err(BasicAbiExpansionError::ReturnValueContract { .. })
        ));

        let wrong_word_return_role = function(vec![pseudo(
            0,
            X86Opcode::ReturnFar,
            vec![register_operand(X86Register::Ax, OperandRole::Def)],
        )]);
        assert!(matches!(
            expand_allocated_basic_abi(&wrong_word_return_role),
            Err(BasicAbiExpansionError::ReturnValueContract { .. })
        ));

        let wrong_word_return_width = function(vec![pseudo(
            0,
            X86Opcode::ReturnFar,
            vec![register_operand(X86Register::Eax, OperandRole::Use)],
        )]);
        assert!(matches!(
            expand_allocated_basic_abi(&wrong_word_return_width),
            Err(BasicAbiExpansionError::WrongRegisterWidth { .. })
        ));

        let stack_pointer = function(vec![pseudo(
            0,
            X86Opcode::LowWord,
            vec![
                register_operand(X86Register::Sp, OperandRole::Def),
                register_operand(X86Register::Eax, OperandRole::Use),
            ],
        )]);
        assert!(matches!(
            expand_allocated_basic_abi(&stack_pointer),
            Err(BasicAbiExpansionError::StackPointerPseudo { .. })
        ));

        let malformed_call = function(vec![pseudo(
            0,
            X86Opcode::CallFar,
            vec![
                callee(),
                register_operand(X86Register::Ax, OperandRole::Def),
                register_operand(X86Register::Dx, OperandRole::Use),
            ],
        )]);
        assert!(matches!(
            expand_allocated_basic_abi(&malformed_call),
            Err(BasicAbiExpansionError::FarCallUsesAfterDefinitions { .. })
        ));

        let virtual_register = MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(0))),
            role: OperandRole::Def,
            constraint: None,
            tied_to: None,
        };
        let virtual_input = function(vec![pseudo(
            0,
            X86Opcode::LowWord,
            vec![
                virtual_register,
                register_operand(X86Register::Eax, OperandRole::Use),
            ],
        )]);
        assert!(matches!(
            expand_allocated_basic_abi(&virtual_input),
            Err(BasicAbiExpansionError::VirtualRegister { .. })
        ));

        let mut constrained = register_operand(X86Register::Ax, OperandRole::Def);
        constrained.constraint = Some(crate::codegen::machine::RegisterConstraint::Fixed(
            X86Register::Ax.physical(),
        ));
        let constrained_input = function(vec![pseudo(
            0,
            X86Opcode::LowWord,
            vec![
                constrained,
                register_operand(X86Register::Eax, OperandRole::Use),
            ],
        )]);
        assert!(matches!(
            expand_allocated_basic_abi(&constrained_input),
            Err(BasicAbiExpansionError::ResidualConstraint { .. })
        ));

        let mut tied = register_operand(X86Register::Ax, OperandRole::Def);
        tied.tied_to = Some(crate::codegen::machine::OperandIndex::new(1));
        let tied_input = function(vec![pseudo(
            0,
            X86Opcode::LowWord,
            vec![tied, register_operand(X86Register::Eax, OperandRole::Use)],
        )]);
        assert!(matches!(
            expand_allocated_basic_abi(&tied_input),
            Err(BasicAbiExpansionError::ResidualTie { .. })
        ));

        let wrong_width = function(vec![pseudo(
            0,
            X86Opcode::LowWord,
            vec![
                register_operand(X86Register::Eax, OperandRole::Def),
                register_operand(X86Register::Ecx, OperandRole::Use),
            ],
        )]);
        assert!(matches!(
            expand_allocated_basic_abi(&wrong_width),
            Err(BasicAbiExpansionError::WrongRegisterWidth { .. })
        ));

        let bad_cleanup = function(vec![pseudo(0, X86Opcode::ReturnFar, vec![immediate(-1)])]);
        assert!(matches!(
            expand_allocated_basic_abi(&bad_cleanup),
            Err(BasicAbiExpansionError::BadReturnCleanup { .. })
        ));

        let aliasing_definitions = function(vec![pseudo(
            0,
            X86Opcode::CallFar,
            vec![
                callee(),
                register_operand(X86Register::Ax, OperandRole::Def),
                register_operand(X86Register::Ax, OperandRole::Def),
            ],
        )]);
        assert!(matches!(
            expand_allocated_basic_abi(&aliasing_definitions),
            Err(BasicAbiExpansionError::FarCallAliasingDefinitions { .. })
        ));
    }
}
