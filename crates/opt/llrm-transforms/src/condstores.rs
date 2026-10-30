//! Two conditional stores to one address, one in each of two if-then-else
//! diamonds or triangles in a row, are one store after them, predicated on
//! either condition: SimplifyCFG's `mergeConditionalStores`.
//!
//! # How it works
//! `merged` takes the branches `pbi` and `qbi` of two blocks where every way
//! out of `pbi` leads to `qbi` and every way out of `qbi` to one block `post`,
//! through blocks of their own (or directly: a triangle). For each address
//! stored to in `pbi`'s arms and in `qbi`'s (`merged_to_address`), where each
//! side has that one store and nothing else touches memory on the way, and
//! the arms are cheap enough to be if-converted afterwards, both stores go,
//! and one store runs in a new block entered from `post`'s start when either
//! store would have run. The value stored is a phi in `post` of the two.
//!
//! # Example
//! Two stores of `%g` become one after both conditions:
//!
//! ```text
//!   p: br %a, pt, pf        pt: store 1, %g     q: br %b, qt, qf      post:
//!   qt: store 2, %g   ==>   post: %v = phi [1, ..], [2, ..]
//!                                 %c = or %a, %b ; br %c, then, rest
//!                           then: store %v, %g
//! ```
//!
//! # LLVM structure
//! | LLVM | here | |
//! |---|---|---|
//! | [`mergeConditionalStores`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4604) | `merged` | MIRROR |
//! | [`mergeConditionalStoreToAddress`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4426) | `merged_to_address` | MIRROR; D1, D2 |
//! | [`ensureValueAvailableInSuccessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4366) | `value_available_in_successor` | MIRROR |
//! | [`findUniqueStoreInBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4351) | `unique_store` | MIRROR |
//! | `simplifycfg-merge-cond-stores` (true), `-merge-cond-stores-aggressively` (false) | always on, `AGGRESSIVE` | MIRROR |
//!
//! - D1. The MIR has no atomic stores, branch weights or `dbg.assign`: a
//!   volatile store is not merged, the weights of the new branch are not set.
//! - D2. The store's alignment is the smaller of the two, as in LLVM; the new
//!   block has no name.

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::instructioncost::instruction_cost;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::tti::CostKind;

use crate::twoentryphi::Speculation;

/// `simplifycfg-merge-cond-stores-aggressively`.
const AGGRESSIVE: bool = false;
/// `phi-node-folding-threshold`, in `TCC_Basic`.
const THRESHOLD: i64 = 2;

/// The one store in `first` and `second`, if there is exactly one.
fn unique_store(
    function: &Function,
    blocks: [Option<BlockId>; 2],
) -> Option<InstId> {
    let mut found = None;
    for block in blocks.into_iter().flatten() {
        for inst in function.iter_block(block) {
            if matches!(function.instruction(inst).opcode, Opcode::Store { .. }) {
                if found.is_some() {
                    return None;
                }
                found = Some(inst);
            }
        }
    }
    found
}

/// The pointer a store writes through.
fn address(
    function: &Function,
    store: InstId,
) -> Operand {
    function.instruction(store).operands[1]
}

fn single_predecessor(
    function: &Function,
    block: BlockId,
) -> Option<BlockId> {
    let preds = function.predecessors(block);
    (preds.len() == 1).then(|| preds[0])
}

fn single_successor(
    function: &Function,
    block: BlockId,
) -> Option<BlockId> {
    function.sole_successor(block)
}

