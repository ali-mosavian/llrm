//! The code that ends every unconditional way into a block in the same way
//! runs once in the block: SimplifyCFG's `sinkCommonCodeFromPredecessors`.
//!
//! # How it works
//! `sunk` takes a block with two or more predecessors that end in a jump. It
//! scans their instructions from the end in step, as far as each column
//! (one instruction of each predecessor) is the same operation with operands
//! that are the same, or that a phi can merge (`can_sink`). Of those the
//! columns whose sinking would need more than one new phi stay, from the first
//! that does. If some predecessor does not end in a jump the predecessors that
//! do are first given a block of their own to sink into (`.sink.split`), and
//! that only when a column holds something that cannot be run speculatively.
//! Each column is then one instruction at the start of the block, its
//! differing operands phis (`sink_last_instruction`).
//!
//! # Example
//! The add both arms end with runs once in the join:
//!
//! ```text
//!   a: %x = add %p, 1 ; br e         a: br e
//!   b: %y = add %q, 1 ; br e   ==>   b: br e
//!   e: %z = phi [%x, a], [%y, b]     e: %s = phi [%p, a], [%q, b]
//!                                       %z = add %s, 1
//! ```
//!
//! # LLVM structure
//! | LLVM | here | |
//! |---|---|---|
//! | [`sinkCommonCodeFromPredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2425) | `sunk` | MIRROR; D1, D2 |
//! | [`canSinkInstructions`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2236) | `can_sink` | MIRROR; D1 |
//! | [`sinkLastInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2340) | `sink_last_instruction` | MIRROR; D1 |
//! | [`canReplaceOperandWithVariable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L3907) | `can_replace_operand_with_variable` | MIRROR; D3 |
//! | [`replacingOperandWithVariableIsCheap`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2219) | `replacing_operand_is_cheap` | MIRROR |
//! | [`IsBlockFollowedByDeoptOrUnreachable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L653) | `followed_by_unreachable` | MIRROR; D4 |
//! | [`simplifyDuplicatePredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8573) and `mergeIdenticalBBs` | `duplicate_predecessors_merged` | MIRROR |
//!
//! - D1. No pseudo probes, inline assembly, `nomerge` or `convergent`
//!   attributes, `swifterror` pointers or `!mmra`-different pairs beyond the
//!   metadata ids being compared as written; debug records go with the
//!   instruction that holds them.
//! - D2. The split block has no name (`.sink.split`) and `invoke`s are not
//!   merged (`mergeCompatibleInvokes`: none in the MIR).
//! - D3. A call's `immarg` parameter is its `Flag("immarg")` argument
//!   attribute; the intrinsic cases for variadic parameters, `gcroot` and
//!   `threadlocal_address` are not in the MIR.
//! - D4. No `llvm.experimental.deoptimize`.

use llrm_mir::context::Context;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, BinaryOp, Flags, Opcode};
use llrm_mir::types::Type;
use llrm_support::hash::HashMap;

use crate::twoentryphi::Speculation;

/// The depth `IsBlockFollowedByDeoptOrUnreachable` looks to.
const DEPTH: u32 = 8;

/// A phi operand site: an instruction and the index of an operand of it.
type Site = (InstId, usize);

/// What each operand site that a phi merges is given by each predecessor.
type PhiOperands = HashMap<Site, Vec<Operand>>;

/// `Operand`'s defining instruction.
fn defined_by(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            _ => None,
        },
        _ => None,
    }
}

/// Whether `inst` calls an `llvm.` function directly.
fn calls_intrinsic(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    inst: InstId,
) -> bool {
    llrm_mir::memory::callee(context, function, inst)
        .and_then(|global| spec.outer.globals.get(global.0 as usize))
        .and_then(|global| global.name.as_deref())
        .is_some_and(|name| name.starts_with("llvm."))
}

/// `replacingOperandWithVariableIsCheap`.
fn replacing_operand_is_cheap(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    inst: InstId,
    index: usize,
) -> bool {
    match &function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) => index != 1,
        Opcode::GetElementPtr { .. }
            if index >= 1
                && matches!(
                    function.instruction(inst).operands[index],
                    Operand::Constant(_)
                )
                && !spec.tti.constant_offset_phi_is_cheap() =>
        {
            false
        }
        _ => !calls_intrinsic(context, spec, function, inst),
    }
}

