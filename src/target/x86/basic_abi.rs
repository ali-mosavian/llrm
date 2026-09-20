//! Expansion of the measured Microsoft BASIC runtime frame protocol.
//!
//! Frame planning is deliberately separate in [`super::frame`].  This module
//! consumes that immutable plan and produces a new Machine-IR function with
//! the `B$ENRA`/`B$EXSA` shell around its already-selected body.  It neither
//! assigns physical stack offsets nor changes the supplied function.

use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    InstructionFlags, MachineBlockId, MachineFunction, MachineInstruction, MachineInstructionId,
    MachineOperand, MachineOperandKind, MachineRegister, OperandRole, RegisterConstraint,
    VirtualRegister, VirtualRegisterId,
};

use super::{
    BasicFramePlan, BasicFramePlanError, BasicRuntime, X86Opcode, X86Register, X86RegisterClass,
    plan_basic_frame,
};

/// A BASIC function together with the immutable frame plan that shaped it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandedBasicFunction {
    pub function: MachineFunction,
    pub frame: BasicFramePlan,
}

/// A refusal while expanding the target-owned BASIC runtime protocol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BasicAbiError {
    Frame(BasicFramePlanError),
    EmptyFunction,
    UnknownEntry(MachineBlockId),
    ReturnNear {
        block: MachineBlockId,
        instruction: MachineInstructionId,
    },
    AlreadyExpanded {
        symbol: &'static str,
    },
    VirtualRegisterIdExhausted,
    InstructionIdExhausted,
}

impl fmt::Display for BasicAbiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => error.fmt(formatter),
            Self::EmptyFunction => write!(formatter, "BASIC runtime frame has no entry block"),
            Self::UnknownEntry(block) => {
                write!(
                    formatter,
                    "BASIC runtime entry block {block} is not in the function"
                )
            }
            Self::ReturnNear { block, instruction } => write!(
                formatter,
                "BASIC runtime frame cannot wrap near return {instruction} in block {block}"
            ),
            Self::AlreadyExpanded { symbol } => {
                write!(formatter, "BASIC runtime frame already contains {symbol}")
            }
            Self::VirtualRegisterIdExhausted => {
                write!(
                    formatter,
                    "BASIC runtime frame exhausted virtual-register IDs"
                )
            }
            Self::InstructionIdExhausted => {
                write!(formatter, "BASIC runtime frame exhausted instruction IDs")
            }
        }
    }
}

impl Error for BasicAbiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            Self::EmptyFunction
            | Self::UnknownEntry(_)
            | Self::ReturnNear { .. }
            | Self::AlreadyExpanded { .. }
            | Self::VirtualRegisterIdExhausted
            | Self::InstructionIdExhausted => None,
        }
    }
}

/// Expands one far-Pascal BASIC procedure with its runtime frame calls.
///
/// The input remains unchanged.  The first two fresh word virtual registers
/// are pinned to CX and BX for `B$ENRA`; each `B$EXSA` reuses the fixed
/// virtual-register return operands from the following `ReturnFar`, keeping a
/// LONG result alive in AX:DX through frame teardown.
pub fn expand_basic_runtime(
    function: &MachineFunction,
    entry: MachineBlockId,
    runtime: BasicRuntime,
    temporary_strings: u32,
) -> Result<ExpandedBasicFunction, BasicAbiError> {
    let frame =
        plan_basic_frame(function, runtime, temporary_strings).map_err(BasicAbiError::Frame)?;
    preflight(function, entry)?;

    let [cx, bx] = reserve_virtual_register_ids(function)?;
    let return_count = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction.opcode == X86Opcode::ReturnFar.machine_opcode())
        .count();
    let added_instructions = 3_usize
        .checked_add(return_count)
        .ok_or(BasicAbiError::InstructionIdExhausted)?;
    let mut instruction_ids = reserve_instruction_ids(function, added_instructions)?.into_iter();

    let enter = [
        instruction(
            instruction_ids
                .next()
                .expect("three entry IDs were reserved"),
            X86Opcode::Mov,
            vec![
                fixed_virtual(cx, OperandRole::Def, X86Register::Cx),
                immediate(i64::from(frame.local_bytes())),
            ],
            InstructionFlags::NONE,
        ),
        instruction(
            instruction_ids
                .next()
                .expect("three entry IDs were reserved"),
            X86Opcode::Mov,
            vec![
                fixed_virtual(bx, OperandRole::Def, X86Register::Bx),
                immediate(i64::from(frame.temporary_strings())),
            ],
            InstructionFlags::NONE,
        ),
        instruction(
            instruction_ids
                .next()
                .expect("three entry IDs were reserved"),
            X86Opcode::CallFar,
            vec![
                external_symbol("B$ENRA"),
                fixed_virtual(cx, OperandRole::Use, X86Register::Cx),
                fixed_virtual(bx, OperandRole::Use, X86Register::Bx),
            ],
            runtime_call_flags(),
        ),
    ];

    // All refusal paths above run before cloning, preserving the input even
    // when a malformed body or exhausted ID space is encountered.
    let mut expanded = function.clone();
    expanded.virtual_registers.extend([
        VirtualRegister {
            id: cx,
            class: X86RegisterClass::Word.machine_class(),
        },
        VirtualRegister {
            id: bx,
            class: X86RegisterClass::Word.machine_class(),
        },
    ]);

    for block in &mut expanded.blocks {
        if block.id == entry {
            block.instructions.splice(0..0, enter.clone());
        }

        let mut instructions = Vec::with_capacity(block.instructions.len());
        for original in std::mem::take(&mut block.instructions) {
            if original.opcode == X86Opcode::ReturnFar.machine_opcode() {
                let mut operands = Vec::with_capacity(original.operands.len());
                operands.push(external_symbol("B$EXSA"));
                operands.extend(return_value_uses(&original));
                instructions.push(instruction(
                    instruction_ids
                        .next()
                        .expect("one exit ID was reserved per far return"),
                    X86Opcode::CallFar,
                    operands,
                    runtime_call_flags(),
                ));
            }
            instructions.push(original);
        }
        block.instructions = instructions;
    }

    Ok(ExpandedBasicFunction {
        function: expanded,
        frame,
    })
}

