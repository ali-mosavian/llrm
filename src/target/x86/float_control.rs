//! Expands x87 toward-zero integer stores through the control word.
//!
//! The 80387 target has no `fisttp` truncating integer-store instruction. A
//! selected truncating store is therefore a pseudo until the x87 source has
//! been stackified to physical `ST0`.  This pass saves the caller's control
//! word once, derives a copy with its rounding-control bits set to truncate,
//! and installs that copy only around each store.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    FrameIndex, FrameObject, FrameObjectKind, InstructionFlags, MachineBlockId, MachineFunction,
    MachineInstruction, MachineInstructionId, MachineOperand, MachineOperandKind, MachineRegister,
    OperandRole, VirtualRegister, VirtualRegisterId,
};

use super::instructions::X87MemoryFormat;
use super::{X86Opcode, X86Register, X86RegisterClass};

/// A refusal while expanding an x87 truncating integer store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86FloatControlError {
    MissingEntry {
        entry: MachineBlockId,
    },
    DuplicateFrameIndex {
        index: FrameIndex,
    },
    DuplicateVirtualRegister {
        register: VirtualRegisterId,
    },
    DuplicateInstructionId {
        id: MachineInstructionId,
    },
    FrameIndexExhausted,
    VirtualRegisterIdExhausted,
    InstructionIdExhausted,
    MalformedTruncation {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        reason: &'static str,
    },
}

impl fmt::Display for X86FloatControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEntry { entry } => {
                write!(formatter, "function has no entry block {entry}")
            }
            Self::DuplicateFrameIndex { index } => {
                write!(formatter, "duplicate frame index {index}")
            }
            Self::DuplicateVirtualRegister { register } => {
                write!(formatter, "duplicate virtual register {register}")
            }
            Self::DuplicateInstructionId { id } => {
                write!(formatter, "duplicate instruction ID {id}")
            }
            Self::FrameIndexExhausted => {
                write!(formatter, "x87 control expansion exhausted frame indices")
            }
            Self::VirtualRegisterIdExhausted => {
                write!(
                    formatter,
                    "x87 control expansion exhausted virtual-register IDs"
                )
            }
            Self::InstructionIdExhausted => {
                write!(formatter, "x87 control expansion exhausted instruction IDs")
            }
            Self::MalformedTruncation {
                block,
                instruction,
                reason,
            } => write!(
                formatter,
                "block {block} instruction {instruction} has malformed truncating x87 store: {reason}"
            ),
        }
    }
}

impl Error for X86FloatControlError {}

/// Synchronization required around materialized popping x87 integer stores.
///
/// The policy belongs to the target boundary: its caller selects the required
/// calling-environment behavior without teaching Machine IR about a source
/// language or frontend.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum X87StoreSynchronization {
    /// Emit only the x87 store and any required control-word sequence.
    #[default]
    None,
    /// Emit `wait` immediately before and after each physical `fistp`.
    BeforeAndAfter,
}

/// Expands every truncating x87 integer-store pseudo in a function.
///
/// The input is never changed.  The saved and derived control words are new,
/// word-aligned temporary frame objects.  Their word virtual registers and
/// all inserted instructions receive IDs strictly after the greatest existing
/// ID in their respective namespaces.  The original pseudo keeps its ID as
/// the actual popping integer store.
pub fn expand_x87_truncation(
    function: &MachineFunction,
) -> Result<MachineFunction, X86FloatControlError> {
    expand_x87_truncation_with_synchronization(function, X87StoreSynchronization::None)
}

