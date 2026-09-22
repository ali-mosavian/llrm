//! Final allocated x86 control-flow placement and jump threading.
//!
//! This is the bounded translation of `qbopt/backend/jumps.py::{placed,
//! _onward,_tests,threaded,_step,_passage,_through,_retargeted,
//! _predecessors,_reachable}`.  It deliberately does *not* port Python's
//! `optimized`: tail merging, machine DCE, and preference by encoded cost are
//! later, separate work.  The two source facts cannot be inferred from a
//! Machine IR instruction's ID or anchor flag; an object frontend supplies
//! them and fresh placement IDs stay outside both sets.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    MachineBlock, MachineBlockId, MachineFunction, MachineInstruction, MachineInstructionId,
    MachineOperandKind,
};

use super::{ConditionCode, X86Opcode};

/// Source and measurement facts not represented by target-neutral Machine IR.
///
/// `non_inserted` is Python LIR's `not Insn.inserted`: removing such an
/// instruction must leave an anchor even if no byte span is currently known.
/// `owns_source_bytes` is narrower: it is Python's nonempty `covers` or
/// `spread`, and only it retains an otherwise unreachable anchor-only block.
/// The sets normally overlap but intentionally are not interchangeable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ControlFlowFacts {
    pub non_inserted: BTreeSet<MachineInstructionId>,
    pub owns_source_bytes: BTreeSet<MachineInstructionId>,
    pub protected_loop_headers: BTreeSet<MachineBlockId>,
}

/// A malformed control-flow shape that Python's assembler would refuse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlFlowError {
    DuplicateBlock(MachineBlockId),
    DuplicateInstruction(MachineInstructionId),
    AmbiguousFallthrough { block: MachineBlockId },
    ExhaustedInstructionIds,
}

impl fmt::Display for ControlFlowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateBlock(id) => write!(formatter, "duplicate machine block {id}"),
            Self::DuplicateInstruction(id) => {
                write!(formatter, "duplicate machine instruction {id}")
            }
            Self::AmbiguousFallthrough { block } => {
                write!(
                    formatter,
                    "block {block} has more than one implicit fallthrough"
                )
            }
            Self::ExhaustedInstructionIds => {
                write!(formatter, "no fresh machine instruction ID remains")
            }
        }
    }
}

impl Error for ControlFlowError {}

/// Materialize falls, choose an execution order, then thread the resulting jumps.
///
/// This maps exactly to Python `threaded(placed(body))`, not to `optimized`.
/// The input is unchanged; returned blocks retain their original IDs and source
/// order whenever Python's source-order walk needs a tie-break.
pub fn place_and_thread(
    function: &MachineFunction,
    facts: &ControlFlowFacts,
) -> Result<MachineFunction, ControlFlowError> {
    checked(function)?;
    let placed = placed(function, facts)?;
    Ok(threaded(&placed, facts))
}

fn checked(function: &MachineFunction) -> Result<(), ControlFlowError> {
    let mut blocks = BTreeSet::new();
    let mut instructions = BTreeSet::new();
    for block in &function.blocks {
        if !blocks.insert(block.id) {
            return Err(ControlFlowError::DuplicateBlock(block.id));
        }
        for instruction in &block.instructions {
            if !instructions.insert(instruction.id) {
                return Err(ControlFlowError::DuplicateInstruction(instruction.id));
            }
        }
    }
    Ok(())
}

