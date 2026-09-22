//! x87 stack allocation for selected x86 Machine IR.
//!
//! The x87 register file is a stack, not a set of independently allocatable
//! registers.  It is therefore deliberately allocated here, before the
//! generic graph allocator sees the remaining general-purpose virtual
//! registers.  The rules are a direct port of the stack mechanics in
//! `qbopt/backend/floatalloc.py`: `_Stack.room`, `_Stack.copy`, `_Stack.store`,
//! `_Stack.compare`, and `_Stack.arithmetic`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

use crate::old::codegen::machine::{
    BlockLiveness, FrameIndex, FrameObject, FrameObjectKind, InstructionFlags, MachineBlock,
    MachineBlockId, MachineFunction, MachineInstruction, MachineInstructionId,
    MachineLivenessError, MachineOperand, MachineOperandKind, MachineRegister, OperandRole,
    RegisterConstraint, VirtualRegister, VirtualRegisterId, compute_liveness,
};

use super::{X86Cpu, X86FloatCosts, X86Opcode, X86Register, X86RegisterClass, X87MemoryFormat};

/// A target-specific refusal while assigning x87 virtual registers to the
/// architectural floating-point stack.
///
/// Every variant carries the function, block, and instruction which exposed
/// the unsupported fact.  The `value` field is present whenever one virtual
/// x87 value caused it; callers can therefore report a useful diagnostic
/// without parsing an error string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86FloatAllocationError {
    InvalidMachineIr {
        function: crate::old::codegen::machine::MachineFunctionId,
        errors: Vec<MachineLivenessError>,
    },
    MissingEntryBlock {
        function: crate::old::codegen::machine::MachineFunctionId,
    },
    UnreachableX87Use {
        function: crate::old::codegen::machine::MachineFunctionId,
        block: MachineBlockId,
        instruction: MachineInstructionId,
        value: VirtualRegisterId,
    },
    LiveAcrossControlFlow {
        function: crate::old::codegen::machine::MachineFunctionId,
        block: MachineBlockId,
        instruction: Option<MachineInstructionId>,
        successor: Option<MachineBlockId>,
        value: VirtualRegisterId,
    },
    MalformedInstruction {
        function: crate::old::codegen::machine::MachineFunctionId,
        block: MachineBlockId,
        instruction: MachineInstructionId,
        value: Option<VirtualRegisterId>,
        reason: &'static str,
    },
    UnsupportedInstruction {
        function: crate::old::codegen::machine::MachineFunctionId,
        block: MachineBlockId,
        instruction: MachineInstructionId,
        value: Option<VirtualRegisterId>,
        opcode: X86Opcode,
    },
    UnavailableValue {
        function: crate::old::codegen::machine::MachineFunctionId,
        block: MachineBlockId,
        instruction: MachineInstructionId,
        value: VirtualRegisterId,
    },
    StackOverflow {
        function: crate::old::codegen::machine::MachineFunctionId,
        block: MachineBlockId,
        instruction: MachineInstructionId,
        value: Option<VirtualRegisterId>,
    },
    IdExhausted {
        function: crate::old::codegen::machine::MachineFunctionId,
        block: MachineBlockId,
        instruction: MachineInstructionId,
    },
}

impl fmt::Display for X86FloatAllocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMachineIr { function, errors } => write!(
                formatter,
                "x87 stack allocation cannot analyse function {function}: {} Machine IR error(s)",
                errors.len()
            ),
            Self::MissingEntryBlock { function } => {
                write!(
                    formatter,
                    "x87 stack allocation cannot find entry block in function {function}"
                )
            }
            Self::UnreachableX87Use {
                function,
                block,
                instruction,
                value,
            } => write!(
                formatter,
                "unreachable block {block}, instruction {instruction} in function {function} uses x87 value {value}"
            ),
            Self::LiveAcrossControlFlow {
                function,
                block,
                instruction,
                successor,
                value,
            } => write!(
                formatter,
                "x87 value {value} is live across unsupported control-flow boundary in function {function}, block {block}, instruction {instruction:?}, successor {successor:?}"
            ),
            Self::MalformedInstruction {
                function,
                block,
                instruction,
                value,
                reason,
            } => write!(
                formatter,
                "malformed x87 instruction {instruction} in function {function}, block {block}, value {value:?}: {reason}"
            ),
            Self::UnsupportedInstruction {
                function,
                block,
                instruction,
                value,
                opcode,
            } => write!(
                formatter,
                "x87 stack allocation has no rule for {opcode:?} at function {function}, block {block}, instruction {instruction}, value {value:?}"
            ),
            Self::UnavailableValue {
                function,
                block,
                instruction,
                value,
            } => write!(
                formatter,
                "x87 value {value} is unavailable at function {function}, block {block}, instruction {instruction}"
            ),
            Self::StackOverflow {
                function,
                block,
                instruction,
                value,
            } => write!(
                formatter,
                "x87 stack needs more than eight values at function {function}, block {block}, instruction {instruction}, value {value:?}"
            ),
            Self::IdExhausted {
                function,
                block,
                instruction,
            } => write!(
                formatter,
                "cannot allocate an x87 temporary identifier at function {function}, block {block}, instruction {instruction}"
            ),
        }
    }
}

impl Error for X86FloatAllocationError {}

/// Stackifies all `X87` virtual registers in `function`.
///
/// Unique straight-line chains retain their stack state.  Calls, opaque
/// instructions, joins, forks, and loop edges use deterministic m80 bridge
/// homes, matching Python `floatregions.bridged` rather than assuming x87
/// state survives a region boundary.
pub fn allocate_x87_stack(
    function: &MachineFunction,
) -> Result<MachineFunction, X86FloatAllocationError> {
    allocate_x87_stack_with_costs(function, X86Cpu::I386.float_costs())
}

/// Stackifies with explicit target ranking costs.  This is public for
/// profile-parity tests; production uses the established i386 default above.
pub fn allocate_x87_stack_with_costs(
    function: &MachineFunction,
    costs: X86FloatCosts,
) -> Result<MachineFunction, X86FloatAllocationError> {
    allocate_x87_stack_once(function, costs)
}

fn allocate_x87_stack_once(
    function: &MachineFunction,
    costs: X86FloatCosts,
) -> Result<MachineFunction, X86FloatAllocationError> {
    let x87 = function
        .virtual_registers
        .iter()
        .filter(|register| register.class == X86RegisterClass::X87.machine_class())
        .map(|register| register.id)
        .collect::<BTreeSet<_>>();
    if x87.is_empty() {
        return Ok(function.clone());
    }

    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    if !blocks.contains_key(&function.entry) {
        return Err(X86FloatAllocationError::MissingEntryBlock {
            function: function.id,
        });
    }

    let reachable = reachable_blocks(function.entry, &blocks);

    let predecessors = reachable_predecessors(&function.blocks, &reachable);
    let follows = function
        .blocks
        .iter()
        .filter_map(|block| {
            let successor = *block.successors.first()?;
            let unique = reachable.contains(&block.id)
                && block.successors.len() == 1
                && successor != function.entry
                && reachable.contains(&successor)
                && block
                    .instructions
                    .iter()
                    .enumerate()
                    .all(|(index, instruction)| {
                        !is_hard_boundary(instruction)
                            || (index + 1 == block.instructions.len()
                                && instruction.flags.terminator
                                && !instruction.flags.call)
                    })
                && predecessors
                    .get(&successor)
                    .is_some_and(|items| items.len() == 1 && items.contains(&block.id));
            unique.then_some((block.id, successor))
        })
        .collect::<BTreeMap<_, _>>();

    // `floatregions.bridged` does not dump an arbitrary current stack at a
    // boundary.  It owns one m80 cell per crossing SSA value, stores the
    // value directly after its definition, and gives every consuming region
    // a fresh local reload.  Doing this before stack allocation makes each
    // remaining region genuinely empty at its boundary.
    let function = bridge_x87_regions(function, &reachable, &follows, &x87)?;
    let x87 = function
        .virtual_registers
        .iter()
        .filter(|register| register.class == X86RegisterClass::X87.machine_class())
        .map(|register| register.id)
        .collect::<BTreeSet<_>>();
    let liveness = compute_liveness(&function).map_err(|errors| {
        X86FloatAllocationError::InvalidMachineIr {
            function: function.id,
            errors,
        }
    })?;
    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let reachable = reachable_blocks(function.entry, &blocks);
    let predecessors = reachable_predecessors(&function.blocks, &reachable);
    let follows = straight_follows(&function, &reachable, &predecessors);
    let live_after = x87_live_after(&function, &liveness.blocks, &x87);
    let crossing = bridge_values(
        &function,
        &reachable,
        &follows,
        &liveness.blocks,
        &live_after,
        &x87,
    );
    if let Some(value) = crossing.iter().next().copied() {
        return Err(X86FloatAllocationError::LiveAcrossControlFlow {
            function: function.id,
            block: function.entry,
            instruction: None,
            successor: None,
            value,
        });
    }

    let destinations = follows.values().copied().collect::<BTreeSet<_>>();
    let mut roots = function
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id) && !destinations.contains(&block.id))
        .map(|block| block.id)
        .collect::<Vec<_>>();
    roots.sort();

    let mut state = FunctionState::new(&function, x87);
    let mut output = BTreeMap::<MachineBlockId, Vec<MachineInstruction>>::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        let mut chain = Vec::new();
        let mut current = root;
        while seen.insert(current) {
            chain.push(blocks[&current]);
            let Some(next) = follows.get(&current).copied() else {
                break;
            };
            current = next;
        }
        process_candidate_chain(&mut state, &chain, &live_after, costs, &mut output)?;
    }

    // Python schedules only reachable edges for stack facts, but it retains
    // dead blocks in deterministic layout order.  Stackify self-contained
    // dead code as separate regions rather than treating it as a CFG input.
    for block in &function.blocks {
        if output.contains_key(&block.id) {
            continue;
        }
        process_candidate_chain(&mut state, &[block], &live_after, costs, &mut output)?;
    }

    let blocks = function
        .blocks
        .iter()
        .map(|block| MachineBlock {
            id: block.id,
            instructions: output
                .remove(&block.id)
                .unwrap_or_else(|| block.instructions.clone()),
            successors: block.successors.clone(),
        })
        .collect();
    let result = MachineFunction {
        virtual_registers: function
            .virtual_registers
            .iter()
            .copied()
            .filter(|register| !state.x87.contains(&register.id))
            .collect(),
        frame_objects: state.frame_objects,
        blocks,
        ..function.clone()
    };
    if let Some((block, instruction, value)) = remaining_x87_virtual(&result, &state.x87) {
        return Err(X86FloatAllocationError::MalformedInstruction {
            function: result.id,
            block,
            instruction,
            value: Some(value),
            reason: "stackifier left an x87 virtual register operand",
        });
    }
    Ok(result)
}

fn reachable_blocks(
    entry: MachineBlockId,
    blocks: &BTreeMap<MachineBlockId, &MachineBlock>,
) -> BTreeSet<MachineBlockId> {
    let mut reached = BTreeSet::new();
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        if !reached.insert(block) {
            continue;
        }
        if let Some(item) = blocks.get(&block) {
            pending.extend(item.successors.iter().copied());
        }
    }
    reached
}

fn reachable_predecessors(
    blocks: &[MachineBlock],
    reachable: &BTreeSet<MachineBlockId>,
) -> BTreeMap<MachineBlockId, BTreeSet<MachineBlockId>> {
    let mut result = BTreeMap::new();
    for block in blocks {
        if !reachable.contains(&block.id) {
            continue;
        }
        for successor in &block.successors {
            if reachable.contains(successor) {
                result
                    .entry(*successor)
                    .or_insert_with(BTreeSet::new)
                    .insert(block.id);
            }
        }
    }
    result
}

fn straight_follows(
    function: &MachineFunction,
    reachable: &BTreeSet<MachineBlockId>,
    predecessors: &BTreeMap<MachineBlockId, BTreeSet<MachineBlockId>>,
) -> BTreeMap<MachineBlockId, MachineBlockId> {
    function
        .blocks
        .iter()
        .filter_map(|block| {
            let successor = *block.successors.first()?;
            let unique = reachable.contains(&block.id)
                && block.successors.len() == 1
                && successor != function.entry
                && reachable.contains(&successor)
                && block
                    .instructions
                    .iter()
                    .enumerate()
                    .all(|(index, instruction)| {
                        !is_hard_boundary(instruction)
                            || (index + 1 == block.instructions.len()
                                && instruction.flags.terminator
                                && !instruction.flags.call)
                    })
                && predecessors
                    .get(&successor)
                    .is_some_and(|items| items.len() == 1 && items.contains(&block.id));
            unique.then_some((block.id, successor))
        })
        .collect()
}