fn preflight(function: &MachineFunction, entry: MachineBlockId) -> Result<(), BasicAbiError> {
    if function.blocks.is_empty() {
        return Err(BasicAbiError::EmptyFunction);
    }
    if !function.blocks.iter().any(|block| block.id == entry) {
        return Err(BasicAbiError::UnknownEntry(entry));
    }

    for block in &function.blocks {
        for instruction in &block.instructions {
            if instruction.opcode == X86Opcode::ReturnNear.machine_opcode() {
                return Err(BasicAbiError::ReturnNear {
                    block: block.id,
                    instruction: instruction.id,
                });
            }
            if instruction.opcode != X86Opcode::CallFar.machine_opcode() {
                continue;
            }
            let Some(MachineOperand {
                kind: MachineOperandKind::ExternalSymbol { name, .. },
                ..
            }) = instruction.operands.first()
            else {
                continue;
            };
            if name == "B$ENRA" || name == "B$EXSA" {
                return Err(BasicAbiError::AlreadyExpanded {
                    symbol: if name == "B$ENRA" { "B$ENRA" } else { "B$EXSA" },
                });
            }
        }
    }
    Ok(())
}

fn reserve_virtual_register_ids(
    function: &MachineFunction,
) -> Result<[VirtualRegisterId; 2], BasicAbiError> {
    let first = function
        .virtual_registers
        .iter()
        .map(|register| register.id)
        .chain(function.blocks.iter().flat_map(|block| {
            block.instructions.iter().flat_map(|instruction| {
                instruction
                    .operands
                    .iter()
                    .filter_map(|operand| match operand.kind {
                        MachineOperandKind::Register(MachineRegister::Virtual(id)) => Some(id),
                        _ => None,
                    })
            })
        }))
        .map(VirtualRegisterId::get)
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1)
                .ok_or(BasicAbiError::VirtualRegisterIdExhausted)
        })?;
    let second = first
        .checked_add(1)
        .ok_or(BasicAbiError::VirtualRegisterIdExhausted)?;
    Ok([
        VirtualRegisterId::new(first),
        VirtualRegisterId::new(second),
    ])
}

fn reserve_instruction_ids(
    function: &MachineFunction,
    count: usize,
) -> Result<Vec<MachineInstructionId>, BasicAbiError> {
    let first = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .map(|instruction| instruction.id.get())
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1)
                .ok_or(BasicAbiError::InstructionIdExhausted)
        })?;
    let count = u32::try_from(count).map_err(|_| BasicAbiError::InstructionIdExhausted)?;
    let additional = count
        .checked_sub(1)
        .ok_or(BasicAbiError::InstructionIdExhausted)?;
    let last = first
        .checked_add(additional)
        .ok_or(BasicAbiError::InstructionIdExhausted)?;
    Ok((first..=last).map(MachineInstructionId::new).collect())
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

fn fixed_virtual(
    register: VirtualRegisterId,
    role: OperandRole,
    physical: X86Register,
) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Register(MachineRegister::Virtual(register)),
        role,
        constraint: Some(RegisterConstraint::Fixed(physical.physical())),
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

fn external_symbol(name: &str) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::ExternalSymbol {
            name: name.to_owned(),
            addend: 0,
        },
        role: OperandRole::None,
        constraint: None,
        tied_to: None,
    }
}