/// `ensureValueAvailableInSuccessor`: a phi in the only successor of `block`
/// that reads `value` from it (and `alternative` from the other predecessor).
fn value_available_in_successor(
    context: &Context,
    function: &mut Function,
    value: Operand,
    block: BlockId,
    alternative: Option<Operand>,
) -> Operand {
    let successor = single_successor(function, block).expect("a block with one successor");
    for phi in crate::edges::phis(function, successor) {
        let arms = llrm_analysis::occurrence::arms(function, phi);
        if arms.iter().find(|&&(_, from)| from == block).map(|&(v, _)| v) != Some(value) {
            continue;
        }
        let result = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
        let Some(alternative) = alternative else { return result };
        let other = arms.iter().find(|&&(_, from)| from != block).map(|&(v, _)| v);
        if other == Some(alternative) {
            return result;
        }
    }
    let defined_here = matches!(
        value,
        Operand::Value(v) if matches!(function.value(v).def, llrm_mir::module::ValueDef::Instruction(d) if function.parent(d) == Some(block))
    );
    if alternative.is_none() && !defined_here {
        return value;
    }
    let ty = function.operand_type(context, value).expect("a value");
    let mut arms = vec![value, Operand::Block(block)];
    for &pred in function.predecessors(successor).iter() {
        if pred != block {
            arms.push(alternative.unwrap_or(Operand::Constant(
                context.constant(llrm_mir::context::Constant { ty, kind: llrm_mir::context::ConstantKind::Poison }),
            )));
            arms.push(Operand::Block(pred));
        }
    }
    let phi = function.create_instruction(Opcode::Phi, ty, arms, Flags::default(), Some("simplifycfg.merge"));
    let first = function.iter_block(successor).next().expect("a terminated block");
    function.insert(phi, Position::Before(first)).expect("a new phi");
    Operand::Value(function.instruction(phi).result.expect("a phi's value"))
}

/// Whether sinking the stores out of `block` leaves it small enough to be
/// if-converted: cheap arithmetic only.
fn worthwhile(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    block: Option<BlockId>,
    free: [InstId; 2],
) -> bool {
    let Some(block) = block else { return true };
    let mut cost = 0;
    for inst in function.iter_block(block) {
        if function.instruction(inst).opcode.is_terminator() || free.contains(&inst) {
            continue;
        }
        if !matches!(
            function.instruction(inst).opcode,
            Opcode::Binary(_) | Opcode::GetElementPtr { .. }
        ) {
            return false;
        }
        let price = instruction_cost(
            spec.tti,
            context,
            spec.layout,
            &spec.outer.globals,
            function,
            inst,
            CostKind::SizeAndLatency,
        );
        cost += price.value;
        if cost > THRESHOLD {
            return false;
        }
    }
    true
}

fn touches_memory(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    inst: InstId,
) -> bool {
    let effects = llrm_mir::memory::of(context, spec.outer.callees(), function, inst);
    effects.reads || effects.writes
}

struct Diamond {
    ptb: Option<BlockId>,
    pfb: BlockId,
    qtb: Option<BlockId>,
    qfb: BlockId,
    post: BlockId,
    invert_p: bool,
    invert_q: bool,
}