/// Materializes Python `floatregions.bridged` over selected Machine IR.
///
/// A crossing extended value is stored into its own m80 home immediately
/// after its sole definition; each region which reads it gets a fresh local
/// `fld` before that read.  In particular, this is intentionally not a
/// boundary-time dump of whichever values happen to remain on the x87 stack.
fn bridge_x87_regions(
    function: &MachineFunction,
    reachable: &BTreeSet<MachineBlockId>,
    follows: &BTreeMap<MachineBlockId, MachineBlockId>,
    x87: &BTreeSet<VirtualRegisterId>,
) -> Result<MachineFunction, X86FloatAllocationError> {
    let labels = x87_region_labels(function, reachable, follows);
    let mut definitions = BTreeMap::<VirtualRegisterId, Vec<(MachineBlockId, usize)>>::new();
    let mut readers = BTreeMap::<VirtualRegisterId, Vec<(MachineBlockId, usize)>>::new();
    for block in &function.blocks {
        for (index, instruction) in block.instructions.iter().enumerate() {
            for operand in &instruction.operands {
                let MachineOperandKind::Register(MachineRegister::Virtual(value)) = operand.kind
                else {
                    continue;
                };
                if !x87.contains(&value) {
                    continue;
                }
                if operand.role.writes() {
                    definitions
                        .entry(value)
                        .or_default()
                        .push((block.id, index));
                }
                if operand.role.reads() {
                    readers.entry(value).or_default().push((block.id, index));
                }
            }
        }
    }
    let crossing = definitions
        .iter()
        .filter_map(|(&value, locations)| {
            let definition = *locations.first()?;
            readers
                .get(&value)
                .is_some_and(|uses| {
                    uses.iter()
                        .any(|use_at| labels[use_at] != labels[&definition])
                })
                .then_some(value)
        })
        .collect::<BTreeSet<_>>();
    if crossing.is_empty() {
        return Ok(function.clone());
    }

    let dominators = machine_dominators(function, reachable);
    for &value in &crossing {
        let Some(locations) = definitions.get(&value) else {
            return Err(X86FloatAllocationError::MalformedInstruction {
                function: function.id,
                block: function.entry,
                instruction: MachineInstructionId::new(0),
                value: Some(value),
                reason: "crossing x87 value has no definition",
            });
        };
        let Some(&(defined_block, defined_index)) = locations.first() else {
            return Err(X86FloatAllocationError::MalformedInstruction {
                function: function.id,
                block: function.entry,
                instruction: MachineInstructionId::new(0),
                value: Some(value),
                reason: "crossing x87 value has no definition",
            });
        };
        let definition_instruction = function
            .blocks
            .iter()
            .find(|block| block.id == defined_block)
            .and_then(|block| block.instructions.get(defined_index))
            .map_or(MachineInstructionId::new(0), |instruction| instruction.id);
        if locations.len() != 1 {
            return Err(X86FloatAllocationError::MalformedInstruction {
                function: function.id,
                block: defined_block,
                instruction: definition_instruction,
                value: Some(value),
                reason: "crossing x87 value requires one SSA definition",
            });
        }
        for &(used_block, used_index) in readers.get(&value).into_iter().flatten() {
            let dominated = defined_block == used_block && defined_index < used_index
                || dominators
                    .get(&used_block)
                    .is_some_and(|items| items.contains(&defined_block));
            if !dominated {
                return Err(X86FloatAllocationError::MalformedInstruction {
                    function: function.id,
                    block: used_block,
                    instruction: function
                        .blocks
                        .iter()
                        .find(|block| block.id == used_block)
                        .and_then(|block| block.instructions.get(used_index))
                        .map_or(MachineInstructionId::new(0), |instruction| instruction.id),
                    value: Some(value),
                    reason: "crossing x87 use is not dominated by its definition",
                });
            }
        }
    }

    let mut next_frame = next_frame_index(function);
    let mut frames = function.frame_objects.clone();
    let mut homes = BTreeMap::new();
    for &value in &crossing {
        let Some(raw) = next_frame.take() else {
            let (block, index) = definitions[&value][0];
            return Err(bridge_id_exhausted(function, block, index));
        };
        next_frame = raw.checked_add(1);
        let index = FrameIndex::new(raw);
        frames.push(FrameObject {
            index,
            size: 10,
            alignment: 2,
            kind: FrameObjectKind::Temporary,
        });
        homes.insert(value, index);
    }
    let mut next_virtual = function
        .virtual_registers
        .iter()
        .map(|register| register.id.get())
        .max()
        .map(|id| id.checked_add(1))
        .unwrap_or(Some(0));
    let mut virtual_registers = function.virtual_registers.clone();
    let mut next_instruction = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .map(|instruction| instruction.id.get())
        .max()
        .map(|id| id.checked_add(1))
        .unwrap_or(Some(0));
    let mut blocks = Vec::with_capacity(function.blocks.len());
    for block in &function.blocks {
        let mut resident = BTreeMap::<VirtualRegisterId, VirtualRegisterId>::new();
        let mut instructions = Vec::new();
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            if region_boundary(
                instruction,
                follows.contains_key(&block.id)
                    && instruction_index + 1 == block.instructions.len(),
            ) {
                resident.clear();
            }
            let mut rewritten = instruction.clone();
            for operand in &mut rewritten.operands {
                let MachineOperandKind::Register(MachineRegister::Virtual(value)) = operand.kind
                else {
                    continue;
                };
                if !crossing.contains(&value) || !operand.role.reads() {
                    continue;
                }
                let local = if let Some(local) = resident.get(&value).copied() {
                    local
                } else {
                    let Some(raw) = next_virtual.take() else {
                        return Err(bridge_id_exhausted(function, block.id, 0));
                    };
                    next_virtual = raw.checked_add(1);
                    let local = VirtualRegisterId::new(raw);
                    virtual_registers.push(VirtualRegister {
                        id: local,
                        class: X86RegisterClass::X87.machine_class(),
                    });
                    let id = next_generated_instruction(
                        &mut next_instruction,
                        function,
                        block.id,
                        instruction.id,
                    )?;
                    let home = Home {
                        reload_opcode: X86Opcode::X87Load,
                        format: X87MemoryFormat::Float80,
                        address: vec![frame_operand(homes[&value])],
                        direct: false,
                    };
                    let mut operands = vec![
                        virtual_x87(local, OperandRole::Def),
                        format_operand(home.format),
                    ];
                    operands.extend(home.address);
                    instructions.push(MachineInstruction {
                        id,
                        opcode: home.reload_opcode.machine_opcode(),
                        operands,
                        flags: InstructionFlags {
                            may_load: true,
                            ..InstructionFlags::NONE
                        },
                    });
                    resident.insert(value, local);
                    local
                };
                operand.kind = MachineOperandKind::Register(MachineRegister::Virtual(local));
            }
            instructions.push(rewritten);
            for operand in &instruction.operands {
                let MachineOperandKind::Register(MachineRegister::Virtual(value)) = operand.kind
                else {
                    continue;
                };
                if !crossing.contains(&value) || !operand.role.writes() {
                    continue;
                }
                if operand.role != OperandRole::Def {
                    return Err(X86FloatAllocationError::MalformedInstruction {
                        function: function.id,
                        block: block.id,
                        instruction: instruction.id,
                        value: Some(value),
                        reason: "crossing x87 definition must be a pure definition",
                    });
                }
                let id = next_generated_instruction(
                    &mut next_instruction,
                    function,
                    block.id,
                    instruction.id,
                )?;
                instructions.push(MachineInstruction {
                    id,
                    opcode: X86Opcode::X87StorePop.machine_opcode(),
                    operands: vec![
                        virtual_x87(value, OperandRole::Use),
                        format_operand(X87MemoryFormat::Float80),
                        frame_operand(homes[&value]),
                    ],
                    flags: InstructionFlags {
                        may_store: true,
                        side_effects: true,
                        ..InstructionFlags::NONE
                    },
                });
            }
        }
        blocks.push(MachineBlock {
            id: block.id,
            instructions,
            successors: block.successors.clone(),
        });
    }
    Ok(MachineFunction {
        virtual_registers,
        frame_objects: frames,
        blocks,
        ..function.clone()
    })
}

fn x87_region_labels(
    function: &MachineFunction,
    reachable: &BTreeSet<MachineBlockId>,
    follows: &BTreeMap<MachineBlockId, MachineBlockId>,
) -> BTreeMap<(MachineBlockId, usize), u32> {
    let destinations = follows.values().copied().collect::<BTreeSet<_>>();
    let mut roots = function
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id) && !destinations.contains(&block.id))
        .map(|block| block.id)
        .collect::<Vec<_>>();
    roots.sort();
    roots.extend(
        function
            .blocks
            .iter()
            .filter(|block| !reachable.contains(&block.id))
            .map(|block| block.id),
    );
    roots.extend(function.blocks.iter().map(|block| block.id));
    let by_id = function
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut labels = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let x87 = function
        .virtual_registers
        .iter()
        .filter(|register| register.class == X86RegisterClass::X87.machine_class())
        .map(|register| register.id)
        .collect::<BTreeSet<_>>();
    let mut region = 0u32;
    for root in roots {
        let mut current = root;
        if seen.contains(&current) {
            continue;
        }
        region = region.saturating_add(1);
        while seen.insert(current) {
            let block = by_id[&current];
            let preserves_edge = follows.contains_key(&current);
            for (index, instruction) in block.instructions.iter().enumerate() {
                let boundary = region_boundary(
                    instruction,
                    preserves_edge && index + 1 == block.instructions.len(),
                );
                let call_result = x87_call_result(instruction, &x87).is_some();
                if boundary && call_result {
                    // Python `floatalloc.allocated` increments before it
                    // records an instruction position.  In particular, a
                    // CALL's pure x87 definition is the value it leaves in
                    // ST0 after the call, whereas any pre-call values have
                    // already been bridged out of the preceding region.
                    region = region.saturating_add(1);
                }
                labels.insert((current, index), region);
                if boundary && !call_result {
                    region = region.saturating_add(1);
                }
            }
            let Some(next) = follows.get(&current).copied() else {
                break;
            };
            current = next;
        }
    }
    labels
}

fn region_boundary(instruction: &MachineInstruction, preserves_straight_edge: bool) -> bool {
    is_hard_boundary(instruction) && !preserves_straight_edge
}

/// The selected form of Python's post-call nameless `FLOAT_LOAD`: one new
/// extended value, constrained to the ST0 the ABI call just produced.  Calls
/// with any x87 input are deliberately not this form—their inputs belong to
/// the preceding region and remain unsupported here until selection spells
/// their ABI materialization separately.
fn x87_call_result(
    instruction: &MachineInstruction,
    x87: &BTreeSet<VirtualRegisterId>,
) -> Option<VirtualRegisterId> {
    if !instruction.flags.call
        || !matches!(
            X86Opcode::from_machine_opcode(instruction.opcode),
            Some(X86Opcode::CallNear | X86Opcode::CallFar)
        )
    {
        return None;
    }
    let results = instruction
        .operands
        .iter()
        .filter_map(|operand| match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(value))
                if x87.contains(&value) && operand.role == OperandRole::Def =>
            {
                Some(value)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    (results.len() == 1
        && instruction.operands.iter().any(|operand| {
            matches!(operand.kind, MachineOperandKind::Register(MachineRegister::Virtual(value))
                if value == results[0]
                    && operand.constraint == Some(RegisterConstraint::Fixed(X86Register::St0.physical())))
        })
        && !instruction.operands.iter().any(|operand| {
            matches!(operand.kind, MachineOperandKind::Register(MachineRegister::Virtual(value))
                if x87.contains(&value) && operand.role.reads())
        }))
    .then(|| results[0])
}

fn machine_dominators(
    function: &MachineFunction,
    reachable: &BTreeSet<MachineBlockId>,
) -> BTreeMap<MachineBlockId, BTreeSet<MachineBlockId>> {
    let predecessors = reachable_predecessors(&function.blocks, reachable);
    let mut result = reachable
        .iter()
        .map(|&block| {
            let initial = if block == function.entry {
                BTreeSet::from([block])
            } else {
                reachable.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for &block in reachable {
            if block == function.entry {
                continue;
            }
            let Some(items) = predecessors.get(&block) else {
                continue;
            };
            let mut next = items
                .iter()
                .filter_map(|predecessor| result.get(predecessor).cloned())
                .reduce(|left, right| left.intersection(&right).copied().collect())
                .unwrap_or_default();
            next.insert(block);
            if result[&block] != next {
                result.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

fn next_generated_instruction(
    next: &mut Option<u32>,
    function: &MachineFunction,
    block: MachineBlockId,
    at: MachineInstructionId,
) -> Result<MachineInstructionId, X86FloatAllocationError> {
    let raw = next.take().ok_or(X86FloatAllocationError::IdExhausted {
        function: function.id,
        block,
        instruction: at,
    })?;
    *next = raw.checked_add(1);
    Ok(MachineInstructionId::new(raw))
}

fn bridge_id_exhausted(
    function: &MachineFunction,
    block: MachineBlockId,
    index: usize,
) -> X86FloatAllocationError {
    X86FloatAllocationError::IdExhausted {
        function: function.id,
        block,
        instruction: function
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)
            .and_then(|candidate| candidate.instructions.get(index))
            .map_or(MachineInstructionId::new(0), |instruction| instruction.id),
    }
}

/// Values needed immediately after every instruction.  This is the Machine-IR
/// equivalent of Python's region table: it includes local readers and the
/// ordinary SSA live-out set of each block.
fn x87_live_after(
    function: &MachineFunction,
    blocks: &BTreeMap<MachineBlockId, BlockLiveness>,
    x87: &BTreeSet<VirtualRegisterId>,
) -> BTreeMap<(MachineBlockId, MachineInstructionId), BTreeSet<VirtualRegisterId>> {
    let mut result = BTreeMap::new();
    for block in &function.blocks {
        let mut live = blocks.get(&block.id).map_or_else(BTreeSet::new, |facts| {
            facts.live_out.intersection(x87).copied().collect()
        });
        for instruction in block.instructions.iter().rev() {
            result.insert((block.id, instruction.id), live.clone());
            for operand in &instruction.operands {
                let MachineOperandKind::Register(MachineRegister::Virtual(value)) = operand.kind
                else {
                    continue;
                };
                if !x87.contains(&value) {
                    continue;
                }
                if operand.role.writes() {
                    live.remove(&value);
                }
                if operand.role.reads() {
                    live.insert(value);
                }
            }
        }
    }
    result
}

/// Values whose extended-precision stack representation crosses an opaque or
/// control-flow region.  One m80 bridge cell per value mirrors Python
/// `floatregions.bridged`'s `("floating-region", value)` cells.
fn bridge_values(
    function: &MachineFunction,
    reachable: &BTreeSet<MachineBlockId>,
    follows: &BTreeMap<MachineBlockId, MachineBlockId>,
    blocks: &BTreeMap<MachineBlockId, BlockLiveness>,
    live_after: &BTreeMap<(MachineBlockId, MachineInstructionId), BTreeSet<VirtualRegisterId>>,
    x87: &BTreeSet<VirtualRegisterId>,
) -> BTreeSet<VirtualRegisterId> {
    let mut result = BTreeSet::new();
    for block in &function.blocks {
        if !reachable.contains(&block.id) {
            continue;
        }
        for (index, instruction) in block.instructions.iter().enumerate() {
            let preserved_straight_edge = follows.contains_key(&block.id)
                && index + 1 == block.instructions.len()
                && instruction.flags.terminator
                && !instruction.flags.call;
            if is_hard_boundary(instruction) && !preserved_straight_edge {
                // A selected call result is Python's separate, post-call
                // nameless FLOAT_LOAD.  The ordinary SSA live-after fact
                // includes that definition because a later instruction reads
                // it, but it is not a pre-call stack value to bridge.
                let call_result = x87_call_result(instruction, x87);
                result.extend(
                    live_after
                        .get(&(block.id, instruction.id))
                        .into_iter()
                        .flatten()
                        .filter(|value| Some(**value) != call_result)
                        .copied(),
                );
            }
        }
        if !follows.contains_key(&block.id) {
            result.extend(
                blocks
                    .get(&block.id)
                    .into_iter()
                    .flat_map(|facts| facts.live_out.iter())
                    .filter(|value| x87.contains(value))
                    .copied(),
            );
        }
    }
    result
}

/// Collapses repeated scalar `fld`/`fild` reads of the same direct cell while
/// Python `_may_write` proves that no intervening instruction changes it.
/// This is Python `_equivalent_loads` expressed over the Machine IR address
/// contract; m80 cells intentionally remain distinct because they are spill
/// values.
/// Python clears every remembered cell before a volatile load independently
/// of `_may_write`; keep that rule explicit rather than folding volatility
/// back into the memory-destination proof.
fn equivalent_loads(
    sequence: &[MachineInstruction],
    x87: &BTreeSet<VirtualRegisterId>,
    register_widths: &BTreeMap<VirtualRegisterId, u32>,
) -> BTreeMap<VirtualRegisterId, VirtualRegisterId> {
    let mut available = Vec::<(
        X86Opcode,
        X87MemoryFormat,
        Vec<MachineOperand>,
        VirtualRegisterId,
    )>::new();
    let mut aliases = BTreeMap::new();
    for instruction in sequence {
        if instruction.flags.volatile {
            available.clear();
            continue;
        }
        available.retain(|(opcode, format, address, _)| {
            !may_write_home(
                instruction,
                &Home {
                    reload_opcode: *opcode,
                    format: *format,
                    address: address.clone(),
                    direct: true,
                },
                register_widths,
            )
        });
        let Some(opcode @ (X86Opcode::X87Load | X86Opcode::X87IntegerLoad)) =
            X86Opcode::from_machine_opcode(instruction.opcode)
        else {
            continue;
        };
        let Some(destination) =
            instruction
                .operands
                .first()
                .and_then(|operand| match operand.kind {
                    MachineOperandKind::Register(MachineRegister::Virtual(value))
                        if x87.contains(&value) && operand.role == OperandRole::Def =>
                    {
                        Some(value)
                    }
                    _ => None,
                })
        else {
            continue;
        };
        let Some(format) = instruction.operands.get(1).and_then(decode_format) else {
            continue;
        };
        let address = instruction.operands[2..].to_vec();
        if format == X87MemoryFormat::Float80 || !stable_address(&address) {
            continue;
        }
        if let Some((_, _, _, canonical)) =
            available
                .iter()
                .find(|(known_opcode, known_format, known_address, _)| {
                    *known_opcode == opcode && *known_format == format && *known_address == address
                })
        {
            aliases.insert(destination, *canonical);
        } else {
            available.push((opcode, format, address, destination));
        }
    }
    aliases
}

fn canonical_alias(
    mut value: VirtualRegisterId,
    aliases: &BTreeMap<VirtualRegisterId, VirtualRegisterId>,
) -> VirtualRegisterId {
    while let Some(next) = aliases.get(&value).copied() {
        value = next;
    }
    value
}

fn remaining_x87_virtual(
    function: &MachineFunction,
    x87: &BTreeSet<VirtualRegisterId>,
) -> Option<(MachineBlockId, MachineInstructionId, VirtualRegisterId)> {
    function.blocks.iter().find_map(|block| {
        block.instructions.iter().find_map(|instruction| {
            instruction
                .operands
                .iter()
                .find_map(|operand| match operand.kind {
                    MachineOperandKind::Register(MachineRegister::Virtual(value))
                        if x87.contains(&value) =>
                    {
                        Some((block.id, instruction.id, value))
                    }
                    _ => None,
                })
        })
    })
}

#[derive(Clone)]
struct FunctionState {
    id: crate::old::codegen::machine::MachineFunctionId,
    x87: BTreeSet<VirtualRegisterId>,
    register_widths: BTreeMap<VirtualRegisterId, u32>,
    frame_objects: Vec<FrameObject>,
    next_instruction: Option<u32>,
    next_frame: Option<u32>,
}

fn next_frame_index(function: &MachineFunction) -> Option<u32> {
    function
        .frame_objects
        .iter()
        .map(|frame| frame.index.get())
        .chain(function.blocks.iter().flat_map(|block| {
            block.instructions.iter().flat_map(|instruction| {
                instruction
                    .operands
                    .iter()
                    .filter_map(|operand| match operand.kind {
                        MachineOperandKind::FrameIndex { index, .. } => Some(index.get()),
                        _ => None,
                    })
            })
        }))
        .max()
        .map(|index| index.checked_add(1))
        .unwrap_or(Some(0))
}

impl FunctionState {
    fn new(function: &MachineFunction, x87: BTreeSet<VirtualRegisterId>) -> Self {
        let maximum_instruction = function
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .map(|instruction| instruction.id.get())
            })
            .max();
        let next_instruction = maximum_instruction
            .map(|id| id.checked_add(1))
            .unwrap_or(Some(0));
        let next_frame = next_frame_index(function);
        let register_widths = function
            .virtual_registers
            .iter()
            .filter_map(|register| {
                let class = X86RegisterClass::from_machine_class(register.class)?;
                let width = match class {
                    X86RegisterClass::Byte => 1,
                    X86RegisterClass::Word | X86RegisterClass::Address16 => 2,
                    X86RegisterClass::Dword => 4,
                    X86RegisterClass::Segment | X86RegisterClass::X87 => return None,
                };
                Some((register.id, width))
            })
            .collect();
        Self {
            id: function.id,
            x87,
            register_widths,
            frame_objects: function.frame_objects.clone(),
            next_instruction,
            next_frame,
        }
    }

    fn instruction_id(
        &mut self,
        block: MachineBlockId,
        at: MachineInstructionId,
    ) -> Result<MachineInstructionId, X86FloatAllocationError> {
        let raw = self
            .next_instruction
            .take()
            .ok_or(X86FloatAllocationError::IdExhausted {
                function: self.id,
                block,
                instruction: at,
            })?;
        let result = MachineInstructionId::new(raw);
        self.next_instruction = raw.checked_add(1);
        Ok(result)
    }

    fn temporary(
        &mut self,
        block: MachineBlockId,
        at: MachineInstructionId,
    ) -> Result<FrameIndex, X86FloatAllocationError> {
        let raw = self
            .next_frame
            .take()
            .ok_or(X86FloatAllocationError::IdExhausted {
                function: self.id,
                block,
                instruction: at,
            })?;
        let index = FrameIndex::new(raw);
        self.next_frame = raw.checked_add(1);
        // Python `_Stack.room` uses an m80 cell.  Two-byte alignment preserves
        // the original 16-bit target's frame discipline without overclaiming
        // a host alignment requirement.
        self.frame_objects.push(FrameObject {
            index,
            size: 10,
            alignment: 2,
            kind: FrameObjectKind::Temporary,
        });
        Ok(index)
    }
}

#[derive(Clone)]
struct Home {
    /// `fld` and `fild` are not interchangeable: the latter converts an
    /// integer memory value.  This retains the source load's instruction
    /// semantics across a stack spill or deferred direct-memory use.
    reload_opcode: X86Opcode,
    format: X87MemoryFormat,
    address: Vec<MachineOperand>,
    direct: bool,
}

struct Stackifier<'a> {
    function: &'a mut FunctionState,
    block: MachineBlockId,
    values: Vec<VirtualRegisterId>, // x87 top first
    homes: BTreeMap<VirtualRegisterId, Home>,
    aliases: BTreeMap<VirtualRegisterId, VirtualRegisterId>,
    definitions: BTreeMap<VirtualRegisterId, usize>,
    reads: BTreeMap<VirtualRegisterId, VecDeque<usize>>,
    sequence: Vec<MachineInstruction>,
    costs: X86FloatCosts,
    retain_homes: bool,
    retained: BTreeSet<VirtualRegisterId>,
    position: usize,
    current: MachineInstructionId,
    output: Vec<MachineInstruction>,
}

/// Materializes Python's baseline and retained-home candidates for one
/// independently bridgeable region, then keeps the cheaper complete output.
/// Candidate state includes generated m80 cells and instruction IDs, so the
/// selected state—not a partially merged instruction list—continues into the
/// following region.
fn process_candidate_chain(
    state: &mut FunctionState,
    chain: &[&MachineBlock],
    live_after: &BTreeMap<(MachineBlockId, MachineInstructionId), BTreeSet<VirtualRegisterId>>,
    costs: X86FloatCosts,
    output: &mut BTreeMap<MachineBlockId, Vec<MachineInstruction>>,
) -> Result<(), X86FloatAllocationError> {
    let mut baseline_state = state.clone();
    let mut baseline_output = BTreeMap::new();
    process_chain(
        &mut baseline_state,
        chain,
        live_after,
        costs,
        false,
        &mut baseline_output,
    )?;
    let mut retained_state = state.clone();
    let mut retained_output = BTreeMap::new();
    process_chain(
        &mut retained_state,
        chain,
        live_after,
        costs,
        true,
        &mut retained_output,
    )?;
    if output_float_score(&retained_output, costs) < output_float_score(&baseline_output, costs) {
        *state = retained_state;
        output.extend(retained_output);
    } else {
        *state = baseline_state;
        output.extend(baseline_output);
    }
    Ok(())
}

fn process_chain(
    function: &mut FunctionState,
    chain: &[&MachineBlock],
    live_after: &BTreeMap<(MachineBlockId, MachineInstructionId), BTreeSet<VirtualRegisterId>>,
    costs: X86FloatCosts,
    retain_homes: bool,
    output: &mut BTreeMap<MachineBlockId, Vec<MachineInstruction>>,
) -> Result<(), X86FloatAllocationError> {
    let sequence = chain
        .iter()
        .flat_map(|block| block.instructions.iter().cloned())
        .collect::<Vec<_>>();
    let aliases = equivalent_loads(&sequence, &function.x87, &function.register_widths);
    let mut definitions = BTreeMap::new();
    for (position, instruction) in sequence.iter().enumerate() {
        for value in instruction
            .operands
            .iter()
            .filter_map(|operand| match operand.kind {
                MachineOperandKind::Register(MachineRegister::Virtual(value))
                    if function.x87.contains(&value) && operand.role.writes() =>
                {
                    Some(canonical_alias(value, &aliases))
                }
                _ => None,
            })
        {
            definitions.entry(value).or_insert(position);
        }
    }
    let mut reads = BTreeMap::<VirtualRegisterId, VecDeque<usize>>::new();
    let mut position = 0;
    for block in chain {
        for instruction in &block.instructions {
            for value in x87_reads(instruction, &function.x87) {
                reads
                    .entry(canonical_alias(value, &aliases))
                    .or_default()
                    .push_back(position);
            }
            position += 1;
        }
    }
    let mut stack = Stackifier {
        function,
        block: chain[0].id,
        values: Vec::new(),
        homes: BTreeMap::new(),
        aliases,
        definitions,
        reads,
        sequence,
        costs,
        retain_homes,
        retained: BTreeSet::new(),
        position: 0,
        current: chain[0]
            .instructions
            .first()
            .map_or(MachineInstructionId::new(0), |one| one.id),
        output: Vec::new(),
    };
    let mut starts = Vec::new();
    for block in chain {
        starts.push((block.id, stack.output.len()));
        stack.block = block.id;
        let next_is_chain = chain.last().is_some_and(|last| block.id != last.id);
        for (index, instruction) in block.instructions.iter().enumerate() {
            stack.current = instruction.id;
            stack.consume_reads(instruction);
            if instruction_has_x87(instruction, &stack.function.x87) {
                stack.allocate(instruction)?;
            } else {
                let preserves_straight_edge = next_is_chain
                    && index + 1 == block.instructions.len()
                    && instruction.flags.terminator
                    && !instruction.flags.call;
                // The definition-time bridge transform leaves no live x87
                // value for a non-chain CFG exit.  Pop any dead residue
                // before its terminator: no generated instruction may ever
                // follow a terminator in a Machine block.
                let terminal_exit = !next_is_chain
                    && index + 1 == block.instructions.len()
                    && instruction.flags.terminator;
                if terminal_exit {
                    let crossing = live_after
                        .get(&(block.id, instruction.id))
                        .cloned()
                        .unwrap_or_default();
                    if let Some(value) = crossing.iter().next().copied() {
                        return Err(stack.live_boundary(value));
                    }
                    stack.discard_dead()?;
                    if let Some(value) = stack.values.first().copied() {
                        return Err(stack.live_boundary(value));
                    }
                }
                if stack.is_boundary(instruction) && !preserves_straight_edge {
                    let crossing = live_after
                        .get(&(block.id, instruction.id))
                        .cloned()
                        .unwrap_or_default();
                    if let Some(value) = crossing.iter().next().copied() {
                        return Err(stack.live_boundary(value));
                    }
                    stack.discard_dead()?;
                    if let Some(value) = stack.values.first().copied() {
                        return Err(stack.live_boundary(value));
                    }
                }
                stack.output.push(instruction.clone());
                stack.invalidate_homes(instruction);
            }
            stack.position += 1;
        }
        let final_is_boundary = block.instructions.last().is_some_and(is_hard_boundary);
        if !next_is_chain && !final_is_boundary {
            let crossing = block
                .instructions
                .last()
                .and_then(|instruction| live_after.get(&(block.id, instruction.id)))
                .cloned()
                .unwrap_or_default();
            if let Some(value) = crossing.iter().next().copied() {
                return Err(stack.live_boundary(value));
            }
        }
        stack.discard_dead()?;
        if !next_is_chain {
            if let Some(value) = stack.values.first().copied() {
                return Err(stack.live_boundary(value));
            }
        }
    }
    let mut spans = Vec::with_capacity(starts.len());
    for (index, (block, begin)) in starts.iter().copied().enumerate() {
        let end = starts
            .get(index + 1)
            .map_or(stack.output.len(), |(_, next)| *next);
        spans.push((block, begin, end));
    }
    for (block, begin, end) in spans {
        output.insert(block, stack.output[begin..end].to_vec());
    }
    Ok(())
}

fn x87_reads<'a>(
    instruction: &'a MachineInstruction,
    x87: &'a BTreeSet<VirtualRegisterId>,
) -> impl Iterator<Item = VirtualRegisterId> + 'a {
    instruction
        .operands
        .iter()
        .filter_map(move |operand| match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(value))
                if x87.contains(&value) && operand.role.reads() =>
            {
                Some(value)
            }
            _ => None,
        })
}