/// Expands truncating stores and applies the target synchronization policy to
/// physical popping integer stores.
///
/// A truncating pseudo becomes `fldcw; fistp; fldcw`; synchronized output
/// places `wait` outside that entire sequence. Direct physical `fistp` forms
/// need no control-word state and receive only their two waits.
pub fn expand_x87_truncation_with_synchronization(
    function: &MachineFunction,
    synchronization: X87StoreSynchronization,
) -> Result<MachineFunction, X86FloatControlError> {
    let plan = preflight(function, synchronization)?;
    if plan.truncations == 0 && plan.unsynchronized_stores == 0 {
        return Ok(function.clone());
    }

    let control_words = plan.truncations != 0;
    let (saved, chopped, loaded, chop) = if control_words {
        let (saved, chopped) = reserve_frames(function)?;
        let (loaded, chop) = reserve_registers(function)?;
        (Some(saved), Some(chopped), Some(loaded), Some(chop))
    } else {
        (None, None, None, None)
    };
    let waits_per_truncation = if synchronization == X87StoreSynchronization::BeforeAndAfter {
        2
    } else {
        0
    };
    let extra = plan
        .truncations
        .checked_mul(
            2usize
                .checked_add(waits_per_truncation)
                .ok_or(X86FloatControlError::InstructionIdExhausted)?,
        )
        .and_then(|count| {
            plan.unsynchronized_stores
                .checked_mul(2)
                .and_then(|stores| count.checked_add(stores))
        })
        .and_then(|count| {
            if control_words {
                count.checked_add(5)
            } else {
                Some(count)
            }
        })
        .ok_or(X86FloatControlError::InstructionIdExhausted)?;
    let mut ids = reserve_instruction_ids(function, extra)?.into_iter();
    let entry = control_words.then(|| {
        function
            .blocks
            .iter()
            .position(|block| block.id == function.entry)
            .expect("preflight established the entry block")
    });

    let mut expanded = function.clone();
    if let (Some(saved), Some(chopped), Some(loaded), Some(chop)) = (saved, chopped, loaded, chop) {
        expanded.frame_objects.extend([
            FrameObject {
                index: saved,
                size: 2,
                alignment: 2,
                kind: FrameObjectKind::Temporary,
            },
            FrameObject {
                index: chopped,
                size: 2,
                alignment: 2,
                kind: FrameObjectKind::Temporary,
            },
        ]);
        expanded.virtual_registers.extend([
            VirtualRegister {
                id: loaded,
                class: X86RegisterClass::Word.machine_class(),
            },
            VirtualRegister {
                id: chop,
                class: X86RegisterClass::Word.machine_class(),
            },
        ]);
    }

    for (position, block) in expanded.blocks.iter_mut().enumerate() {
        let originals = std::mem::take(&mut block.instructions);
        let mut instructions = Vec::with_capacity(
            originals.len()
                + if entry == Some(position) { 5 } else { 0 }
                + originals.len().saturating_mul(2),
        );
        if entry == Some(position) {
            instructions.extend(entry_prefix(
                &mut ids,
                saved.expect("control words have saved frame"),
                chopped.expect("control words have chopped frame"),
                loaded.expect("control words have loaded register"),
                chop.expect("control words have chopped register"),
            ));
        }
        for (original_index, original) in originals.iter().cloned().enumerate() {
            let opcode = X86Opcode::from_machine_opcode(original.opcode);
            if opcode == Some(X86Opcode::X87IntegerStoreTrunc) {
                let address = original.operands[2..].to_vec();
                if synchronization == X87StoreSynchronization::BeforeAndAfter {
                    instructions.push(wait(next_id(&mut ids)));
                }
                instructions.push(control_load(
                    next_id(&mut ids),
                    chopped.expect("truncation has chopped control word"),
                ));
                instructions.push(MachineInstruction {
                    id: original.id,
                    opcode: X86Opcode::X87IntegerStorePop.machine_opcode(),
                    operands: integer_store_operands(&original, address),
                    flags: original.flags,
                });
                instructions.push(control_load(
                    next_id(&mut ids),
                    saved.expect("truncation has saved control word"),
                ));
                if synchronization == X87StoreSynchronization::BeforeAndAfter {
                    instructions.push(wait(next_id(&mut ids)));
                }
            } else if synchronization == X87StoreSynchronization::BeforeAndAfter
                && is_physical_integer_store_pop(&original)
                && !is_synchronized_store(&originals, original_index)
            {
                instructions.push(wait(next_id(&mut ids)));
                instructions.push(original);
                instructions.push(wait(next_id(&mut ids)));
                continue;
            } else {
                instructions.push(original);
            }
        }
        block.instructions = instructions;
    }

    Ok(expanded)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpansionPlan {
    truncations: usize,
    unsynchronized_stores: usize,
}

fn preflight(
    function: &MachineFunction,
    synchronization: X87StoreSynchronization,
) -> Result<ExpansionPlan, X86FloatControlError> {
    let mut truncations = 0_usize;
    let mut unsynchronized_stores = 0_usize;
    for block in &function.blocks {
        for (index, instruction) in block.instructions.iter().enumerate() {
            match X86Opcode::from_machine_opcode(instruction.opcode) {
                Some(X86Opcode::X87IntegerStoreTrunc) => {
                    validate_truncation(block.id, instruction)?;
                    truncations = truncations
                        .checked_add(1)
                        .ok_or(X86FloatControlError::InstructionIdExhausted)?;
                }
                Some(X86Opcode::X87IntegerStorePop)
                    if synchronization == X87StoreSynchronization::BeforeAndAfter
                        && is_physical_integer_store_pop(instruction)
                        && !is_synchronized_store(&block.instructions, index) =>
                {
                    unsynchronized_stores = unsynchronized_stores
                        .checked_add(1)
                        .ok_or(X86FloatControlError::InstructionIdExhausted)?;
                }
                _ => {}
            }
        }
    }
    if truncations == 0 && unsynchronized_stores == 0 {
        return Ok(ExpansionPlan {
            truncations,
            unsynchronized_stores,
        });
    }

    if truncations != 0
        && !function
            .blocks
            .iter()
            .any(|block| block.id == function.entry)
    {
        return Err(X86FloatControlError::MissingEntry {
            entry: function.entry,
        });
    }
    duplicates(function)?;
    Ok(ExpansionPlan {
        truncations,
        unsynchronized_stores,
    })
}

fn is_physical_integer_store_pop(instruction: &MachineInstruction) -> bool {
    X86Opcode::from_machine_opcode(instruction.opcode) == Some(X86Opcode::X87IntegerStorePop)
        && matches!(
            instruction.operands.first(),
            Some(MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
                role: OperandRole::Use,
                constraint: None,
                tied_to: None,
            }) if *register == X86Register::St0.physical()
        )
}