/// `canReplaceOperandWithVariable`: the operand `index` of `inst` may be a phi
/// or a select instead of what it is.
fn can_replace_operand_with_variable(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    inst: InstId,
    index: usize,
) -> bool {
    let instruction = function.instruction(inst);
    let operand = instruction.operands[index];
    if matches!(operand, Operand::Block(_)) {
        return false;
    }
    if crate::local::is_lifetime(context, &spec.outer.globals, function, inst) {
        return false;
    }
    if !matches!(operand, Operand::Constant(_)) {
        return true;
    }
    match &instruction.opcode {
        Opcode::Call(info) => {
            let arguments = instruction.operands.len() - 1;
            if index < arguments {
                let immarg = info
                    .argument_attrs
                    .get(index)
                    .is_some_and(
                        |list| list.iter().any(|one| matches!(one, Attribute::Flag(name) if name == "immarg")),
                    );
                !immarg
            } else {
                // The callee: never an intrinsic's.
                !calls_intrinsic(context, spec, function, inst)
            }
        }
        Opcode::ExtractValue(_) => index == 0,
        Opcode::InsertValue(_) => index < 2,
        Opcode::Alloca { .. } => false,
        Opcode::GetElementPtr { source } => {
            if index == 0 {
                return true;
            }
            // `It.isStruct()` of each step up to this operand: the type an
            // index operand indexes, the first index (of the pointer) none.
            let mut current = *source;
            for _ in 1..index {
                match context.types.get(current) {
                    Type::Struct { .. } | Type::Named(_) => return false,
                    Type::Array { element, .. } | Type::Vector { element, .. } => current = *element,
                    _ => return true,
                }
            }
            true
        }
        _ => true,
    }
}

/// `canSinkInstructions`: the instructions `insts`, one of each predecessor,
/// can be one instruction with phis for the operands that differ, which are
/// recorded in `phi_operands`.
fn can_sink(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    insts: &[InstId],
    phi_operands: &mut PhiOperands,
) -> bool {
    let mut uses = None;
    for &inst in insts {
        let instruction = function.instruction(inst);
        if matches!(
            instruction.opcode,
            Opcode::Phi | Opcode::LandingPad { .. } | Opcode::Alloca { .. }
        ) {
            return false;
        }
        // An infinite loop of one block: sinking into it would never stop.
        let block = function.parent(inst).expect("a placed instruction");
        if function.successors(block).len() == 1 && function.successors(block)[0] == block {
            return false;
        }
        let count = instruction.result.map_or(0, |result| function.users(result).iter().count());
        match uses {
            None => uses = Some(count),
            Some(found) if found != count => return false,
            Some(_) => {}
        }
    }
    let first = insts[0];
    for &inst in &insts[1..] {
        if !crate::hoistcommon::same_operation(context, function, first, inst)
            || crate::hoistcommon::mmra(function, inst) != crate::hoistcommon::mmra(function, first)
        {
            return false;
        }
    }
    if crate::hoistcommon::mmra(function, first) != crate::hoistcommon::mmra(function, insts[0]) {
        return false;
    }
    // Uses must be consistent: a use in a phi of the sink target needs the
    // other phi operands to be the instructions of `insts`.
    if let Some(result) = function.instruction(first).result {
        for one in function.users(result).iter() {
            let Some(operands) = phi_operands.get(&(one.user, one.index as usize)) else { return false };
            let same = operands.len() == insts.len()
                && operands.iter().zip(insts).all(|(&operand, &inst)| {
                    function.instruction(inst).result.is_some_and(|result| operand == Operand::Value(result))
                });
            if !same {
                return false;
            }
        }
    }
    // Calls: all indirect, or all of one callee.
    if matches!(function.instruction(first).opcode, Opcode::Call(_)) {
        let direct: Vec<_> = insts.iter().map(|&inst| llrm_mir::memory::callee(context, function, inst)).collect();
        if direct.iter().any(Option::is_none) {
            if !direct.iter().all(Option::is_none) {
                return false;
            }
        } else if direct.iter().any(|one| *one != direct[0]) {
            return false;
        }
    }
    let count = function.instruction(first).operands.len();
    for index in 0..count {
        let operand = function.instruction(first).operands[index];
        if insts.iter().all(|&inst| function.instruction(inst).operands[index] == operand) {
            continue;
        }
        let replaceable =
            insts.iter().all(|&inst| can_replace_operand_with_variable(context, spec, function, inst, index));
        if (matches!(operand, Operand::Constant(_))
            && !replacing_operand_is_cheap(context, spec, function, first, index))
            || !replaceable
        {
            return false;
        }
        phi_operands
            .insert((first, index), insts.iter().map(|&inst| function.instruction(inst).operands[index]).collect());
    }
    true
}