fn instruction_has_x87(
    instruction: &MachineInstruction,
    x87: &BTreeSet<VirtualRegisterId>,
) -> bool {
    instruction
        .operands
        .iter()
        .any(|operand| match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(value)) => x87.contains(&value),
            _ => false,
        })
}

fn is_hard_boundary(instruction: &MachineInstruction) -> bool {
    // Direct spelling of `floatregions.boundary`: an unknown/opaque selected
    // operation, a call, explicit physical x87 stack state, and the x87
    // control-state barriers close a region.  Memory effects only invalidate
    // a retained home; an ordinary `fstp` is not a stack-region boundary.
    if instruction.flags.call
        || instruction.operands.iter().any(|operand| {
            matches!(operand.kind,
                MachineOperandKind::Register(MachineRegister::Physical(register))
                    if matches!(X86Register::from_physical(register), Some(
                        X86Register::St0 | X86Register::St1 | X86Register::St2 | X86Register::St3
                        | X86Register::St4 | X86Register::St5 | X86Register::St6 | X86Register::St7
                    ))
            )
        })
    {
        return true;
    }
    match X86Opcode::from_machine_opcode(instruction.opcode) {
        Some(X86Opcode::Wait | X86Opcode::X87StoreControlWord | X86Opcode::X87LoadControlWord) => {
            true
        }
        Some(_) => false,
        None => true,
    }
}