/// `mergeConditionalStoreToAddress`.
fn merged_to_address(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    shape: &mut Diamond,
    target: Operand,
) -> bool {
    let (ptb, pfb, qtb, qfb) = (shape.ptb, shape.pfb, shape.qtb, shape.qfb);
    let by_address = |function: &Function, blocks: [Option<BlockId>; 2]| {
        let mut found = None;
        for block in blocks.into_iter().flatten() {
            for inst in function.iter_block(block) {
                if matches!(function.instruction(inst).opcode, Opcode::Store { .. }) {
                    if found.is_some() {
                        return None;
                    }
                    found = Some(inst);
                }
            }
        }
        found
    };
    let _ = unique_store;
    let (Some(pstore), Some(qstore)) = (by_address(function, [ptb, Some(pfb)]), by_address(function, [qtb, Some(qfb)]))
    else {
        return false;
    };
    let _ = target;
    let (p, q) = (function.instruction(pstore), function.instruction(qstore));
    let volatile = |op: &Opcode| matches!(op, Opcode::Store { volatile: true, .. });
    if volatile(&p.opcode) || volatile(&q.opcode) {
        return false;
    }
    if function.operand_type(context, p.operands[0]) != function.operand_type(context, q.operands[0]) {
        return false;
    }
    // Nothing else touches memory where the stores move over.
    let q_head = single_predecessor(function, qfb).expect("a single predecessor");
    for inst in function.iter_block(q_head) {
        if touches_memory(context, spec, function, inst) {
            return false;
        }
    }
    for inst in function.iter_block(qfb) {
        if inst != qstore && touches_memory(context, spec, function, inst) {
            return false;
        }
    }
    if let Some(qtb) = qtb {
        for inst in function.iter_block(qtb) {
            if inst != qstore && touches_memory(context, spec, function, inst) {
                return false;
            }
        }
    }
    let p_block = function.parent(pstore).expect("a placed store");
    let mut after = false;
    for inst in function.iter_block(p_block) {
        if inst == pstore {
            after = true;
            continue;
        }
        if after && touches_memory(context, spec, function, inst) {
            return false;
        }
    }
    let free = [pstore, qstore];
    if !AGGRESSIVE
        && !(worthwhile(context, spec, function, ptb, free)
            && worthwhile(context, spec, function, Some(pfb), free)
            && worthwhile(context, spec, function, qtb, free)
            && worthwhile(context, spec, function, Some(qfb), free))
    {
        return false;
    }

    // Past two predecessors, `post` is split so that the store can go in.
    let mut post = shape.post;
    if function.predecessors(post).len() > 2 {
        let true_pred = qtb.unwrap_or(q_head);
        post = crate::edges::split_block_predecessors(function, post, &[qfb, true_pred]);
        shape.post = post;
    }
    let p_branch = function.terminator(single_predecessor(function, pfb).expect("a predecessor")).expect("a branch");
    let q_branch = function.terminator(single_predecessor(function, qfb).expect("a predecessor")).expect("a branch");
    let (p_cond, q_cond) = (function.instruction(p_branch).operands[0], function.instruction(q_branch).operands[0]);

    let p_value = function.instruction(pstore).operands[0];
    let q_value = function.instruction(qstore).operands[0];
    let p_block_of_store = function.parent(pstore).expect("a placed store");
    let q_block_of_store = function.parent(qstore).expect("a placed store");
    let p_phi = value_available_in_successor(context, function, p_value, p_block_of_store, None);
    let q_phi = value_available_in_successor(context, function, q_value, q_block_of_store, Some(p_phi));

    let first = crate::widen::first_insertion_point(function, post);
    let invert_p = shape.invert_p ^ (p_block_of_store != ptb.unwrap_or(BlockId(u32::MAX)));
    let invert_q = shape.invert_q ^ (q_block_of_store != qtb.unwrap_or(BlockId(u32::MAX)));
    let mut make = |function: &mut Function, opcode: Opcode, operands: Vec<Operand>, name: &str| {
        let ty = function.operand_type(context, operands[0]).expect("a condition");
        let inst = function.create_instruction(opcode, ty, operands, Flags::default(), Some(name));
        function.insert(inst, Position::Before(first)).expect("a new instruction");
        Operand::Value(function.instruction(inst).result.expect("a value"))
    };
    let not = |function: &mut Function,
               make: &mut dyn FnMut(&mut Function, Opcode, Vec<Operand>, &str) -> Operand,
               condition: Operand| {
        let ty = function.operand_type(context, condition).expect("a condition");
        let one = Operand::Constant(context.int(ty, 1));
        make(function, Opcode::Binary(BinaryOp::Xor), vec![condition, one], "not")
    };
    let p_pred = if invert_p { not(function, &mut make, p_cond) } else { p_cond };
    let q_pred = if invert_q { not(function, &mut make, q_cond) } else { q_cond };
    let combined = make(function, Opcode::Binary(BinaryOp::Or), vec![p_pred, q_pred], "or");

    // `SplitBlockAndInsertIfThen(combined, first)`: the head branches to the
    // new block or on.
    let tail = crate::edges::split_block_at(function, post, first);
    let void = function.instruction(function.terminator(post).expect("a jump")).ty;
    let then = function.create_block(None);
    function.insert_block(then, Some(post)).expect("a new block");
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(tail)], Flags::default(), None);
    function.insert(jump, Position::End(then)).expect("a placed block");
    let old = function.terminator(post).expect("a jump");
    let branch = function.create_instruction(
        Opcode::Br,
        void,
        vec![combined, Operand::Block(then), Operand::Block(tail)],
        Flags::default(),
        None,
    );
    function.insert(branch, Position::Before(old)).expect("a new branch");
    function.erase(old).expect("a jump defines nothing");
    let (ty, align) = match (&function.instruction(pstore).opcode, &function.instruction(qstore).opcode) {
        (Opcode::Store { align: a, .. }, Opcode::Store { align: b, .. }) => (void, (*a).min(*b)),
        _ => unreachable!("stores"),
    };
    let store = function.create_instruction(
        Opcode::Store { align, volatile: false },
        ty,
        vec![q_phi, address(function, qstore)],
        Flags::default(),
        None,
    );
    function.insert(store, Position::Before(jump)).expect("a new store");
    crate::local::combine_metadata_for_cse(&spec.outer.metadata, function, qstore, pstore, true);
    for (kind, id) in function.instruction(qstore).metadata.clone() {
        function.set_metadata(store, &kind, Some(id));
    }
    function.erase(qstore).expect("a store defines nothing");
    function.erase(pstore).expect("a store defines nothing");
    true
}