fn is_synchronized_store(instructions: &[MachineInstruction], index: usize) -> bool {
    let opcode = |offset: isize| {
        index
            .checked_add_signed(offset)
            .and_then(|position| instructions.get(position))
            .and_then(|instruction| X86Opcode::from_machine_opcode(instruction.opcode))
    };
    matches!(
        (opcode(-1), opcode(1)),
        (Some(X86Opcode::Wait), Some(X86Opcode::Wait))
    ) || matches!(
        (opcode(-2), opcode(-1), opcode(1), opcode(2)),
        (
            Some(X86Opcode::Wait),
            Some(X86Opcode::X87LoadControlWord),
            Some(X86Opcode::X87LoadControlWord),
            Some(X86Opcode::Wait)
        )
    )
}

fn validate_truncation(
    block: MachineBlockId,
    instruction: &MachineInstruction,
) -> Result<(), X86FloatControlError> {
    if instruction.operands.len() < 3 {
        return malformed(
            block,
            instruction,
            "expected [ST0 use, signed format, address tail]",
        );
    }
    let source = &instruction.operands[0];
    if source.role != OperandRole::Use || source.constraint.is_some() || source.tied_to.is_some() {
        return malformed(
            block,
            instruction,
            "first operand must be an unconstrained ST0 use",
        );
    }
    if source.kind
        != MachineOperandKind::Register(MachineRegister::Physical(X86Register::St0.physical()))
    {
        return malformed(block, instruction, "first operand must be physical ST0");
    }
    let format = &instruction.operands[1];
    if format.role != OperandRole::None || format.constraint.is_some() || format.tied_to.is_some() {
        return malformed(
            block,
            instruction,
            "format operand must be a plain immediate",
        );
    }
    let MachineOperandKind::Immediate(raw) = format.kind else {
        return malformed(block, instruction, "format operand must be an immediate");
    };
    if !matches!(
        X87MemoryFormat::from_raw(u8::try_from(raw).ok().unwrap_or_default()),
        Some(X87MemoryFormat::Signed16 | X87MemoryFormat::Signed32 | X87MemoryFormat::Signed64)
    ) {
        return malformed(
            block,
            instruction,
            "format must be a signed x87 integer format",
        );
    }
    Ok(())
}