impl Stackifier<'_> {
    fn error(
        &self,
        value: Option<VirtualRegisterId>,
        reason: &'static str,
    ) -> X86FloatAllocationError {
        X86FloatAllocationError::MalformedInstruction {
            function: self.function.id,
            block: self.block,
            instruction: self.current,
            value,
            reason,
        }
    }

    fn live_boundary(&self, value: VirtualRegisterId) -> X86FloatAllocationError {
        X86FloatAllocationError::LiveAcrossControlFlow {
            function: self.function.id,
            block: self.block,
            instruction: Some(self.current),
            successor: None,
            value,
        }
    }

    fn consume_reads(&mut self, instruction: &MachineInstruction) {
        for value in x87_reads(instruction, &self.function.x87) {
            let value = self.canonical(value);
            if let Some(queue) = self.reads.get_mut(&value) {
                if queue.front() == Some(&self.position) {
                    queue.pop_front();
                }
            }
        }
    }

    fn survives(&self, value: VirtualRegisterId) -> bool {
        let value = self.canonical(value);
        let Some(reads) = self.reads.get(&value) else {
            return false;
        };
        if self.retained.contains(&value) && self.values.contains(&value) {
            return !reads.is_empty();
        }
        let Some(home) = self.homes.get(&value) else {
            return !reads.is_empty();
        };
        reads
            .iter()
            .copied()
            .any(|position| !self.reads_cell(value, position, home))
    }

    fn reads_cell(&self, value: VirtualRegisterId, position: usize, home: &Home) -> bool {
        let Some(instruction) = self.sequence.get(position) else {
            return false;
        };
        let Some(opcode) = X86Opcode::from_machine_opcode(instruction.opcode) else {
            return false;
        };
        if !matches!(
            opcode,
            X86Opcode::X87Add
                | X86Opcode::X87Subtract
                | X86Opcode::X87SubtractReverse
                | X86Opcode::X87Multiply
                | X86Opcode::X87Divide
                | X86Opcode::X87DivideReverse
        ) || instruction.operands.len() != 3
        {
            return false;
        }
        let Some(left) = x87_virtual_use(&instruction.operands[1], &self.function.x87)
            .map(|item| self.canonical(item))
        else {
            return false;
        };
        let Some(right) = x87_virtual_use(&instruction.operands[2], &self.function.x87)
            .map(|item| self.canonical(item))
        else {
            return false;
        };
        left != right
            && (left == value || right == value)
            && home.direct
            && direct_fold_format(home.format)
            && stable_address(&home.address)
            && memory_arithmetic_opcode(opcode, left == value).is_ok()
    }

    fn next_use(&self, value: VirtualRegisterId) -> Option<usize> {
        self.reads
            .get(&self.canonical(value))
            .and_then(|queue| queue.front().copied())
    }

    fn canonical(&self, value: VirtualRegisterId) -> VirtualRegisterId {
        canonical_alias(value, &self.aliases)
    }

    fn rereadable(&self, value: VirtualRegisterId, home: &Home) -> bool {
        if !home.direct {
            return false;
        }
        let Some(reads) = self.reads.get(&value) else {
            return false;
        };
        let Some(last) = reads.back().copied() else {
            return false;
        };
        // Python `_rereadable` checks `position + 1 .. reads[-1]`.  The last
        // reader is excluded: it may itself be the store which consumes the
        // value, and only writes after its read.
        if last > self.position
            && self.sequence[self.position + 1..last]
                .iter()
                .any(|instruction| {
                    may_write_home(instruction, home, &self.function.register_widths)
                })
        {
            return false;
        }
        let Some(first) = reads.front().copied() else {
            return false;
        };
        if first <= self.position {
            return false;
        }
        let intrinsically_quiet = home.reload_opcode == X86Opcode::X87IntegerLoad
            || home.format == X87MemoryFormat::Float80;
        intrinsically_quiet
            || !self.sequence[self.position + 1..first]
                .iter()
                .any(may_raise_x87)
            || quiet_home_before(
                &self.sequence,
                self.position,
                home,
                &self.function.register_widths,
            )
    }

    fn retain_home(&self, value: VirtualRegisterId) -> bool {
        if !self.retain_homes
            || self.values.len() > 6
            || self.retained.iter().any(|retained| {
                self.values.contains(retained)
                    && self
                        .reads
                        .get(retained)
                        .is_some_and(|reads| !reads.is_empty())
            })
        {
            return false;
        }
        let Some(reads) = self.reads.get(&value) else {
            return false;
        };
        let positions = reads
            .iter()
            .copied()
            .fold(Vec::new(), |mut unique, position| {
                if unique.last() != Some(&position) {
                    unique.push(position);
                }
                unique
            });
        if positions.is_empty() {
            return false;
        }
        // This is Python `_Stack.retain_home`'s safe starting case: retention
        // begins at a self-use, where its necessary duplicate is fully
        // accounted for.  Ordinary first uses are deliberately left to the
        // baseline memory candidate rather than guessing an exchange cost.
        let first = &self.sequence[positions[0]];
        if first.operands.len() != 3
            || !matches!(first.operands[1].kind, MachineOperandKind::Register(MachineRegister::Virtual(left)) if self.canonical(left) == value)
            || !matches!(first.operands[2].kind, MachineOperandKind::Register(MachineRegister::Virtual(right)) if self.canonical(right) == value)
        {
            return false;
        }
        let Some(home) = self.homes.get(&value) else {
            return false;
        };
        let mut home_cost = 0u64;
        let mut retained_cost = u64::from(self.costs.load);
        for (ordinal, position) in positions.iter().copied().enumerate() {
            let instruction = &self.sequence[position];
            let Some(opcode) = X86Opcode::from_machine_opcode(instruction.opcode) else {
                return false;
            };
            let Some((register, memory)) = arithmetic_costs(opcode, self.costs) else {
                return false;
            };
            let Some(left) = instruction.operands.get(1).and_then(|operand| {
                x87_virtual_use(operand, &self.function.x87).map(|item| self.canonical(item))
            }) else {
                return false;
            };
            let Some(right) = instruction.operands.get(2).and_then(|operand| {
                x87_virtual_use(operand, &self.function.x87).map(|item| self.canonical(item))
            }) else {
                return false;
            };
            if value != left && value != right {
                return false;
            }
            if left == right {
                home_cost += u64::from(self.costs.load) + u64::from(register);
                retained_cost += u64::from(register)
                    + u64::from(
                        (ordinal + 1 != positions.len())
                            .then_some(self.costs.load)
                            .unwrap_or(0),
                    );
            } else {
                let cell_form = self.reads_cell(value, position, home);
                home_cost += if cell_form {
                    u64::from(memory).min(u64::from(self.costs.load) + u64::from(register))
                } else {
                    u64::from(self.costs.load) + u64::from(register)
                };
                retained_cost += u64::from(register);
            }
        }
        retained_cost < home_cost
    }

    fn is_boundary(&self, instruction: &MachineInstruction) -> bool {
        // Python `floatregions.boundary` keeps ordinary memory effects inside
        // a region.  Calls, opaque operations, explicit x87 stack operands,
        // and x87 control-state barriers are the boundaries here.
        is_hard_boundary(instruction)
    }

    fn allocate(
        &mut self,
        instruction: &MachineInstruction,
    ) -> Result<(), X86FloatAllocationError> {
        let Some(opcode) = X86Opcode::from_machine_opcode(instruction.opcode) else {
            return Err(self.error(None, "unknown target opcode has an x87 virtual operand"));
        };
        let result = match opcode {
            X86Opcode::X87Load | X86Opcode::X87IntegerLoad => self.load(instruction, opcode),
            X86Opcode::X87LoadZero | X86Opcode::X87LoadOne => self.constant_load(instruction),
            X86Opcode::X87Store
            | X86Opcode::X87StorePop
            | X86Opcode::X87IntegerStore
            | X86Opcode::X87IntegerStorePop
            | X86Opcode::X87IntegerStoreTrunc => self.store(instruction, opcode),
            X86Opcode::X87StackLoad => self.copy(instruction),
            X86Opcode::Copy
                if instruction
                    .operands
                    .iter()
                    .any(|operand| is_x87_virtual(operand, &self.function.x87)) =>
            {
                self.copy(instruction)
            }
            X86Opcode::X87Add
            | X86Opcode::X87Subtract
            | X86Opcode::X87SubtractReverse
            | X86Opcode::X87Multiply
            | X86Opcode::X87Divide
            | X86Opcode::X87DivideReverse => self.arithmetic(instruction, opcode),
            X86Opcode::X87Compare => self.compare(instruction),
            X86Opcode::X87ChangeSign | X86Opcode::X87Absolute | X86Opcode::X87SquareRoot => {
                self.unary(instruction, opcode)
            }
            X86Opcode::CallNear | X86Opcode::CallFar if instruction.flags.call => {
                self.call_result(instruction)
            }
            X86Opcode::ReturnNear | X86Opcode::ReturnFar => self.return_float(instruction),
            _ => Err(X86FloatAllocationError::UnsupportedInstruction {
                function: self.function.id,
                block: self.block,
                instruction: instruction.id,
                value: first_x87_instruction_operand(instruction, &self.function.x87),
                opcode,
            }),
        };
        if result.is_ok() {
            self.invalidate_homes(instruction);
        }
        result
    }

    fn call_result(
        &mut self,
        instruction: &MachineInstruction,
    ) -> Result<(), X86FloatAllocationError> {
        // `lower.lowered` gives Python FloatAlloc a separate nameless
        // FLOAT_LOAD immediately after a CALL for a floating result.  Selected
        // Machine IR represents that same ABI fact as the call's one pure x87
        // definition constrained to ST0.  It belongs to the post-call region;
        // it is neither a value carried into the call nor a general x87 call
        // operand.
        let result = x87_call_result(instruction, &self.function.x87).ok_or_else(|| {
            self.error(
                first_x87_instruction_operand(instruction, &self.function.x87),
                "x87 call result must be one pure fixed-st0 definition",
            )
        })?;

        // This is the ordinary pre-boundary cleanup from `_allocate_stack`.
        // `floatregions.bridged` has already saved genuinely live values; a
        // result may only arrive on the now-empty architectural stack.
        self.discard_dead()?;
        if let Some(value) = self.values.first().copied() {
            return Err(self.live_boundary(value));
        }

        let mut emitted = instruction.clone();
        for operand in &mut emitted.operands {
            if matches!(operand.kind, MachineOperandKind::Register(MachineRegister::Virtual(value)) if value == result)
            {
                *operand = st(0, OperandRole::Def);
            }
        }
        self.output.push(emitted);
        self.values.insert(0, result);
        Ok(())
    }

    fn load(
        &mut self,
        instruction: &MachineInstruction,
        reload_opcode: X86Opcode,
    ) -> Result<(), X86FloatAllocationError> {
        let destination = self.x87_operand(instruction, 0, OperandRole::Def)?;
        let (format, address) = self.memory_tail(instruction, 1)?;
        if self.canonical(destination) != destination {
            // `_equivalent_loads` proved this is another scalar read of the
            // still-current direct cell.  All readers were rewritten to its
            // canonical value before stack allocation.
            return Ok(());
        }
        if self.values.contains(&destination) {
            return Err(self.error(Some(destination), "x87 load defines an already-live value"));
        }
        let direct = stable_address(&address) && !instruction.flags.volatile;
        let home = Home {
            reload_opcode,
            format,
            address: address.clone(),
            direct,
        };
        if direct && self.rereadable(destination, &home) {
            self.homes.insert(destination, home.clone());
            if !self.retain_home(destination) {
                return Ok(());
            }
        }
        self.room(None)?;
        let mut emitted = instruction.clone();
        emitted.operands[0] = st(0, OperandRole::Def);
        self.output.push(emitted);
        self.values.insert(0, destination);
        Ok(())
    }

    fn constant_load(
        &mut self,
        instruction: &MachineInstruction,
    ) -> Result<(), X86FloatAllocationError> {
        let destination = self.x87_operand(instruction, 0, OperandRole::Def)?;
        if instruction.operands.len() != 1 {
            return Err(self.error(Some(destination), "x87 constant load takes one destination"));
        }
        self.room(None)?;
        let mut emitted = instruction.clone();
        emitted.operands[0] = st(0, OperandRole::Def);
        self.output.push(emitted);
        self.values.insert(0, destination);
        Ok(())
    }

    fn copy(&mut self, instruction: &MachineInstruction) -> Result<(), X86FloatAllocationError> {
        if instruction.operands.len() != 2 {
            return Err(self.error(None, "x87 stack copy takes destination and source"));
        }
        let destination = self.x87_operand(instruction, 0, OperandRole::Def)?;
        let source = self.canonical(self.x87_operand(instruction, 1, OperandRole::Use)?);
        if self.values.contains(&destination) {
            return Err(self.error(Some(destination), "x87 copy defines an already-live value"));
        }
        // `lower.lowered` gives Python FloatAlloc one extended x87 Held value
        // directly from `fld m32`.  Rust selection represents that same
        // f32-to-f80 stack-name change as `Copy`.  Transfer only a dying,
        // deferred direct home across that no-op copy: a live copy remains an
        // actual x87 duplicate and must take the normal path below.
        if instruction.opcode == X86Opcode::Copy.machine_opcode()
            && !self.values.contains(&source)
            && !self.survives(source)
        {
            if let Some(home) = self.homes.remove(&source) {
                if home.direct && self.rereadable(destination, &home) {
                    self.homes.insert(destination, home);
                    return Ok(());
                }
                self.homes.insert(source, home);
            }
        }
        if let Some(slot) = self.values.iter().position(|value| *value == source) {
            if !self.survives(source) {
                // Python `_Stack.copy`: move_for_stack_reg renames a dying
                // stack source instead of emitting a needless `fld st(i)`.
                self.values[slot] = destination;
                return Ok(());
            }
            self.room(None)?;
            let mut emitted = instruction.clone();
            emitted.opcode = X86Opcode::X87StackLoad.machine_opcode();
            emitted.operands = vec![st(0, OperandRole::Def), st(slot, OperandRole::Use)];
            // This is physical `fld st(i)`, not the selected generic Copy
            // pseudo.  The physical x87 verifier consequently requires no
            // Machine-IR copy flag.
            emitted.flags = InstructionFlags::NONE;
            self.output.push(emitted);
            self.values.insert(0, destination);
            return Ok(());
        }
        self.materialize(source)?;
        self.values[0] = destination;
        Ok(())
    }

    fn unary(
        &mut self,
        instruction: &MachineInstruction,
        opcode: X86Opcode,
    ) -> Result<(), X86FloatAllocationError> {
        if instruction.operands.len() != 2 {
            return Err(self.error(None, "x87 unary operation takes result and source"));
        }
        let destination = self.x87_operand(instruction, 0, OperandRole::Def)?;
        let source = self.canonical(self.x87_operand(instruction, 1, OperandRole::Use)?);
        self.top(source)?;
        if self.survives(source) {
            self.duplicate(source)?;
        }
        let mut emitted = instruction.clone();
        emitted.opcode = opcode.machine_opcode();
        emitted.operands = vec![st(0, OperandRole::UseDef)];
        self.output.push(emitted);
        self.values[0] = destination;
        Ok(())
    }

    fn store(
        &mut self,
        instruction: &MachineInstruction,
        selected: X86Opcode,
    ) -> Result<(), X86FloatAllocationError> {
        let source = self.canonical(self.x87_operand(instruction, 0, OperandRole::Use)?);
        let (_format, _address) = self.memory_tail(instruction, 1)?;
        self.top(source)?;
        let survives = self.survives(source);
        let opcode = match selected {
            X86Opcode::X87Store | X86Opcode::X87StorePop => {
                if survives {
                    X86Opcode::X87Store
                } else {
                    X86Opcode::X87StorePop
                }
            }
            X86Opcode::X87IntegerStore | X86Opcode::X87IntegerStorePop => {
                if survives {
                    X86Opcode::X87IntegerStore
                } else {
                    X86Opcode::X87IntegerStorePop
                }
            }
            X86Opcode::X87IntegerStoreTrunc => X86Opcode::X87IntegerStoreTrunc,
            _ => return Err(self.error(Some(source), "not an x87 store opcode")),
        };
        // `fisttp` has only the popping form.  Keep a live source with a
        // stack duplicate and leave the opcode distinct for the later control
        // word expansion (`floatalloc._truncating`).
        if selected == X86Opcode::X87IntegerStoreTrunc && survives {
            self.duplicate(source)?;
        }
        let mut emitted = instruction.clone();
        emitted.opcode = opcode.machine_opcode();
        emitted.operands[0] = st(0, OperandRole::Use);
        self.output.push(emitted);
        if opcode != X86Opcode::X87Store && opcode != X86Opcode::X87IntegerStore {
            self.values.remove(0);
        }
        Ok(())
    }

    fn return_float(
        &mut self,
        instruction: &MachineInstruction,
    ) -> Result<(), X86FloatAllocationError> {
        // Direct port of `floatalloc._Stack.allocate`'s nameless
        // `FLOAT_STORE` return arm.  A C float return is already resident on
        // the x87 stack; put the requested SSA value in ST(0), ensure it is
        // the sole live x87 value, and leave that physical stack entry for
        // the caller rather than emitting a store or pop.
        let source = self.canonical(self.x87_operand(instruction, 0, OperandRole::Use)?);
        if instruction
            .operands
            .iter()
            .skip(1)
            .any(|operand| is_x87_virtual(operand, &self.function.x87))
        {
            return Err(self.error(Some(source), "x87 return has more than one stack operand"));
        }
        self.top(source)?;
        if self.values.len() != 1 {
            return Err(self.error(
                Some(source),
                "a returned float leaves other values on the stack",
            ));
        }
        let mut emitted = instruction.clone();
        emitted.operands[0] = st(0, OperandRole::Use);
        self.output.push(emitted);
        self.values.pop();
        Ok(())
    }

    fn arithmetic(
        &mut self,
        instruction: &MachineInstruction,
        opcode: X86Opcode,
    ) -> Result<(), X86FloatAllocationError> {
        if instruction.operands.len() != 3 {
            return Err(self.error(None, "x87 binary operation takes result and two sources"));
        }
        let result = self.x87_operand(instruction, 0, OperandRole::Def)?;
        let left = self.canonical(self.x87_operand(instruction, 1, OperandRole::Use)?);
        let right = self.canonical(self.x87_operand(instruction, 2, OperandRole::Use)?);
        if left != right && self.try_memory_arithmetic(instruction, opcode, result, left, right)? {
            return Ok(());
        }
        let mut missing = [left, right]
            .into_iter()
            .filter(|value| !self.values.contains(value))
            .collect::<Vec<_>>();
        missing.sort_unstable();
        missing.dedup();
        missing.sort_by_key(|value| {
            self.definitions
                .get(value)
                .and_then(|position| position.checked_add(1))
                .unwrap_or(0)
        });
        for value in missing {
            self.materialize_if_needed(value)?;
        }
        let mut dies_left = !self.survives(left);
        let dies_right = !self.survives(right);
        if left == right {
            self.top(left)?;
            if !dies_left {
                self.duplicate(left)?;
            }
            let slot = if !dies_left {
                let later = self.next_use(left);
                let product = self.next_use(result);
                if later.is_some() && product.is_some() && later < product {
                    1
                } else {
                    0
                }
            } else {
                0
            };
            let mut emitted = instruction.clone();
            emitted.opcode = opcode.machine_opcode();
            emitted.operands = if dies_left {
                vec![st(0, OperandRole::UseDef), st(0, OperandRole::Use)]
            } else {
                vec![
                    st(slot, OperandRole::UseDef),
                    st(1 - slot, OperandRole::Use),
                ]
            };
            self.output.push(emitted);
            self.values[slot] = result;
            return Ok(());
        }

        // Literal port of Python `_Stack.arithmetic`: preserve an operand
        // which is already at ST(0).  Only choose a dying input when neither
        // input is on top; if both survive, duplicate the left input and let
        // the copy become the result.
        if self.values[0] != left && self.values[0] != right {
            if dies_left || dies_right {
                self.top(if dies_left { left } else { right })?;
            } else {
                self.duplicate(left)?;
                dies_left = true;
            }
        } else if !dies_left && !dies_right {
            self.duplicate(left)?;
            dies_left = true;
        }
        let top_is_left = self.values[0] == left;
        let other = if top_is_left { right } else { left };
        let other_slot = self
            .values
            .iter()
            .position(|value| *value == other)
            .ok_or_else(|| self.error(Some(other), "x87 arithmetic source is not on the stack"))?;
        let mut emitted = instruction.clone();
        if (top_is_left && !dies_right) || (!top_is_left && !dies_left) {
            emitted.opcode = arithmetic_opcode(opcode, top_is_left, false)?.machine_opcode();
            emitted.operands = vec![st(0, OperandRole::UseDef), st(other_slot, OperandRole::Use)];
            self.output.push(emitted);
            self.values[0] = result;
        } else if dies_left && dies_right {
            emitted.opcode = arithmetic_opcode(opcode, top_is_left, true)?.machine_opcode();
            emitted.operands = vec![st(other_slot, OperandRole::UseDef), st(0, OperandRole::Use)];
            self.output.push(emitted);
            self.values[other_slot] = result;
            self.values.remove(0);
        } else {
            emitted.opcode = arithmetic_opcode(opcode, !top_is_left, false)?.machine_opcode();
            emitted.operands = vec![st(other_slot, OperandRole::UseDef), st(0, OperandRole::Use)];
            self.output.push(emitted);
            self.values[other_slot] = result;
        }
        Ok(())
    }

    fn try_memory_arithmetic(
        &mut self,
        instruction: &MachineInstruction,
        opcode: X86Opcode,
        result: VirtualRegisterId,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<bool, X86FloatAllocationError> {
        for (cell, kept, cell_is_left) in [(right, left, false), (left, right, true)] {
            if cell == kept || self.values.contains(&cell) {
                continue;
            }
            let Some(home) = self.homes.get(&cell).cloned() else {
                continue;
            };
            if !home.direct || !direct_fold_format(home.format) || !stable_address(&home.address) {
                continue;
            }
            if !self.memory_arithmetic_is_profitable(opcode, self.survives(kept)) {
                continue;
            }
            self.materialize_if_needed(kept)?;
            self.top(kept)?;
            if self.survives(kept) {
                self.duplicate(kept)?;
            }
            let physical = memory_arithmetic_opcode(opcode, cell_is_left)?;
            let mut emitted = instruction.clone();
            emitted.opcode = physical.machine_opcode();
            emitted.operands = vec![st(0, OperandRole::UseDef), format_operand(home.format)];
            emitted.operands.extend(home.address);
            // This is a newly selected physical memory form, not the
            // selected three-virtual-register pseudo we cloned for source
            // location and identity.  In particular it must not retain any
            // arithmetic pseudo effects: the verifier and encoder contract
            // for `f{add,sub,mul,div} m*` is exactly a load effect.
            emitted.flags = InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            };
            self.output.push(emitted);
            self.values[0] = result;
            return Ok(true);
        }
        Ok(false)
    }

    fn memory_arithmetic_is_profitable(&self, opcode: X86Opcode, preserve_kept: bool) -> bool {
        let Some((register, memory)) = arithmetic_costs(opcode, self.costs) else {
            return false;
        };
        // Python `_Stack.memory_arithmetic`: a direct form combines one load
        // and the register operation, but preserving its stack peer costs a
        // duplicate `fld st(i)`.
        u64::from(memory) + u64::from(preserve_kept) * u64::from(self.costs.load)
            <= u64::from(self.costs.load) + u64::from(register)
    }

    fn compare(&mut self, instruction: &MachineInstruction) -> Result<(), X86FloatAllocationError> {
        if instruction.operands.len() != 2 {
            return Err(self.error(None, "x87 comparison takes two sources"));
        }
        let left = self.canonical(self.x87_operand(instruction, 0, OperandRole::Use)?);
        let right = self.canonical(self.x87_operand(instruction, 1, OperandRole::Use)?);
        if right != left && !self.values.contains(&right) {
            if let Some(home) = self.homes.get(&right).cloned() {
                if home.direct
                    && matches!(
                        home.format,
                        X87MemoryFormat::Float32 | X87MemoryFormat::Float64
                    )
                {
                    // Python `_Stack.compare` prefers `fcomp m32/m64` when the
                    // right value still has a proven direct home.  It preserves
                    // the left input by duplicating only when it survives.
                    self.top(left)?;
                    if self.survives(left) {
                        self.duplicate(left)?;
                    }
                    let mut compare = instruction.clone();
                    compare.opcode = X86Opcode::X87ComparePop.machine_opcode();
                    compare.operands = std::iter::once(st(0, OperandRole::Use))
                        .chain(std::iter::once(format_operand(home.format)))
                        .chain(home.address)
                        .collect();
                    compare.flags = InstructionFlags {
                        may_load: true,
                        ..InstructionFlags::NONE
                    };
                    self.output.push(compare);
                    self.values.remove(0);
                    return self.status_to_flags();
                }
            }
        }
        self.materialize_if_needed(left)?;
        self.materialize_if_needed(right)?;
        self.top(left)?;
        if self.survives(left) {
            self.duplicate(left)?;
        }
        if right == left || self.survives(right) {
            self.duplicate(right)?;
            self.exchange(1)?;
        } else {
            self.place_at_one(right)?;
        }
        let mut compare = instruction.clone();
        compare.opcode = X86Opcode::X87ComparePop2.machine_opcode();
        compare.operands = vec![st(0, OperandRole::Use), st(1, OperandRole::Use)];
        self.output.push(compare);
        self.values.drain(..2);
        self.status_to_flags()
    }

    fn status_to_flags(&mut self) -> Result<(), X86FloatAllocationError> {
        self.generated(
            X86Opcode::X87StoreStatusWord,
            vec![physical(X86Register::Ax, OperandRole::Def)],
            InstructionFlags::NONE,
        )?;
        self.generated(X86Opcode::Sahf, Vec::new(), InstructionFlags::NONE)?;
        Ok(())
    }

    fn materialize_if_needed(
        &mut self,
        value: VirtualRegisterId,
    ) -> Result<(), X86FloatAllocationError> {
        if self.values.contains(&value) {
            Ok(())
        } else {
            self.materialize(value)
        }
    }

    fn materialize(&mut self, value: VirtualRegisterId) -> Result<(), X86FloatAllocationError> {
        let home =
            self.homes
                .get(&value)
                .cloned()
                .ok_or(X86FloatAllocationError::UnavailableValue {
                    function: self.function.id,
                    block: self.block,
                    instruction: self.current,
                    value,
                })?;
        self.room(None)?;
        self.generated(
            home.reload_opcode,
            std::iter::once(st(0, OperandRole::Def))
                .chain(std::iter::once(format_operand(home.format)))
                .chain(home.address)
                .collect(),
            InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            },
        )?;
        self.values.insert(0, value);
        Ok(())
    }

    fn top(&mut self, value: VirtualRegisterId) -> Result<(), X86FloatAllocationError> {
        self.materialize_if_needed(value)?;
        let slot = self
            .values
            .iter()
            .position(|item| *item == value)
            .ok_or_else(|| {
                self.error(Some(value), "x87 value disappeared during materialization")
            })?;
        self.exchange(slot)
    }

    fn exchange(&mut self, slot: usize) -> Result<(), X86FloatAllocationError> {
        if slot == 0 {
            return Ok(());
        }
        if slot >= self.values.len() || slot >= 8 {
            return Err(self.error(
                self.values.get(slot).copied(),
                "x87 exchange slot is out of range",
            ));
        }
        self.generated(
            X86Opcode::X87Exchange,
            vec![st(0, OperandRole::UseDef), st(slot, OperandRole::UseDef)],
            InstructionFlags::NONE,
        )?;
        self.values.swap(0, slot);
        Ok(())
    }

    fn place_at_one(&mut self, value: VirtualRegisterId) -> Result<(), X86FloatAllocationError> {
        let slot = self
            .values
            .iter()
            .position(|item| *item == value)
            .ok_or_else(|| self.error(Some(value), "x87 comparison source is not on the stack"))?;
        if slot == 1 {
            return Ok(());
        }
        if slot == 0 {
            return Err(self.error(Some(value), "x87 comparison needs distinct source values"));
        }
        // Python `_Stack.compare` uses exactly this three-exchange rotation:
        // it preserves ST0 while moving a buried dying right operand to ST1.
        self.exchange(slot)?;
        self.exchange(1)?;
        self.exchange(slot)
    }

    fn duplicate(&mut self, value: VirtualRegisterId) -> Result<(), X86FloatAllocationError> {
        let slot = self
            .values
            .iter()
            .position(|item| *item == value)
            .ok_or_else(|| self.error(Some(value), "cannot duplicate unavailable x87 value"))?;
        self.room(None)?;
        self.generated(
            X86Opcode::X87StackLoad,
            vec![st(0, OperandRole::Def), st(slot, OperandRole::Use)],
            InstructionFlags::NONE,
        )?;
        self.values.insert(0, value);
        Ok(())
    }

    fn room(
        &mut self,
        keep: Option<&BTreeSet<VirtualRegisterId>>,
    ) -> Result<(), X86FloatAllocationError> {
        while self.values.len() >= 8 {
            let victim = self
                .values
                .iter()
                .enumerate()
                .filter(|(_, value)| keep.is_none_or(|items| !items.contains(value)))
                .max_by_key(|(_, value)| self.next_use(**value).unwrap_or(usize::MAX))
                .map(|(slot, _)| slot)
                .ok_or(X86FloatAllocationError::StackOverflow {
                    function: self.function.id,
                    block: self.block,
                    instruction: self.current,
                    value: None,
                })?;
            self.exchange(victim)?;
            let value = self.values.remove(0);
            let frame = self.function.temporary(self.block, self.current)?;
            self.generated(
                X86Opcode::X87StorePop,
                vec![
                    st(0, OperandRole::Use),
                    format_operand(X87MemoryFormat::Float80),
                    frame_operand(frame),
                ],
                InstructionFlags {
                    may_store: true,
                    side_effects: true,
                    ..InstructionFlags::NONE
                },
            )?;
            self.homes.insert(
                value,
                Home {
                    reload_opcode: X86Opcode::X87Load,
                    format: X87MemoryFormat::Float80,
                    address: vec![frame_operand(frame)],
                    direct: false,
                },
            );
        }
        Ok(())
    }

    fn discard_dead(&mut self) -> Result<(), X86FloatAllocationError> {
        while let Some(slot) = self.values.iter().position(|value| !self.survives(*value)) {
            self.exchange(slot)?;
            self.generated(
                X86Opcode::X87StackStorePop,
                vec![st(0, OperandRole::Def), st(0, OperandRole::Use)],
                InstructionFlags::NONE,
            )?;
            self.values.remove(0);
        }
        Ok(())
    }

    fn x87_operand(
        &self,
        instruction: &MachineInstruction,
        index: usize,
        required: OperandRole,
    ) -> Result<VirtualRegisterId, X86FloatAllocationError> {
        let Some(operand) = instruction.operands.get(index) else {
            return Err(self.error(None, "missing x87 register operand"));
        };
        let MachineOperandKind::Register(MachineRegister::Virtual(value)) = operand.kind else {
            return Err(self.error(None, "x87 selected form requires a virtual register"));
        };
        if !self.function.x87.contains(&value) || operand.role != required {
            return Err(self.error(Some(value), "x87 selected operand has wrong class or role"));
        }
        Ok(value)
    }

    fn memory_tail(
        &self,
        instruction: &MachineInstruction,
        format_at: usize,
    ) -> Result<(X87MemoryFormat, Vec<MachineOperand>), X86FloatAllocationError> {
        let Some(format) = instruction.operands.get(format_at).and_then(decode_format) else {
            return Err(self.error(
                None,
                "x87 memory operation requires a known format immediate",
            ));
        };
        let address = instruction.operands[format_at + 1..].to_vec();
        if address.is_empty()
            || address.iter().any(|operand| {
                is_x87_virtual(operand, &self.function.x87)
                    || match operand.kind {
                        MachineOperandKind::Register(_) => operand.role != OperandRole::Use,
                        _ => operand.role != OperandRole::None,
                    }
            })
        {
            return Err(self.error(None, "x87 memory operation has malformed address operands"));
        }
        Ok((format, address))
    }

    fn generated(
        &mut self,
        opcode: X86Opcode,
        operands: Vec<MachineOperand>,
        flags: InstructionFlags,
    ) -> Result<(), X86FloatAllocationError> {
        let id = self.function.instruction_id(self.block, self.current)?;
        self.output.push(MachineInstruction {
            id,
            opcode: opcode.machine_opcode(),
            operands,
            flags,
        });
        Ok(())
    }

    fn invalidate_homes(&mut self, instruction: &MachineInstruction) {
        self.homes.retain(|_, home| {
            !home.direct || !may_write_home(instruction, home, &self.function.register_widths)
        });
    }
}

