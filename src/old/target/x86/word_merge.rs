//! Final expansion of allocated x86 word composition.
//!
//! `MergeWords` joins two word values into one dword value.  It is a target
//! operation, not an ABI fact: both the BASIC and C selectors can produce it.
//! It must remain visible through allocation, since the allocator owns the
//! overlapping word and dword register views.  Once every operand is physical,
//! the balanced push-high, push-low, pop-dword sequence performs the join
//! without consuming a scratch register or changing arithmetic flags.

use std::error::Error;
use std::fmt;

use crate::old::codegen::machine::{
    InstructionFlags, MachineBlockId, MachineFunction, MachineInstruction, MachineInstructionId,
    MachineOperand, MachineOperandKind, MachineRegister, OperandRole, PhysicalRegister,
    VirtualRegisterId,
};

use super::{X86Opcode, X86Register, X86RegisterClass};

/// A refusal while expanding allocated x86 word composition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WordMergeExpansionError {
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
    MalformedMerge {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        reason: &'static str,
    },
    WrongRegisterWidth {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        expected: &'static str,
        actual: X86Register,
    },
    StackPointerOperand {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        register: X86Register,
    },
}

impl fmt::Display for WordMergeExpansionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeclaredVirtualRegister { register } => write!(
                formatter,
                "word-merge expansion retains virtual register declaration {register}"
            ),
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
                write!(formatter, "word-merge expansion exhausted instruction IDs")
            }
            Self::MalformedMerge {
                block,
                instruction,
                reason,
            } => write!(
                formatter,
                "block {block} instruction {instruction} has malformed MergeWords: {reason}"
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
            Self::StackPointerOperand {
                block,
                instruction,
                operand,
                register,
            } => write!(
                formatter,
                "block {block} instruction {instruction} operand {operand} cannot use stack pointer {register:?} in MergeWords"
            ),
        }
    }
}

impl Error for WordMergeExpansionError {}

/// Expands allocated `MergeWords` pseudos into physical x86 stack operations.
///
/// The input is never changed.  This boundary accepts fully allocated Machine
/// IR only: no virtual-register declarations or uses, allocation constraints,
/// or operand ties may remain.  Replacement leaders retain their original ID;
/// additional instructions receive fresh IDs after the greatest input ID in
/// stored block/instruction order.  All validation and ID reservation happens
/// before cloning, so failures leave the input untouched.
pub fn expand_allocated_word_merges(
    function: &MachineFunction,
) -> Result<MachineFunction, WordMergeExpansionError> {
    let extra = preflight(function)?;
    if extra == 0 {
        return Ok(function.clone());
    }
    let mut fresh_ids = reserve_extra_ids(function, extra)?.into_iter();
    let mut expanded = function.clone();

    for block in &mut expanded.blocks {
        let mut instructions = Vec::with_capacity(block.instructions.len() + extra);
        for original in std::mem::take(&mut block.instructions) {
            if X86Opcode::from_machine_opcode(original.opcode) != Some(X86Opcode::MergeWords) {
                instructions.push(original);
                continue;
            }
            let [destination, low, high] = original.operands.as_slice() else {
                unreachable!("preflight validated MergeWords arity");
            };
            let destination = register(destination, block.id, &original, 0)?;
            let low = register(low, block.id, &original, 1)?;
            let high = register(high, block.id, &original, 2)?;
            instructions.push(instruction(
                original.id,
                X86Opcode::Push,
                vec![physical_operand(high, OperandRole::Use)],
            ));
            instructions.push(instruction(
                next_id(&mut fresh_ids),
                X86Opcode::Push,
                vec![physical_operand(low, OperandRole::Use)],
            ));
            instructions.push(instruction(
                next_id(&mut fresh_ids),
                X86Opcode::Pop,
                vec![physical_operand(destination, OperandRole::Def)],
            ));
        }
        block.instructions = instructions;
    }

    Ok(expanded)
}

fn preflight(function: &MachineFunction) -> Result<usize, WordMergeExpansionError> {
    let anchor_registers = function.anchor_virtual_registers(X86Opcode::Nothing.machine_opcode());
    if let Some(register) = function
        .virtual_registers
        .iter()
        .find(|register| !anchor_registers.contains(&register.id))
    {
        return Err(WordMergeExpansionError::DeclaredVirtualRegister {
            register: register.id,
        });
    }

    let mut extra = 0_usize;
    for block in &function.blocks {
        for instruction in &block.instructions {
            validate_allocated_operands(block.id, instruction)?;
            if X86Opcode::from_machine_opcode(instruction.opcode) == Some(X86Opcode::MergeWords) {
                validate_merge(block.id, instruction)?;
                extra = extra
                    .checked_add(2)
                    .ok_or(WordMergeExpansionError::InstructionIdExhausted)?;
            }
        }
    }
    Ok(extra)
}

