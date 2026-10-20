//! Where a loop holds exactly one other loop: the nest's blocks, and what may
//! be asked of it (an inner loop that counts the same in every outer trip, the
//! accesses of its body).
//!
//! # How it works
//! A nest is a loop that holds exactly one other loop and nothing else besides
//! pure work. The outer header tests and has one way into the loop. A chain of
//! single-successor blocks `pre` leads from it to the inner header, and a
//! chain `post` from the inner exit to the outer latch; no other block is in
//! the outer loop. Work in `pre`, `post` and the outer header is pure (no
//! memory, no division), and `pre`, `post` have no phis. The inner loop has one
//! latch and leaves only from its header, into a block only it enters.
//!
//! The inner loop runs the same in every outer iteration
//! (`inner_is_invariant`): its test and the starts and steps of its counters
//! read only values the nest does not change, and nothing it computes is read
//! outside it. Its memory accesses (`accesses`) are each an affine function of
//! the two counters over an identified object.
//!
//! ```text
//!   outer header --> pre... --> inner header <--+
//!        ^                        |  body ------+
//!        |                        v
//!        +------ outer latch <-- post...
//! ```
//!
//! # Example
//! The blocks of the nest of `a_column_walk_is_swapped` in
//! `loopinterchange_tests.rs`:
//!
//! ```mir
//! b1:  %j = phi ..; br %run, b2, b6        ; the outer header
//! b2:  %c = mul %j, 2; br b3               ; pre: pure work, one block
//! b3:  %i = phi ..; br %more, b4, b5       ; the inner header
//! b4:  .. store; %i.next = add %i, 1; br b3 ; the inner body and latch
//! b5:  %j.next = add %j, 1; br b1          ; post, the outer latch
//! ```
//!
//! `pre` is `[b2]`, `post` `[b5]`; the inner count is the same each outer trip.
//!
//! # Provides
//! `Nest`, `nest_of`, `inner_is_invariant`, `accesses`. Consumers: `unroll_jam`
//! and `loopinterchange`.
//!
//! # See also
//! [`crate::unroll_jam`], [`crate::loopinterchange`].

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::access::{self, Roots};
use llrm_analysis::dependence::Access;
use llrm_analysis::graph::loops::{self, Loop};
use llrm_analysis::{cfg, memory};
use llrm_mir::dense::IdSet;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::lcssa::arms;

/// A nest of two loops: `outer`'s header, then `pre` (pure blocks, in a
/// chain) to the inner loop's header, then the inner loop, whose exit starts
/// `post`, a chain to the outer loop's latch.
#[derive(Clone)]
pub(crate) struct Nest {
    pub(crate) outer: Loop,
    pub(crate) inner: Loop,
    pub(crate) pre: Vec<BlockId>,
    pub(crate) post: Vec<BlockId>,
}

impl Nest {
    pub(crate) fn outer_header(&self) -> BlockId {
        cfg::block(self.outer.header)
    }

    pub(crate) fn inner_header(&self) -> BlockId {
        cfg::block(self.inner.header)
    }

    pub(crate) fn inner_latch(&self) -> BlockId {
        cfg::block(*self.inner.latches.first().expect("one latch"))
    }

    /// The last of the blocks before the inner loop: where it is entered.
    pub(crate) fn entry(&self) -> BlockId {
        *self.pre.last().expect("a block before the inner loop")
    }
}

/// Work with no memory access and no fault, which may run earlier or later in
/// the outer iteration.
pub(crate) fn pure(opcode: &Opcode) -> bool {
    match opcode {
        Opcode::Binary(op) => !matches!(
            op,
            BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem | BinaryOp::FDiv | BinaryOp::FRem
        ),
        Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::Select | Opcode::GetElementPtr { .. } => true,
        Opcode::Freeze | Opcode::FNeg => true,
        _ => false,
    }
}

pub(crate) fn work(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function
        .iter_block(block)
        .filter(|&inst| {
            let op = &function.instruction(inst).opcode;
            *op != Opcode::Phi && !op.is_terminator()
        })
        .collect()
}

pub(crate) fn phis(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function.iter_block(block).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect()
}

