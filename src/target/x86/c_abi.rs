//! 16-bit x86 caller-cleanup frame layout and post-allocation expansion.
//!
//! The general rule is that an ABI owns stack placement and entry/exit
//! protocol, while selected instructions carry only abstract frame indices.
//! This module therefore plans C-family frames without knowing which frontend
//! produced the function, and consumes only allocated Machine IR.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    FrameIndex, FrameObjectKind, InstructionFlags, MachineAddressSpace, MachineBlockId,
    MachineCallingConvention, MachineFunction, MachineFunctionId, MachineInstruction,
    MachineInstructionId, MachineOperand, MachineOperandKind, MachineRegister, MachineValueType,
    OperandRole, VirtualRegisterId,
};

use super::{X86FrameLayout, X86Opcode, X86Register, X86RegisterClass};

const NEAR_FIRST_ARGUMENT: u32 = 4;
const FAR_FIRST_ARGUMENT: u32 = 6;
// A signed 16-bit BP displacement must not wrap into the incoming-argument
// half of the frame.
const MAX_LOCAL_BYTES: u32 = 0x7ffe;

/// Complete caller-cleanup layout for one 16-bit C-family function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CFramePlan {
    layout: X86FrameLayout,
    calling_convention: MachineCallingConvention,
    framed: bool,
    local_bytes: u16,
}

impl CFramePlan {
    pub const fn layout(&self) -> &X86FrameLayout {
        &self.layout
    }

    pub const fn function(&self) -> MachineFunctionId {
        self.layout.function()
    }

    pub const fn calling_convention(&self) -> MachineCallingConvention {
        self.calling_convention
    }

    pub const fn framed(&self) -> bool {
        self.framed
    }

    /// Word-aligned stack reservation below BP.
    pub const fn local_bytes(&self) -> u16 {
        self.local_bytes
    }

    pub fn offset(&self, index: FrameIndex) -> Option<i32> {
        self.layout.offset(index)
    }
}

/// A refusal while representing a C-family 16-bit stack frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CFramePlanError {
    UnsupportedCallingConvention(MachineCallingConvention),
    VariadicFunction,
    UnsupportedResult(MachineValueType),
    UnsupportedParameter {
        parameter: usize,
        value_type: MachineValueType,
    },
    MissingIncomingArgument {
        parameter: usize,
    },
    DuplicateIncomingArgument {
        parameter: usize,
    },
    IncomingArgumentOutOfBounds {
        frame: FrameIndex,
        parameter: u32,
    },
    IncomingArgumentSize {
        frame: FrameIndex,
        parameter: usize,
        actual: u32,
    },
    IncomingArgumentAlignment {
        frame: FrameIndex,
        parameter: usize,
        alignment: u32,
    },
    DuplicateFrameIndex(FrameIndex),
    InvalidFrameObject {
        frame: FrameIndex,
        size: u32,
        alignment: u32,
    },
    UnsupportedOutgoingArgument(FrameIndex),
    LocalAlignment {
        frame: FrameIndex,
        alignment: u32,
    },
    LocalReservationTooLarge(u32),
    ArithmeticOverflow,
}

impl fmt::Display for CFramePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedCallingConvention(convention) => write!(
                formatter,
                "16-bit C frame planning does not support {convention:?} calling convention"
            ),
            Self::VariadicFunction => write!(
                formatter,
                "16-bit C frame planning does not support variadic functions"
            ),
            Self::UnsupportedResult(value_type) => write!(
                formatter,
                "16-bit C frame planning does not support result type {value_type:?}"
            ),
            Self::UnsupportedParameter {
                parameter,
                value_type,
            } => write!(
                formatter,
                "16-bit C parameter {parameter} has unsupported ABI type {value_type:?}"
            ),
            Self::MissingIncomingArgument { parameter } => write!(
                formatter,
                "16-bit C parameter {parameter} has no incoming frame object"
            ),
            Self::DuplicateIncomingArgument { parameter } => write!(
                formatter,
                "16-bit C parameter {parameter} has multiple incoming frame objects"
            ),
            Self::IncomingArgumentOutOfBounds { frame, parameter } => write!(
                formatter,
                "frame index {frame} names incoming C parameter {parameter} outside the signature"
            ),
            Self::IncomingArgumentSize {
                frame,
                parameter,
                actual,
            } => write!(
                formatter,
                "frame index {frame} for C parameter {parameter} has size {actual}, expected 2"
            ),
            Self::IncomingArgumentAlignment {
                frame,
                parameter,
                alignment,
            } => write!(
                formatter,
                "frame index {frame} for C parameter {parameter} has alignment {alignment}, expected 2"
            ),
            Self::DuplicateFrameIndex(frame) => {
                write!(formatter, "C frame contains duplicate frame index {frame}")
            }
            Self::InvalidFrameObject {
                frame,
                size,
                alignment,
            } => write!(
                formatter,
                "frame index {frame} has invalid size {size} or alignment {alignment}"
            ),
            Self::UnsupportedOutgoingArgument(frame) => write!(
                formatter,
                "frame index {frame} is an outgoing argument; C calls currently push arguments directly"
            ),
            Self::LocalAlignment { frame, alignment } => write!(
                formatter,
                "frame index {frame} requests alignment {alignment}; 16-bit C locals are word-aligned"
            ),
            Self::LocalReservationTooLarge(bytes) => write!(
                formatter,
                "16-bit C local reservation is {bytes} bytes; maximum is {MAX_LOCAL_BYTES}"
            ),
            Self::ArithmeticOverflow => {
                write!(formatter, "16-bit C frame size arithmetic overflowed")
            }
        }
    }
}

impl Error for CFramePlanError {}