fn is_x87_virtual(operand: &MachineOperand, x87: &BTreeSet<VirtualRegisterId>) -> bool {
    matches!(operand.kind, MachineOperandKind::Register(MachineRegister::Virtual(value)) if x87.contains(&value))
}

fn x87_virtual_use(
    operand: &MachineOperand,
    x87: &BTreeSet<VirtualRegisterId>,
) -> Option<VirtualRegisterId> {
    match operand.kind {
        MachineOperandKind::Register(MachineRegister::Virtual(value))
            if x87.contains(&value) && operand.role.reads() =>
        {
            Some(value)
        }
        _ => None,
    }
}

fn first_x87_instruction_operand(
    instruction: &MachineInstruction,
    x87: &BTreeSet<VirtualRegisterId>,
) -> Option<VirtualRegisterId> {
    instruction
        .operands
        .iter()
        .find_map(|operand| match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(value))
                if x87.contains(&value) =>
            {
                Some(value)
            }
            _ => None,
        })
}

fn st(slot: usize, role: OperandRole) -> MachineOperand {
    let register = match slot {
        0 => X86Register::St0,
        1 => X86Register::St1,
        2 => X86Register::St2,
        3 => X86Register::St3,
        4 => X86Register::St4,
        5 => X86Register::St5,
        6 => X86Register::St6,
        _ => X86Register::St7,
    };
    physical(register, role)
}

fn physical(register: X86Register, role: OperandRole) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Register(MachineRegister::Physical(register.physical())),
        role,
        constraint: None,
        tied_to: None,
    }
}

fn virtual_x87(register: VirtualRegisterId, role: OperandRole) -> MachineOperand {
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

fn format_operand(format: X87MemoryFormat) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Immediate(format as i64),
        role: OperandRole::None,
        constraint: None,
        tied_to: None,
    }
}

fn decode_format(operand: &MachineOperand) -> Option<X87MemoryFormat> {
    let MachineOperandKind::Immediate(raw) = operand.kind else {
        return None;
    };
    [
        X87MemoryFormat::Float32,
        X87MemoryFormat::Float64,
        X87MemoryFormat::Float80,
        X87MemoryFormat::Signed16,
        X87MemoryFormat::Signed32,
        X87MemoryFormat::Signed64,
        X87MemoryFormat::Control16,
    ]
    .into_iter()
    .find(|format| *format as i64 == raw)
}

fn direct_fold_format(format: X87MemoryFormat) -> bool {
    matches!(
        format,
        X87MemoryFormat::Float32
            | X87MemoryFormat::Float64
            | X87MemoryFormat::Signed16
            | X87MemoryFormat::Signed32
            | X87MemoryFormat::Signed64
    )
}

fn stable_address(address: &[MachineOperand]) -> bool {
    // This is Python `_stable` in Machine-IR terms.  Selection has already
    // made an address a concrete tail: a frame/global spelling, one
    // address16 value, BP plus a displacement, or address16 plus ES.  The
    // lifetime proof below rejects any intervening definition of an address
    // register, so a register tail is just as exact as a frame index here.
    matches!(
        address,
        [MachineOperand {
            kind: MachineOperandKind::FrameIndex { .. } | MachineOperandKind::Global { .. },
            role: OperandRole::None,
            ..
        }] | [MachineOperand {
            kind: MachineOperandKind::Register(_),
            role: OperandRole::Use,
            ..
        }] | [
            MachineOperand {
                kind: MachineOperandKind::Register(_),
                role: OperandRole::Use,
                ..
            },
            MachineOperand {
                kind: MachineOperandKind::Immediate(_),
                role: OperandRole::None,
                ..
            }
        ] | [
            MachineOperand {
                kind: MachineOperandKind::Register(_),
                role: OperandRole::Use,
                ..
            },
            MachineOperand {
                kind: MachineOperandKind::Register(_),
                role: OperandRole::Use,
                ..
            }
        ]
    )
}

fn may_write_home(
    instruction: &MachineInstruction,
    home: &Home,
    register_widths: &BTreeMap<VirtualRegisterId, u32>,
) -> bool {
    // Literal Machine-IR port of Python `floatalloc._may_write`:
    //
    // * an unknown semantic operation (`what is None`) may write;
    // * FILL may write, but Machine IR has no FILL opcode at this boundary;
    // * an explicit overlapping memory destination may write;
    // * a definition of the cell's base/index/selector changes its address.
    //
    // `call`, `volatile`, `side_effects`, and `may_store` flags are not
    // semantic destinations and therefore are deliberately absent here. A
    // future opcode with an unrepresentable memory destination must add an
    // explicit case rather than being inferred from those flags.
    let Some(opcode) = X86Opcode::from_machine_opcode(instruction.opcode) else {
        return true;
    };

    let destination = match opcode {
        X86Opcode::Store => plain_store_address(instruction, register_widths),
        X86Opcode::X87Store
        | X86Opcode::X87StorePop
        | X86Opcode::X87IntegerStore
        | X86Opcode::X87IntegerStorePop
        | X86Opcode::X87IntegerStoreTrunc => x87_store_address(instruction),
        _ => None,
    };
    if matches!(
        opcode,
        X86Opcode::Store
            | X86Opcode::X87Store
            | X86Opcode::X87StorePop
            | X86Opcode::X87IntegerStore
            | X86Opcode::X87IntegerStorePop
            | X86Opcode::X87IntegerStoreTrunc
    ) {
        let Some(home_address) = home.address.first() else {
            return true;
        };
        let Some((address, width)) = destination else {
            // Python represents an unproved memory destination as
            // `Mem(addr=None)`, which reaches every cell. Register-indirect
            // and malformed selected stores have that exact status here.
            return true;
        };
        if direct_cells_overlap(home_address, x87_format_width(home.format), address, width) {
            return true;
        }
    }

    home.address.iter().any(|address_operand| {
        let MachineOperandKind::Register(address_register) = address_operand.kind else {
            return false;
        };
        instruction.operands.iter().any(|operand| {
            operand.role.writes()
                && matches!(operand.kind, MachineOperandKind::Register(register) if register == address_register)
        })
    })
}

