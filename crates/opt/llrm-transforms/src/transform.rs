//! The helpers [`crate::gvn`], [`crate::dead`], [`crate::decide`],
//! share: equal calls, liveness of
//! values, and cleanup of phis after an edge goes.
//!
//! # How it works
//! `equal_calls` makes two calls with one callee and arguments one when the
//! caller's `silent` says they touch no memory: the later takes the earlier's
//! result where the earlier has certainly run.
//!
//! `live` marks, to a fixed point, every value that something staying reads;
//! `dead` removes the rest. `_comparison` finds the `icmp` in a block that
//! defines a branch's condition. `_trivial_phis` resolves joins after an edge
//! disappears: each phi keeps only the inputs of its remaining predecessors, a
//! phi with one other value is that value, and a phi with the arms of an
//! earlier phi in its block is that phi (LLVM's `EliminateDuplicatePHINodes`).
//!
//! # Example
//! No honest example: these are helpers shared by passes, and none changes a
//! function on its own. Read [`crate::gvn`] and its tests for them in use.
//!
//! # Provides
//! `equal_calls` ([`crate::gvn`]); `live` and `live_except` ([`crate::dead`],
//! [`crate::floatloop`]); `_comparison` ([`crate::decide`]);
//! `_trivial_phis` ([`crate::decide`]).
//!
//! # Used by
//! [`crate::gvn`] calls `equal_calls` on each function it numbers;
//! [`crate::dead`] and [`crate::floatloop`] call `live` to decide what to
//! delete; [`crate::decide`]
//! calls `_trivial_phis` after it drops an edge. Not a
//! pass: it runs only inside those.
//!
//! # Needs
//! `cfg::Dominance` (`llrm_analysis`).

use std::collections::BTreeMap;

use llrm_analysis::{cfg, ssa};
use llrm_mir::context::Context;
use llrm_mir::dense::IdSet;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::TypeId;
use llrm_support::hash::IndexMap;

use crate::edges;
use crate::lcssa::{arms, from_arms};

/// Values something that stays reads, to a fixed point.
pub fn live(
    context: &Context,
    callees: &Callees,
    function: &Function,
) -> IdSet<ValueId> {
    live_except(context, callees, function, |_| false)
}

/// `live`, as if the instructions `skipped` picks did not stay.
pub fn live_except(
    context: &Context,
    callees: &Callees,
    function: &Function,
    skipped: impl Fn(InstId) -> bool,
) -> IdSet<ValueId> {
    let mut alive = IdSet::new();
    let mut pending = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| !skipped(inst) && crate::dead::_kept(context, callees, function, inst))
        .collect::<Vec<_>>();
    while let Some(inst) = pending.pop() {
        for &operand in &function.instruction(inst).operands {
            if let Operand::Value(value) = operand
                && alive.insert(value)
                && let ValueDef::Instruction(defining) = function.value(value).def
            {
                pending.push(defining);
            }
        }
    }
    alive
}

/// The comparison supplying `branch`'s condition: the `icmp` in `block`
/// that defines it.
///
/// The old one found the flag-setting `cmp`, or a `test`-like and/or/xor
/// read for equality, whose flags the branch read; both are an `icmp` here.
/// A float compare was refused as floating work, so `fcmp` is not one.
pub fn _comparison(
    function: &Function,
    block: BlockId,
    branch: InstId,
) -> Option<InstId> {
    let instruction = function.instruction(branch);
    if instruction.opcode != Opcode::Br || instruction.operands.len() != 3 {
        return None;
    }
    let Operand::Value(condition) = instruction.operands[0] else {
        return None;
    };
    function
        .iter_block(block)
        .find(|&one| function.instruction(one).result == Some(condition))
        .filter(|&one| matches!(function.instruction(one).opcode, Opcode::ICmp(_)))
}