/// `sinkLastInstruction`: the last instruction of each of `blocks` is one
/// instruction at the start of their common successor.
fn sink_last_instruction(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    blocks: &[BlockId],
) {
    let Some(Operand::Block(end)) = function.terminator(blocks[0]).map(|last| function.instruction(last).operands[0])
    else {
        unreachable!("a jump");
    };
    let insts: Vec<InstId> = blocks
        .iter()
        .map(|&block| {
            let last = function.terminator(block).expect("a terminated block");
            function.iter_block_rev(block).find(|&inst| inst != last).expect("an instruction to sink")
        })
        .collect();
    let first = insts[0];
    let mut operands = Vec::new();
    for index in 0..function.instruction(first).operands.len() {
        let operand = function.instruction(first).operands[index];
        let differs = insts.iter().any(|&inst| function.instruction(inst).operands[index] != operand);
        if !differs {
            operands.push(operand);
            continue;
        }
        let ty = function.operand_type(context, operand).expect("a value operand");
        let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), Some("sink"));
        let at = function.iter_block(end).next().expect("a terminated block");
        function.insert(phi, Position::Before(at)).expect("a new phi");
        let mut arms = Vec::new();
        for &inst in &insts {
            arms.push(function.instruction(inst).operands[index]);
            arms.push(Operand::Block(function.parent(inst).expect("a placed instruction")));
        }
        function.set_operands(phi, arms);
        operands.push(Operand::Value(function.instruction(phi).result.expect("a phi's value")));
    }
    for (index, operand) in operands.into_iter().enumerate() {
        function.set_operand(first, index, operand);
    }
    let at = crate::widen::first_insertion_point(function, end);
    function.move_to(first, Position::Before(at)).expect("a placed instruction");
    for &inst in &insts[1..] {
        crate::local::combine_metadata_for_cse(&spec.outer.metadata, function, first, inst, true);
        crate::local::and_ir_flags(context, function, first, inst);
        crate::local::try_intersect_attributes(function, first, inst);
    }
    // The phis of the target that read the instructions are the instruction.
    if let Some(result) = function.instruction(first).result {
        let phis: Vec<InstId> = function.users(result).iter().map(|one| one.user).collect();
        for phi in phis {
            if !function.is_erased(phi) {
                if let Some(value) = function.instruction(phi).result {
                    function.replace_all_uses_with(value, Operand::Value(result));
                }
                function.erase(phi).expect("a phi nothing reads");
            }
        }
    }
    for &inst in &insts[1..] {
        if let (Some(old), Some(new)) = (function.instruction(inst).result, function.instruction(first).result) {
            function.replace_all_uses_with(old, Operand::Value(new));
        }
        function.erase(inst).expect("a sunk copy nothing reads");
    }
}

/// `IsBlockFollowedByDeoptOrUnreachable`.
fn followed_by_unreachable(
    function: &Function,
    mut block: BlockId,
) -> bool {
    let mut seen = IdSet::new();
    let mut depth = 0;
    loop {
        depth += 1;
        if depth > DEPTH || !seen.insert(block) {
            return false;
        }
        let Some(last) = function.terminator(block) else { return false };
        if function.instruction(last).opcode == Opcode::Unreachable {
            return true;
        }
        let successors = function.successors(block);
        if successors.len() != 1 {
            return false;
        }
        block = successors[0];
    }
}