fn placed(
    function: &MachineFunction,
    facts: &ControlFlowFacts,
) -> Result<MachineFunction, ControlFlowError> {
    let mut ids = FreshIds::new(function, facts);
    let mut explicit = Vec::with_capacity(function.blocks.len());
    for block in &function.blocks {
        let mut block = block.clone();
        if let Some(fall) = falls_to(&block)? {
            block.instructions.push(jump(ids.next()?, fall));
        }
        explicit.push(block);
    }

    let index = block_indices(&explicit);
    let natural = loops(&explicit, function.entry, &index);
    let tests = tests(&natural, function.entry, &explicit, &index);

    // Python's loops() is innermost first.  `or_insert` preserves that first,
    // nearest loop for nested blocks.
    let mut inside = BTreeMap::new();
    for natural_loop in &natural {
        for block in &natural_loop.body {
            inside
                .entry(*block)
                .or_insert_with(|| natural_loop.body.clone());
        }
    }

    let mut order = Vec::with_capacity(explicit.len());
    let mut done = BTreeSet::new();
    let mut current = Some(function.entry);
    let mut source = None;
    while order.len() < explicit.len() {
        if current.is_none_or(|id| done.contains(&id) || !index.contains_key(&id)) {
            current = explicit
                .iter()
                .find(|block| !done.contains(&block.id))
                .map(|block| block.id);
            source = None;
        }
        let current_id = current.expect("an unplaced source-order block exists");
        if let Some((inner, latches)) = tests.get(&current_id) {
            if !source.is_some_and(|id| latches.contains(&id)) && !done.contains(inner) {
                current = Some(*inner);
                source = None;
                continue;
            }
        }
        let block = explicit[*index.get(&current_id).expect("checked block index")].clone();
        order.push(block.clone());
        done.insert(current_id);
        current = onward(
            &block,
            &done,
            inside.get(&current_id).cloned().unwrap_or_default(),
            &explicit,
            &index,
            facts,
        );
        source = Some(current_id);
    }

    let mut result = function.clone();
    result.blocks = order;
    Ok(result)
}

struct FreshIds {
    used: BTreeSet<MachineInstructionId>,
    next: u32,
}

impl FreshIds {
    fn new(function: &MachineFunction, facts: &ControlFlowFacts) -> Self {
        let mut used = facts.non_inserted.clone();
        used.extend(facts.owns_source_bytes.iter().copied());
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            used.insert(instruction.id);
        }
        let next = used.last().map_or(0, |id| id.get());
        Self { used, next }
    }

    fn next(&mut self) -> Result<MachineInstructionId, ControlFlowError> {
        loop {
            let Some(raw) = self.next.checked_add(1) else {
                return Err(ControlFlowError::ExhaustedInstructionIds);
            };
            self.next = raw;
            let id = MachineInstructionId::new(raw);
            if self.used.insert(id) {
                return Ok(id);
            }
        }
    }
}

fn jump(id: MachineInstructionId, target: MachineBlockId) -> MachineInstruction {
    MachineInstruction {
        id,
        opcode: X86Opcode::Jump.machine_opcode(),
        operands: vec![block_operand(target)],
        flags: crate::codegen::machine::InstructionFlags {
            terminator: true,
            ..crate::codegen::machine::InstructionFlags::NONE
        },
    }
}

fn falls_to(block: &MachineBlock) -> Result<Option<MachineBlockId>, ControlFlowError> {
    let last = real(&block.instructions).last().copied();
    if matches!(
        last.and_then(opcode),
        Some(X86Opcode::Jump | X86Opcode::ReturnNear | X86Opcode::ReturnFar)
    ) {
        return Ok(None);
    }
    let taken = last.and_then(target);
    let mut rest = block
        .successors
        .iter()
        .copied()
        .filter(|successor| Some(*successor) != taken)
        .collect::<Vec<_>>();
    if rest.is_empty() {
        rest = block.successors.clone();
    }
    match rest.as_slice() {
        [] => Ok(None),
        [fall] => Ok(Some(*fall)),
        _ => Err(ControlFlowError::AmbiguousFallthrough { block: block.id }),
    }
}

fn onward(
    block: &MachineBlock,
    done: &BTreeSet<MachineBlockId>,
    inside: BTreeSet<MachineBlockId>,
    blocks: &[MachineBlock],
    index: &BTreeMap<MachineBlockId, usize>,
    facts: &ControlFlowFacts,
) -> Option<MachineBlockId> {
    let real = real(&block.instructions);
    let last = real.last().copied()?;
    if opcode(last) != Some(X86Opcode::Jump) {
        return None;
    }
    let mut targets = vec![target(last)?];
    if let Some(branch) = real.get(real.len().saturating_sub(2)).copied() {
        if opcode(branch) == Some(X86Opcode::JumpConditional) {
            if let Some(branch_target) = target(branch) {
                targets.push(branch_target);
                let jump_target = targets[0];
                let arm = index.get(&branch_target).map(|at| &blocks[*at]);
                let passage = index
                    .get(&jump_target)
                    .and_then(|at| passage(&blocks[*at], facts));
                let join = passage.unwrap_or(jump_target);
                if arm.is_some_and(|arm| arm.successors.as_slice() == [join]) {
                    targets.swap(0, 1);
                }
            }
        }
    }
    targets
        .iter()
        .copied()
        .find(|target| !done.contains(target) && inside.contains(target))
        .or_else(|| targets.into_iter().find(|target| !done.contains(target)))
}