fn duplicates(function: &MachineFunction) -> Result<(), X86FloatControlError> {
    let mut frames = BTreeSet::new();
    for frame in &function.frame_objects {
        if !frames.insert(frame.index) {
            return Err(X86FloatControlError::DuplicateFrameIndex { index: frame.index });
        }
    }
    let mut registers = BTreeSet::new();
    for register in &function.virtual_registers {
        if !registers.insert(register.id) {
            return Err(X86FloatControlError::DuplicateVirtualRegister {
                register: register.id,
            });
        }
    }
    let mut instructions = BTreeSet::new();
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        if !instructions.insert(instruction.id) {
            return Err(X86FloatControlError::DuplicateInstructionId { id: instruction.id });
        }
    }
    Ok(())
}

fn reserve_frames(
    function: &MachineFunction,
) -> Result<(FrameIndex, FrameIndex), X86FloatControlError> {
    let first = function
        .frame_objects
        .iter()
        .map(|frame| frame.index.get())
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1)
                .ok_or(X86FloatControlError::FrameIndexExhausted)
        })?;
    let second = first
        .checked_add(1)
        .ok_or(X86FloatControlError::FrameIndexExhausted)?;
    Ok((FrameIndex::new(first), FrameIndex::new(second)))
}

fn reserve_registers(
    function: &MachineFunction,
) -> Result<(VirtualRegisterId, VirtualRegisterId), X86FloatControlError> {
    let first = function
        .virtual_registers
        .iter()
        .map(|register| register.id.get())
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1)
                .ok_or(X86FloatControlError::VirtualRegisterIdExhausted)
        })?;
    let second = first
        .checked_add(1)
        .ok_or(X86FloatControlError::VirtualRegisterIdExhausted)?;
    Ok((
        VirtualRegisterId::new(first),
        VirtualRegisterId::new(second),
    ))
}

fn reserve_instruction_ids(
    function: &MachineFunction,
    count: usize,
) -> Result<Vec<MachineInstructionId>, X86FloatControlError> {
    let count = u32::try_from(count).map_err(|_| X86FloatControlError::InstructionIdExhausted)?;
    let first = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .map(|instruction| instruction.id.get())
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1)
                .ok_or(X86FloatControlError::InstructionIdExhausted)
        })?;
    let last = first
        .checked_add(
            count
                .checked_sub(1)
                .ok_or(X86FloatControlError::InstructionIdExhausted)?,
        )
        .ok_or(X86FloatControlError::InstructionIdExhausted)?;
    Ok((first..=last).map(MachineInstructionId::new).collect())
}

fn entry_prefix(
    ids: &mut impl Iterator<Item = MachineInstructionId>,
    saved: FrameIndex,
    chopped: FrameIndex,
    loaded: VirtualRegisterId,
    chop: VirtualRegisterId,
) -> [MachineInstruction; 5] {
    [
        control_store(next_id(ids), saved),
        load(next_id(ids), loaded, saved),
        copy(next_id(ids), chop, loaded),
        or_truncation(next_id(ids), chop),
        store(next_id(ids), chopped, chop),
    ]
}