fn plain_store_address<'a>(
    instruction: &'a MachineInstruction,
    register_widths: &BTreeMap<VirtualRegisterId, u32>,
) -> Option<(&'a MachineOperand, u32)> {
    if X86Opcode::from_machine_opcode(instruction.opcode) != Some(X86Opcode::Store) {
        return None;
    }
    let address = instruction.operands.first()?;
    if !matches!(
        address.kind,
        MachineOperandKind::FrameIndex { .. } | MachineOperandKind::Global { .. }
    ) {
        return None;
    }
    let width = match instruction.operands.as_slice() {
        [
            _,
            width,
            MachineOperand {
                kind: MachineOperandKind::Immediate(_),
                ..
            },
        ] => match width.kind {
            MachineOperandKind::Immediate(8) => 1,
            MachineOperandKind::Immediate(16) => 2,
            MachineOperandKind::Immediate(32) => 4,
            _ => return None,
        },
        [
            _,
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(value)),
                ..
            },
        ] => *register_widths.get(value)?,
        [
            _,
            MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
                ..
            },
        ] => match X86Register::from_physical(*register)? {
            X86Register::Al
            | X86Register::Cl
            | X86Register::Dl
            | X86Register::Bl
            | X86Register::Ah
            | X86Register::Ch
            | X86Register::Dh
            | X86Register::Bh => 1,
            X86Register::Ax
            | X86Register::Cx
            | X86Register::Dx
            | X86Register::Bx
            | X86Register::Sp
            | X86Register::Bp
            | X86Register::Si
            | X86Register::Di => 2,
            X86Register::Eax
            | X86Register::Ecx
            | X86Register::Edx
            | X86Register::Ebx
            | X86Register::Esp
            | X86Register::Ebp
            | X86Register::Esi
            | X86Register::Edi => 4,
            X86Register::Es
            | X86Register::Cs
            | X86Register::Ss
            | X86Register::Ds
            | X86Register::Fs
            | X86Register::Gs
            | X86Register::St0
            | X86Register::St1
            | X86Register::St2
            | X86Register::St3
            | X86Register::St4
            | X86Register::St5
            | X86Register::St6
            | X86Register::St7 => return None,
        },
        _ => return None,
    };
    Some((address, width))
}