/// The stores of the two diamonds `pbi` and `qbi` end are one. Whether any
/// was merged.
pub fn merged(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    pbi: InstId,
    qbi: InstId,
) -> bool {
    let operands = |inst: InstId| match function.instruction(inst).operands[..] {
        [_, Operand::Block(t), Operand::Block(f)] => Some((t, f)),
        _ => None,
    };
    let (Some((mut ptb, mut pfb)), Some((qtb, mut qfb))) = (operands(pbi), operands(qbi)) else { return false };
    let p_block = function.parent(pbi).expect("a placed branch");
    let q_block = function.parent(qbi).expect("a placed branch");
    let mut post = single_successor(function, qfb);
    if single_successor(function, qtb) == Some(qfb) {
        post = Some(qfb);
    }
    let Some(post) = post else { return false };
    let (mut invert_p, mut invert_q) = (false, false);
    let mut qtb = qtb;
    if pfb == q_block {
        std::mem::swap(&mut pfb, &mut ptb);
        invert_p = true;
    }
    if qfb == post {
        std::mem::swap(&mut qfb, &mut qtb);
        invert_q = true;
    }
    let ptb = (ptb != q_block).then_some(ptb);
    let qtb = (qtb != post).then_some(qtb);
    let one_pred_one_succ = |function: &Function, block: BlockId, pred: BlockId, succ: BlockId| {
        single_predecessor(function, block) == Some(pred) && single_successor(function, block) == Some(succ)
    };
    if !one_pred_one_succ(function, pfb, p_block, q_block) || !one_pred_one_succ(function, qfb, q_block, post) {
        return false;
    }
    if ptb.is_some_and(|b| !one_pred_one_succ(function, b, p_block, q_block))
        || qtb.is_some_and(|b| !one_pred_one_succ(function, b, q_block, post))
    {
        return false;
    }
    // `q_block` is reached from exactly the two arms.
    let edges_in: usize = function
        .predecessors(q_block)
        .iter()
        .map(|&pred| {
            function
                .instruction(function.terminator(pred).expect("a terminator"))
                .operands
                .iter()
                .filter(|&&one| one == Operand::Block(q_block))
                .count()
        })
        .sum();
    if edges_in != 2 {
        return false;
    }
    let addresses = |function: &Function, blocks: [Option<BlockId>; 2]| {
        let mut out: Vec<Operand> = Vec::new();
        for block in blocks.into_iter().flatten() {
            for inst in function.iter_block(block) {
                if matches!(function.instruction(inst).opcode, Opcode::Store { .. }) {
                    let at = address(function, inst);
                    if !out.contains(&at) {
                        out.push(at);
                    }
                }
            }
        }
        out
    };
    let q_addresses = addresses(function, [qtb, Some(qfb)]);
    let common: Vec<Operand> =
        addresses(function, [ptb, Some(pfb)]).into_iter().filter(|one| q_addresses.contains(one)).collect();
    let mut shape = Diamond { ptb, pfb, qtb, qfb, post, invert_p, invert_q };
    let mut changed = false;
    for target in common {
        changed |= merged_to_address(context, spec, function, &mut shape, target);
    }
    changed
}

#[cfg(test)]
#[path = "condstores_tests.rs"]
mod tests;