/// Plans a 16-bit near-C or far-cdecl frame without changing Machine IR.
pub fn plan_c_frame(function: &MachineFunction) -> Result<CFramePlan, CFramePlanError> {
    let calling_convention = function.signature.calling_convention;
    if !matches!(
        calling_convention,
        MachineCallingConvention::C | MachineCallingConvention::FarCdecl
    ) {
        return Err(CFramePlanError::UnsupportedCallingConvention(
            calling_convention,
        ));
    }
    if function.signature.variadic {
        return Err(CFramePlanError::VariadicFunction);
    }
    if let Some(value_type) = function.signature.result {
        require_c_result_value(value_type)
            .map_err(|_| CFramePlanError::UnsupportedResult(value_type))?;
    }
    for (parameter, value_type) in function.signature.parameters.iter().copied().enumerate() {
        require_word_value(value_type).map_err(|_| CFramePlanError::UnsupportedParameter {
            parameter,
            value_type,
        })?;
    }

    let mut frame_indices = BTreeSet::new();
    let mut incoming = BTreeMap::new();
    let mut local_depths = Vec::new();
    let mut local_depth = 0_u32;
    for frame in &function.frame_objects {
        if !frame_indices.insert(frame.index) {
            return Err(CFramePlanError::DuplicateFrameIndex(frame.index));
        }
        if frame.size == 0 || frame.alignment == 0 || !frame.alignment.is_power_of_two() {
            return Err(CFramePlanError::InvalidFrameObject {
                frame: frame.index,
                size: frame.size,
                alignment: frame.alignment,
            });
        }
        match frame.kind {
            FrameObjectKind::IncomingArgument { parameter } => {
                let parameter_index = usize::try_from(parameter).map_err(|_| {
                    CFramePlanError::IncomingArgumentOutOfBounds {
                        frame: frame.index,
                        parameter,
                    }
                })?;
                if parameter_index >= function.signature.parameters.len() {
                    return Err(CFramePlanError::IncomingArgumentOutOfBounds {
                        frame: frame.index,
                        parameter,
                    });
                }
                if frame.size != 2 {
                    return Err(CFramePlanError::IncomingArgumentSize {
                        frame: frame.index,
                        parameter: parameter_index,
                        actual: frame.size,
                    });
                }
                if frame.alignment != 2 {
                    return Err(CFramePlanError::IncomingArgumentAlignment {
                        frame: frame.index,
                        parameter: parameter_index,
                        alignment: frame.alignment,
                    });
                }
                if incoming.insert(parameter_index, frame.index).is_some() {
                    return Err(CFramePlanError::DuplicateIncomingArgument {
                        parameter: parameter_index,
                    });
                }
            }
            FrameObjectKind::Local | FrameObjectKind::Spill => {
                if frame.alignment > 2 {
                    return Err(CFramePlanError::LocalAlignment {
                        frame: frame.index,
                        alignment: frame.alignment,
                    });
                }
                let size = align_word(frame.size)?;
                local_depth = local_depth
                    .checked_add(size)
                    .ok_or(CFramePlanError::ArithmeticOverflow)?;
                local_depth = align_word(local_depth)?;
                local_depths.push((frame.index, local_depth));
            }
            FrameObjectKind::OutgoingArgument => {
                return Err(CFramePlanError::UnsupportedOutgoingArgument(frame.index));
            }
        }
    }
    for parameter in 0..function.signature.parameters.len() {
        if !incoming.contains_key(&parameter) {
            return Err(CFramePlanError::MissingIncomingArgument { parameter });
        }
    }
    if local_depth > MAX_LOCAL_BYTES {
        return Err(CFramePlanError::LocalReservationTooLarge(local_depth));
    }

    let first_argument = match calling_convention {
        MachineCallingConvention::C => NEAR_FIRST_ARGUMENT,
        MachineCallingConvention::FarCdecl => FAR_FIRST_ARGUMENT,
        MachineCallingConvention::FarPascal => unreachable!("calling convention was checked"),
    };
    let mut offsets = BTreeMap::new();
    for parameter in 0..function.signature.parameters.len() {
        let offset = first_argument
            .checked_add(
                u32::try_from(parameter)
                    .map_err(|_| CFramePlanError::ArithmeticOverflow)?
                    .checked_mul(2)
                    .ok_or(CFramePlanError::ArithmeticOverflow)?,
            )
            .ok_or(CFramePlanError::ArithmeticOverflow)?;
        let offset = i32::try_from(offset).map_err(|_| CFramePlanError::ArithmeticOverflow)?;
        offsets.insert(incoming[&parameter], offset);
    }
    for (frame, depth) in local_depths {
        let offset = i32::try_from(depth).map_err(|_| CFramePlanError::ArithmeticOverflow)?;
        offsets.insert(frame, -offset);
    }

    let local_bytes = u16::try_from(local_depth)
        .map_err(|_| CFramePlanError::LocalReservationTooLarge(local_depth))?;
    let framed = !offsets.is_empty();
    Ok(CFramePlan {
        layout: X86FrameLayout::new(function.id, offsets),
        calling_convention,
        framed,
        local_bytes,
    })
}

fn require_word_value(value_type: MachineValueType) -> Result<(), ()> {
    // In the 16-bit C data model, a near data pointer is passed as its
    // offset word. Other pointer representations are not one-word ABI values.
    matches!(
        value_type,
        MachineValueType::Integer { bits: 16 }
            | MachineValueType::Pointer {
                bits: 16,
                address_space: MachineAddressSpace::NearData,
            }
    )
    .then_some(())
    .ok_or(())
}

fn require_c_result_value(value_type: MachineValueType) -> Result<(), ()> {
    matches!(value_type, MachineValueType::Integer { bits: 16 | 32 })
        .then_some(())
        .ok_or(())
}

fn align_word(value: u32) -> Result<u32, CFramePlanError> {
    value
        .checked_add(1)
        .map(|value| value & !1)
        .ok_or(CFramePlanError::ArithmeticOverflow)
}

/// A refusal while inserting a C-family frame protocol after allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CAbiExpansionError {
    Frame(CFramePlanError),
    MismatchedFramePlan {
        function: MachineFunctionId,
        planned: MachineFunctionId,
    },
    StaleFramePlan,
    EmptyFunction,
    UnknownEntry(MachineBlockId),
    DeclaredVirtualRegister {
        register: VirtualRegisterId,
    },
    ResidualVirtualRegister {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        operand: usize,
        register: VirtualRegisterId,
    },
    AlreadyExpanded {
        block: MachineBlockId,
    },
    WrongReturn {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        expected: X86Opcode,
        actual: Option<X86Opcode>,
    },
    MalformedReturn {
        block: MachineBlockId,
        instruction: MachineInstructionId,
        reason: &'static str,
    },
    MalformedCall {
        block: MachineBlockId,
        instruction: MachineInstructionId,
    },
    InstructionIdExhausted,
}