#[derive(Clone)]
struct NaturalLoop {
    header: MachineBlockId,
    latches: BTreeSet<MachineBlockId>,
    body: BTreeSet<MachineBlockId>,
}

fn loops(
    blocks: &[MachineBlock],
    entry: MachineBlockId,
    index: &BTreeMap<MachineBlockId, usize>,
) -> Vec<NaturalLoop> {
    let reachable = reachable_ids(blocks, entry, index);
    let all = reachable.clone();
    let predecessors = predecessors(blocks, &reachable);
    let mut dominators = BTreeMap::new();
    for block in blocks {
        dominators.insert(
            block.id,
            if reachable.contains(&block.id) {
                all.clone()
            } else {
                BTreeSet::new()
            },
        );
    }
    if index.contains_key(&entry) {
        dominators.insert(entry, BTreeSet::from([entry]));
    }
    loop {
        let mut changed = false;
        for block in blocks {
            if block.id == entry || !reachable.contains(&block.id) {
                continue;
            }
            let reaching = predecessors
                .get(&block.id)
                .into_iter()
                .flatten()
                .filter_map(|predecessor| dominators.get(predecessor))
                .collect::<Vec<_>>();
            let mut now = reaching
                .first()
                .map(|dominators| (*dominators).clone())
                .unwrap_or_default();
            for other in reaching.iter().skip(1) {
                now = now.intersection(other).copied().collect();
            }
            now.insert(block.id);
            if dominators.get(&block.id) != Some(&now) {
                dominators.insert(block.id, now);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut latches = BTreeMap::<MachineBlockId, BTreeSet<MachineBlockId>>::new();
    let mut bodies = BTreeMap::<MachineBlockId, BTreeSet<MachineBlockId>>::new();
    let mut header_order = Vec::new();
    for block in blocks {
        for successor in &block.successors {
            if index.contains_key(successor)
                && dominators
                    .get(&block.id)
                    .is_some_and(|doms| doms.contains(successor))
            {
                if !latches.contains_key(successor) {
                    header_order.push(*successor);
                }
                latches.entry(*successor).or_default().insert(block.id);
                let body = natural_body(block.id, *successor, &predecessors);
                bodies.entry(*successor).or_default().extend(body);
            }
        }
    }
    // Python's dict remembers the header when its first back edge is found.
    // Keep that latch/source-block discovery order, then perform the same
    // stable sort by body size so equal-sized loops retain it.
    let mut found = header_order
        .into_iter()
        .map(|header| NaturalLoop {
            header,
            latches: latches[&header].clone(),
            body: bodies.get(&header).cloned().unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    found.sort_by_key(|natural_loop| natural_loop.body.len());
    found
}

fn natural_body(
    latch: MachineBlockId,
    header: MachineBlockId,
    predecessors: &BTreeMap<MachineBlockId, BTreeSet<MachineBlockId>>,
) -> BTreeSet<MachineBlockId> {
    let mut body = BTreeSet::from([header]);
    if latch == header {
        return body;
    }
    body.insert(latch);
    let mut pending = vec![latch];
    while let Some(block) = pending.pop() {
        for predecessor in predecessors.get(&block).into_iter().flatten() {
            if body.insert(*predecessor) {
                pending.push(*predecessor);
            }
        }
    }
    body
}

fn tests(
    natural: &[NaturalLoop],
    entry: MachineBlockId,
    blocks: &[MachineBlock],
    index: &BTreeMap<MachineBlockId, usize>,
) -> BTreeMap<MachineBlockId, (MachineBlockId, BTreeSet<MachineBlockId>)> {
    let mut found = BTreeMap::new();
    for natural_loop in natural {
        let block = &blocks[index[&natural_loop.header]];
        let real = real(&block.instructions);
        let inner = block
            .successors
            .iter()
            .copied()
            .filter(|successor| {
                natural_loop.body.contains(successor) && *successor != natural_loop.header
            })
            .collect::<Vec<_>>();
        let distinct_successors = block.successors.iter().copied().collect::<BTreeSet<_>>();
        if natural_loop.header != entry
            && real.len() >= 2
            && opcode(real[real.len() - 2]) == Some(X86Opcode::JumpConditional)
            && distinct_successors.len() == 2
            && inner.len() == 1
        {
            found.insert(
                natural_loop.header,
                (inner[0], natural_loop.latches.clone()),
            );
        }
    }
    found
}

fn threaded(function: &MachineFunction, facts: &ControlFlowFacts) -> MachineFunction {
    let mut result = reachable(function, function.blocks.clone(), facts);
    loop {
        let (next, changed) = step(&result, facts);
        result = next;
        if !changed {
            return result;
        }
    }
}

fn step(function: &MachineFunction, facts: &ControlFlowFacts) -> (MachineFunction, bool) {
    let mut blocks = function.blocks.clone();
    let index = block_indices(&blocks);
    for position in 0..blocks.len() {
        let block = blocks[position].clone();
        let real_instructions = real(&block.instructions);
        let Some(last) = real_instructions.last().copied() else {
            continue;
        };
        let Some(original_target) = target(last) else {
            continue;
        };
        if !matches!(
            opcode(last),
            Some(X86Opcode::Jump | X86Opcode::JumpConditional)
        ) {
            continue;
        }
        let after = blocks.get(position + 1).map(|block| block.id);
        let destination = through(
            &blocks,
            &index,
            original_target,
            &facts.protected_loop_headers,
            facts,
        );
        if destination != original_target {
            blocks[position] = retargeted(&block, last.id, destination);
            return (reachable(function, blocks, facts), true);
        }
        if opcode(last) == Some(X86Opcode::Jump) && Some(destination) == after {
            let instructions = block
                .instructions
                .iter()
                .filter_map(|instruction| {
                    if instruction.id != last.id {
                        Some(instruction.clone())
                    } else if facts.non_inserted.contains(&instruction.id) {
                        Some(instruction.anchor(X86Opcode::Nothing.machine_opcode()))
                    } else {
                        None
                    }
                })
                .collect();
            blocks[position].instructions = instructions;
            return (reachable(function, blocks, facts), true);
        }
        if opcode(last) == Some(X86Opcode::Jump) && real_instructions.len() > 1 {
            let branch = real_instructions[real_instructions.len() - 2];
            if opcode(branch) == Some(X86Opcode::JumpConditional)
                && target(branch) == after
                && valid_condition(branch).is_some()
            {
                let mut inverted = invert(branch.clone()).expect("checked legal condition");
                retarget_instruction(&mut inverted, destination);
                blocks[position].instructions = block
                    .instructions
                    .iter()
                    .filter(|instruction| instruction.id != last.id)
                    .map(|instruction| {
                        if instruction.id == branch.id {
                            inverted.clone()
                        } else {
                            instruction.clone()
                        }
                    })
                    .collect();
                return (reachable(function, blocks, facts), true);
            }
        }
        if opcode(last) == Some(X86Opcode::JumpConditional)
            && after.is_some()
            && valid_condition(last).is_some()
        {
            let over = &blocks[position + 1];
            let beyond = blocks.get(position + 2).map(|block| block.id);
            let onward = passage(over, facts);
            if onward.is_some()
                && !facts.protected_loop_headers.contains(&over.id)
                && !real(&over.instructions).is_empty()
                && target(last) == beyond
                && predecessors(&blocks, &blocks.iter().map(|block| block.id).collect())
                    .get(&over.id)
                    == Some(&BTreeSet::from([block.id]))
            {
                let onward = onward.expect("checked above");
                let mut inverted = invert(last.clone()).expect("checked legal condition");
                retarget_instruction(&mut inverted, onward);
                blocks[position].instructions = block
                    .instructions
                    .iter()
                    .map(|instruction| {
                        if instruction.id == last.id {
                            inverted.clone()
                        } else {
                            instruction.clone()
                        }
                    })
                    .collect();
                blocks[position].successors =
                    vec![onward, beyond.expect("target equality requires beyond")];
                blocks[position + 1].successors.clear();
                return (reachable(function, blocks, facts), true);
            }
        }
    }
    (function.clone(), false)
}

fn passage(block: &MachineBlock, facts: &ControlFlowFacts) -> Option<MachineBlockId> {
    // A Nothing opcode is transparent only as a fresh, operand-free anchor.
    // This is intentionally more restrictive than `flags.anchor` alone.
    if block.instructions.iter().any(|instruction| {
        opcode(instruction) == Some(X86Opcode::Nothing)
            && (!is_anchor(instruction)
                || facts.non_inserted.contains(&instruction.id)
                || facts.owns_source_bytes.contains(&instruction.id)
                || !instruction.operands.is_empty())
    }) {
        return None;
    }
    let real = real(&block.instructions);
    match real.as_slice() {
        [] if block.successors.len() == 1 => Some(block.successors[0]),
        [jump] if opcode(jump) == Some(X86Opcode::Jump) => target(jump),
        _ => None,
    }
}

fn through(
    blocks: &[MachineBlock],
    index: &BTreeMap<MachineBlockId, usize>,
    mut target: MachineBlockId,
    protected: &BTreeSet<MachineBlockId>,
    facts: &ControlFlowFacts,
) -> MachineBlockId {
    let start = target;
    let mut seen = BTreeSet::new();
    while !protected.contains(&target) {
        let Some(position) = index.get(&target) else {
            break;
        };
        let Some(onward) = passage(&blocks[*position], facts) else {
            break;
        };
        if !seen.insert(target) {
            return start;
        }
        target = onward;
    }
    target
}

fn retargeted(
    block: &MachineBlock,
    last: MachineInstructionId,
    destination: MachineBlockId,
) -> MachineBlock {
    let old = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == last)
        .and_then(target);
    let mut result = block.clone();
    for instruction in &mut result.instructions {
        if instruction.id == last {
            retarget_instruction(instruction, destination);
        }
    }
    result.successors = dedup(block.successors.iter().map(|successor| {
        if Some(*successor) == old {
            destination
        } else {
            *successor
        }
    }));
    result
}

fn predecessors(
    blocks: &[MachineBlock],
    known: &BTreeSet<MachineBlockId>,
) -> BTreeMap<MachineBlockId, BTreeSet<MachineBlockId>> {
    let mut found = BTreeMap::new();
    for block in blocks {
        for successor in &block.successors {
            if known.contains(successor) {
                found
                    .entry(*successor)
                    .or_insert_with(BTreeSet::new)
                    .insert(block.id);
            }
        }
    }
    found
}

fn reachable(
    function: &MachineFunction,
    blocks: Vec<MachineBlock>,
    facts: &ControlFlowFacts,
) -> MachineFunction {
    let index = block_indices(&blocks);
    let reached = reachable_ids(&blocks, function.entry, &index);
    let kept = blocks
        .into_iter()
        .filter_map(|mut block| {
            if reached.contains(&block.id) {
                return Some(block);
            }
            let inert_owned = !block.instructions.is_empty()
                && block.instructions.iter().all(is_anchor)
                && block
                    .instructions
                    .iter()
                    .any(|instruction| facts.owns_source_bytes.contains(&instruction.id));
            if inert_owned {
                block.successors.clear();
                Some(block)
            } else {
                None
            }
        })
        .collect();
    let mut result = function.clone();
    result.blocks = kept;
    result
}

fn reachable_ids(
    blocks: &[MachineBlock],
    entry: MachineBlockId,
    index: &BTreeMap<MachineBlockId, usize>,
) -> BTreeSet<MachineBlockId> {
    let mut reached = BTreeSet::new();
    let mut work = vec![entry];
    while let Some(block) = work.pop() {
        let Some(position) = index.get(&block) else {
            continue;
        };
        if !reached.insert(block) {
            continue;
        }
        work.extend(blocks[*position].successors.iter().copied());
    }
    reached
}

fn real(instructions: &[MachineInstruction]) -> Vec<&MachineInstruction> {
    instructions
        .iter()
        .filter(|instruction| !is_anchor(instruction))
        .collect()
}

fn is_anchor(instruction: &MachineInstruction) -> bool {
    instruction.is_logical_anchor(X86Opcode::Nothing.machine_opcode())
}

fn opcode(instruction: &MachineInstruction) -> Option<X86Opcode> {
    X86Opcode::from_machine_opcode(instruction.opcode)
}

fn target(instruction: &MachineInstruction) -> Option<MachineBlockId> {
    match opcode(instruction) {
        Some(X86Opcode::Jump) => match instruction.operands.as_slice() {
            [operand] => match &operand.kind {
                MachineOperandKind::Block(block) => Some(*block),
                _ => None,
            },
            _ => None,
        },
        Some(X86Opcode::JumpConditional) => match instruction.operands.as_slice() {
            [_, operand] => match &operand.kind {
                MachineOperandKind::Block(block) => Some(*block),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn valid_condition(instruction: &MachineInstruction) -> Option<ConditionCode> {
    if opcode(instruction) != Some(X86Opcode::JumpConditional) {
        return None;
    }
    let [condition, target] = instruction.operands.as_slice() else {
        return None;
    };
    if !matches!(target.kind, MachineOperandKind::Block(_)) {
        return None;
    }
    let MachineOperandKind::Immediate(raw) = &condition.kind else {
        return None;
    };
    ConditionCode::ALL
        .into_iter()
        .find(|condition| i64::from(*condition as u8) == *raw)
}

fn invert(mut instruction: MachineInstruction) -> Option<MachineInstruction> {
    let condition = valid_condition(&instruction)?;
    instruction.operands[0].kind =
        MachineOperandKind::Immediate(i64::from(condition.inverted() as u8));
    Some(instruction)
}

fn retarget_instruction(instruction: &mut MachineInstruction, target: MachineBlockId) {
    let operand = match opcode(instruction) {
        Some(X86Opcode::Jump) => instruction.operands.get_mut(0),
        Some(X86Opcode::JumpConditional) => instruction.operands.get_mut(1),
        _ => None,
    };
    if let Some(operand) = operand {
        operand.kind = MachineOperandKind::Block(target);
    }
}

fn block_indices(blocks: &[MachineBlock]) -> BTreeMap<MachineBlockId, usize> {
    blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.id, index))
        .collect()
}

fn dedup(items: impl IntoIterator<Item = MachineBlockId>) -> Vec<MachineBlockId> {
    let mut seen = BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(*item))
        .collect()
}

fn block_operand(block: MachineBlockId) -> crate::codegen::machine::MachineOperand {
    crate::codegen::machine::MachineOperand {
        kind: MachineOperandKind::Block(block),
        role: crate::codegen::machine::OperandRole::None,
        constraint: None,
        tied_to: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        InstructionFlags, MachineCallingConvention, MachineFunctionId, MachineLinkage,
        MachineRegister, MachineSignature, OperandRole, VirtualRegister, VirtualRegisterId,
    };
    use crate::target::x86::X86RegisterClass;

    fn function(blocks: Vec<MachineBlock>) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(0),
            name: "f".into(),
            linkage: MachineLinkage::Internal,
            signature: MachineSignature {
                result: None,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: MachineCallingConvention::C,
            },
            entry: MachineBlockId::new(1),
            virtual_registers: Vec::new(),
            blocks,
            frame_objects: Vec::new(),
        }
    }

    #[test]
    fn equal_sized_loops_keep_python_back_edge_discovery_order() {
        let blocks = vec![
            block(0, Vec::new(), &[1, 2]),
            block(1, Vec::new(), &[4]),
            block(2, Vec::new(), &[3]),
            block(3, Vec::new(), &[2]),
            block(4, Vec::new(), &[1]),
        ];
        let index = block_indices(&blocks);

        assert_eq!(
            loops(&blocks, MachineBlockId::new(0), &index)
                .iter()
                .map(|natural_loop| natural_loop.header.get())
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
    }

    fn block(id: u32, instructions: Vec<MachineInstruction>, successors: &[u32]) -> MachineBlock {
        MachineBlock {
            id: MachineBlockId::new(id),
            instructions,
            successors: successors
                .iter()
                .copied()
                .map(MachineBlockId::new)
                .collect(),
        }
    }

    fn instruction(
        id: u32,
        opcode: X86Opcode,
        operands: Vec<crate::codegen::machine::MachineOperand>,
    ) -> MachineInstruction {
        MachineInstruction {
            id: MachineInstructionId::new(id),
            opcode: opcode.machine_opcode(),
            operands,
            flags: InstructionFlags::NONE,
        }
    }

    fn jump(id: u32, target: u32) -> MachineInstruction {
        super::jump(MachineInstructionId::new(id), MachineBlockId::new(target))
    }

    fn branch(id: u32, condition: i64, target: u32) -> MachineInstruction {
        instruction(
            id,
            X86Opcode::JumpConditional,
            vec![
                crate::codegen::machine::MachineOperand {
                    kind: MachineOperandKind::Immediate(condition),
                    role: crate::codegen::machine::OperandRole::None,
                    constraint: None,
                    tied_to: None,
                },
                block_operand(MachineBlockId::new(target)),
            ],
        )
    }

    fn ret(id: u32) -> MachineInstruction {
        instruction(id, X86Opcode::ReturnNear, Vec::new())
    }

    #[test]
    fn jump_next_fresh_is_removed() {
        let result = place_and_thread(
            &function(vec![
                block(1, vec![jump(1, 4)], &[4]),
                block(4, vec![ret(2)], &[]),
            ]),
            &ControlFlowFacts::default(),
        )
        .unwrap();
        assert!(result.blocks[0].instructions.is_empty());
    }

    #[test]
    fn jump_next_non_inserted_becomes_same_id_anchor() {
        let facts = ControlFlowFacts {
            non_inserted: BTreeSet::from([MachineInstructionId::new(1)]),
            ..ControlFlowFacts::default()
        };
        let result = place_and_thread(
            &function(vec![
                block(1, vec![jump(1, 4)], &[4]),
                block(4, vec![ret(2)], &[]),
            ]),
            &facts,
        )
        .unwrap();
        let kept = &result.blocks[0].instructions[0];
        assert_eq!(kept.id, MachineInstructionId::new(1));
        assert!(is_anchor(kept));
        assert!(facts.non_inserted.contains(&kept.id));
    }

    #[test]
    fn branch_then_jump_inverts_only_a_legal_condition() {
        let result = threaded(
            &function(vec![
                block(
                    1,
                    vec![branch(1, ConditionCode::Equal as i64, 4), jump(2, 9)],
                    &[4, 9],
                ),
                block(4, vec![ret(3)], &[]),
                block(9, vec![ret(4)], &[]),
            ]),
            &ControlFlowFacts::default(),
        );
        assert_eq!(
            valid_condition(&result.blocks[0].instructions[0]),
            Some(ConditionCode::NotEqual)
        );
        assert_eq!(
            target(&result.blocks[0].instructions[0]),
            Some(MachineBlockId::new(9))
        );

        let invalid = threaded(
            &function(vec![
                block(1, vec![branch(1, 0, 4), jump(2, 9)], &[4, 9]),
                block(4, vec![ret(3)], &[]),
                block(9, vec![ret(4)], &[]),
            ]),
            &ControlFlowFacts::default(),
        );
        assert_eq!(invalid.blocks[0].instructions.len(), 2);
    }

    #[test]
    fn conditional_arm_precedes_join_and_removes_both_jumps() {
        let result = place_and_thread(
            &function(vec![
                block(
                    1,
                    vec![branch(1, ConditionCode::Greater as i64, 4)],
                    &[4, 3],
                ),
                block(3, vec![jump(2, 7)], &[7]),
                block(
                    4,
                    vec![instruction(3, X86Opcode::Mov, Vec::new()), jump(4, 7)],
                    &[7],
                ),
                block(7, vec![ret(5)], &[]),
            ]),
            &ControlFlowFacts::default(),
        )
        .unwrap();
        assert_eq!(
            result
                .blocks
                .iter()
                .map(|block| block.id.get())
                .collect::<Vec<_>>(),
            vec![1, 4, 7]
        );
        assert_eq!(result.blocks[0].instructions.len(), 1);
        assert_eq!(
            target(&result.blocks[0].instructions[0]),
            Some(MachineBlockId::new(7))
        );
    }

    #[test]
    fn logical_anchor_blocks_passage() {
        let anchor = instruction(
            2,
            X86Opcode::Mov,
            vec![
                crate::codegen::machine::MachineOperand {
                    kind: MachineOperandKind::Register(MachineRegister::Virtual(
                        VirtualRegisterId::new(0),
                    )),
                    role: OperandRole::Def,
                    constraint: None,
                    tied_to: None,
                },
                crate::codegen::machine::MachineOperand {
                    kind: MachineOperandKind::Register(MachineRegister::Virtual(
                        VirtualRegisterId::new(0),
                    )),
                    role: OperandRole::Use,
                    constraint: None,
                    tied_to: None,
                },
            ],
        )
        .anchor(X86Opcode::Nothing.machine_opcode());
        let mut input = function(vec![
            block(1, vec![branch(1, ConditionCode::Equal as i64, 4)], &[4, 7]),
            block(7, vec![ret(3)], &[]),
            block(4, vec![anchor], &[8]),
            block(8, vec![ret(4)], &[]),
        ]);
        input.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId::new(0),
            class: X86RegisterClass::Word.machine_class(),
        });
        let result = place_and_thread(&input, &ControlFlowFacts::default()).unwrap();
        assert_eq!(
            target(&result.blocks[0].instructions[0]),
            Some(MachineBlockId::new(4))
        );
    }

    #[test]
    fn malformed_nothing_is_not_treated_as_a_transparent_anchor() {
        let mut malformed = instruction(2, X86Opcode::Nothing, Vec::new())
            .anchor(X86Opcode::Nothing.machine_opcode());
        malformed.flags.terminator = true;
        let carrier = block(4, vec![malformed], &[8]);

        assert_eq!(passage(&carrier, &ControlFlowFacts::default()), None);
        assert_eq!(real(&carrier.instructions).len(), 1);
    }

    #[test]
    fn owned_inert_orphan_survives_but_unowned_and_carrier_orphans_drop() {
        let owned = instruction(2, X86Opcode::Nothing, Vec::new())
            .anchor(X86Opcode::Nothing.machine_opcode());
        let carrier = instruction(3, X86Opcode::Nothing, Vec::new())
            .anchor(X86Opcode::Nothing.machine_opcode());
        let facts = ControlFlowFacts {
            owns_source_bytes: BTreeSet::from([owned.id]),
            non_inserted: BTreeSet::from([carrier.id]),
            ..ControlFlowFacts::default()
        };
        let result = threaded(
            &function(vec![
                block(1, vec![ret(1)], &[]),
                block(4, vec![owned], &[9]),
                block(7, vec![carrier], &[]),
                block(9, vec![], &[]),
            ]),
            &facts,
        );
        assert_eq!(
            result
                .blocks
                .iter()
                .map(|block| block.id.get())
                .collect::<Vec<_>>(),
            vec![1, 4]
        );
        assert!(result.blocks[1].successors.is_empty());
    }

    #[test]
    fn reachable_non_inserted_carrier_is_not_a_passage() {
        let carrier = instruction(3, X86Opcode::Nothing, Vec::new())
            .anchor(X86Opcode::Nothing.machine_opcode());
        let facts = ControlFlowFacts {
            non_inserted: BTreeSet::from([carrier.id]),
            ..ControlFlowFacts::default()
        };
        let result = place_and_thread(
            &function(vec![
                block(1, vec![branch(1, ConditionCode::Equal as i64, 4)], &[4, 7]),
                block(7, vec![ret(2)], &[]),
                block(4, vec![carrier], &[8]),
                block(8, vec![ret(4)], &[]),
            ]),
            &facts,
        )
        .unwrap();
        assert_eq!(
            target(&result.blocks[0].instructions[0]),
            Some(MachineBlockId::new(4))
        );
        assert!(
            result
                .blocks
                .iter()
                .any(|block| block.id == MachineBlockId::new(4))
        );
    }

    #[test]
    fn protected_empty_loop_header_is_retained() {
        let facts = ControlFlowFacts {
            protected_loop_headers: BTreeSet::from([MachineBlockId::new(4)]),
            ..ControlFlowFacts::default()
        };
        let result = place_and_thread(
            &function(vec![
                block(1, vec![jump(1, 4)], &[4]),
                block(4, Vec::new(), &[7]),
                block(7, vec![ret(2)], &[]),
            ]),
            &facts,
        )
        .unwrap();
        assert!(
            result
                .blocks
                .iter()
                .any(|block| block.id == MachineBlockId::new(4))
        );
    }

    #[test]
    fn result_is_idempotent() {
        let input = function(vec![
            block(
                1,
                vec![branch(1, ConditionCode::Equal as i64, 4), jump(2, 9)],
                &[4, 9],
            ),
            block(4, vec![ret(3)], &[]),
            block(9, vec![ret(4)], &[]),
        ]);
        let once = place_and_thread(&input, &ControlFlowFacts::default()).unwrap();
        let twice = place_and_thread(&once, &ControlFlowFacts::default()).unwrap();
        assert_eq!(once, twice);
    }
}