fn integer_store_operands(
    original: &MachineInstruction,
    address: Vec<MachineOperand>,
) -> Vec<MachineOperand> {
    let mut operands = vec![st0_operand(), original.operands[1].clone()];
    operands.extend(address);
    operands
}

fn next_id(ids: &mut impl Iterator<Item = MachineInstructionId>) -> MachineInstructionId {
    ids.next()
        .expect("preflight reserved every inserted instruction ID")
}

fn malformed<T>(
    block: MachineBlockId,
    instruction: &MachineInstruction,
    reason: &'static str,
) -> Result<T, X86FloatControlError> {
    Err(X86FloatControlError::MalformedTruncation {
        block,
        instruction: instruction.id,
        reason,
    })
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

fn wait(id: MachineInstructionId) -> MachineInstruction {
    instruction(id, X86Opcode::Wait, Vec::new(), InstructionFlags::NONE)
}

fn control_store(id: MachineInstructionId, frame: FrameIndex) -> MachineInstruction {
    instruction(
        id,
        X86Opcode::X87StoreControlWord,
        vec![
            format_operand(X87MemoryFormat::Control16),
            frame_operand(frame),
        ],
        InstructionFlags {
            side_effects: true,
            may_store: true,
            ..InstructionFlags::NONE
        },
    )
}

fn control_load(id: MachineInstructionId, frame: FrameIndex) -> MachineInstruction {
    instruction(
        id,
        X86Opcode::X87LoadControlWord,
        vec![
            format_operand(X87MemoryFormat::Control16),
            frame_operand(frame),
        ],
        InstructionFlags {
            side_effects: true,
            may_load: true,
            ..InstructionFlags::NONE
        },
    )
}

fn load(
    id: MachineInstructionId,
    destination: VirtualRegisterId,
    frame: FrameIndex,
) -> MachineInstruction {
    instruction(
        id,
        X86Opcode::Load,
        vec![
            virtual_operand(destination, OperandRole::Def),
            frame_operand(frame),
        ],
        InstructionFlags {
            may_load: true,
            ..InstructionFlags::NONE
        },
    )
}

fn copy(
    id: MachineInstructionId,
    destination: VirtualRegisterId,
    source: VirtualRegisterId,
) -> MachineInstruction {
    instruction(
        id,
        X86Opcode::Copy,
        vec![
            virtual_operand(destination, OperandRole::Def),
            virtual_operand(source, OperandRole::Use),
        ],
        InstructionFlags {
            copy: true,
            ..InstructionFlags::NONE
        },
    )
}

fn or_truncation(id: MachineInstructionId, value: VirtualRegisterId) -> MachineInstruction {
    instruction(
        id,
        X86Opcode::Or,
        vec![
            virtual_operand(value, OperandRole::UseDef),
            immediate_operand(0x0c00),
        ],
        InstructionFlags::NONE,
    )
}

fn store(
    id: MachineInstructionId,
    frame: FrameIndex,
    source: VirtualRegisterId,
) -> MachineInstruction {
    instruction(
        id,
        X86Opcode::Store,
        vec![
            frame_operand(frame),
            virtual_operand(source, OperandRole::Use),
        ],
        InstructionFlags {
            side_effects: true,
            may_store: true,
            ..InstructionFlags::NONE
        },
    )
}

fn st0_operand() -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Register(MachineRegister::Physical(X86Register::St0.physical())),
        role: OperandRole::Use,
        constraint: None,
        tied_to: None,
    }
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

fn immediate_operand(value: i64) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Immediate(value),
        role: OperandRole::None,
        constraint: None,
        tied_to: None,
    }
}