impl fmt::Display for CAbiExpansionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => error.fmt(formatter),
            Self::MismatchedFramePlan { function, planned } => write!(
                formatter,
                "machine function {function} cannot use C frame plan for function {planned}"
            ),
            Self::StaleFramePlan => write!(
                formatter,
                "C frame plan does not match the supplied function's ABI facts"
            ),
            Self::EmptyFunction => write!(formatter, "C ABI frame has no entry block"),
            Self::UnknownEntry(block) => {
                write!(formatter, "C ABI frame entry block {block} is absent")
            }
            Self::DeclaredVirtualRegister { register } => write!(
                formatter,
                "C ABI expansion retains virtual register declaration {register}"
            ),
            Self::ResidualVirtualRegister {
                block,
                instruction,
                operand,
                register,
            } => write!(
                formatter,
                "block {block} instruction {instruction} operand {operand} retains virtual register {register}"
            ),
            Self::AlreadyExpanded { block } => write!(
                formatter,
                "C ABI frame is already expanded in entry block {block}"
            ),
            Self::WrongReturn {
                block,
                instruction,
                expected,
                actual,
            } => write!(
                formatter,
                "block {block} instruction {instruction} has {actual:?} return, expected {expected:?}"
            ),
            Self::MalformedReturn {
                block,
                instruction,
                reason,
            } => write!(
                formatter,
                "block {block} instruction {instruction} has malformed C return: {reason}"
            ),
            Self::MalformedCall { block, instruction } => write!(
                formatter,
                "block {block} instruction {instruction} has no direct C call target"
            ),
            Self::InstructionIdExhausted => {
                write!(formatter, "C ABI frame exhausted instruction IDs")
            }
        }
    }
}

impl Error for CAbiExpansionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            _ => None,
        }
    }
}

/// Inserts a conventional BP frame around an allocated C-family function.
///
/// The input is not changed.  All validation precedes cloning, and fresh IDs
/// are allocated after the greatest existing instruction ID in stored order.
pub fn expand_allocated_c_abi(
    function: &MachineFunction,
    plan: &CFramePlan,
) -> Result<MachineFunction, CAbiExpansionError> {
    let saved = callee_saved_registers(function);
    preflight(function, plan, &saved)?;
    let return_count = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| is_return(instruction))
        .count();
    let frame_entry_instructions = if plan.framed() {
        2 + usize::from(plan.local_bytes() != 0)
    } else {
        0
    };
    let entry_instructions = frame_entry_instructions
        .checked_add(saved.len())
        .ok_or(CAbiExpansionError::InstructionIdExhausted)?;
    let return_instructions_each = saved
        .len()
        .checked_add(usize::from(plan.framed()))
        .ok_or(CAbiExpansionError::InstructionIdExhausted)?;
    let return_instructions = return_count
        .checked_mul(return_instructions_each)
        .ok_or(CAbiExpansionError::InstructionIdExhausted)?;
    let added = entry_instructions
        .checked_add(return_instructions)
        .ok_or(CAbiExpansionError::InstructionIdExhausted)?;
    let mut fresh_ids = reserve_ids(function, added)?.into_iter();
    let mut expanded = function.clone();
    for block in &mut expanded.blocks {
        if block.id == function.entry && (plan.framed() || !saved.is_empty()) {
            let mut prefix = Vec::new();
            if plan.framed() {
                prefix.extend([
                    instruction(
                        next_id(&mut fresh_ids),
                        X86Opcode::Push,
                        vec![physical(X86Register::Bp, OperandRole::Use)],
                    ),
                    instruction(
                        next_id(&mut fresh_ids),
                        X86Opcode::Mov,
                        vec![
                            physical(X86Register::Bp, OperandRole::Def),
                            physical(X86Register::Sp, OperandRole::Use),
                        ],
                    ),
                ]);
            }
            if plan.local_bytes() != 0 {
                prefix.push(instruction(
                    next_id(&mut fresh_ids),
                    X86Opcode::Sub,
                    vec![
                        physical(X86Register::Sp, OperandRole::UseDef),
                        immediate(i64::from(plan.local_bytes())),
                    ],
                ));
            }
            prefix.extend(saved.iter().copied().map(|register| {
                instruction(
                    next_id(&mut fresh_ids),
                    X86Opcode::Push,
                    vec![physical(register, OperandRole::Use)],
                )
            }));
            block.instructions.splice(0..0, prefix);
        }
        let mut instructions = Vec::with_capacity(block.instructions.len() + 1);
        for mut original in std::mem::take(&mut block.instructions) {
            match X86Opcode::from_machine_opcode(original.opcode) {
                Some(X86Opcode::LowWord) => {
                    let source = dword_source(block.id, &original, &original.operands[1])?;
                    let [low] = source.sub_registers() else {
                        unreachable!("preflight validated a dword source")
                    };
                    if *low != X86Register::Ax {
                        original.opcode = X86Opcode::Mov.machine_opcode();
                        original.operands = vec![
                            physical(X86Register::Ax, OperandRole::Def),
                            physical(*low, OperandRole::Use),
                        ];
                        original.flags = InstructionFlags::NONE;
                        instructions.push(original);
                    }
                    continue;
                }
                Some(X86Opcode::HighWord) => {
                    let source = dword_source(block.id, &original, &original.operands[1])?;
                    original.opcode = X86Opcode::ShiftLeftDouble.machine_opcode();
                    original.operands = vec![
                        physical(X86Register::Edx, OperandRole::UseDef),
                        physical(source, OperandRole::Use),
                        immediate(16),
                    ];
                    original.flags = InstructionFlags::NONE;
                }
                Some(X86Opcode::CallNear | X86Opcode::CallFar) => {
                    original.operands.truncate(1);
                }
                Some(X86Opcode::ReturnNear) => {
                    instructions.extend(saved.iter().rev().copied().map(|register| {
                        instruction(
                            next_id(&mut fresh_ids),
                            X86Opcode::Pop,
                            vec![physical(register, OperandRole::Def)],
                        )
                    }));
                    if plan.framed() {
                        instructions.push(frame_exit(next_id(&mut fresh_ids), plan));
                    }
                    original.operands.clear();
                }
                Some(X86Opcode::ReturnFar) => {
                    instructions.extend(saved.iter().rev().copied().map(|register| {
                        instruction(
                            next_id(&mut fresh_ids),
                            X86Opcode::Pop,
                            vec![physical(register, OperandRole::Def)],
                        )
                    }));
                    if plan.framed() {
                        instructions.push(frame_exit(next_id(&mut fresh_ids), plan));
                    }
                    original.operands = vec![immediate(0)];
                }
                _ => {}
            }
            instructions.push(original);
        }
        block.instructions = instructions;
    }
    Ok(expanded)
}