fn validate_allocated_operands(
    block: MachineBlockId,
    instruction: &MachineInstruction,
) -> Result<(), WordMergeExpansionError> {
    for (position, operand) in instruction.operands.iter().enumerate() {
        if operand.constraint.is_some() {
            return Err(WordMergeExpansionError::ResidualConstraint {
                block,
                instruction: instruction.id,
                operand: position,
            });
        }
        if operand.tied_to.is_some() {
            return Err(WordMergeExpansionError::ResidualTie {
                block,
                instruction: instruction.id,
                operand: position,
            });
        }
        match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register))
                if !instruction.is_logical_anchor(X86Opcode::Nothing.machine_opcode()) =>
            {
                return Err(WordMergeExpansionError::VirtualRegister {
                    block,
                    instruction: instruction.id,
                    operand: position,
                    register,
                });
            }
            MachineOperandKind::Register(MachineRegister::Physical(register))
                if X86Register::from_physical(register).is_none() =>
            {
                return Err(WordMergeExpansionError::UnknownPhysicalRegister {
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

fn validate_merge(
    block: MachineBlockId,
    instruction: &MachineInstruction,
) -> Result<(), WordMergeExpansionError> {
    let [destination, low, high] = instruction.operands.as_slice() else {
        return malformed(
            block,
            instruction,
            "expected [dword def, word low use, word high use]",
        );
    };
    if instruction.flags != InstructionFlags::NONE {
        return malformed(block, instruction, "must have no flags");
    }
    require_register(
        block,
        instruction,
        0,
        destination,
        OperandRole::Def,
        X86RegisterClass::Dword,
    )?;
    require_register(
        block,
        instruction,
        1,
        low,
        OperandRole::Use,
        X86RegisterClass::Word,
    )?;
    require_register(
        block,
        instruction,
        2,
        high,
        OperandRole::Use,
        X86RegisterClass::Word,
    )
}

fn require_register(
    block: MachineBlockId,
    instruction: &MachineInstruction,
    position: usize,
    operand: &MachineOperand,
    role: OperandRole,
    class: X86RegisterClass,
) -> Result<(), WordMergeExpansionError> {
    if operand.role != role {
        return malformed(
            block,
            instruction,
            "operand role does not match pseudo contract",
        );
    }
    let register = register(operand, block, instruction, position)?;
    if matches!(register, X86Register::Sp | X86Register::Esp) {
        return Err(WordMergeExpansionError::StackPointerOperand {
            block,
            instruction: instruction.id,
            operand: position,
            register,
        });
    }
    if class.members().contains(&register) {
        Ok(())
    } else {
        Err(WordMergeExpansionError::WrongRegisterWidth {
            block,
            instruction: instruction.id,
            operand: position,
            expected: match class {
                X86RegisterClass::Word => "word",
                X86RegisterClass::Dword => "dword",
                _ => unreachable!("MergeWords only has word and dword operands"),
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
) -> Result<X86Register, WordMergeExpansionError> {
    let MachineOperandKind::Register(MachineRegister::Physical(physical)) = operand.kind else {
        return malformed(block, instruction, "operand must be a physical register");
    };
    X86Register::from_physical(physical).ok_or(WordMergeExpansionError::UnknownPhysicalRegister {
        block,
        instruction: instruction.id,
        operand: position,
        register: physical,
    })
}

fn malformed<T>(
    block: MachineBlockId,
    instruction: &MachineInstruction,
    reason: &'static str,
) -> Result<T, WordMergeExpansionError> {
    Err(WordMergeExpansionError::MalformedMerge {
        block,
        instruction: instruction.id,
        reason,
    })
}

fn reserve_extra_ids(
    function: &MachineFunction,
    count: usize,
) -> Result<Vec<MachineInstructionId>, WordMergeExpansionError> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let count =
        u32::try_from(count).map_err(|_| WordMergeExpansionError::InstructionIdExhausted)?;
    let first = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .map(|instruction| instruction.id.get())
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1)
                .ok_or(WordMergeExpansionError::InstructionIdExhausted)
        })?;
    let last = first
        .checked_add(count - 1)
        .ok_or(WordMergeExpansionError::InstructionIdExhausted)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::old::codegen::machine::{
        MachineBlock, MachineCallingConvention, MachineFunctionId, MachineLinkage,
        MachineSignature, RegisterConstraint,
    };
    use crate::old::target::x86::{encode, lower_instruction};

    fn function(instructions: Vec<MachineInstruction>) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(1),
            name: "word_merge".into(),
            linkage: MachineLinkage::External,
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

    fn merge(
        id: u32,
        destination: X86Register,
        low: X86Register,
        high: X86Register,
    ) -> MachineInstruction {
        instruction(
            MachineInstructionId::new(id),
            X86Opcode::MergeWords,
            vec![
                physical_operand(destination, OperandRole::Def),
                physical_operand(low, OperandRole::Use),
                physical_operand(high, OperandRole::Use),
            ],
        )
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
    fn merges_dx_ax_in_high_low_stack_order_with_fresh_ids() {
        let input = function(vec![merge(
            7,
            X86Register::Eax,
            X86Register::Ax,
            X86Register::Dx,
        )]);

        let expanded = expand_allocated_word_merges(&input).unwrap();
        assert_eq!(
            opcodes(&expanded),
            vec![X86Opcode::Push, X86Opcode::Push, X86Opcode::Pop]
        );
        let instructions = &expanded.blocks[0].instructions;
        assert_eq!(
            instructions
                .iter()
                .map(|one| one.id.get())
                .collect::<Vec<_>>(),
            vec![7, 8, 9]
        );
        assert_eq!(
            instructions[0].operands,
            vec![physical_operand(X86Register::Dx, OperandRole::Use)]
        );
        assert_eq!(
            instructions[1].operands,
            vec![physical_operand(X86Register::Ax, OperandRole::Use)]
        );
        assert_eq!(
            instructions[2].operands,
            vec![physical_operand(X86Register::Eax, OperandRole::Def)]
        );
        assert!(
            instructions
                .iter()
                .all(|instruction| instruction.flags == InstructionFlags::NONE)
        );
        assert_eq!(encoded_bytes(&expanded), vec![0x52, 0x50, 0x66, 0x58]);
    }

    #[test]
    fn merge_allows_overlapping_registers_after_both_words_are_pushed() {
        let input = function(vec![merge(
            0,
            X86Register::Edx,
            X86Register::Dx,
            X86Register::Ax,
        )]);
        let expanded = expand_allocated_word_merges(&input).unwrap();
        assert_eq!(
            opcodes(&expanded),
            vec![X86Opcode::Push, X86Opcode::Push, X86Opcode::Pop]
        );
        assert_eq!(
            expanded.blocks[0].instructions[2].operands,
            vec![physical_operand(X86Register::Edx, OperandRole::Def)]
        );
    }

    #[test]
    fn leaves_input_unchanged_and_assigns_ids_after_all_existing_instructions() {
        let unchanged = instruction(
            MachineInstructionId::new(9),
            X86Opcode::Mov,
            vec![
                physical_operand(X86Register::Ax, OperandRole::Def),
                physical_operand(X86Register::Bx, OperandRole::Use),
            ],
        );
        let input = function(vec![
            merge(2, X86Register::Ecx, X86Register::Ax, X86Register::Dx),
            unchanged.clone(),
            merge(4, X86Register::Ebx, X86Register::Bx, X86Register::Cx),
        ]);
        let baseline = input.clone();

        let first = expand_allocated_word_merges(&input).unwrap();
        let second = expand_allocated_word_merges(&input).unwrap();
        assert_eq!(input, baseline);
        assert_eq!(first, second);
        assert_eq!(
            first.blocks[0]
                .instructions
                .iter()
                .map(|one| one.id.get())
                .collect::<Vec<_>>(),
            vec![2, 10, 11, 9, 4, 12, 13]
        );
        assert_eq!(first.blocks[0].instructions[3], unchanged);
    }

    #[test]
    fn refuses_malformed_or_residual_unallocated_merges() {
        let mut declared_virtual = function(vec![merge(
            0,
            X86Register::Eax,
            X86Register::Ax,
            X86Register::Dx,
        )]);
        declared_virtual
            .virtual_registers
            .push(crate::old::codegen::machine::VirtualRegister {
                id: VirtualRegisterId::new(0),
                class: X86RegisterClass::Dword.machine_class(),
            });
        assert!(matches!(
            expand_allocated_word_merges(&declared_virtual),
            Err(WordMergeExpansionError::DeclaredVirtualRegister { .. })
        ));

        let mut constrained = physical_operand(X86Register::Eax, OperandRole::Def);
        constrained.constraint = Some(RegisterConstraint::Fixed(X86Register::Eax.physical()));
        let constrained_input = function(vec![instruction(
            MachineInstructionId::new(0),
            X86Opcode::MergeWords,
            vec![
                constrained,
                physical_operand(X86Register::Ax, OperandRole::Use),
                physical_operand(X86Register::Dx, OperandRole::Use),
            ],
        )]);
        assert!(matches!(
            expand_allocated_word_merges(&constrained_input),
            Err(WordMergeExpansionError::ResidualConstraint { .. })
        ));

        let virtual_input = function(vec![instruction(
            MachineInstructionId::new(0),
            X86Opcode::MergeWords,
            vec![
                MachineOperand {
                    kind: MachineOperandKind::Register(MachineRegister::Virtual(
                        VirtualRegisterId::new(0),
                    )),
                    role: OperandRole::Def,
                    constraint: None,
                    tied_to: None,
                },
                physical_operand(X86Register::Ax, OperandRole::Use),
                physical_operand(X86Register::Dx, OperandRole::Use),
            ],
        )]);
        assert!(matches!(
            expand_allocated_word_merges(&virtual_input),
            Err(WordMergeExpansionError::VirtualRegister { .. })
        ));

        let wrong_width = function(vec![instruction(
            MachineInstructionId::new(0),
            X86Opcode::MergeWords,
            vec![
                physical_operand(X86Register::Eax, OperandRole::Def),
                physical_operand(X86Register::Ecx, OperandRole::Use),
                physical_operand(X86Register::Dx, OperandRole::Use),
            ],
        )]);
        assert!(matches!(
            expand_allocated_word_merges(&wrong_width),
            Err(WordMergeExpansionError::WrongRegisterWidth { .. })
        ));

        let mut malformed = function(vec![merge(
            0,
            X86Register::Eax,
            X86Register::Ax,
            X86Register::Dx,
        )]);
        malformed.blocks[0].instructions[0].operands[2] =
            physical_operand(X86Register::Ecx, OperandRole::Def);
        assert!(matches!(
            expand_allocated_word_merges(&malformed),
            Err(WordMergeExpansionError::MalformedMerge { .. })
        ));
    }

    #[test]
    fn accepts_anchor_only_logical_virtuals_after_allocation() {
        let logical = VirtualRegisterId::new(0);
        let mut input = function(vec![
            instruction(
                MachineInstructionId::new(0),
                X86Opcode::Mov,
                vec![MachineOperand {
                    kind: MachineOperandKind::Register(MachineRegister::Virtual(logical)),
                    role: OperandRole::Def,
                    constraint: None,
                    tied_to: None,
                }],
            )
            .anchor(X86Opcode::Nothing.machine_opcode()),
            merge(1, X86Register::Eax, X86Register::Ax, X86Register::Dx),
        ]);
        input
            .virtual_registers
            .push(crate::old::codegen::machine::VirtualRegister {
                id: logical,
                class: X86RegisterClass::Dword.machine_class(),
            });

        let expanded = expand_allocated_word_merges(&input)
            .expect("an anchor's logical virtual is not an encodable operand");

        assert!(expanded.blocks[0].instructions[0].flags.anchor);
        assert_eq!(expanded.virtual_registers, input.virtual_registers);
    }

    #[test]
    fn rejects_an_encodable_instruction_marked_as_an_anchor() {
        let logical = VirtualRegisterId::new(0);
        let mut marked_mov = instruction(
            MachineInstructionId::new(0),
            X86Opcode::Mov,
            vec![MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(logical)),
                role: OperandRole::Def,
                constraint: None,
                tied_to: None,
            }],
        );
        marked_mov.flags.anchor = true;
        let mut input = function(vec![marked_mov]);
        input
            .virtual_registers
            .push(crate::old::codegen::machine::VirtualRegister {
                id: logical,
                class: X86RegisterClass::Dword.machine_class(),
            });

        assert_eq!(
            expand_allocated_word_merges(&input),
            Err(WordMergeExpansionError::DeclaredVirtualRegister { register: logical })
        );
    }

    #[test]
    fn refuses_exhausted_fresh_instruction_ids() {
        let exhausted = function(vec![merge(
            u32::MAX,
            X86Register::Eax,
            X86Register::Ax,
            X86Register::Dx,
        )]);
        assert_eq!(
            expand_allocated_word_merges(&exhausted),
            Err(WordMergeExpansionError::InstructionIdExhausted)
        );
    }
}