fn runtime_call_flags() -> InstructionFlags {
    InstructionFlags {
        call: true,
        side_effects: true,
        may_load: true,
        may_store: true,
        ..InstructionFlags::NONE
    }
}

fn return_value_uses(return_far: &MachineInstruction) -> impl Iterator<Item = MachineOperand> + '_ {
    return_far.operands.iter().filter_map(|operand| {
        matches!(
            operand,
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(_)),
                role: OperandRole::Use,
                constraint: Some(RegisterConstraint::Fixed(_)),
                ..
            }
        )
        .then(|| operand.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        FrameIndex, FrameObject, FrameObjectKind, MachineBlock, MachineCallingConvention,
        MachineFunctionId, MachineLinkage, MachineSignature, MachineValueType,
    };

    fn procedure(blocks: Vec<MachineBlock>) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(7),
            name: "twice".into(),
            linkage: MachineLinkage::External,
            signature: MachineSignature {
                result: Some(MachineValueType::Integer { bits: 32 }),
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::Basic,
            },
            virtual_registers: vec![
                VirtualRegister {
                    id: VirtualRegisterId::new(4),
                    class: X86RegisterClass::Word.machine_class(),
                },
                VirtualRegister {
                    id: VirtualRegisterId::new(5),
                    class: X86RegisterClass::Word.machine_class(),
                },
            ],
            blocks,
            frame_objects: vec![FrameObject {
                index: FrameIndex::new(0),
                size: 4,
                alignment: 2,
                kind: FrameObjectKind::Local,
            }],
        }
    }

    fn block(id: u32, instructions: Vec<MachineInstruction>) -> MachineBlock {
        MachineBlock {
            id: MachineBlockId::new(id),
            instructions,
            successors: Vec::new(),
        }
    }

    fn return_far(id: u32) -> MachineInstruction {
        instruction(
            MachineInstructionId::new(id),
            X86Opcode::ReturnFar,
            vec![
                fixed_virtual(VirtualRegisterId::new(4), OperandRole::Use, X86Register::Ax),
                fixed_virtual(VirtualRegisterId::new(5), OperandRole::Use, X86Register::Dx),
                immediate(0),
            ],
            InstructionFlags {
                terminator: true,
                ..InstructionFlags::NONE
            },
        )
    }

    #[test]
    fn inserts_runtime_entry_at_the_explicit_entry_before_incoming_loads() {
        // PDS emitted B$ENRA before its formal loads.  Inserting it into the
        // first stored block instead made multi-entry procedures read a caller
        // frame before the runtime had installed the BASIC frame chain.
        let incoming_load = instruction(
            MachineInstructionId::new(8),
            X86Opcode::Load,
            vec![
                MachineOperand {
                    kind: MachineOperandKind::Register(MachineRegister::Virtual(
                        VirtualRegisterId::new(4),
                    )),
                    role: OperandRole::Def,
                    constraint: None,
                    tied_to: None,
                },
                MachineOperand {
                    kind: MachineOperandKind::FrameIndex {
                        index: FrameIndex::new(0),
                        addend: 0,
                    },
                    role: OperandRole::None,
                    constraint: None,
                    tied_to: None,
                },
            ],
            InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            },
        );
        let function = procedure(vec![
            block(1, vec![return_far(9)]),
            block(4, vec![incoming_load, return_far(10)]),
        ]);

        let expanded =
            expand_basic_runtime(&function, MachineBlockId::new(4), BasicRuntime::Pds71, 3)
                .expect("a selected far-Pascal procedure expands");

        assert_eq!(expanded.frame.local_bytes(), 4);
        assert_eq!(expanded.frame.temporary_strings(), 3);
        assert_eq!(
            expanded.function.blocks[0].instructions[0].opcode,
            X86Opcode::CallFar.machine_opcode(),
            "the non-entry block remains untouched at its front"
        );
        let entry = &expanded.function.blocks[1].instructions;
        assert_eq!(entry[0].opcode, X86Opcode::Mov.machine_opcode());
        assert_eq!(entry[1].opcode, X86Opcode::Mov.machine_opcode());
        assert_eq!(entry[2].opcode, X86Opcode::CallFar.machine_opcode());
        assert_eq!(entry[3].id, MachineInstructionId::new(8));
        assert!(matches!(
            entry[0].operands[1].kind,
            MachineOperandKind::Immediate(4)
        ));
        assert!(matches!(
            entry[1].operands[1].kind,
            MachineOperandKind::Immediate(3)
        ));
        assert_eq!(entry[2].flags, runtime_call_flags());
    }

    #[test]
    fn tears_down_before_each_far_return_without_losing_a_long_result() {
        // LONG function results occupy AX:DX.  Calling B$EXSA without these
        // constrained uses once let allocation destroy the value immediately
        // before RETF, producing the wrong BASIC result after frame teardown.
        let function = procedure(vec![
            block(0, vec![return_far(2)]),
            block(1, vec![return_far(3)]),
        ]);

        let expanded =
            expand_basic_runtime(&function, MachineBlockId::new(0), BasicRuntime::Qb45, 0)
                .expect("both exits receive runtime teardown");
        for block in &expanded.function.blocks {
            let return_far = block.instructions.last().unwrap();
            let exit = &block.instructions[block.instructions.len() - 2];
            assert_eq!(exit.opcode, X86Opcode::CallFar.machine_opcode());
            assert!(matches!(
                &exit.operands[0].kind,
                MachineOperandKind::ExternalSymbol { name, .. } if name == "B$EXSA"
            ));
            assert_eq!(&exit.operands[1..], &return_far.operands[..2]);
            assert_eq!(exit.flags, runtime_call_flags());
        }
    }

    #[test]
    fn preserves_the_source_function_and_reports_expansion_boundaries() {
        // Reapplying the shell shifted locals below an extra runtime header;
        // failure must be explicit instead of producing a subtly corrupted BP
        // layout.
        let function = procedure(vec![block(0, vec![return_far(2)])]);
        let original = function.clone();
        let expanded =
            expand_basic_runtime(&function, MachineBlockId::new(0), BasicRuntime::Vbdos, 0)
                .expect("initial expansion succeeds");

        assert_eq!(function, original);
        assert!(matches!(
            expand_basic_runtime(
                &expanded.function,
                MachineBlockId::new(0),
                BasicRuntime::Vbdos,
                0
            ),
            Err(BasicAbiError::AlreadyExpanded { .. })
        ));
        assert!(matches!(
            expand_basic_runtime(&function, MachineBlockId::new(99), BasicRuntime::Vbdos, 0),
            Err(BasicAbiError::UnknownEntry(block)) if block == MachineBlockId::new(99)
        ));
    }

    #[test]
    fn refuses_near_returns_and_exhausted_ids() {
        // A native RET would bypass B$EXSA and leave the runtime's frame chain
        // corrupted; refusing it is safer than emitting a mixed ABI body.
        let near = instruction(
            MachineInstructionId::new(2),
            X86Opcode::ReturnNear,
            Vec::new(),
            InstructionFlags {
                terminator: true,
                ..InstructionFlags::NONE
            },
        );
        let near_function = procedure(vec![block(0, vec![near])]);
        assert!(matches!(
            expand_basic_runtime(
                &near_function,
                MachineBlockId::new(0),
                BasicRuntime::Qb45,
                0
            ),
            Err(BasicAbiError::ReturnNear { .. })
        ));

        let mut exhausted = procedure(vec![block(0, vec![return_far(u32::MAX)])]);
        exhausted.virtual_registers[1].id = VirtualRegisterId::new(u32::MAX);
        assert!(matches!(
            expand_basic_runtime(&exhausted, MachineBlockId::new(0), BasicRuntime::Qb45, 0),
            Err(BasicAbiError::VirtualRegisterIdExhausted)
        ));

        let instruction_exhausted = procedure(vec![block(0, vec![return_far(u32::MAX)])]);
        assert!(matches!(
            expand_basic_runtime(
                &instruction_exhausted,
                MachineBlockId::new(0),
                BasicRuntime::Qb45,
                0
            ),
            Err(BasicAbiError::InstructionIdExhausted)
        ));

        let mut terminal_ids = procedure(vec![block(0, Vec::new())]);
        terminal_ids.virtual_registers[0].id = VirtualRegisterId::new(u32::MAX - 2);
        terminal_ids.virtual_registers[1].id = VirtualRegisterId::new(5);
        assert_eq!(
            reserve_virtual_register_ids(&terminal_ids).unwrap(),
            [
                VirtualRegisterId::new(u32::MAX - 1),
                VirtualRegisterId::new(u32::MAX)
            ]
        );
    }

    #[test]
    fn accepts_a_no_return_unreachable_basic_body() {
        let function = procedure(vec![block(0, Vec::new())]);
        let expanded =
            expand_basic_runtime(&function, MachineBlockId::new(0), BasicRuntime::Qb45, 0)
                .expect("a noreturn body still needs entry frame setup");
        assert_eq!(expanded.function.blocks[0].instructions.len(), 3);
    }

    #[test]
    fn refuses_an_empty_function_instead_of_inventing_an_entry() {
        // Inventing a synthetic entry for a body with no blocks concealed a
        // malformed compiler boundary and left B$ENRA unreachable.
        let function = procedure(Vec::new());
        assert_eq!(
            expand_basic_runtime(&function, MachineBlockId::new(0), BasicRuntime::Qb45, 0),
            Err(BasicAbiError::EmptyFunction)
        );
    }
}