fn preflight(
    function: &MachineFunction,
    plan: &CFramePlan,
    saved: &[X86Register],
) -> Result<(), CAbiExpansionError> {
    if plan.function() != function.id {
        return Err(CAbiExpansionError::MismatchedFramePlan {
            function: function.id,
            planned: plan.function(),
        });
    }
    let canonical = plan_c_frame(function).map_err(CAbiExpansionError::Frame)?;
    if &canonical != plan {
        return Err(CAbiExpansionError::StaleFramePlan);
    }
    if function.blocks.is_empty() {
        return Err(CAbiExpansionError::EmptyFunction);
    }
    if !function
        .blocks
        .iter()
        .any(|block| block.id == function.entry)
    {
        return Err(CAbiExpansionError::UnknownEntry(function.entry));
    }
    if let Some(register) = function.virtual_registers.first() {
        return Err(CAbiExpansionError::DeclaredVirtualRegister {
            register: register.id,
        });
    }
    let expected = match plan.calling_convention() {
        MachineCallingConvention::C => X86Opcode::ReturnNear,
        MachineCallingConvention::FarCdecl => X86Opcode::ReturnFar,
        MachineCallingConvention::FarPascal => unreachable!("plan validated convention"),
    };
    for block in &function.blocks {
        if block.id == function.entry && starts_expanded(block, saved) {
            return Err(CAbiExpansionError::AlreadyExpanded { block: block.id });
        }
        for instruction in &block.instructions {
            for (operand, value) in instruction.operands.iter().enumerate() {
                if let MachineOperandKind::Register(MachineRegister::Virtual(register)) = value.kind
                {
                    return Err(CAbiExpansionError::ResidualVirtualRegister {
                        block: block.id,
                        instruction: instruction.id,
                        operand,
                        register,
                    });
                }
            }
            if is_return(instruction) {
                validate_return(block.id, instruction, expected, function.signature.result)?;
            }
            if is_call(instruction)
                && !matches!(
                    instruction.operands.first().map(|operand| &operand.kind),
                    Some(
                        MachineOperandKind::Function(_) | MachineOperandKind::ExternalSymbol { .. }
                    )
                )
            {
                return Err(CAbiExpansionError::MalformedCall {
                    block: block.id,
                    instruction: instruction.id,
                });
            }
        }
        validate_return_extractions(block.id, &block.instructions, function.signature.result)?;
    }
    Ok(())
}

/// Validates the two selected pseudos immediately feeding an i32 C return.
///
/// This is intentionally a terminal ABI shape, rather than a general pseudo
/// lowering: `shld` changes flags and all but DX's low word. The following
/// return is the proof that neither effect has a remaining Machine IR user.
fn validate_return_extractions(
    block: MachineBlockId,
    instructions: &[MachineInstruction],
    result: Option<MachineValueType>,
) -> Result<(), CAbiExpansionError> {
    for (position, instruction) in instructions.iter().enumerate() {
        let opcode = X86Opcode::from_machine_opcode(instruction.opcode);
        if matches!(opcode, Some(X86Opcode::LowWord | X86Opcode::HighWord)) {
            let return_position = match opcode {
                Some(X86Opcode::LowWord) => position.checked_add(2),
                Some(X86Opcode::HighWord) => position.checked_add(1),
                _ => unreachable!(),
            };
            let Some(return_position) = return_position else {
                return malformed_return(
                    block,
                    instruction,
                    "word extraction does not feed a terminal i32 return",
                );
            };
            let Some(returned) = instructions.get(return_position) else {
                return malformed_return(
                    block,
                    instruction,
                    "word extraction does not feed a terminal i32 return",
                );
            };
            if result != Some(MachineValueType::Integer { bits: 32 })
                || !is_return(returned)
                || return_position + 1 != instructions.len()
            {
                return malformed_return(
                    block,
                    instruction,
                    "word extraction does not feed a terminal i32 return",
                );
            }
        }
        if !is_return(instruction) || result != Some(MachineValueType::Integer { bits: 32 }) {
            continue;
        }
        let Some(low_position) = position.checked_sub(2) else {
            return malformed_return(
                block,
                instruction,
                "i32 return lacks low/high word extraction",
            );
        };
        let Some(high_position) = position.checked_sub(1) else {
            return malformed_return(
                block,
                instruction,
                "i32 return lacks low/high word extraction",
            );
        };
        let low = &instructions[low_position];
        let high = &instructions[high_position];
        if X86Opcode::from_machine_opcode(low.opcode) != Some(X86Opcode::LowWord)
            || X86Opcode::from_machine_opcode(high.opcode) != Some(X86Opcode::HighWord)
        {
            return malformed_return(
                block,
                instruction,
                "i32 return lacks low/high word extraction",
            );
        }
        let source = validate_low_extraction(block, low)?;
        validate_high_extraction(block, high, source)?;
    }
    Ok(())
}

fn validate_low_extraction(
    block: MachineBlockId,
    instruction: &MachineInstruction,
) -> Result<X86Register, CAbiExpansionError> {
    let [destination, source] = instruction.operands.as_slice() else {
        return malformed_return(
            block,
            instruction,
            "low word extraction requires AX and one dword source",
        );
    };
    if instruction.flags != InstructionFlags::NONE
        || *destination != physical(X86Register::Ax, OperandRole::Def)
    {
        return malformed_return(
            block,
            instruction,
            "low word extraction must define AX without flags",
        );
    }
    dword_source(block, instruction, source)
}

fn validate_high_extraction(
    block: MachineBlockId,
    instruction: &MachineInstruction,
    expected_source: X86Register,
) -> Result<(), CAbiExpansionError> {
    let [destination, source] = instruction.operands.as_slice() else {
        return malformed_return(
            block,
            instruction,
            "high word extraction requires DX and one dword source",
        );
    };
    if instruction.flags != InstructionFlags::NONE
        || *destination != physical(X86Register::Dx, OperandRole::Def)
    {
        return malformed_return(
            block,
            instruction,
            "high word extraction must define DX without flags",
        );
    }
    if dword_source(block, instruction, source)? != expected_source {
        return malformed_return(
            block,
            instruction,
            "low and high word extractions use different sources",
        );
    }
    Ok(())
}