// Blocks `_trivial_phis` looked at on this thread, for a test that a pass
// calling it does not call it once per change.
#[cfg(test)]
thread_local! {
    pub(crate) static BLOCKS_VISITED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Resolve single-valued joins after an edge disappears: each phi keeps
/// only its predecessors' inputs, one naming a single other value is that
/// value, and one with an earlier phi's arms is that phi (LLVM's
/// `EliminateDuplicatePHINodes`).
pub fn _trivial_phis(function: &mut Function) -> Result<(), String> {
    let blocks = function.layout().to_vec();
    _trivial_phis_in(function, &blocks)
}

/// `_trivial_phis` of `blocks` alone: the joins whose edges changed, where a
/// single-entry phi elsewhere (a loop's closing phi) must stay.
pub fn _trivial_phis_in(
    function: &mut Function,
    blocks: &[BlockId],
) -> Result<(), String> {
    let predecessors = blocks.iter().map(|&block| (block, function.predecessors(block))).collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for &block in blocks {
            #[cfg(test)]
            BLOCKS_VISITED.with(|visited| visited.set(visited.get() + 1));
            for phi in edges::phis(function, block) {
                let result = function.instruction(phi).result.expect("a phi's value");
                let incoming = arms(function, phi)
                    .into_iter()
                    .filter(|(_, at)| predecessors[&block].contains(at))
                    .collect::<Vec<_>>();
                let mut values = incoming
                    .iter()
                    .map(|&(value, _)| value)
                    .filter(|&value| value != Operand::Value(result))
                    .collect::<Vec<_>>();
                values.dedup();
                if let [value] = values[..] {
                    function.replace_value(result, value);
                    function.set_operands(phi, Vec::new());
                    function.erase(phi)?;
                    changed = true;
                } else if from_arms(&incoming) != function.instruction(phi).operands {
                    function.set_operands(phi, from_arms(&incoming));
                }
            }
        }
        let mut duplicates = false;
        for &block in blocks {
            duplicates |= duplicate_phis_in(function, block)?;
        }
        if !duplicates && !changed {
            return Ok(());
        }
    }
}

#[cfg(test)]
thread_local! {
    /// How many blocks `duplicate_phis_in` walked, for the test that a block of
    /// fewer than two phis is not.
    pub(crate) static WALKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `PHICSENumPHISmallSize`: up to this many phis `EliminateDuplicatePHINodes`
/// compares pair by pair (`EliminateDuplicatePHINodesNaiveImpl`), more it
/// hashes.
const NAIVE_PHIS: usize = 32;

#[cfg(test)]
thread_local! {
    /// How many blocks `duplicate_phis_in` hashed (more than `NAIVE_PHIS`
    /// phis).
    pub(crate) static HASHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `isIdenticalToWhenDefined` of two phis: the same type and, for each
/// predecessor, the same value, in any order of the arms.
fn identical_phis(
    function: &Function,
    a: InstId,
    b: InstId,
) -> bool {
    let (one, other) = (function.instruction(a), function.instruction(b));
    if one.ty != other.ty || one.operands.len() != other.operands.len() {
        return false;
    }
    one.operands.chunks(2).all(|arm| other.operands.chunks(2).any(|candidate| candidate == arm))
}

/// `EliminateDuplicatePHINodesNaiveImpl`: each later phi identical to an
/// earlier one is replaced by it, and the comparison starts over from the first
/// phi (the replacement can make earlier pairs identical). Whether any was.
fn duplicate_phis_compared(
    function: &mut Function,
    mut phis: Vec<InstId>,
) -> Result<bool, String> {
    let mut changed = false;
    let mut at = 0;
    while at < phis.len() {
        let kept = phis[at];
        let duplicate = (at + 1..phis.len()).find(|&later| identical_phis(function, kept, phis[later]));
        let Some(later) = duplicate else {
            at += 1;
            continue;
        };
        let phi = phis.remove(later);
        let (result, keeps) = (
            function.instruction(phi).result.expect("a phi's value"),
            function.instruction(kept).result.expect("a phi's value"),
        );
        function.replace_value(result, Operand::Value(keeps));
        function.set_operands(phi, Vec::new());
        function.erase(phi)?;
        changed = true;
        at = 0;
    }
    Ok(changed)
}

/// Each phi whose arms, edge for edge, are an earlier phi's in `block`
/// replaced by that phi: LLVM's `EliminateDuplicatePHINodes(BB)`. Whether any
/// was.
pub fn duplicate_phis_in(
    function: &mut Function,
    block: BlockId,
) -> Result<bool, String> {
    // Two phis at least make a duplicate: most blocks have none or one, and
    // are not walked (d_faces -O2: 581 Ir a block visit against the pin's 142).
    if !edges::has_two_phis(function, block) {
        return Ok(false);
    }
    #[cfg(test)]
    WALKED.with(|walked| walked.set(walked.get() + 1));
    let phis = edges::phis(function, block);
    if phis.len() <= NAIVE_PHIS {
        return duplicate_phis_compared(function, phis);
    }
    #[cfg(test)]
    HASHED.with(|hashed| hashed.set(hashed.get() + 1));
    let mut changed = false;
    let mut seen = llrm_support::hash::HashMap::<(TypeId, Vec<(BlockId, Operand)>), ValueId>::default();
    for phi in phis {
        let op = function.instruction(phi);
        let (ty, result) = (op.ty, op.result.expect("a phi's value"));
        let mut incoming = arms(function, phi).into_iter().map(|(value, from)| (from, value)).collect::<Vec<_>>();
        incoming.sort_by_key(|&(from, _)| from);
        match seen.entry((ty, incoming)) {
            std::collections::hash_map::Entry::Occupied(earlier) => {
                function.replace_value(result, Operand::Value(*earlier.get()));
                function.set_operands(phi, Vec::new());
                function.erase(phi)?;
                changed = true;
            }
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(result);
            }
        }
    }
    Ok(changed)
}

// CSE only ever removes an operation whose whole answer is in its operands.
//
// By mnemonic, each old kind's instruction: `sub` is also Neg, `xor` Not,
// `getelementptr` PtrOffset and Address, `icmp` each comparison, the casts
// Convert, SignExtend and Extract. FixedMul, Smulhi and Concat were
// machine idioms with no one instruction; Copy has none. The pointer casts
// are the rich MIR's own: a segment made a pointer twice is one pointer.
pub const _PURE: [&str; 22] = [
    "add",
    "sub",
    "mul",
    "getelementptr",
    "udiv",
    "sdiv",
    "urem",
    "srem",
    "and",
    "or",
    "xor",
    "shl",
    "lshr",
    "ashr",
    "trunc",
    "zext",
    "sext",
    "icmp",
    "inttoptr",
    "ptrtoint",
    "bitcast",
    "addrspacecast",
];

/// The old `op.floating` kinds `_computation` took: arithmetic and the
/// conversions to and from integers. Fsqrt is an intrinsic call here.
pub fn _floating(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv)
            | Opcode::Cast(
                CastOp::FPTrunc
                    | CastOp::FPExt
                    | CastOp::FPRound
                    | CastOp::FPToUI
                    | CastOp::FPToSI
                    | CastOp::UIToFP
                    | CastOp::SIToFP
            )
    )
}