/// The code that ends every unconditional predecessor of `block` the same way
/// runs in `block`. Whether it did.
pub fn sunk(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let mut jumps = Vec::new();
    let mut listed: IdSet<BlockId> = IdSet::new();
    let mut other = false;
    for &pred in function.predecessors(block).iter() {
        let jump = function.terminator(pred).is_some_and(|last| {
            function.instruction(last).opcode == Opcode::Br && function.instruction(last).operands.len() == 1
        });
        if jump {
            if listed.insert(pred) {
                jumps.push(pred);
            }
        } else {
            other = true;
        }
    }
    if jumps.len() < 2 {
        return false;
    }
    let mut phi_operands = PhiOperands::default();
    // Each jump by its place in `jumps`: a phi's arms are read once.
    let mut jump_at: IdMap<BlockId, usize> = IdMap::new();
    for (at, &pred) in jumps.iter().enumerate() {
        jump_at.insert(pred, at);
    }
    for phi in crate::edges::phis(function, block) {
        let mut values: Vec<Option<Operand>> = vec![None; jumps.len()];
        let mut first_arm = 0;
        for (arm, &(value, from)) in llrm_analysis::occurrence::arms(function, phi).iter().enumerate() {
            if let Some(&at) = jump_at.get(&from) {
                values[at] = Some(value);
                if at == 0 {
                    first_arm = arm;
                }
            }
        }
        let values = values.into_iter().map(|one| one.expect("an arm per jump")).collect();
        phi_operands.insert((phi, 2 * first_arm), values);
    }
    // The columns: the instruction `depth` before the terminator of each.
    let bodies: Vec<Vec<InstId>> = jumps
        .iter()
        .map(|&pred| {
            let mut body: Vec<InstId> = function.iter_block(pred).collect();
            body.pop();
            body
        })
        .collect();
    let mut columns: Vec<Vec<InstId>> = Vec::new();
    let mut to_sink: IdSet<InstId> = IdSet::new();
    while bodies.iter().all(|body| body.len() > columns.len()) {
        let column: Vec<InstId> = bodies.iter().map(|body| body[body.len() - 1 - columns.len()]).collect();
        if !can_sink(context, spec, function, &column, &mut phi_operands) {
            break;
        }
        for &inst in &column {
            to_sink.insert(inst);
        }
        columns.push(column);
    }
    let mut scan = columns.len();
    if scan == 0 {
        return false;
    }
    let unreachable_follows = followed_by_unreachable(function, block);
    if !unreachable_follows {
        // One new phi at most for a column; the operands of a column that
        // another column sinks do not count.
        let profitable = |function: &Function, column: &[InstId], to_sink: &IdSet<InstId>| {
            let mut phis = 0;
            for (index, _) in function.instruction(column[0]).operands.iter().enumerate() {
                let Some(operands) = phi_operands.get(&(column[0], index)) else { continue };
                let all_sunk = operands
                    .iter()
                    .all(|&operand| defined_by(function, operand).is_some_and(|def| to_sink.contains(&def)));
                if all_sunk {
                    continue;
                }
                phis += 1;
                let memory_operand = match &function.instruction(column[0]).opcode {
                    Opcode::Load { .. } => index == 0,
                    Opcode::Store { .. } => index == 1,
                    _ => false,
                };
                if memory_operand
                    && operands.iter().any(|&operand| {
                        defined_by(function, operand)
                            .is_some_and(|def| matches!(function.instruction(def).opcode, Opcode::GetElementPtr { .. }))
                    })
                {
                    return false;
                }
            }
            phis <= 1
        };
        let mut kept: IdSet<InstId> = IdSet::new();
        let mut at = 0;
        while at < scan {
            if !profitable(function, &columns[at], &to_sink) {
                break;
            }
            for &inst in &columns[at] {
                kept.insert(inst);
            }
            at += 1;
        }
        if at == 0 {
            return false;
        }
        if at < scan {
            scan = at;
            to_sink = kept.clone();
            // Earlier columns may stop being profitable once these stay.
            let mut index = at as isize - 1;
            while index >= 0 {
                for &inst in &columns[index as usize] {
                    kept.remove(&inst);
                }
                if !profitable(function, &columns[index as usize], &to_sink) {
                    scan = index as usize;
                    to_sink = kept.clone();
                }
                index -= 1;
            }
        }
        if scan == 0 {
            return false;
        }
    }

    let mut changed = false;
    if other {
        if !unreachable_follows {
            // Only worth it when a column holds what cannot run speculatively.
            let worth = columns[..scan]
                .iter()
                .any(
                    |column| !crate::licm::hoist::is_safe_to_speculatively_execute_for_cfg(
                        context,
                        spec.layout,
                        function,
                        spec.outer,
                        column[0],
                    ),
                );
            if !worth {
                return false;
            }
        }
        crate::edges::split_block_predecessors(function, block, &jumps);
        changed = true;
    }
    for _ in 0..scan {
        sink_last_instruction(context, spec, function, &jumps);
        changed = true;
    }
    changed
}