fn dword_source(
    block: MachineBlockId,
    instruction: &MachineInstruction,
    operand: &MachineOperand,
) -> Result<X86Register, CAbiExpansionError> {
    let MachineOperand {
        kind: MachineOperandKind::Register(MachineRegister::Physical(physical)),
        role: OperandRole::Use,
        constraint: None,
        tied_to: None,
    } = operand
    else {
        return malformed_return(
            block,
            instruction,
            "word extraction source must be an unconstrained physical dword use",
        );
    };
    let Some(register) = X86Register::from_physical(*physical) else {
        return malformed_return(
            block,
            instruction,
            "word extraction source has an unknown physical register",
        );
    };
    if !X86RegisterClass::Dword.members().contains(&register) {
        return malformed_return(
            block,
            instruction,
            "word extraction source must be a dword register",
        );
    }
    Ok(register)
}

fn malformed_return<T>(
    block: MachineBlockId,
    instruction: &MachineInstruction,
    reason: &'static str,
) -> Result<T, CAbiExpansionError> {
    Err(CAbiExpansionError::MalformedReturn {
        block,
        instruction: instruction.id,
        reason,
    })
}

fn callee_saved_registers(function: &MachineFunction) -> Vec<X86Register> {
    // Borland's C ABI saves only the low word of SI and DI.  Python masm
    // selects those saves from the ESI/EDI roots, so either physical view in
    // allocated Machine IR asks for the same low-word preservation.
    let roots = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .flat_map(|instruction| &instruction.operands)
        .filter_map(|operand| match operand.kind {
            MachineOperandKind::Register(MachineRegister::Physical(register)) => {
                X86Register::from_physical(register)
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    [
        (X86Register::Esi, X86Register::Si),
        (X86Register::Edi, X86Register::Di),
    ]
    .into_iter()
    .filter_map(|(whole, low)| (roots.contains(&whole) || roots.contains(&low)).then_some(low))
    .collect()
}

fn starts_expanded(block: &crate::codegen::machine::MachineBlock, saved: &[X86Register]) -> bool {
    let framed = matches!(block.instructions.get(0), Some(instruction) if instruction.opcode == X86Opcode::Push.machine_opcode() && instruction.operands.as_slice() == [physical(X86Register::Bp, OperandRole::Use)])
        && matches!(block.instructions.get(1), Some(instruction) if instruction.opcode == X86Opcode::Mov.machine_opcode() && instruction.operands.as_slice() == [physical(X86Register::Bp, OperandRole::Def), physical(X86Register::Sp, OperandRole::Use)]);
    let start = usize::from(framed) * 2;
    let saves = !saved.is_empty()
        && block
            .instructions
            .get(start..start + saved.len())
            .is_some_and(|instructions| {
                instructions
                    .iter()
                    .zip(saved)
                    .all(|(instruction, register)| {
                        instruction.opcode == X86Opcode::Push.machine_opcode()
                            && instruction.operands == [physical(*register, OperandRole::Use)]
                    })
            });
    framed || saves
}

fn validate_return(
    block: MachineBlockId,
    instruction: &MachineInstruction,
    expected: X86Opcode,
    result: Option<MachineValueType>,
) -> Result<(), CAbiExpansionError> {
    let actual = X86Opcode::from_machine_opcode(instruction.opcode);
    if actual != Some(expected) {
        return Err(CAbiExpansionError::WrongReturn {
            block,
            instruction: instruction.id,
            expected,
            actual,
        });
    }
    if instruction.flags
        != (InstructionFlags {
            terminator: true,
            ..InstructionFlags::NONE
        })
    {
        return Err(CAbiExpansionError::MalformedReturn {
            block,
            instruction: instruction.id,
            reason: "return must have only the terminator flag",
        });
    }
    let ax = physical(X86Register::Ax, OperandRole::Use);
    let dx = physical(X86Register::Dx, OperandRole::Use);
    let valid_operands = match (expected, result) {
        (X86Opcode::ReturnNear, None) => instruction.operands.is_empty(),
        (X86Opcode::ReturnNear, Some(MachineValueType::Integer { bits: 16 })) => {
            instruction.operands == [ax]
        }
        (X86Opcode::ReturnNear, Some(MachineValueType::Integer { bits: 32 })) => {
            instruction.operands == [ax, dx]
        }
        (X86Opcode::ReturnFar, None) => instruction.operands == [immediate(0)],
        (X86Opcode::ReturnFar, Some(MachineValueType::Integer { bits: 16 })) => {
            instruction.operands == [ax, immediate(0)]
        }
        (X86Opcode::ReturnFar, Some(MachineValueType::Integer { bits: 32 })) => {
            instruction.operands == [ax, dx, immediate(0)]
        }
        _ => false,
    };
    if !valid_operands {
        return Err(CAbiExpansionError::MalformedReturn {
            block,
            instruction: instruction.id,
            reason: "return operands do not preserve the C ABI result and cleanup",
        });
    }
    Ok(())
}

fn is_return(instruction: &MachineInstruction) -> bool {
    matches!(
        X86Opcode::from_machine_opcode(instruction.opcode),
        Some(X86Opcode::ReturnNear | X86Opcode::ReturnFar)
    )
}

fn is_call(instruction: &MachineInstruction) -> bool {
    matches!(
        X86Opcode::from_machine_opcode(instruction.opcode),
        Some(X86Opcode::CallNear | X86Opcode::CallFar)
    )
}

fn reserve_ids(
    function: &MachineFunction,
    count: usize,
) -> Result<Vec<MachineInstructionId>, CAbiExpansionError> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let first = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .map(|instruction| instruction.id.get())
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1)
                .ok_or(CAbiExpansionError::InstructionIdExhausted)
        })?;
    let count = u32::try_from(count).map_err(|_| CAbiExpansionError::InstructionIdExhausted)?;
    let last = first
        .checked_add(
            count
                .checked_sub(1)
                .ok_or(CAbiExpansionError::InstructionIdExhausted)?,
        )
        .ok_or(CAbiExpansionError::InstructionIdExhausted)?;
    Ok((first..=last).map(MachineInstructionId::new).collect())
}

