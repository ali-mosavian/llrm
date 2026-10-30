//! A branch on a value known in some predecessors, threaded: LLVM's
//! [`SimplifyCFGOpt::foldCondBranchOnValueKnownInPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3814)
//! and its `Impl`
//! ([L3624](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3624)).
//!
//! A predecessor that gives the condition a constant (a phi input, or a
//! predecessor that branches on the condition itself) goes straight to the
//! arm that constant chooses: the predecessors are split off to a block that
//! holds a copy of what the branching block computes, folded on the constant,
//! and that block jumps to the arm.
//!
//! # Example
//! ```text
//!   b0: br i1 %c1, b1, b2                b0: br i1 %c1, b1, b3
//!   b1: %c = ...; br b2                  b1: %c = ...; br b2
//!   b2: %p = phi i1 [ false, b0 ],  ==>  b2: %p = phi i1 [ %c, b1 ]; br i1 %p, b3, b4
//!                   [ %c, b1 ]
//!       br i1 %p, b3, b4
//! ```
//!
//! Only blocks of at most 11 instructions are threaded through
//! (`simplifycfg-max-small-block-size`), and a value of the block live in more
//! than 24 blocks stops it (`max-jump-threading-live-blocks`).
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | Ephemeral values (feeding only `llvm.assume`) are counted as instructions. |
//! | D2 | `TTI.hasBranchDivergence` is not asked: no llrm target diverges. |
//! | D3 | The new block is not moved before the destination; it is merged into its predecessor when it can be, and otherwise stays last in the layout. |
//! | D4 | No indirect branches exist, so none is skipped. |
//! | D5 | `mayHaveSideEffects` of a folded copy reads no module globals: a call is kept. |

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::dense::IdSet;
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::simplify::simplify_instruction;

use crate::twoentryphi::Speculation;
use crate::{cfg, edges, local};

/// `simplifycfg-max-small-block-size`.
const MAX_SMALL_BLOCK_SIZE: usize = 10;
/// `max-jump-threading-live-blocks`.
const MAX_LIVE_BLOCKS: usize = 24;

/// What one `foldCondBranchOnValueKnownInPredecessorImpl` did.
enum Outcome {
    Unchanged,
    Changed,
    /// Changed, and the next constant may be threaded too.
    Again,
}

/// `block`'s branch, threaded for each predecessor that knows its condition:
/// whether anything changed. `canonical_headers`: the loop headers a
/// canonical loop keeps (`NeedCanonicalLoop`), none otherwise.
pub fn folded(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
    canonical_headers: &IdSet<BlockId>,
) -> bool {
    // Threading into a loop header could move a back edge.
    if canonical_headers.contains(&block) {
        return false;
    }
    let mut ever = false;
    loop {
        match once(context, spec, function, block) {
            Outcome::Unchanged => return ever,
            Outcome::Changed => return true,
            Outcome::Again => ever = true,
        }
    }
}

/// The predecessors that give a condition each of its two values, in the order
/// they were found (`SmallMapVector<ConstantInt *, SmallSetVector<..>>`).
#[derive(Default)]
struct Known {
    arms: Vec<(bool, Vec<BlockId>)>,
}

impl Known {
    fn insert(
        &mut self,
        value: bool,
        pred: BlockId,
    ) {
        let at = match self.arms.iter().position(|&(one, _)| one == value) {
            Some(at) => at,
            None => {
                self.arms.push((value, Vec::new()));
                self.arms.len() - 1
            }
        };
        if !self.arms[at].1.contains(&pred) {
            self.arms[at].1.push(pred);
        }
    }
}

/// `foldCondBranchOnValueKnownInPredecessorImpl`.
fn once(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> Outcome {
    let Some(branch) = function.terminator(block) else { return Outcome::Unchanged };
    let &[cond, Operand::Block(if_true), Operand::Block(if_false)] = &function.instruction(branch).operands[..] else {
        return Outcome::Unchanged;
    };
    let mut known = Known::default();
    match defining_phi_in(function, cond, block) {
        Some(phi) => {
            // The degenerate phi of one input.
            if function.instruction(phi).operands.len() == 2 {
                local::fold_single_entry_phi_nodes(context, function, block);
                return Outcome::Changed;
            }
            for (value, from) in llrm_analysis::occurrence::arms(function, phi) {
                if let Some(constant) = int(context, value) {
                    known.insert(constant != 0, from);
                }
            }
        }
        None => {
            for pred in function.predecessors(block) {
                if let Some(value) = known_on_edge(function, cond, pred, block) {
                    known.insert(value, pred);
                }
            }
        }
    }
    if known.arms.is_empty() {
        return Outcome::Unchanged;
    }

    // Block is small enough; what its values reach is where they are live.
    let Some(used_in) = simple_enough_to_thread_through(function, block) else { return Outcome::Unchanged };
    // A value of the block live at both arms cannot be threaded.
    if used_in.contains(&if_true) && used_in.contains(&if_false) {
        return Outcome::Unchanged;
    }
    let Some(reaches) = reaching(function, block, &used_in) else { return Outcome::Unchanged };

    for (value, preds) in known.arms {
        let destination = if value { if_true } else { if_false };
        // A self loop, or a destination the block's values are live in.
        if destination == block || reaches.contains(&destination) {
            continue;
        }
        thread(context, spec, function, block, branch, cond, value, &preds, destination);
        return Outcome::Again;
    }
    Outcome::Unchanged
}

/// The phi `cond` is, if it is one of `block`'s.
fn defining_phi_in(
    function: &Function,
    cond: Operand,
    block: BlockId,
) -> Option<InstId> {
    let Operand::Value(value) = cond else { return None };
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    (function.instruction(inst).opcode == Opcode::Phi && function.parent(inst) == Some(block)).then_some(inst)
}

/// The value of an integer constant.
fn int(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(value) => Some(value),
        _ => None,
    }
}