/// `inner`'s nest in `outer`, where `outer` holds nothing else.
pub(crate) fn nest_of(
    function: &Function,
    graph: &[cfg::Block],
    outer: &Loop,
    inner: &Loop,
) -> Option<Nest> {
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(graph);
    let [lo] = outer.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    let [li] = inner.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    let (ho, hi) = (outer.header, inner.header);
    if ho == hi || li == hi || blocks[&lo].succ != [ho] {
        return None;
    }
    // The outer header tests, one way into the loop.
    let [first] = blocks[&ho].succ.iter().copied().filter(|at| outer.body.contains(at)).collect::<Vec<_>>()[..] else {
        return None;
    };
    if blocks[&ho].succ.len() != 2 {
        return None;
    }
    let inside = |at: &i64| outer.body.contains(at) && !inner.body.contains(at);
    let mut pre = Vec::new();
    let mut at = first;
    while at != hi {
        if !inside(&at) || at == ho || blocks[&at].succ.len() != 1 || pre.contains(&at) {
            return None;
        }
        pre.push(at);
        at = blocks[&at].succ[0];
    }
    if pre.is_empty() || !predecessors.are(&hi, &[*pre.last().expect("one"), li]) {
        return None;
    }
    // The inner loop leaves from its header alone, into a block only it enters.
    let leaving = inner.body.iter().filter(|at| blocks[at].succ.iter().any(|to| !inner.body.contains(to))).count();
    let [xi] = blocks[&hi].succ.iter().copied().filter(|at| !inner.body.contains(at)).collect::<Vec<_>>()[..] else {
        return None;
    };
    if leaving != 1 || blocks[&hi].succ.len() != 2 || !predecessors.are(&xi, &[hi]) {
        return None;
    }
    let mut post = Vec::new();
    let mut at = xi;
    loop {
        if !inside(&at) || at == ho || blocks[&at].succ.len() != 1 || post.contains(&at) {
            return None;
        }
        post.push(at);
        if at == lo {
            break;
        }
        at = blocks[&at].succ[0];
    }
    let covered = std::iter::once(ho)
        .chain(pre.iter().copied())
        .chain(post.iter().copied())
        .chain(inner.body.iter().copied())
        .collect::<BTreeSet<_>>();
    if outer.body != covered {
        return None;
    }
    let block = cfg::block;
    let pre: Vec<BlockId> = pre.into_iter().map(block).collect();
    let post: Vec<BlockId> = post.into_iter().map(block).collect();
    // Between the loops: pure work and no phis. The outer header's own work is
    // pure too, and runs again ahead of the fused loop.
    let between = pre.iter().chain(&post).copied().chain([block(ho)]);
    for at in between {
        if at != block(ho) && !phis(function, at).is_empty() {
            return None;
        }
        if work(function, at).iter().any(|&inst| !pure(&function.instruction(inst).opcode)) {
            return None;
        }
    }
    if !phis(function, block(xi)).is_empty() {
        return None;
    }
    Some(Nest { outer: outer.clone(), inner: inner.clone(), pre, post })
}

/// Whether `value` is computed in the outer loop outside the inner one.
pub(crate) fn outer_only(
    function: &Function,
    nest: &Nest,
    value: ValueId,
) -> bool {
    defined(function, value)
        .and_then(|inst| function.parent(inst))
        .is_some_and(
            |block| {
                let at = cfg::id(block);
                nest.outer.body.contains(&at) && !nest.inner.body.contains(&at)
            },
        )
}

/// The inner loop counts the same in every outer iteration: its counters start
/// at, and its test reads, only values the nest does not change, and nothing
/// it computes is read outside it.
pub(crate) fn inner_is_invariant(
    function: &Function,
    nest: &Nest,
) -> bool {
    let hi = nest.inner_header();
    let entry = nest.entry();
    // The control cone: the test and the counters' starts and steps.
    let mut pending: Vec<Operand> = Vec::new();
    let branch = function.terminator(hi).expect("a terminated header");
    pending.push(function.instruction(branch).operands[0]);
    for phi in phis(function, hi) {
        for (value, from) in arms(function, phi) {
            if from == entry {
                if matches!(
                    value,
                    Operand::Value(one) if defined(function, one).is_some_and(|inst| function.parent(inst).is_some_and(|block| nest.outer.body.contains(&cfg::id(block))))
                ) {
                    return false;
                }
            } else {
                pending.push(value);
            }
        }
        pending.push(Operand::Value(function.instruction(phi).result.expect("a phi's value")));
    }
    let mut seen = IdSet::new();
    while let Some(operand) = pending.pop() {
        let Operand::Value(value) = operand else { continue };
        if !seen.insert(value) {
            continue;
        }
        if outer_only(function, nest, value) {
            return false;
        }
        let Some(inst) = defined(function, value) else { continue };
        let inside = function.parent(inst).is_some_and(|block| nest.inner.body.contains(&cfg::id(block)));
        if !inside {
            continue;
        }
        let op = function.instruction(inst);
        if op.opcode == Opcode::Phi {
            continue;
        }
        if !pure(&op.opcode) {
            return false;
        }
        pending.extend(op.operands.iter().copied());
    }
    // Nothing it computes is read outside it.
    for &at in &nest.inner.body {
        for inst in function.iter_block(cfg::block(at)) {
            if let Some(result) = function.instruction(inst).result {
                let read_outside = function.users(result).iter().any(|used| {
                    function.parent(used.user).is_some_and(|block| !nest.inner.body.contains(&cfg::id(block)))
                });
                if read_outside {
                    return false;
                }
            }
        }
    }
    true
}

/// The instruction that defines `value`, if one does.
pub(crate) fn defined(
    function: &Function,
    value: ValueId,
) -> Option<InstId> {
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// The accesses of the inner loop, for the dependence tester; None where one
/// is not affine over an identified object, or the loop calls out.
pub(crate) fn accesses(
    unit: &memory::Unit,
    nest: &Nest,
) -> Option<Vec<Access>> {
    let counters = [&nest.outer, &nest.inner];
    let found = access::references(unit, &counters, &nest.outer.body, &nest.inner.body, &mut Roots::default())?;
    found.iter().all(|one| one.identified).then(|| found.into_iter().map(|one| one.access).collect())
}