/// Two calls with the same callee and arguments that touch no memory (`silent`
/// says which) are one: the later one is the earlier's value, where the earlier
/// has certainly run. LLVM's GVN numbers a call that `doesNotAccessMemory` and
/// gcc's FRE a call to a const function. `silent` is asked only of calls that
/// have an equal. Whether anything changed.
pub fn equal_calls(
    function: &mut Function,
    silent: &dyn Fn(&Function, InstId) -> bool,
) -> Result<bool, String> {
    // By callee and arguments, in layout order: only groups of two or more
    // are looked at again.
    let mut groups: IndexMap<(TypeId, &[Operand]), Vec<(usize, usize, InstId)>> = IndexMap::default();
    for (at, &block) in function.layout().iter().enumerate() {
        for (index, inst) in function.iter_block(block).enumerate() {
            let op = function.instruction(inst);
            if matches!(op.opcode, Opcode::Call(_)) && op.result.is_some() {
                groups.entry((op.ty, &op.operands[..])).or_default().push((at, index, inst));
            }
        }
    }
    let groups: Vec<Vec<(usize, usize, InstId)>> = groups
        .into_values()
        .filter(|calls| calls.len() > 1)
        .map(|calls| calls.into_iter().filter(|&(_, _, inst)| silent(function, inst)).collect::<Vec<_>>())
        .filter(|calls| calls.len() > 1)
        .collect();
    if groups.is_empty() {
        return Ok(false);
    }
    let dominance = cfg::Dominance::of(function);
    let layout = function.layout().to_vec();
    let rank: IndexMap<BlockId, usize> =
        dominators_first(function, &dominance).into_iter().enumerate().map(|(rank, block)| (block, rank)).collect();
    let mut groups = groups;
    for calls in &mut groups {
        calls.sort_by_key(|&(at, index, _)| (rank[&layout[at]], index));
    }
    let mut swap: Vec<(ValueId, ValueId)> = Vec::new();
    for calls in &groups {
        let mut leaders: Vec<(usize, usize, InstId)> = Vec::new();
        for &(at, index, inst) in calls {
            let earlier = leaders
                .iter()
                .rev()
                .find(|leader| _reaches(leader.0, leader.1, at, index, &dominance, function, &layout, layout[at]))
                .copied();
            match earlier {
                Some((_, _, leader)) => {
                    let (mine, theirs) = (function.instruction(inst).result, function.instruction(leader).result);
                    if let (Some(mine), Some(theirs)) = (mine, theirs) {
                        swap.push((mine, theirs));
                    }
                }
                None => leaders.push((at, index, inst)),
            }
        }
    }
    if swap.is_empty() {
        return Ok(false);
    }
    for &(mine, theirs) in &swap {
        function.replace_value(mine, Operand::Value(theirs));
        if let ValueDef::Instruction(inst) = function.value(mine).def {
            function.erase(inst)?;
        }
    }
    Ok(true)
}

/// Python's `_computation` key tuple.
///
/// `name` was the old operation's variant; it is the opcode, whose
/// predicate, cast or element type is part of it, with the flags. The
/// result widths are the result type. `operands` is `(unordered, named)`:
/// a commutative pair is sorted and deduplicated.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct _Computation {
    pub opcode: Opcode,
    pub flags: Flags,
    pub ty: TypeId,
    pub operands: (bool, Vec<Operand>),
}