fn x87_store_address(instruction: &MachineInstruction) -> Option<(&MachineOperand, u32)> {
    match X86Opcode::from_machine_opcode(instruction.opcode)? {
        X86Opcode::X87Store
        | X86Opcode::X87StorePop
        | X86Opcode::X87IntegerStore
        | X86Opcode::X87IntegerStorePop
        | X86Opcode::X87IntegerStoreTrunc => {
            let format = instruction.operands.get(1).and_then(decode_format)?;
            if instruction.operands.len() == 3 {
                Some((instruction.operands.get(2)?, x87_format_width(format)))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn x87_format_width(format: X87MemoryFormat) -> u32 {
    match format {
        X87MemoryFormat::Float32 | X87MemoryFormat::Signed32 => 4,
        X87MemoryFormat::Float64 | X87MemoryFormat::Signed64 => 8,
        X87MemoryFormat::Float80 => 10,
        X87MemoryFormat::Signed16 | X87MemoryFormat::Control16 => 2,
    }
}

fn direct_cells_overlap(
    home: &MachineOperand,
    home_width: u32,
    store: &MachineOperand,
    store_width: u32,
) -> bool {
    match (&home.kind, &store.kind) {
        (
            MachineOperandKind::FrameIndex {
                index: left,
                addend: left_offset,
            },
            MachineOperandKind::FrameIndex {
                index: right,
                addend: right_offset,
            },
        ) if left == right => ranges_overlap(*left_offset, home_width, *right_offset, store_width),
        (
            MachineOperandKind::Global {
                name: left,
                addend: left_offset,
            },
            MachineOperandKind::Global {
                name: right,
                addend: right_offset,
            },
        ) if left == right => ranges_overlap(*left_offset, home_width, *right_offset, store_width),
        (
            MachineOperandKind::FrameIndex { .. } | MachineOperandKind::Global { .. },
            MachineOperandKind::FrameIndex { .. } | MachineOperandKind::Global { .. },
        ) => false,
        _ => true,
    }
}

fn ranges_overlap(left: i64, left_width: u32, right: i64, right_width: u32) -> bool {
    let left_end = left.saturating_add(i64::from(left_width));
    let right_end = right.saturating_add(i64::from(right_width));
    left < right_end && right < left_end
}

fn may_raise_x87(instruction: &MachineInstruction) -> bool {
    let Some(opcode) = X86Opcode::from_machine_opcode(instruction.opcode) else {
        // An unrecognised target operation is Python's `what is None`: its
        // exception behaviour is not a fact the stackifier may reorder past.
        return true;
    };
    matches!(
        opcode,
        X86Opcode::X87Add
            | X86Opcode::X87Subtract
            | X86Opcode::X87SubtractReverse
            | X86Opcode::X87Multiply
            | X86Opcode::X87Divide
            | X86Opcode::X87DivideReverse
            | X86Opcode::X87AddPop
            | X86Opcode::X87SubtractPop
            | X86Opcode::X87SubtractReversePop
            | X86Opcode::X87MultiplyPop
            | X86Opcode::X87DividePop
            | X86Opcode::X87DivideReversePop
            | X86Opcode::X87ChangeSign
            | X86Opcode::X87Absolute
            | X86Opcode::X87SquareRoot
            | X86Opcode::Wait
    ) || matches!(opcode, X86Opcode::X87Store | X86Opcode::X87StorePop)
        && instruction.operands.get(1).and_then(decode_format) != Some(X87MemoryFormat::Float80)
}

fn quiet_home_before(
    sequence: &[MachineInstruction],
    position: usize,
    home: &Home,
    register_widths: &BTreeMap<VirtualRegisterId, u32>,
) -> bool {
    for instruction in sequence[..position].iter().rev() {
        let opcode = X86Opcode::from_machine_opcode(instruction.opcode);
        let writes_home = matches!(opcode, Some(X86Opcode::X87Store | X86Opcode::X87StorePop))
            && instruction.operands.get(1).and_then(decode_format) == Some(home.format)
            && instruction.operands.get(2..) == Some(home.address.as_slice());
        if writes_home {
            return true;
        }
        if may_write_home(instruction, home, register_widths) {
            return false;
        }
    }
    false
}

fn arithmetic_opcode(
    opcode: X86Opcode,
    top_is_left: bool,
    pop: bool,
) -> Result<X86Opcode, X86FloatAllocationError> {
    use X86Opcode::*;
    let selected_reverse = matches!(opcode, X87SubtractReverse | X87DivideReverse);
    // Python `_Stack.arithmetic` has deliberately different truth tables for
    // non-pop and pop forms.  With both operands dying, a forward/top-left
    // subtract or divide is `_REVERSED[name] + "p"`; this is the form whose
    // destination is the buried operand and whose result is still left op
    // right.  Non-pop forms reverse in the opposite stack orientation.
    let stack_reverse = if pop { top_is_left } else { !top_is_left };
    let reverse = selected_reverse ^ stack_reverse;
    match opcode {
        X87Add | X87Multiply | X87Subtract | X87SubtractReverse | X87Divide | X87DivideReverse => {}
        _ => unreachable!("caller admits only arithmetic opcode"),
    }
    Ok(match (opcode, reverse, pop) {
        (X87Add, _, false) => X87Add,
        (X87Add, _, true) => X87AddPop,
        (X87Subtract | X87SubtractReverse, false, false) => X87Subtract,
        (X87Subtract | X87SubtractReverse, true, false) => X87SubtractReverse,
        (X87Subtract | X87SubtractReverse, false, true) => X87SubtractPop,
        (X87Subtract | X87SubtractReverse, true, true) => X87SubtractReversePop,
        (X87Multiply, _, false) => X87Multiply,
        (X87Multiply, _, true) => X87MultiplyPop,
        (X87Divide | X87DivideReverse, false, false) => X87Divide,
        (X87Divide | X87DivideReverse, true, false) => X87DivideReverse,
        (X87Divide | X87DivideReverse, false, true) => X87DividePop,
        (X87Divide | X87DivideReverse, true, true) => X87DivideReversePop,
        _ => unreachable!("arithmetic mapping is complete"),
    })
}

fn arithmetic_costs(opcode: X86Opcode, costs: X86FloatCosts) -> Option<(u32, u32)> {
    match opcode {
        X86Opcode::X87Add | X86Opcode::X87Subtract | X86Opcode::X87SubtractReverse => {
            Some((costs.add, costs.add_memory))
        }
        X86Opcode::X87Multiply => Some((costs.multiply, costs.multiply_memory)),
        X86Opcode::X87Divide | X86Opcode::X87DivideReverse => {
            Some((costs.divide, costs.divide_memory))
        }
        _ => None,
    }
}

/// The same form-level accounting used by Python `_region_scores`.  Forms
/// without a published x87 ranking intentionally contribute zero: they are
/// identical barriers/status transfers in both candidates and must not be
/// mistaken for an invented performance fact.
fn output_float_score(
    output: &BTreeMap<MachineBlockId, Vec<MachineInstruction>>,
    costs: X86FloatCosts,
) -> (u64, usize) {
    let cost = output
        .values()
        .flatten()
        .map(|instruction| instruction_float_cost(instruction, costs))
        .sum();
    // `_region_scores` breaks a price tie by emitted instruction count.  The
    // Machine IR has no implicit empty `NOTHING` operation, so every item in
    // these candidate streams is an emitted instruction.
    let count = output.values().map(Vec::len).sum();
    (cost, count)
}

fn instruction_float_cost(instruction: &MachineInstruction, costs: X86FloatCosts) -> u64 {
    let Some(opcode) = X86Opcode::from_machine_opcode(instruction.opcode) else {
        return 0;
    };
    let memory_form = instruction
        .operands
        .get(1)
        .is_some_and(|operand| matches!(operand.kind, MachineOperandKind::Immediate(_)));
    let cost = match opcode {
        X86Opcode::X87Load | X86Opcode::X87IntegerLoad | X86Opcode::X87StackLoad => costs.load,
        X86Opcode::X87Exchange => costs.exchange,
        X86Opcode::X87Store | X86Opcode::X87StorePop | X86Opcode::X87StackStorePop => costs.store,
        X86Opcode::X87IntegerStore
        | X86Opcode::X87IntegerStorePop
        | X86Opcode::X87IntegerStoreTrunc => costs.convert_store,
        X86Opcode::X87Add | X86Opcode::X87Subtract | X86Opcode::X87SubtractReverse => {
            if memory_form {
                costs.add_memory
            } else {
                costs.add
            }
        }
        X86Opcode::X87Multiply => {
            if memory_form {
                costs.multiply_memory
            } else {
                costs.multiply
            }
        }
        X86Opcode::X87Divide | X86Opcode::X87DivideReverse => {
            if memory_form {
                costs.divide_memory
            } else {
                costs.divide
            }
        }
        X86Opcode::X87AddPop | X86Opcode::X87SubtractPop | X86Opcode::X87SubtractReversePop => {
            costs.add
        }
        X86Opcode::X87MultiplyPop => costs.multiply,
        X86Opcode::X87DividePop | X86Opcode::X87DivideReversePop => costs.divide,
        _ => 0,
    };
    u64::from(cost)
}

fn memory_arithmetic_opcode(
    opcode: X86Opcode,
    memory_is_left: bool,
) -> Result<X86Opcode, X86FloatAllocationError> {
    use X86Opcode::*;
    Ok(match opcode {
        X87Add => X87Add,
        X87Subtract => {
            if memory_is_left {
                X87SubtractReverse
            } else {
                X87Subtract
            }
        }
        X87SubtractReverse => {
            if memory_is_left {
                X87Subtract
            } else {
                X87SubtractReverse
            }
        }
        X87Multiply => X87Multiply,
        X87Divide => {
            if memory_is_left {
                X87DivideReverse
            } else {
                X87Divide
            }
        }
        X87DivideReverse => {
            if memory_is_left {
                X87Divide
            } else {
                X87DivideReverse
            }
        }
        _ => {
            return Err(X86FloatAllocationError::MalformedInstruction {
                function: crate::old::codegen::machine::MachineFunctionId::new(0),
                block: MachineBlockId::new(0),
                instruction: MachineInstructionId::new(0),
                value: None,
                reason: "memory arithmetic requested for non-arithmetic opcode",
            });
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::old::codegen::machine::{
        MachineCallingConvention, MachineFunctionId, MachineLinkage, MachineSignature,
        VirtualRegister,
    };

    fn vreg(id: u32, role: OperandRole) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(
                id,
            ))),
            role,
            constraint: None,
            tied_to: None,
        }
    }

    fn frame(index: u32) -> MachineOperand {
        frame_operand(FrameIndex::new(index))
    }

    fn format(format: X87MemoryFormat) -> MachineOperand {
        format_operand(format)
    }

    fn immediate(value: i64) -> MachineOperand {
        MachineOperand {
            kind: MachineOperandKind::Immediate(value),
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        }
    }

    fn instruction(
        id: u32,
        opcode: X86Opcode,
        operands: Vec<MachineOperand>,
        flags: InstructionFlags,
    ) -> MachineInstruction {
        MachineInstruction {
            id: MachineInstructionId::new(id),
            opcode: opcode.machine_opcode(),
            operands,
            flags,
        }
    }

    fn function(instructions: Vec<MachineInstruction>) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(0),
            name: "x87".into(),
            linkage: MachineLinkage::Internal,
            signature: MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::C,
            },
            entry: MachineBlockId::new(0),
            virtual_registers: (0..16)
                .map(|id| VirtualRegister {
                    id: VirtualRegisterId::new(id),
                    class: X86RegisterClass::X87.machine_class(),
                })
                .collect(),
            blocks: vec![MachineBlock {
                id: MachineBlockId::new(0),
                instructions,
                successors: Vec::new(),
            }],
            frame_objects: Vec::new(),
        }
    }

    fn opcodes(function: &MachineFunction) -> Vec<X86Opcode> {
        function.blocks[0]
            .instructions
            .iter()
            .filter_map(|one| X86Opcode::from_machine_opcode(one.opcode))
            .collect()
    }

    #[test]
    fn renames_a_dead_copy_instead_of_duplicate_loading() {
        // Python `_Stack.copy` calls move_for_stack_reg: a source that dies in
        // the copy is renamed, rather than emitting `fld st(i)`.
        let result = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87StackLoad,
                vec![vreg(1, OperandRole::Def), vreg(0, OperandRole::Use)],
                InstructionFlags {
                    copy: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87StorePop,
                vec![
                    vreg(1, OperandRole::Use),
                    format(X87MemoryFormat::Float32),
                    frame(2),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]))
        .unwrap();
        assert_eq!(
            opcodes(&result),
            vec![X86Opcode::X87Load, X86Opcode::X87StorePop]
        );
    }

    #[test]
    fn leaves_c_float_returns_in_st0_and_empties_the_local_stack() {
        // Python `floatalloc._Stack.allocate` recognizes the nameless
        // FLOAT_STORE used for a C return: it exchanges the returned value to
        // ST(0), rejects any other live stack value, then vacates the pseudo.
        // Selection represents that same ABI boundary as ReturnNear/ReturnFar
        // with the x87 virtual return operand followed (for far) by RETF's
        // immediate cleanup.
        for (opcode, tail) in [
            (X86Opcode::ReturnNear, Vec::new()),
            (X86Opcode::ReturnFar, vec![immediate(0)]),
        ] {
            let mut operands = vec![vreg(0, OperandRole::Use)];
            operands.extend(tail.clone());
            let result = allocate_x87_stack(&function(vec![
                instruction(
                    0,
                    X86Opcode::X87Load,
                    vec![
                        vreg(0, OperandRole::Def),
                        format(X87MemoryFormat::Float32),
                        frame(0),
                    ],
                    InstructionFlags {
                        may_load: true,
                        ..InstructionFlags::NONE
                    },
                ),
                instruction(
                    1,
                    opcode,
                    operands,
                    InstructionFlags {
                        terminator: true,
                        ..InstructionFlags::NONE
                    },
                ),
            ]))
            .unwrap();

            assert_eq!(opcodes(&result), vec![X86Opcode::X87Load, opcode]);
            let returned = &result.blocks[0].instructions[1];
            assert_eq!(returned.operands[0], st(0, OperandRole::Use));
            assert_eq!(&returned.operands[1..], tail.as_slice());
            assert!(result.virtual_registers.is_empty());
        }

        let error = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87Load,
                vec![
                    vreg(1, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(1),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::ReturnFar,
                vec![vreg(0, OperandRole::Use), immediate(0)],
                InstructionFlags {
                    terminator: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]))
        .unwrap_err();
        assert!(matches!(
            error,
            X86FloatAllocationError::MalformedInstruction {
                instruction,
                value: Some(value),
                reason: "a returned float leaves other values on the stack",
                ..
            } if instruction == MachineInstructionId::new(2) && value == VirtualRegisterId::new(0)
        ));
    }

    #[test]
    fn physical_stack_loads_drop_selected_copy_flags() {
        let result = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    volatile: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Copy,
                vec![vreg(1, OperandRole::Def), vreg(0, OperandRole::Use)],
                InstructionFlags {
                    copy: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87StorePop,
                vec![
                    vreg(1, OperandRole::Use),
                    format(X87MemoryFormat::Float80),
                    frame(1),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                3,
                X86Opcode::X87StorePop,
                vec![
                    vreg(0, OperandRole::Use),
                    format(X87MemoryFormat::Float80),
                    frame(2),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]))
        .unwrap();
        let loads = result.blocks[0]
            .instructions
            .iter()
            .filter(|item| item.opcode == X86Opcode::X87StackLoad.machine_opcode())
            .collect::<Vec<_>>();
        assert_eq!(loads.len(), 1);
        assert_eq!(loads[0].flags, InstructionFlags::NONE);
    }

    #[test]
    fn materializes_an_integer_home_with_fild_not_fld() {
        // A deferred `fild dword [frame]` reaches a stack copy rather than a
        // binary memory form.  The reload must preserve integer conversion;
        // reloading it as `fld` would reinterpret the same four bytes.
        let result = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87IntegerLoad,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Signed32),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87StackLoad,
                vec![vreg(1, OperandRole::Def), vreg(0, OperandRole::Use)],
                InstructionFlags {
                    copy: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87StorePop,
                vec![
                    vreg(1, OperandRole::Use),
                    format(X87MemoryFormat::Float32),
                    frame(2),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]))
        .unwrap();
        assert_eq!(
            opcodes(&result),
            vec![X86Opcode::X87IntegerLoad, X86Opcode::X87StorePop]
        );
    }

    #[test]
    fn accepts_a_selected_dynamic_address_tail_on_x87_load() {
        // Selection represents `[base + disp]` by a non-x87 address virtual
        // with `Use` role in the tail.  It is a legal ordinary load but not a
        // direct-memory-fold candidate, so the stackifier must preserve it.
        let mut item = function(vec![instruction(
            0,
            X86Opcode::X87Load,
            vec![
                vreg(0, OperandRole::Def),
                format(X87MemoryFormat::Float32),
                vreg(6, OperandRole::Use),
            ],
            InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            },
        )]);
        item.virtual_registers[6].class = X86RegisterClass::Address16.machine_class();
        let result = allocate_x87_stack(&item).unwrap();
        let load = &result.blocks[0].instructions[0];
        assert_eq!(load.opcode, X86Opcode::X87Load.machine_opcode());
        assert_eq!(load.operands[0], st(0, OperandRole::Def));
        assert_eq!(load.operands[2], vreg(6, OperandRole::Use));
    }

    #[test]
    fn dead_stack_pop_uses_the_exact_two_operand_physical_form() {
        let result = allocate_x87_stack(&function(vec![instruction(
            0,
            X86Opcode::X87Load,
            vec![
                vreg(0, OperandRole::Def),
                format(X87MemoryFormat::Float80),
                frame(0),
            ],
            InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            },
        )]))
        .unwrap();
        let pop = result.blocks[0]
            .instructions
            .iter()
            .find(|item| item.opcode == X86Opcode::X87StackStorePop.machine_opcode())
            .expect("dead x87 value must be popped");
        assert_eq!(
            pop.operands,
            vec![st(0, OperandRole::Def), st(0, OperandRole::Use)]
        );
    }

    #[test]
    fn exchanges_a_buried_operand_to_the_top() {
        let result = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    volatile: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87Load,
                vec![
                    vreg(1, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(1),
                ],
                InstructionFlags {
                    may_load: true,
                    volatile: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87Load,
                vec![
                    vreg(3, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(3),
                ],
                InstructionFlags {
                    may_load: true,
                    volatile: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                3,
                X86Opcode::X87Add,
                vec![
                    vreg(2, OperandRole::Def),
                    vreg(0, OperandRole::Use),
                    vreg(1, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
            instruction(
                4,
                X86Opcode::X87StorePop,
                vec![
                    vreg(3, OperandRole::Use),
                    format(X87MemoryFormat::Float80),
                    frame(4),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]))
        .unwrap();
        assert!(opcodes(&result).contains(&X86Opcode::X87Exchange));
    }

    #[test]
    fn uses_pop_form_when_both_add_inputs_die() {
        let result = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87Load,
                vec![
                    vreg(1, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(1),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87Add,
                vec![
                    vreg(2, OperandRole::Def),
                    vreg(0, OperandRole::Use),
                    vreg(1, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
        ]))
        .unwrap();
        assert!(opcodes(&result).contains(&X86Opcode::X87AddPop));
    }

    #[test]
    fn preserves_reverse_subtract_polarity() {
        let result = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(1, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(1),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87Subtract,
                vec![
                    vreg(2, OperandRole::Def),
                    vreg(0, OperandRole::Use),
                    vreg(1, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
        ]))
        .unwrap();
        assert!(opcodes(&result).contains(&X86Opcode::X87SubtractReversePop));
    }

    #[test]
    fn pop_arithmetic_uses_the_python_orientation_truth_table() {
        // `floatalloc._Stack.arithmetic` has different orientation for pop
        // forms: with the left input at ST0, subtraction/division need `r`;
        // with the right input at ST0, they do not.  Check operation and the
        // exact `[st(i) use-def, st0 use]` encoding-direction contract.
        let cases = [
            (
                X86Opcode::X87Add,
                X86Opcode::X87AddPop,
                X86Opcode::X87AddPop,
            ),
            (
                X86Opcode::X87Subtract,
                X86Opcode::X87SubtractReversePop,
                X86Opcode::X87SubtractPop,
            ),
            (
                X86Opcode::X87Multiply,
                X86Opcode::X87MultiplyPop,
                X86Opcode::X87MultiplyPop,
            ),
            (
                X86Opcode::X87Divide,
                X86Opcode::X87DivideReversePop,
                X86Opcode::X87DividePop,
            ),
        ];
        for (selected, left_on_top, right_on_top) in cases {
            for (left_first, expected) in [(false, left_on_top), (true, right_on_top)] {
                let (first, second) = if left_first { (0, 1) } else { (1, 0) };
                let result = allocate_x87_stack(&function(vec![
                    instruction(
                        0,
                        X86Opcode::X87Load,
                        vec![
                            vreg(first, OperandRole::Def),
                            format(X87MemoryFormat::Float80),
                            frame(first),
                        ],
                        InstructionFlags {
                            may_load: true,
                            ..InstructionFlags::NONE
                        },
                    ),
                    instruction(
                        1,
                        X86Opcode::X87Load,
                        vec![
                            vreg(second, OperandRole::Def),
                            format(X87MemoryFormat::Float80),
                            frame(second),
                        ],
                        InstructionFlags {
                            may_load: true,
                            ..InstructionFlags::NONE
                        },
                    ),
                    instruction(
                        2,
                        selected,
                        vec![
                            vreg(2, OperandRole::Def),
                            vreg(0, OperandRole::Use),
                            vreg(1, OperandRole::Use),
                        ],
                        InstructionFlags::NONE,
                    ),
                ]))
                .unwrap();
                let operation = result.blocks[0]
                    .instructions
                    .iter()
                    .find(|item| item.opcode == expected.machine_opcode())
                    .expect("both-dead arithmetic emits one selected pop operation");
                assert_eq!(operation.opcode, expected.machine_opcode());
                assert_eq!(
                    operation.operands,
                    vec![st(1, OperandRole::UseDef), st(0, OperandRole::Use)]
                );
            }
        }
    }

    #[test]
    fn forked_value_gets_the_python_m80_bridge_despite_an_unrelated_store() {
        // Python `floatregions.bridged` first gives every value crossing this
        // fork an m80 home.  The disjoint frame-1 store does not invalidate
        // frame 0, but `_may_write` cannot erase the CFG bridge requirement.
        let mut item = function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Mov,
                vec![frame(1), immediate(0)],
                InstructionFlags {
                    may_store: true,
                    side_effects: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]);
        item.blocks[0].successors = vec![MachineBlockId::new(1), MachineBlockId::new(2)];
        for (id, destination) in [(1, 2), (2, 3)] {
            item.blocks.push(MachineBlock {
                id: MachineBlockId::new(id),
                instructions: vec![instruction(
                    id,
                    X86Opcode::X87StorePop,
                    vec![
                        vreg(0, OperandRole::Use),
                        format(X87MemoryFormat::Float32),
                        frame(destination),
                    ],
                    InstructionFlags {
                        may_store: true,
                        ..InstructionFlags::NONE
                    },
                )],
                successors: Vec::new(),
            });
        }
        let result = allocate_x87_stack(&item).unwrap();
        assert_eq!(
            result
                .frame_objects
                .iter()
                .filter(|frame| frame.size == 10 && matches!(frame.kind, FrameObjectKind::Temporary))
                .count(),
            1
        );
        assert_eq!(
            opcodes(&result)[..3],
            [X86Opcode::X87Load, X86Opcode::X87StorePop, X86Opcode::Mov]
        );
    }

    #[test]
    fn reread_moves_to_a_last_reader_that_writes_the_same_cell() {
        // Python `floatalloc._rereadable` scans `position + 1 .. reads[-1]`:
        // the last reader is deliberately excluded even when that reader is
        // an `fstp` to the load's own cell.  Including it kept the original
        // `fld` before unrelated integer work instead of rereading at use.
        let result = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(1, X86Opcode::Mov, Vec::new(), InstructionFlags::NONE),
            instruction(
                2,
                X86Opcode::X87StorePop,
                vec![
                    vreg(0, OperandRole::Use),
                    format(X87MemoryFormat::Float32),
                    frame(0),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]))
        .unwrap();

        assert_eq!(
            opcodes(&result),
            vec![X86Opcode::Mov, X86Opcode::X87Load, X86Opcode::X87StorePop,]
        );
    }

    #[test]
    fn known_effect_flags_do_not_invent_memory_destinations() {
        // Python `floatalloc._may_write` reads semantic destinations.  A
        // known call, volatile read, or side-effect-only wait has no memory
        // destination merely because its Machine-IR flags describe effects.
        let home = Home {
            reload_opcode: X86Opcode::X87Load,
            format: X87MemoryFormat::Float32,
            address: vec![frame(0)],
            direct: true,
        };
        for instruction in [
            instruction(
                0,
                X86Opcode::CallFar,
                Vec::new(),
                InstructionFlags {
                    call: true,
                    side_effects: true,
                    may_load: true,
                    may_store: true,
                    volatile: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Load,
                vec![frame(1)],
                InstructionFlags {
                    may_load: true,
                    volatile: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::Wait,
                Vec::new(),
                InstructionFlags {
                    side_effects: true,
                    ..InstructionFlags::NONE
                },
            ),
        ] {
            assert!(!may_write_home(&instruction, &home, &BTreeMap::new()));
        }
    }

    #[test]
    fn volatile_load_clears_equivalent_cells_without_becoming_a_write() {
        // Python `floatalloc._equivalent_loads` clears its complete table for
        // a volatile read before consulting `_may_write`.  Keeping that rule
        // separate lets `_may_write` remain the literal destination proof.
        let load = |id, destination, volatile| {
            instruction(
                id,
                X86Opcode::X87Load,
                vec![
                    vreg(destination, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    volatile,
                    ..InstructionFlags::NONE
                },
            )
        };
        let sequence = vec![load(0, 0, false), load(1, 1, true), load(2, 2, false)];
        let x87 = BTreeSet::from([
            VirtualRegisterId::new(0),
            VirtualRegisterId::new(1),
            VirtualRegisterId::new(2),
        ]);

        assert!(equivalent_loads(&sequence, &x87, &BTreeMap::new()).is_empty());
    }

    #[test]
    fn defining_an_address_component_invalidates_only_that_home() {
        // Python `floatalloc._may_write` invalidates a cell when an
        // instruction defines its base, index, or selector SSA value.
        let home = Home {
            reload_opcode: X86Opcode::X87Load,
            format: X87MemoryFormat::Float32,
            address: vec![vreg(7, OperandRole::Use)],
            direct: true,
        };
        let writes = |register| {
            instruction(
                register,
                X86Opcode::Mov,
                vec![vreg(register, OperandRole::Def)],
                InstructionFlags::NONE,
            )
        };

        assert!(may_write_home(&writes(7), &home, &BTreeMap::new()));
        assert!(!may_write_home(&writes(8), &home, &BTreeMap::new()));
    }

    #[test]
    fn cell_specific_store_ranges_preserve_only_disjoint_homes() {
        // `_may_write` is cell-specific: a different frame slot may not
        // force an m80 bridge, while same-slot, overlapping-range, and
        // register-indirect writes must still invalidate the source home.
        let home = Home {
            reload_opcode: X86Opcode::X87Load,
            format: X87MemoryFormat::Float32,
            address: vec![frame(0)],
            direct: true,
        };
        let store = |address: MachineOperand| {
            instruction(
                0,
                X86Opcode::X87StorePop,
                vec![
                    vreg(1, OperandRole::Use),
                    format(X87MemoryFormat::Float32),
                    address,
                ],
                InstructionFlags {
                    may_store: true,
                    side_effects: true,
                    ..InstructionFlags::NONE
                },
            )
        };
        assert!(!may_write_home(&store(frame(1)), &home, &BTreeMap::new()));
        assert!(may_write_home(&store(frame(0)), &home, &BTreeMap::new()));
        let overlapping = MachineOperand {
            kind: MachineOperandKind::FrameIndex {
                index: FrameIndex::new(0),
                addend: 2,
            },
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        };
        assert!(may_write_home(&store(overlapping), &home, &BTreeMap::new()));
        let indirect = vreg(15, OperandRole::Use);
        assert!(may_write_home(&store(indirect), &home, &BTreeMap::new()));
    }

    #[test]
    fn register_store_overlap_uses_the_machine_value_width() {
        // Python `_may_write` compares the complete destination width.  A
        // dword store beginning two bytes before this f32 cell overlaps it;
        // treating every register store as one byte would miss the write.
        let home = Home {
            reload_opcode: X86Opcode::X87Load,
            format: X87MemoryFormat::Float32,
            address: vec![frame(0)],
            direct: true,
        };
        let destination = MachineOperand {
            kind: MachineOperandKind::FrameIndex {
                index: FrameIndex::new(0),
                addend: -2,
            },
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        };
        let store = instruction(
            0,
            X86Opcode::Store,
            vec![destination, vreg(7, OperandRole::Use)],
            InstructionFlags {
                may_store: true,
                side_effects: true,
                ..InstructionFlags::NONE
            },
        );
        let widths = BTreeMap::from([(VirtualRegisterId::new(7), 4)]);

        assert!(may_write_home(&store, &home, &widths));
    }

    #[test]
    fn stores_a_live_value_without_popping_it() {
        let result = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87StorePop,
                vec![
                    vreg(0, OperandRole::Use),
                    format(X87MemoryFormat::Float80),
                    frame(1),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87StorePop,
                vec![
                    vreg(0, OperandRole::Use),
                    format(X87MemoryFormat::Float80),
                    frame(2),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]))
        .unwrap();
        assert_eq!(opcodes(&result)[1], X86Opcode::X87Store);
    }

    #[test]
    fn spills_the_ninth_value_to_a_temporary_extended_cell() {
        let mut instructions = Vec::new();
        for id in 0..9 {
            instructions.push(instruction(
                id,
                X86Opcode::X87Load,
                vec![
                    vreg(id, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(id),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ));
        }
        let result = allocate_x87_stack(&function(instructions)).unwrap();
        assert!(result.frame_objects.iter().any(|item| item.size == 10
            && item.alignment == 2
            && matches!(item.kind, FrameObjectKind::Temporary)));
        assert_eq!(
            opcodes(&result)
                .iter()
                .filter(|opcode| **opcode == X86Opcode::X87StorePop)
                .count(),
            1
        );
        let spill = result.blocks[0]
            .instructions
            .iter()
            .find(|item| {
                item.id.get() > 8 && item.opcode == X86Opcode::X87StorePop.machine_opcode()
            })
            .expect("ninth live value spills through a generated m80 store");
        assert_eq!(
            spill.flags,
            InstructionFlags {
                may_store: true,
                side_effects: true,
                ..InstructionFlags::NONE
            }
        );
    }

    #[test]
    fn comparison_transfers_status_word_to_flags() {
        let result = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87Load,
                vec![
                    vreg(1, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(1),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87Compare,
                vec![vreg(0, OperandRole::Use), vreg(1, OperandRole::Use)],
                InstructionFlags::NONE,
            ),
        ]))
        .unwrap();
        assert!(opcodes(&result).windows(3).any(|items| items
            == [
                X86Opcode::X87ComparePop2,
                X86Opcode::X87StoreStatusWord,
                X86Opcode::Sahf
            ]));
    }

    #[test]
    fn casted_memory_comparison_keeps_its_load_effect() {
        // qmove first exposed this after its f32-to-f80 copies began
        // preserving deferred homes: Python selects `fcomp m32`, whose
        // Machine form must still say that it reads memory.
        let result = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Copy,
                vec![vreg(1, OperandRole::Def), vreg(0, OperandRole::Use)],
                InstructionFlags {
                    copy: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87Load,
                vec![
                    vreg(2, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(1),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                3,
                X86Opcode::Copy,
                vec![vreg(3, OperandRole::Def), vreg(2, OperandRole::Use)],
                InstructionFlags {
                    copy: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                4,
                X86Opcode::X87Compare,
                vec![vreg(1, OperandRole::Use), vreg(3, OperandRole::Use)],
                InstructionFlags::NONE,
            ),
        ]))
        .unwrap();
        let compare = result.blocks[0]
            .instructions
            .iter()
            .find(|instruction| instruction.opcode == X86Opcode::X87ComparePop.machine_opcode())
            .expect("comparison uses Python's direct memory form");
        assert_eq!(
            compare.flags,
            InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            }
        );
    }

    #[test]
    fn keeps_a_pure_x87_call_result_in_the_post_call_region() {
        // Python `floatalloc.allocated` increments the region before recording
        // a CALL instruction's position.  A pure x87 definition on the call
        // is therefore born after the ABI boundary: it is ST0's result, not
        // an input carried through the call.  `_r_point_leaf` in qbsp needs
        // exactly this result for its following comparison.
        let mut result = vreg(1, OperandRole::Def);
        result.constraint = Some(crate::old::codegen::machine::RegisterConstraint::Fixed(
            X86Register::St0.physical(),
        ));
        let item = function(vec![
            instruction(
                0,
                X86Opcode::CallNear,
                vec![result],
                InstructionFlags {
                    call: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87StorePop,
                vec![
                    vreg(1, OperandRole::Use),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]);
        let reachable = BTreeSet::from([MachineBlockId::new(0)]);
        let labels = x87_region_labels(&item, &reachable, &BTreeMap::new());
        assert_eq!(
            labels[&(MachineBlockId::new(0), 0)],
            labels[&(MachineBlockId::new(0), 1)],
            "the call result and its first reader share the post-call region",
        );

        let allocated = allocate_x87_stack(&item)
            .expect("a call result is born in ST0 after the call boundary");
        assert_eq!(
            opcodes(&allocated),
            vec![X86Opcode::CallNear, X86Opcode::X87StorePop],
            "the result is not pointlessly bridged through m80 before its first use",
        );
        assert!(allocated.frame_objects.is_empty());
        assert_eq!(
            allocated.blocks[0].instructions[0].operands[0],
            st(0, OperandRole::Def),
            "the call result remains the ABI-defined ST0 value",
        );

        // Ordinary calls have no x87 definition; the recognizer must not
        // index an empty result list while deciding that fact.
        let x87 = BTreeSet::from([VirtualRegisterId::new(1)]);
        let ordinary_call = instruction(
            2,
            X86Opcode::CallNear,
            Vec::new(),
            InstructionFlags {
                call: true,
                ..InstructionFlags::NONE
            },
        );
        assert_eq!(x87_call_result(&ordinary_call, &x87), None);

        // The post-call rule is narrowly the ABI-defined ST0 result, not a
        // license to rewrite an arbitrary x87 call operand as ST0.
        let malformed = function(vec![instruction(
            3,
            X86Opcode::CallNear,
            vec![vreg(1, OperandRole::Def)],
            InstructionFlags {
                call: true,
                ..InstructionFlags::NONE
            },
        )]);
        assert!(matches!(
            allocate_x87_stack(&malformed),
            Err(X86FloatAllocationError::MalformedInstruction {
                reason: "x87 call result must be one pure fixed-st0 definition",
                ..
            })
        ));
    }

    #[test]
    fn bridges_a_live_value_across_an_opaque_call() {
        // Regression for the C qmove symptom: a value surviving a call must
        // be written to its m80 bridge immediately after its definition, then
        // reloaded afterwards.  A boundary-time dump could instead `fld` an
        // uninitialized bridge cell and write that garbage back to itself.
        let item = function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::CallNear,
                Vec::new(),
                InstructionFlags {
                    call: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87StorePop,
                vec![
                    vreg(0, OperandRole::Use),
                    format(X87MemoryFormat::Float80),
                    frame(1),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]);
        let result = allocate_x87_stack(&item).unwrap();
        assert_eq!(
            opcodes(&result),
            vec![
                X86Opcode::X87Load,
                X86Opcode::X87StorePop,
                X86Opcode::CallNear,
                X86Opcode::X87Load,
                X86Opcode::X87StorePop,
            ]
        );
        assert!(result.frame_objects.iter().any(|frame| {
            frame.size == 10
                && frame.alignment == 2
                && matches!(frame.kind, FrameObjectKind::Temporary)
        }));
        let instructions = &result.blocks[0].instructions;
        let bridge = &instructions[1];
        let reload = &instructions[3];
        assert_eq!(bridge.opcode, X86Opcode::X87StorePop.machine_opcode());
        assert_eq!(reload.opcode, X86Opcode::X87Load.machine_opcode());
        assert_eq!(instructions[2].opcode, X86Opcode::CallNear.machine_opcode());
        assert_eq!(bridge.operands[2], reload.operands[2]);
        assert!(result.frame_objects.iter().any(|frame| {
            frame_operand(frame.index) == bridge.operands[2]
                && matches!(frame.kind, FrameObjectKind::Temporary)
        }));
        assert_eq!(
            bridge.flags,
            InstructionFlags {
                may_store: true,
                side_effects: true,
                ..InstructionFlags::NONE
            }
        );
    }

    #[test]
    fn ordinary_float_store_keeps_its_region_but_a_call_ends_it() {
        // `floatregions.boundary` does not split on `fstp`: it is an ordinary
        // memory effect which only invalidates a retained home.  Treating it
        // as a boundary made the bridge store manufacture a second crossing
        // local in qmove.  Calls remain region boundaries.
        let item = function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87Store,
                vec![
                    vreg(0, OperandRole::Use),
                    format(X87MemoryFormat::Float80),
                    frame(1),
                ],
                InstructionFlags {
                    may_store: true,
                    side_effects: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::CallNear,
                Vec::new(),
                InstructionFlags {
                    call: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                3,
                X86Opcode::X87StorePop,
                vec![
                    vreg(0, OperandRole::Use),
                    format(X87MemoryFormat::Float80),
                    frame(2),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]);
        let labels = x87_region_labels(
            &item,
            &BTreeSet::from([MachineBlockId::new(0)]),
            &BTreeMap::new(),
        );
        assert_eq!(
            labels[&(MachineBlockId::new(0), 0)],
            labels[&(MachineBlockId::new(0), 1)]
        );
        assert_ne!(
            labels[&(MachineBlockId::new(0), 1)],
            labels[&(MachineBlockId::new(0), 3)]
        );
    }

    #[test]
    fn bridges_a_live_value_to_each_side_of_a_cfg_fork() {
        let mut item = function(vec![instruction(
            0,
            X86Opcode::X87Load,
            vec![
                vreg(0, OperandRole::Def),
                format(X87MemoryFormat::Float80),
                frame(0),
            ],
            InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            },
        )]);
        item.blocks[0].successors = vec![MachineBlockId::new(1), MachineBlockId::new(2)];
        for (id, frame_index) in [(1, 1), (2, 2)] {
            item.blocks.push(MachineBlock {
                id: MachineBlockId::new(id),
                instructions: vec![instruction(
                    id,
                    X86Opcode::X87StorePop,
                    vec![
                        vreg(0, OperandRole::Use),
                        format(X87MemoryFormat::Float80),
                        frame(frame_index),
                    ],
                    InstructionFlags {
                        may_store: true,
                        ..InstructionFlags::NONE
                    },
                )],
                successors: Vec::new(),
            });
        }
        let result = allocate_x87_stack(&item).unwrap();
        assert_eq!(
            opcodes(&result),
            vec![X86Opcode::X87Load, X86Opcode::X87StorePop]
        );
        for block in result.blocks.iter().skip(1) {
            assert_eq!(
                block
                    .instructions
                    .iter()
                    .filter_map(|item| X86Opcode::from_machine_opcode(item.opcode))
                    .collect::<Vec<_>>(),
                vec![X86Opcode::X87Load, X86Opcode::X87StorePop],
            );
        }
    }

    #[test]
    fn bridges_before_a_fork_terminator_and_keeps_it_last() {
        let mut item = function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Jump,
                Vec::new(),
                InstructionFlags {
                    terminator: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]);
        item.blocks[0].successors = vec![MachineBlockId::new(1), MachineBlockId::new(2)];
        for id in [1, 2] {
            item.blocks.push(MachineBlock {
                id: MachineBlockId::new(id),
                instructions: vec![instruction(
                    id + 1,
                    X86Opcode::X87StorePop,
                    vec![
                        vreg(0, OperandRole::Use),
                        format(X87MemoryFormat::Float80),
                        frame(id),
                    ],
                    InstructionFlags {
                        may_store: true,
                        ..InstructionFlags::NONE
                    },
                )],
                successors: Vec::new(),
            });
        }
        let result = allocate_x87_stack(&item).unwrap();
        assert_eq!(
            X86Opcode::from_machine_opcode(result.blocks[0].instructions.last().unwrap().opcode),
            Some(X86Opcode::Jump),
        );
        assert!(
            result.blocks[0]
                .instructions
                .iter()
                .any(|item| { item.opcode == X86Opcode::X87StorePop.machine_opcode() })
        );
        assert!(result.blocks.iter().all(|block| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.flags.terminator)
                .is_none_or(|position| position + 1 == block.instructions.len())
        }));
    }

    #[test]
    fn keeps_x87_state_across_a_proven_straight_terminator_edge() {
        let mut item = function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    volatile: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Jump,
                Vec::new(),
                InstructionFlags {
                    terminator: true,
                    ..InstructionFlags::NONE
                },
            ),
        ]);
        item.blocks[0].successors = vec![MachineBlockId::new(1)];
        item.blocks.push(MachineBlock {
            id: MachineBlockId::new(1),
            instructions: vec![instruction(
                2,
                X86Opcode::X87StorePop,
                vec![
                    vreg(0, OperandRole::Use),
                    format(X87MemoryFormat::Float80),
                    frame(1),
                ],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            )],
            successors: Vec::new(),
        });
        let result = allocate_x87_stack(&item).unwrap();
        assert_eq!(opcodes(&result), vec![X86Opcode::X87Load, X86Opcode::Jump]);
        assert_eq!(
            result.blocks[1]
                .instructions
                .iter()
                .filter_map(|item| X86Opcode::from_machine_opcode(item.opcode))
                .collect::<Vec<_>>(),
            vec![X86Opcode::X87StorePop],
        );
        assert!(result.frame_objects.is_empty());
    }

    #[test]
    fn folds_a_safe_direct_multiply_but_not_after_a_store() {
        let folded = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87Load,
                vec![
                    vreg(1, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(1),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87Multiply,
                vec![
                    vreg(2, OperandRole::Def),
                    vreg(0, OperandRole::Use),
                    vreg(1, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
        ]))
        .unwrap();
        assert_eq!(
            opcodes(&folded),
            vec![
                X86Opcode::X87Load,
                X86Opcode::X87Multiply,
                X86Opcode::X87StackStorePop
            ]
        );

        let unfurled = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float80),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::X87Load,
                vec![
                    vreg(1, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(1),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::Store,
                vec![frame(1), immediate(32), immediate(0)],
                InstructionFlags {
                    may_store: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                3,
                X86Opcode::X87Multiply,
                vec![
                    vreg(2, OperandRole::Def),
                    vreg(0, OperandRole::Use),
                    vreg(1, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
        ]))
        .unwrap();
        assert_eq!(
            opcodes(&unfurled),
            vec![
                X86Opcode::X87Load,
                X86Opcode::Store,
                X86Opcode::X87Load,
                X86Opcode::X87MultiplyPop,
                X86Opcode::X87StackStorePop,
            ],
            "the same-cell store forces the scalar input onto the stack before the write"
        );
    }

    #[test]
    fn folds_a_casted_scalar_home_into_memory_arithmetic() {
        // qmove exposed this at the first `vec3_dot`: selection represents
        // the Python LIR `fld` result as an x87 `Copy` for f32 -> f80.  The
        // copy is a stack-name change, so `_Stack.arithmetic` must retain the
        // direct scalar home and still select `fmul m32`, rather than loading
        // both inputs and using `fmulp`.
        let result = allocate_x87_stack(&function(vec![
            instruction(
                0,
                X86Opcode::X87Load,
                vec![
                    vreg(0, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(0),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                1,
                X86Opcode::Copy,
                vec![vreg(1, OperandRole::Def), vreg(0, OperandRole::Use)],
                InstructionFlags {
                    copy: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                2,
                X86Opcode::X87Load,
                vec![
                    vreg(2, OperandRole::Def),
                    format(X87MemoryFormat::Float32),
                    frame(1),
                ],
                InstructionFlags {
                    may_load: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                3,
                X86Opcode::Copy,
                vec![vreg(3, OperandRole::Def), vreg(2, OperandRole::Use)],
                InstructionFlags {
                    copy: true,
                    ..InstructionFlags::NONE
                },
            ),
            instruction(
                4,
                X86Opcode::X87Multiply,
                vec![
                    vreg(4, OperandRole::Def),
                    vreg(1, OperandRole::Use),
                    vreg(3, OperandRole::Use),
                ],
                InstructionFlags::NONE,
            ),
        ]))
        .unwrap();
        assert_eq!(
            opcodes(&result),
            vec![
                X86Opcode::X87Load,
                X86Opcode::X87Multiply,
                X86Opcode::X87StackStorePop,
            ]
        );
        let multiply = &result.blocks[0].instructions[1];
        assert_eq!(
            multiply.operands,
            vec![
                st(0, OperandRole::UseDef),
                format(X87MemoryFormat::Float32),
                frame(1),
            ]
        );
    }

    #[test]
    fn leaves_no_x87_virtual_declaration_or_operand() {
        let result = allocate_x87_stack(&function(vec![instruction(
            0,
            X86Opcode::X87Load,
            vec![
                vreg(0, OperandRole::Def),
                format(X87MemoryFormat::Float80),
                frame(0),
            ],
            InstructionFlags {
                may_load: true,
                ..InstructionFlags::NONE
            },
        )]))
        .unwrap();
        assert!(result.virtual_registers.is_empty());
        assert!(
            remaining_x87_virtual(&result, &(0..16).map(VirtualRegisterId::new).collect())
                .is_none()
        );
    }
}