fn format_operand(format: X87MemoryFormat) -> MachineOperand {
    immediate_operand(i64::from(format.raw()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        MachineBlock, MachineCallingConvention, MachineFunctionId, MachineLinkage, MachineSignature,
    };

    fn function(instructions: Vec<MachineInstruction>) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(1),
            name: "x87_control".into(),
            linkage: MachineLinkage::Internal,
            signature: MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::C,
            },
            entry: MachineBlockId::new(2),
            virtual_registers: vec![VirtualRegister {
                id: VirtualRegisterId::new(4),
                class: X86RegisterClass::Dword.machine_class(),
            }],
            blocks: vec![MachineBlock {
                id: MachineBlockId::new(2),
                instructions,
                successors: Vec::new(),
            }],
            frame_objects: vec![FrameObject {
                index: FrameIndex::new(6),
                size: 4,
                alignment: 2,
                kind: FrameObjectKind::Temporary,
            }],
        }
    }

    fn truncation(id: u32, format: X87MemoryFormat) -> MachineInstruction {
        instruction(
            MachineInstructionId::new(id),
            X86Opcode::X87IntegerStoreTrunc,
            vec![
                st0_operand(),
                format_operand(format),
                frame_operand(FrameIndex::new(6)),
            ],
            InstructionFlags {
                side_effects: true,
                may_store: true,
                ..InstructionFlags::NONE
            },
        )
    }

    fn dynamic_store(id: u32, format: X87MemoryFormat) -> MachineInstruction {
        instruction(
            MachineInstructionId::new(id),
            X86Opcode::X87IntegerStorePop,
            vec![
                st0_operand(),
                format_operand(format),
                frame_operand(FrameIndex::new(6)),
            ],
            InstructionFlags {
                side_effects: true,
                may_store: true,
                ..InstructionFlags::NONE
            },
        )
    }

    fn opcodes(function: &MachineFunction) -> Vec<X86Opcode> {
        function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| {
                X86Opcode::from_machine_opcode(instruction.opcode).expect("x86 opcode")
            })
            .collect()
    }

    #[test]
    fn expands_multiple_stores_with_one_saved_control_word() {
        let expanded = expand_x87_truncation(&function(vec![
            truncation(9, X87MemoryFormat::Signed32),
            truncation(12, X87MemoryFormat::Signed16),
        ]))
        .expect("well-formed truncating stores expand");

        assert_eq!(expanded.frame_objects.len(), 3);
        assert_eq!(expanded.frame_objects[1].index, FrameIndex::new(7));
        assert_eq!(expanded.frame_objects[2].index, FrameIndex::new(8));
        assert_eq!(expanded.virtual_registers.len(), 3);
        assert_eq!(
            expanded.virtual_registers[1].class,
            X86RegisterClass::Word.machine_class()
        );
        assert_eq!(
            opcodes(&expanded),
            vec![
                X86Opcode::X87StoreControlWord,
                X86Opcode::Load,
                X86Opcode::Copy,
                X86Opcode::Or,
                X86Opcode::Store,
                X86Opcode::X87LoadControlWord,
                X86Opcode::X87IntegerStorePop,
                X86Opcode::X87LoadControlWord,
                X86Opcode::X87LoadControlWord,
                X86Opcode::X87IntegerStorePop,
                X86Opcode::X87LoadControlWord,
            ]
        );
        assert!(!opcodes(&expanded).contains(&X86Opcode::X87IntegerStoreTrunc));
        assert_eq!(
            expanded.blocks[0].instructions[6].id,
            MachineInstructionId::new(9)
        );
        assert_eq!(
            expanded.blocks[0].instructions[9].id,
            MachineInstructionId::new(12)
        );
    }

    #[test]
    fn leaves_functions_without_a_truncating_store_exactly_unchanged() {
        let original = function(vec![instruction(
            MachineInstructionId::new(3),
            X86Opcode::Wait,
            Vec::new(),
            InstructionFlags::NONE,
        )]);
        assert_eq!(expand_x87_truncation(&original), Ok(original));
    }

    #[test]
    fn synchronizes_a_dynamic_integer_store_without_control_word_state() {
        let original = function(vec![dynamic_store(9, X87MemoryFormat::Signed32)]);
        let expanded = expand_x87_truncation_with_synchronization(
            &original,
            X87StoreSynchronization::BeforeAndAfter,
        )
        .expect("a physical dynamic store synchronizes");

        assert_eq!(
            opcodes(&expanded),
            vec![
                X86Opcode::Wait,
                X86Opcode::X87IntegerStorePop,
                X86Opcode::Wait
            ]
        );
        assert_eq!(expanded.frame_objects, original.frame_objects);
        assert_eq!(expanded.virtual_registers, original.virtual_registers);
        assert_eq!(
            expanded.blocks[0].instructions[0].id,
            MachineInstructionId::new(10)
        );
        assert_eq!(
            expanded.blocks[0].instructions[1],
            original.blocks[0].instructions[0]
        );
        assert_eq!(
            expanded.blocks[0].instructions[2].id,
            MachineInstructionId::new(11)
        );
    }

    #[test]
    fn synchronizes_a_truncating_store_around_its_full_control_word_sequence() {
        let expanded = expand_x87_truncation_with_synchronization(
            &function(vec![truncation(9, X87MemoryFormat::Signed16)]),
            X87StoreSynchronization::BeforeAndAfter,
        )
        .expect("a truncating store expands and synchronizes");

        assert_eq!(
            opcodes(&expanded),
            vec![
                X86Opcode::X87StoreControlWord,
                X86Opcode::Load,
                X86Opcode::Copy,
                X86Opcode::Or,
                X86Opcode::Store,
                X86Opcode::Wait,
                X86Opcode::X87LoadControlWord,
                X86Opcode::X87IntegerStorePop,
                X86Opcode::X87LoadControlWord,
                X86Opcode::Wait,
            ]
        );
        assert_eq!(
            expanded.blocks[0].instructions[5].id,
            MachineInstructionId::new(15)
        );
        assert_eq!(
            expanded.blocks[0].instructions[7].id,
            MachineInstructionId::new(9)
        );
        assert_eq!(
            expanded.blocks[0].instructions[9].id,
            MachineInstructionId::new(18)
        );
    }

    #[test]
    fn none_preserves_dynamic_integer_stores_without_waits() {
        let original = function(vec![dynamic_store(9, X87MemoryFormat::Signed64)]);
        assert_eq!(
            expand_x87_truncation_with_synchronization(&original, X87StoreSynchronization::None),
            Ok(original)
        );
    }

    #[test]
    fn does_not_double_wrap_an_already_synchronized_integer_store() {
        let original = function(vec![dynamic_store(9, X87MemoryFormat::Signed32)]);
        let once = expand_x87_truncation_with_synchronization(
            &original,
            X87StoreSynchronization::BeforeAndAfter,
        )
        .expect("first synchronization succeeds");
        assert_eq!(
            expand_x87_truncation_with_synchronization(
                &once,
                X87StoreSynchronization::BeforeAndAfter,
            ),
            Ok(once)
        );
    }

    #[test]
    fn synchronized_store_checks_existing_instruction_id_uniqueness() {
        let mut duplicate = dynamic_store(9, X87MemoryFormat::Signed16);
        duplicate.operands[1] = format_operand(X87MemoryFormat::Signed32);
        assert!(matches!(
            expand_x87_truncation_with_synchronization(
                &function(vec![dynamic_store(9, X87MemoryFormat::Signed16), duplicate]),
                X87StoreSynchronization::BeforeAndAfter,
            ),
            Err(X86FloatControlError::DuplicateInstructionId { id })
                if id == MachineInstructionId::new(9)
        ));
    }

    #[test]
    fn rejects_a_non_stack_top_source() {
        let mut bad = truncation(9, X87MemoryFormat::Signed32);
        bad.operands[0] = MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Physical(
                X86Register::St1.physical(),
            )),
            role: OperandRole::Use,
            constraint: None,
            tied_to: None,
        };
        assert!(matches!(
            expand_x87_truncation(&function(vec![bad])),
            Err(X86FloatControlError::MalformedTruncation {
                reason: "first operand must be physical ST0",
                ..
            })
        ));
    }
}