fn next_id(ids: &mut impl Iterator<Item = MachineInstructionId>) -> MachineInstructionId {
    ids.next()
        .expect("preflight reserved every expansion instruction ID")
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
fn frame_exit(id: MachineInstructionId, plan: &CFramePlan) -> MachineInstruction {
    if plan.local_bytes() == 0 {
        instruction(
            id,
            X86Opcode::Pop,
            vec![physical(X86Register::Bp, OperandRole::Def)],
        )
    } else {
        instruction(id, X86Opcode::Leave, Vec::new())
    }
}
fn physical(register: X86Register, role: OperandRole) -> MachineOperand {
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
    use crate::codegen::machine::{FrameObject, MachineBlock, MachineLinkage, MachineSignature};

    fn function(
        convention: MachineCallingConvention,
        parameters: Vec<MachineValueType>,
        frames: Vec<FrameObject>,
        return_value: bool,
    ) -> MachineFunction {
        let result = return_value.then_some(MachineValueType::Integer { bits: 16 });
        let operands = match (convention, return_value) {
            (MachineCallingConvention::C, false) => vec![],
            (MachineCallingConvention::C, true) => {
                vec![physical(X86Register::Ax, OperandRole::Use)]
            }
            (MachineCallingConvention::FarCdecl, false) => vec![immediate(0)],
            (MachineCallingConvention::FarCdecl, true) => {
                vec![physical(X86Register::Ax, OperandRole::Use), immediate(0)]
            }
            _ => unreachable!(),
        };
        MachineFunction {
            id: MachineFunctionId::new(2),
            name: "word_function".into(),
            linkage: MachineLinkage::External,
            signature: MachineSignature {
                result,
                parameters,
                variadic: false,
                calling_convention: convention,
            },
            entry: MachineBlockId::new(4),
            virtual_registers: Vec::new(),
            blocks: vec![MachineBlock {
                id: MachineBlockId::new(4),
                instructions: vec![MachineInstruction {
                    id: MachineInstructionId::new(8),
                    opcode: match convention {
                        MachineCallingConvention::C => X86Opcode::ReturnNear,
                        MachineCallingConvention::FarCdecl => X86Opcode::ReturnFar,
                        MachineCallingConvention::FarPascal => unreachable!(),
                    }
                    .machine_opcode(),
                    operands,
                    flags: InstructionFlags {
                        terminator: true,
                        ..InstructionFlags::NONE
                    },
                }],
                successors: Vec::new(),
            }],
            frame_objects: frames,
        }
    }

    fn incoming(index: u32, parameter: u32) -> crate::codegen::machine::FrameObject {
        FrameObject {
            index: FrameIndex::new(index),
            size: 2,
            alignment: 2,
            kind: FrameObjectKind::IncomingArgument { parameter },
        }
    }
    fn local(index: u32, size: u32) -> crate::codegen::machine::FrameObject {
        FrameObject {
            index: FrameIndex::new(index),
            size,
            alignment: 2,
            kind: FrameObjectKind::Local,
        }
    }

    fn function_operand(function: u32) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Function(MachineFunctionId::new(function)),
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        }
    }

    #[test]
    fn plans_near_and_far_cdecl_parameters_in_c_stack_order() {
        // C pushes source arguments right-to-left, so the first formal is
        // closest to the return address in both near and far forms.
        let frames = vec![incoming(0, 0), incoming(1, 1)];
        let near = function(
            MachineCallingConvention::C,
            vec![MachineValueType::Integer { bits: 16 }; 2],
            frames.clone(),
            false,
        );
        let far = function(
            MachineCallingConvention::FarCdecl,
            vec![MachineValueType::Integer { bits: 16 }; 2],
            frames,
            false,
        );
        let near = plan_c_frame(&near).unwrap();
        let far = plan_c_frame(&far).unwrap();
        assert_eq!(near.offset(FrameIndex::new(0)), Some(4));
        assert_eq!(near.offset(FrameIndex::new(1)), Some(6));
        assert_eq!(far.offset(FrameIndex::new(0)), Some(6));
        assert_eq!(far.offset(FrameIndex::new(1)), Some(8));
    }

    #[test]
    fn plans_near_data_pointer_parameters_as_words_and_refuses_far_pointers() {
        // A far-cdecl return address occupies BP+2 and BP+4. Each near data
        // pointer is its one-word offset, so the two formals occupy BP+6/+8.
        let near_data_pointer = MachineValueType::Pointer {
            bits: 16,
            address_space: MachineAddressSpace::NearData,
        };
        let input = function(
            MachineCallingConvention::FarCdecl,
            vec![near_data_pointer; 2],
            vec![incoming(0, 0), incoming(1, 1)],
            false,
        );
        let plan = plan_c_frame(&input).unwrap();
        assert_eq!(plan.offset(FrameIndex::new(0)), Some(6));
        assert_eq!(plan.offset(FrameIndex::new(1)), Some(8));

        let far_data_pointer = MachineValueType::Pointer {
            bits: 16,
            address_space: MachineAddressSpace::FarData,
        };
        let unsupported = function(
            MachineCallingConvention::FarCdecl,
            vec![far_data_pointer],
            vec![incoming(0, 0)],
            false,
        );
        assert_eq!(
            plan_c_frame(&unsupported),
            Err(CFramePlanError::UnsupportedParameter {
                parameter: 0,
                value_type: far_data_pointer,
            })
        );
    }

    #[test]
    fn leaves_a_c_function_without_frame_objects_frameless() {
        let input = function(MachineCallingConvention::C, vec![], vec![], false);
        let plan = plan_c_frame(&input).unwrap();
        let expanded = expand_allocated_c_abi(&input, &plan).unwrap();

        assert!(!plan.framed());
        assert_eq!(expanded.blocks[0].instructions.len(), 1);
        assert_eq!(
            X86Opcode::from_machine_opcode(expanded.blocks[0].instructions[0].opcode),
            Some(X86Opcode::ReturnNear)
        );
        assert!(expanded.blocks[0].instructions[0].operands.is_empty());
    }

    #[test]
    fn restores_an_argument_only_frame_with_pop_bp() {
        let input = function(
            MachineCallingConvention::C,
            vec![MachineValueType::Integer { bits: 16 }],
            vec![incoming(0, 0)],
            false,
        );
        let plan = plan_c_frame(&input).unwrap();
        let expanded = expand_allocated_c_abi(&input, &plan).unwrap();

        assert!(plan.framed());
        assert_eq!(plan.local_bytes(), 0);
        assert_eq!(
            expanded.blocks[0]
                .instructions
                .iter()
                .map(|instruction| X86Opcode::from_machine_opcode(instruction.opcode))
                .collect::<Vec<_>>(),
            vec![
                Some(X86Opcode::Push),
                Some(X86Opcode::Mov),
                Some(X86Opcode::Pop),
                Some(X86Opcode::ReturnNear),
            ]
        );
        assert_eq!(
            expanded.blocks[0].instructions[2].operands,
            vec![physical(X86Register::Bp, OperandRole::Def)]
        );
    }

    #[test]
    fn inserts_bp_shell_and_consumes_allocated_c_abi_operands() {
        let mut input = function(
            MachineCallingConvention::FarCdecl,
            vec![],
            vec![local(9, 3)],
            true,
        );
        input.blocks[0].instructions.insert(
            0,
            MachineInstruction {
                id: MachineInstructionId::new(7),
                opcode: X86Opcode::CallFar.machine_opcode(),
                operands: vec![
                    function_operand(9),
                    physical(X86Register::Ax, OperandRole::Def),
                ],
                flags: InstructionFlags {
                    call: true,
                    ..InstructionFlags::NONE
                },
            },
        );
        let plan = plan_c_frame(&input).unwrap();
        let expanded = expand_allocated_c_abi(&input, &plan).unwrap();
        let instructions = &expanded.blocks[0].instructions;
        assert_eq!(plan.local_bytes(), 4);
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| X86Opcode::from_machine_opcode(instruction.opcode))
                .collect::<Vec<_>>(),
            vec![
                Some(X86Opcode::Push),
                Some(X86Opcode::Mov),
                Some(X86Opcode::Sub),
                Some(X86Opcode::CallFar),
                Some(X86Opcode::Leave),
                Some(X86Opcode::ReturnFar)
            ]
        );
        assert_eq!(instructions[3].operands, vec![function_operand(9)]);
        assert_eq!(instructions.last().unwrap().operands, vec![immediate(0)]);
        assert_eq!(
            instructions[2].operands,
            vec![physical(X86Register::Sp, OperandRole::UseDef), immediate(4),]
        );
    }

    #[test]
    fn expands_allocated_far_cdecl_i32_return_into_dx_ax() {
        // The selected i32 result is split after allocation. It must reach
        // the ABI's DX:AX return pair without leaving pseudos for MC.
        let mut input = function(
            MachineCallingConvention::FarCdecl,
            vec![],
            vec![local(9, 2)],
            true,
        );
        input.signature.result = Some(MachineValueType::Integer { bits: 32 });
        input.blocks[0].instructions[0].operands = vec![
            physical(X86Register::Ax, OperandRole::Use),
            physical(X86Register::Dx, OperandRole::Use),
            immediate(0),
        ];
        input.blocks[0].instructions.insert(
            0,
            MachineInstruction {
                id: MachineInstructionId::new(6),
                opcode: X86Opcode::LowWord.machine_opcode(),
                operands: vec![
                    physical(X86Register::Ax, OperandRole::Def),
                    physical(X86Register::Ecx, OperandRole::Use),
                ],
                flags: InstructionFlags::NONE,
            },
        );
        input.blocks[0].instructions.insert(
            1,
            MachineInstruction {
                id: MachineInstructionId::new(7),
                opcode: X86Opcode::HighWord.machine_opcode(),
                operands: vec![
                    physical(X86Register::Dx, OperandRole::Def),
                    physical(X86Register::Ecx, OperandRole::Use),
                ],
                flags: InstructionFlags::NONE,
            },
        );

        let plan = plan_c_frame(&input).unwrap();
        let expanded = expand_allocated_c_abi(&input, &plan).unwrap();
        let instructions = &expanded.blocks[0].instructions;

        assert!(plan.framed());
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| X86Opcode::from_machine_opcode(instruction.opcode))
                .collect::<Vec<_>>(),
            vec![
                Some(X86Opcode::Push),
                Some(X86Opcode::Mov),
                Some(X86Opcode::Sub),
                Some(X86Opcode::Mov),
                Some(X86Opcode::ShiftLeftDouble),
                Some(X86Opcode::Leave),
                Some(X86Opcode::ReturnFar),
            ]
        );
        assert_eq!(
            instructions[3].operands,
            vec![
                physical(X86Register::Ax, OperandRole::Def),
                physical(X86Register::Cx, OperandRole::Use),
            ]
        );
        assert_eq!(instructions[3].id, MachineInstructionId::new(6));
        assert_eq!(
            instructions[4].operands,
            vec![
                physical(X86Register::Edx, OperandRole::UseDef),
                physical(X86Register::Ecx, OperandRole::Use),
                immediate(16),
            ]
        );
        assert_eq!(instructions[4].id, MachineInstructionId::new(7));
        assert!(instructions.iter().all(|instruction| !matches!(
            X86Opcode::from_machine_opcode(instruction.opcode),
            Some(X86Opcode::LowWord | X86Opcode::HighWord)
        )));
        assert_eq!(instructions.last().unwrap().operands, vec![immediate(0)]);

        // AX already is EAX's low-word view, so its extraction is deletion,
        // while the high extraction still owns the original HighWord ID.
        let mut same_view = input.clone();
        for instruction in &mut same_view.blocks[0].instructions[..2] {
            instruction.operands[1] = physical(X86Register::Eax, OperandRole::Use);
        }
        let same_view_plan = plan_c_frame(&same_view).unwrap();
        let same_view = expand_allocated_c_abi(&same_view, &same_view_plan).unwrap();
        let same_view_instructions = &same_view.blocks[0].instructions;
        assert_eq!(
            same_view_instructions
                .iter()
                .map(|instruction| X86Opcode::from_machine_opcode(instruction.opcode))
                .collect::<Vec<_>>(),
            vec![
                Some(X86Opcode::Push),
                Some(X86Opcode::Mov),
                Some(X86Opcode::Sub),
                Some(X86Opcode::ShiftLeftDouble),
                Some(X86Opcode::Leave),
                Some(X86Opcode::ReturnFar),
            ]
        );
        assert_eq!(same_view_instructions[3].id, MachineInstructionId::new(7));
        assert_eq!(
            same_view_instructions[3].operands,
            vec![
                physical(X86Register::Edx, OperandRole::UseDef),
                physical(X86Register::Eax, OperandRole::Use),
                immediate(16),
            ]
        );
    }

    #[test]
    fn saves_si_and_di_after_a_framed_reservation_and_restores_each_return() {
        // Python masm._frame_parts saves the low 16-bit callee views after
        // the BP/local shell, in SI then DI order. Both exits unwind that
        // pair in reverse before their normal frame teardown.
        let mut input = function(
            MachineCallingConvention::FarCdecl,
            vec![],
            vec![local(0, 2)],
            false,
        );
        input.blocks[0].instructions.insert(
            0,
            instruction(
                MachineInstructionId::new(6),
                X86Opcode::Mov,
                vec![
                    physical(X86Register::Esi, OperandRole::Def),
                    physical(X86Register::Eax, OperandRole::Use),
                ],
            ),
        );
        input.blocks[0].instructions.insert(
            1,
            instruction(
                MachineInstructionId::new(7),
                X86Opcode::Mov,
                vec![
                    physical(X86Register::Edi, OperandRole::Def),
                    physical(X86Register::Ecx, OperandRole::Use),
                ],
            ),
        );
        input.blocks.push(MachineBlock {
            id: MachineBlockId::new(5),
            instructions: vec![MachineInstruction {
                id: MachineInstructionId::new(9),
                opcode: X86Opcode::ReturnFar.machine_opcode(),
                operands: vec![immediate(0)],
                flags: InstructionFlags {
                    terminator: true,
                    ..InstructionFlags::NONE
                },
            }],
            successors: Vec::new(),
        });

        let plan = plan_c_frame(&input).unwrap();
        let expanded = expand_allocated_c_abi(&input, &plan).unwrap();
        let entry = &expanded.blocks[0].instructions;
        assert_eq!(
            entry
                .iter()
                .map(|one| X86Opcode::from_machine_opcode(one.opcode))
                .collect::<Vec<_>>(),
            vec![
                Some(X86Opcode::Push),
                Some(X86Opcode::Mov),
                Some(X86Opcode::Sub),
                Some(X86Opcode::Push),
                Some(X86Opcode::Push),
                Some(X86Opcode::Mov),
                Some(X86Opcode::Mov),
                Some(X86Opcode::Pop),
                Some(X86Opcode::Pop),
                Some(X86Opcode::Leave),
                Some(X86Opcode::ReturnFar),
            ]
        );
        assert_eq!(
            entry[3].operands,
            vec![physical(X86Register::Si, OperandRole::Use)]
        );
        assert_eq!(
            entry[4].operands,
            vec![physical(X86Register::Di, OperandRole::Use)]
        );
        assert_eq!(
            entry[7].operands,
            vec![physical(X86Register::Di, OperandRole::Def)]
        );
        assert_eq!(
            entry[8].operands,
            vec![physical(X86Register::Si, OperandRole::Def)]
        );
        assert_eq!(
            entry.iter().map(|one| one.id.get()).collect::<Vec<_>>(),
            vec![10, 11, 12, 13, 14, 6, 7, 15, 16, 17, 8]
        );

        let second = &expanded.blocks[1].instructions;
        assert_eq!(
            second
                .iter()
                .map(|one| X86Opcode::from_machine_opcode(one.opcode))
                .collect::<Vec<_>>(),
            vec![
                Some(X86Opcode::Pop),
                Some(X86Opcode::Pop),
                Some(X86Opcode::Leave),
                Some(X86Opcode::ReturnFar),
            ]
        );
        assert_eq!(
            second[0].operands,
            vec![physical(X86Register::Di, OperandRole::Def)]
        );
        assert_eq!(
            second[1].operands,
            vec![physical(X86Register::Si, OperandRole::Def)]
        );
        assert_eq!(
            second.iter().map(|one| one.id.get()).collect::<Vec<_>>(),
            vec![18, 19, 20, 9]
        );
    }

    #[test]
    fn saves_only_the_low_si_view_for_an_unframed_esi_user() {
        let mut input = function(MachineCallingConvention::C, vec![], vec![], false);
        input.blocks[0].instructions.insert(
            0,
            instruction(
                MachineInstructionId::new(7),
                X86Opcode::Mov,
                vec![
                    physical(X86Register::Esi, OperandRole::Def),
                    physical(X86Register::Eax, OperandRole::Use),
                ],
            ),
        );

        let plan = plan_c_frame(&input).unwrap();
        let expanded = expand_allocated_c_abi(&input, &plan).unwrap();
        let instructions = &expanded.blocks[0].instructions;
        assert!(!plan.framed());
        assert_eq!(
            instructions
                .iter()
                .map(|one| X86Opcode::from_machine_opcode(one.opcode))
                .collect::<Vec<_>>(),
            vec![
                Some(X86Opcode::Push),
                Some(X86Opcode::Mov),
                Some(X86Opcode::Pop),
                Some(X86Opcode::ReturnNear),
            ]
        );
        assert_eq!(
            instructions[0].operands,
            vec![physical(X86Register::Si, OperandRole::Use)]
        );
        assert_eq!(
            instructions[2].operands,
            vec![physical(X86Register::Si, OperandRole::Def)]
        );
    }

    #[test]
    fn unused_callee_saved_roots_add_no_save_restore_traffic() {
        let input = function(MachineCallingConvention::C, vec![], vec![], false);
        let plan = plan_c_frame(&input).unwrap();
        let expanded = expand_allocated_c_abi(&input, &plan).unwrap();

        assert_eq!(expanded, input);
    }

    #[test]
    fn callee_save_id_exhaustion_refuses_without_mutating_input() {
        let mut input = function(MachineCallingConvention::C, vec![], vec![], false);
        input.blocks[0].instructions.insert(
            0,
            instruction(
                MachineInstructionId::new(7),
                X86Opcode::Mov,
                vec![
                    physical(X86Register::Esi, OperandRole::Def),
                    physical(X86Register::Eax, OperandRole::Use),
                ],
            ),
        );
        input.blocks[0].instructions[1].id = MachineInstructionId::new(u32::MAX);
        let baseline = input.clone();
        let plan = plan_c_frame(&input).unwrap();

        assert_eq!(
            expand_allocated_c_abi(&input, &plan),
            Err(CAbiExpansionError::InstructionIdExhausted)
        );
        assert_eq!(input, baseline);
    }
}