/// `getKnownValueOnEdge`: `cond` on the edge `from` to `to`, if `from` branches
/// on it.
fn known_on_edge(
    function: &Function,
    cond: Operand,
    from: BlockId,
    to: BlockId,
) -> Option<bool> {
    // Not past the block defining the value: it may be the last iteration's.
    if let Operand::Value(value) = cond
        && let ValueDef::Instruction(inst) = function.value(value).def
        && function.parent(inst) == Some(to)
    {
        return None;
    }
    let last = function.terminator(from)?;
    let &[condition, Operand::Block(if_true), Operand::Block(if_false)] = &function.instruction(last).operands[..]
    else {
        return None;
    };
    (condition == cond && if_true != if_false).then_some(if_true == to)
}

/// `blockIsSimpleEnoughToThreadThrough`: the blocks that use a value `block`
/// defines, if it is small and a phi does not use one of its own values.
fn simple_enough_to_thread_through(
    function: &Function,
    block: BlockId,
) -> Option<IdSet<BlockId>> {
    let mut used_in = IdSet::new();
    let mut size = 0;
    for inst in function.iter_block(block) {
        let instruction = function.instruction(inst);
        // A copy of a `noduplicate` or `convergent` call is another call.
        if let Opcode::Call(call) = &instruction.opcode
            && call.attrs.iter().any(|attr| {
                matches!(
                    attr,
                    Attribute::Flag(flag) if flag == "noduplicate" || flag == "convergent"
                )
            })
        {
            return None;
        }
        // The phis are deleted while threading; they are no size.
        if instruction.opcode != Opcode::Phi {
            size += 1;
            if size > MAX_SMALL_BLOCK_SIZE + 1 {
                return None;
            }
        }
        let Some(result) = instruction.result else { continue };
        for one in function.users(result).iter() {
            match function.parent(one.user) {
                Some(user_block) if user_block == block => {
                    if function.instruction(one.user).opcode == Opcode::Phi {
                        return None;
                    }
                }
                Some(user_block) => {
                    used_in.insert(user_block);
                }
                None => {}
            }
        }
    }
    Some(used_in)
}

/// `findReaching` from each of `used_in`: the blocks from which a use of a
/// value of `block` is reached, back to `block`; none if there are too many.
fn reaching(
    function: &Function,
    block: BlockId,
    used_in: &IdSet<BlockId>,
) -> Option<IdSet<BlockId>> {
    let mut reaches = IdSet::new();
    let mut count = 0;
    let mut pending: Vec<BlockId> = used_in.iter().collect();
    while let Some(one) = pending.pop() {
        if one == block || !reaches.insert(one) {
            continue;
        }
        count += 1;
        if count > MAX_LIVE_BLOCKS {
            return None;
        }
        pending.extend(function.predecessors(one));
    }
    Some(reaches)
}

/// The edges from `preds` into `block` sent to `destination` instead, through
/// a block that holds `block`'s instructions folded on `value`.
#[allow(clippy::too_many_arguments)]
fn thread(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
    branch: InstId,
    cond: Operand,
    value: bool,
    preds: &[BlockId],
    destination: BlockId,
) {
    let edge = edges::split_block_predecessors(function, block, preds);
    edges::predecessor_added(function, destination, edge, block);

    let i1 = context.types.int(1);
    let known = Operand::Constant(context.constant(Constant { ty: i1, kind: ConstantKind::Int(u128::from(value)) }));
    let mut translate = ValueMap::default();
    if let Operand::Value(cond) = cond {
        translate.values.insert(cond, known);
    }
    let jump = function.terminator(edge).expect("a jump");
    let insts: Vec<InstId> = function.iter_block(block).take_while(|&inst| inst != branch).collect();
    for inst in insts {
        let result = function.instruction(inst).result;
        if function.instruction(inst).opcode == Opcode::Phi {
            let (incoming, _) = llrm_analysis::occurrence::arms(function, inst)
                .into_iter()
                .find(|&(_, from)| from == edge)
                .expect("an input from the new block");
            translate.values.insert(result.expect("a phi's value"), incoming);
            continue;
        }
        let copy = function.clone_instruction(inst);
        function.insert(copy, Position::Before(jump)).expect("a placed jump");
        function.remap_instruction(copy, &translate);
        let copy_result = function.instruction(copy).result;
        if let (Some(from), Some(to)) = (result, copy_result)
            && let Some(name) = function.value(from).name.clone()
        {
            function.set_name(to, &format!("{name}.c"));
        }
        // A copy that folds is its value; the instruction stays only for what
        // it does.
        match simplify_instruction(context, spec.layout, function, copy) {
            Some(simple) => {
                if let Some(from) = result {
                    if function.users(from).iter().next().is_some() {
                        translate.values.insert(from, simple);
                    }
                }
                if !local::may_have_side_effects(context, spec.outer.callees(), &[], function, copy) {
                    function.erase(copy).expect("a copy nothing reads");
                }
            }
            None => {
                if let (Some(from), Some(to)) = (result, copy_result)
                    && function.users(from).iter().next().is_some()
                {
                    translate.values.insert(from, Operand::Value(to));
                }
            }
        }
    }

    function.predecessor_removed(context, block, edge, false);
    function.set_operand(jump, 0, Operand::Block(destination));
    // The block was made for the edge: back into its predecessor, when it can.
    cfg::merged_into_predecessor(function, edge);
}

#[cfg(test)]
#[path = "knownpred_tests.rs"]
mod tests;