/// `EqualBBWrapper::canBeMerged`: `block` holds a jump and nothing else, and
/// is not the entry or unreachable.
pub(crate) fn can_be_merged(
    function: &Function,
    block: BlockId,
) -> bool {
    let mut body = function.iter_block(block);
    function.entry() != Some(block)
        && !function.predecessors(block).is_empty()
        && body.next().is_some_and(|first| {
            function.instruction(first).opcode == Opcode::Br
                && function.instruction(first).operands.len() == 1
                && body.next().is_none()
        })
}

/// `simplifyDuplicatePredecessors` over `block`: the predecessors that are a
/// jump alone to it, and agree on every phi, are one.
pub fn duplicate_predecessors_merged(
    function: &mut Function,
    block: BlockId,
) -> bool {
    if function.predecessors(block).len() < 2 {
        return false;
    }
    let candidates: Vec<BlockId> =
        function.predecessors(block).iter().copied().filter(|&pred| can_be_merged(function, pred)).collect();
    identical_merged(function, candidates)
}

/// `mergeIdenticalBBs`: of the `candidates`, distinct blocks that
/// [`can_be_merged`], those that jump to the same block and give its phis the
/// same values are one; the first of each kind stays.
pub fn identical_merged(
    function: &mut Function,
    candidates: Vec<BlockId>,
) -> bool {
    if candidates.len() < 2 {
        return false;
    }
    let successor = |function: &Function, block: BlockId| match function
        .instruction(function.terminator(block).expect("a jump"))
        .operands[..]
    {
        [Operand::Block(target)] => target,
        _ => unreachable!("a candidate is a jump"),
    };
    let targets: Vec<BlockId> = candidates.iter().map(|&one| successor(function, one)).collect();
    let mut candidate_at: IdMap<BlockId, usize> = IdMap::new();
    for (at, &candidate) in candidates.iter().enumerate() {
        candidate_at.insert(candidate, at);
    }
    // What each successor's phis take from each candidate, once
    // (`PhiPredIVs`).
    let mut taken: Vec<Vec<Operand>> = vec![Vec::new(); candidates.len()];
    let mut seen: IdSet<BlockId> = IdSet::new();
    for &target in &targets {
        if !seen.insert(target) {
            continue;
        }
        for phi in crate::edges::phis(function, target) {
            for (value, from) in llrm_analysis::occurrence::arms(function, phi) {
                if let Some(&at) = candidate_at.get(&from) {
                    taken[at].push(value);
                }
            }
        }
    }
    let mut changed = false;
    let mut kept: HashMap<(BlockId, Vec<Operand>), BlockId> = HashMap::default();
    for (at, &candidate) in candidates.iter().enumerate() {
        match kept.entry((targets[at], std::mem::take(&mut taken[at]))) {
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(candidate);
            }
            std::collections::hash_map::Entry::Occupied(slot) => {
                let keep = *slot.get();
                // Everything that reached the duplicate reaches the kept one.
                for source in function.predecessors(candidate).iter().copied().collect::<Vec<_>>() {
                    let last = function.terminator(source).expect("a terminated predecessor");
                    crate::edges::retarget(function, last, candidate, keep);
                }
                changed = true;
            }
        }
    }
    changed
}

#[cfg(test)]
#[path = "sinkcommon_tests.rs"]
mod tests;