/// What this operation computes, or None where that is not only its
/// operands. Each operand is named as `stands` rewrites it.
///
/// A load is its pointer's bytes, as an old load was its cell's; a
/// volatile one is the old barrier.
pub fn _computation(
    op: &Instruction,
    stands: &BTreeMap<ValueId, Operand>,
) -> Option<_Computation> {
    let floating = _floating(&op.opcode);
    let load = matches!(op.opcode, Opcode::Load { volatile: false, .. });
    if !(_PURE.contains(&op.opcode.mnemonic()) || load) && !floating {
        return None;
    }
    if op.result.is_none() || op.operands.is_empty() {
        return None;
    }
    let mut named = Vec::new();
    for &one in &op.operands {
        match one {
            Operand::Value(_) => named.push(ssa::provider(one, stands).ok()?),
            Operand::Constant(_) => named.push(one),
            Operand::Block(_) => return None,
        }
    }
    let unordered = named.len() == 2
        && matches!(
            op.opcode,
            Opcode::Binary(BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)
                | Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne)
        );
    if unordered {
        // Any total order will do.
        if named[0] > named[1] {
            named.swap(0, 1);
        }
        if named[0] == named[1] {
            named.pop();
        }
    }
    Some(_Computation { opcode: op.opcode.clone(), flags: op.flags, ty: op.ty, operands: (unordered, named) })
}

/// The blocks with each before the ones it dominates: by dominator depth, not
/// by the layout, which a removed join leaves behind the blocks it came to
/// dominate.
pub fn dominators_first(
    function: &Function,
    dominance: &cfg::Dominance,
) -> Vec<BlockId> {
    let depths = dominance.depths(function);
    let mut blocks = function.layout().to_vec();
    blocks.sort_by_key(|&block| depths.get(&cfg::id(block)).copied().filter(|&depth| depth > 0).unwrap_or(usize::MAX));
    blocks
}

/// Whether the earlier operation has certainly run by the later one.
pub fn _reaches(
    at: usize,
    where_: usize,
    then: usize,
    index: usize,
    dominance: &cfg::Dominance,
    function: &Function,
    layout: &[BlockId],
    block: BlockId,
) -> bool {
    if at == then {
        return where_ < index;
    }
    dominance.dominates(function, cfg::id(layout[at]), cfg::id(block))
}

#[cfg(test)]
mod tests {

    use super::_trivial_phis;
    use crate::testing::{parsed, printed};

    /// A dead arm and the block it came from go; the join it leaves with one
    /// value is that value.
    #[test]
    fn test_a_dead_arm_leaves_a_join_that_is_its_one_value() {
        let mut module = parsed(
            "define i16 @f(i16 %x) {
b0:
  br label %b2

b1:
  %y = add i16 %x, 1
  br label %b2

b2:
  %r = phi i16 [ %x, %b0 ], [ %y, %b1 ]
  %s = add i16 %r, 2
  ret i16 %s
}
",
        );
        let mut function = module.function_mut("f").unwrap().1.clone();
        llrm_analysis::cfg::_unreachable(&mut module.context, &mut function);
        _trivial_phis(&mut function).unwrap();
        *module.function_mut("f").unwrap().1 = function;
        assert_eq!(
            printed(&module),
            "define i16 @f(i16 %x) {
b0:
  br label %b2

b2:
  %s = add i16 %x, 2
  ret i16 %s
}
"
        );
    }

    /// A phi that reads only itself and one other value is that value.
    #[test]
    fn test_a_loop_phi_of_one_outside_value_is_that_value() {
        let mut module = parsed(
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  %r = phi i16 [ %x, %b0 ], [ %r, %b1 ]
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %r
}
",
        );
        _trivial_phis(module.function_mut("f").unwrap().1).unwrap();
        assert_eq!(
            printed(&module),
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %x
}
"
        );
    }

    /// SROA built a loop phi per slice beside promote's identical ones,
    /// and nbody carried twelve i32 values twice.
    #[test]
    fn a_phi_with_an_earlier_phis_arms_is_that_phi() {
        let mut module = parsed(
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  %r = phi i16 [ %x, %b0 ], [ %t, %b1 ]
  %s = phi i16 [ %t, %b1 ], [ %x, %b0 ]
  %t = add i16 %r, 1
  %u = add i16 %s, %t
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %u
}
",
        );
        _trivial_phis(module.function_mut("f").unwrap().1).unwrap();
        let after = printed(&module);
        assert!(after.matches("phi").count() == 1 && after.contains("add i16 %r, %t"), "{after}");
    }
}
