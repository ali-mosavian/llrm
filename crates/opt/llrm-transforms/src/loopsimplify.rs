//! Loop canonical form: every reducible loop gets one preheader, one latch
//! and dedicated exits, so the loop passes can assume them.
//!
//! # How it works
//! For each loop the pass finds the edges to group and routes each group
//! through one new block, which the target's phis read (`grouped`):
//! - a preheader, when the header has more than one predecessor outside the
//!   loop, or the one it has also branches elsewhere;
//! - one latch, when the header has several backedges;
//! - a dedicated exit, for each exit block that is also reached from outside
//!   the loop, so all its predecessors are in the loop.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   a["<b>a</b>"]:::mid
//!   b["<b>b</b>"]:::mid
//!   h["<b>H</b>"]:::mid
//!   l1["<b>L1</b>"]:::mid
//!   l2["<b>L2</b>"]:::mid
//!   x["<b>X</b>"]:::mid
//!   y["<b>y</b>"]:::mid
//!   a --> h
//!   b --> h
//!   h --> l1
//!   h --> l2
//!   l1 --> h
//!   l2 --> h
//!   h --> x
//!   y --> x
//! ```
//!
//! *Before: two entries, two latches, an exit also reached from y*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   a["<b>a</b>"]:::mid
//!   b["<b>b</b>"]:::mid
//!   p["<b>P</b>"]:::mid
//!   h["<b>H</b>"]:::mid
//!   l1["<b>L1</b>"]:::mid
//!   l2["<b>L2</b>"]:::mid
//!   l["<b>L</b>"]:::mid
//!   e["<b>E</b>"]:::mid
//!   x["<b>X</b>"]:::mid
//!   y["<b>y</b>"]:::mid
//!   a --> p
//!   b --> p
//!   p --> h
//!   h --> l1
//!   h --> l2
//!   l1 --> l
//!   l2 --> l
//!   l --> h
//!   h --> e
//!   e --> x
//!   y --> x
//! ```
//!
//! *After: preheader P, one latch L, dedicated exit E*
//!
//! A phi on the target is split: the grouped arms become a phi of the new
//! block (or the one value they all share) and one arm of the target's phi.
//! Grouping needs every source to end in a `br` whose edge can be retargeted,
//! and every phi of the target to name exactly the target's predecessors.
//!
//! The loops are processed one at a time. What a loop needs grouped is worked
//! out from the edges of its blocks and of the targets, and every grouping is
//! checked before the first is made, so a refused one leaves the body as it
//! was. No copy of the function and no graph of it is made. The loops of
//! the shape are updated for the new blocks rather than found again
//! (`LLRM_CHECK_SHAPE` compares them with a new shape). A function with an
//! irreducible loop is left alone.
//!
//! # Example
//! Before:
//! ```mir
//! define i16 @f(i1 %c, i16 %s, i16 %t) {
//! b0:
//!   br i1 %c, label %b5, label %b4
//!
//! b5:
//!   br label %b1
//!
//! b4:
//!   br label %b1
//!
//! b1:
//!   %carried = phi i16 [ %s, %b5 ], [ %t, %b4 ], [ %stepped, %b2 ]
//!   br i1 %c, label %b2, label %b3
//!
//! b2:
//!   %stepped = add i16 %carried, 1
//!   br label %b1
//!
//! b3:
//!   ret i16 %carried
//! }
//! ```
//! After:
//! ```mir
//! define i16 @f(i1 %c, i16 %s, i16 %t) {
//! b0:
//!   br i1 %c, label %b5, label %b4
//!
//! b5:
//!   br label %0
//!
//! b4:
//!   br label %0
//!
//! b1:
//!   %carried = phi i16 [ %stepped, %b2 ], [ %1, %0 ]
//!   br i1 %c, label %b2, label %b3
//!
//! b2:
//!   %stepped = add i16 %carried, 1
//!   br label %b1
//!
//! b3:
//!   ret i16 %carried
//!
//! 0:
//!   %1 = phi i16 [ %s, %b5 ], [ %t, %b4 ]
//!   br label %b1
//! }
//! ```
//! `b1` was entered from `b5` and `b4`. Both now jump to the new preheader
//! `%0`, whose phi holds the two seeds, so the header's phi has one arm from
//! outside the loop.
//!
//! From unit test `test_grouping_preserves_each_phi_edge_value` in
//! `loopsimplify_tests.rs`.
//!
//! # Runs
//! In [`crate::pipeline`]'s per-body fixed point, after `tailrec` and before
//! `lcssa`; again after the header copy of `-ftree-ch`, where it
//! re-simplifies the loops that copy made; [`simplified`] is called by
//! [`crate::lsr`] after it guards a loop. On at every level above -O0.
//!
//! # Keeps
//! Nothing where it changes a body; it reads only the CFG's structure.
//!
//! # See also
//! LLVM's `LoopSimplify`.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_mir::edit::Position;
use llrm_mir::footprint::Footprint;
use llrm_mir::module::{BlockId, Function, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::edges;

pub struct LoopSimplify;

impl FunctionPass for LoopSimplify {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "loopsimplify"
    }

    /// Preheaders, latches and dedicated exits are made from the CFG's shape
    /// alone.
    fn reads(&self) -> Footprint {
        Footprint::STRUCTURE
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let shape = (*analyses.get::<cfg::Shape>(unit.context, unit.layout, unit.function)).clone();
        if _simplified(unit.function, shape) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// Whether the edges from `sources` into `target` can be routed through one
/// block: they are edges of the target, it is not the entry, every source ends
/// in a branch that takes the edge, and the target's phis read each predecessor
/// once. Asked of the target's own predecessors, not of the graph of the
/// function.
fn groupable(
    function: &Function,
    target: i64,
    sources: &BTreeSet<i64>,
) -> bool {
    let Some(entry) = function.entry().map(cfg::id) else { return false };
    let destination = cfg::block(target);
    let reaching: BTreeSet<i64> = function.predecessors(destination).into_iter().map(cfg::id).collect();
    if sources.is_empty() || !sources.is_subset(&reaching) || target == entry {
        return false;
    }
    for &parent in sources {
        let Some(last) = function.terminator(cfg::block(parent)) else { return false };
        match function.instruction(last).operands.len() {
            _ if function.instruction(last).opcode != Opcode::Br => return false,
            3 if !edges::conditional(function, cfg::block(parent), destination) => return false,
            1 if function.successors(cfg::block(parent)) != [destination] => return false,
            _ => {}
        }
    }
    let at = |block: Operand| match block {
        Operand::Block(block) => cfg::id(block),
        _ => unreachable!("a phi's block operand"),
    };
    edges::phis(function, destination).iter().all(|&phi| {
        function.instruction(phi).operands.chunks(2).map(|pair| at(pair[1])).collect::<BTreeSet<_>>() == reaching
    })
}

/// The edges from `sources` into `target` routed through one new block,
/// which `target`'s phis read; the new block, or `None`, changing nothing,
/// where a source's terminator or a phi cannot be regrouped.
pub fn grouped(
    function: &mut Function,
    target: i64,
    sources: &BTreeSet<i64>,
) -> Option<BlockId> {
    if !groupable(function, target, sources) {
        return None;
    }
    let destination = cfg::block(target);
    let phis = edges::phis(function, destination);
    let incoming = |function: &Function, phi| {
        function.instruction(phi).operands.chunks(2).map(|pair| (pair[0], pair[1])).collect::<Vec<_>>()
    };
    let at = |block: Operand| match block {
        Operand::Block(block) => cfg::id(block),
        _ => unreachable!("a phi's block operand"),
    };
    let bridge = function.create_block(None);
    function.insert_block(bridge, None).expect("a new block");
    for phi in phis {
        let pairs = incoming(function, phi);
        let (grouped, kept): (Vec<_>, Vec<_>) = pairs.into_iter().partition(|&(_, block)| sources.contains(&at(block)));
        let values = grouped.iter().map(|&(value, _)| value).collect::<Vec<_>>();
        let result = if values.iter().all(|&one| one == values[0]) {
            values[0]
        } else {
            let ty = function.instruction(phi).ty;
            let operands = grouped.iter().flat_map(|&(value, block)| [value, block]).collect();
            let merged = function.create_instruction(Opcode::Phi, ty, operands, Flags::default(), None);
            function.insert(merged, Position::End(bridge)).expect("a placed block");
            Operand::Value(function.instruction(merged).result.expect("a phi's value"))
        };
        let operands = kept
            .into_iter()
            .chain([(result, Operand::Block(bridge))])
            .flat_map(|(value, block)| [value, block])
            .collect();
        function.set_operands(phi, operands);
    }
    let void = function
        .instruction(function.terminator(cfg::block(*sources.first().expect("nonempty"))).expect("checked above"))
        .ty;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(destination)], Flags::default(), None);
    function.insert(jump, Position::End(bridge)).expect("a placed block");
    for &parent in sources {
        let last = function.terminator(cfg::block(parent)).expect("checked above");
        edges::retarget(function, last, destination, bridge);
    }
    Some(bridge)
}

/// Whether `loop_` has a block to group in `function`: its entries from outside
/// are not one block that only enters it, it has more than one latch, or an
/// exit is reached from outside the loop too.
fn _needs_grouping(
    function: &Function,
    loop_: &loops::Loop,
) -> bool {
    let outside = _outside(function, loop_);
    let Some(&parent) = outside.first() else { return false };
    if outside.len() != 1
        || function.successors(cfg::block(parent)) != [cfg::block(loop_.header)]
        || loop_.latches.len() != 1
    {
        return true;
    }
    _exits(function, loop_).iter().any(|&target| !_reached_only_from(function, target, &loop_.body))
}

/// The predecessors of `loop_`'s header that are not in it.
fn _outside(
    function: &Function,
    loop_: &loops::Loop,
) -> BTreeSet<i64> {
    function
        .predecessors(cfg::block(loop_.header))
        .into_iter()
        .map(cfg::id)
        .filter(|at| !loop_.body.contains(at))
        .collect()
}

thread_local! {
    /// Blocks looked at to find loops' exits, for a test of the pass's slope.
    pub(crate) static EXIT_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn exit_work(blocks: usize) {
    EXIT_WORK.with(|work| work.set(work.get() + blocks));
}

/// Whether any of `loops` is not in the form: `_needs_grouping` of each, with
/// the exits of all of them found in one sweep over the blocks and their
/// predecessors, not by a walk over every loop's body (the blocks times how
/// deep they nest, on every run of a pass that finds nothing to do).
fn any_needs_grouping(
    function: &Function,
    loops: &[loops::Loop],
) -> bool {
    let Some(first) = loops.first() else { return false };
    let forest = std::rc::Rc::clone(first.body.forest());
    let mut entered = vec![false; forest.loops.len()];
    for loop_ in loops {
        let outside = _outside(function, loop_);
        let Some(&parent) = outside.first() else { continue };
        if outside.len() != 1
            || function.successors(cfg::block(parent)) != [cfg::block(loop_.header)]
            || loop_.latches.len() != 1
        {
            return true;
        }
        entered[loop_.body.index()] = true;
    }
    // An exit `target` of a loop is dedicated when every predecessor is in the
    // loop. A predecessor is in the loops from its innermost one up to the
    // first that holds `target`: a loop on some of those chains and not on all
    // has an exit reached from outside it too.
    let mut seen: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for target in function.layout().to_vec() {
        let predecessors = function.predecessors(target);
        exit_work(1 + predecessors.len());
        if predecessors.len() < 2 {
            continue;
        }
        seen.clear();
        let at = cfg::id(target);
        for &from in predecessors.iter() {
            let mut inner = forest.innermost(cfg::id(from));
            while let Some(index) = inner.filter(|&index| !forest.contains(index, at)) {
                exit_work(1);
                *seen.entry(index).or_default() += 1;
                inner = forest.loops[index].parent;
            }
        }
        if seen.iter().any(|(&index, &count)| count < predecessors.len() && entered[index]) {
            return true;
        }
    }
    false
}

/// The blocks outside `loop_` that its blocks branch to.
fn _exits(
    function: &Function,
    loop_: &loops::Loop,
) -> BTreeSet<i64> {
    exit_work(loop_.body.len());
    loop_
        .body
        .iter()
        .flat_map(|&at| function.successors(cfg::block(at)))
        .map(cfg::id)
        .filter(|at| !loop_.body.contains(at))
        .collect()
}

/// Whether every predecessor of `target` is in `body`.
fn _reached_only_from(
    function: &Function,
    target: i64,
    body: &llrm_mir::forest::Body,
) -> bool {
    function.predecessors(cfg::block(target)).into_iter().all(|from| body.contains(&cfg::id(from)))
}

/// Whether any loop changed.
pub fn simplified(function: &mut Function) -> bool {
    let shape = cfg::Shape::of(function);
    _simplified(function, shape)
}

/// `simplified`, `shape` being `function`'s.
fn _simplified(
    function: &mut Function,
    mut shape: cfg::Shape,
) -> bool {
    if function.entry().is_none() || !shape.dominance.irreducible(function).is_empty() {
        return false;
    }
    if !any_needs_grouping(function, &shape.loops) {
        return false;
    }
    let mut changed = false;
    let headers = shape.loops.iter().map(|loop_| loop_.header).collect::<Vec<_>>();
    for header in headers {
        let original = shape.loops.iter().find(|loop_| loop_.header == header).expect("StopIteration").clone();
        // A loop that is already in the form has nothing to group.
        if !_needs_grouping(function, &original) {
            continue;
        }
        // What is to be grouped, as (target, sources), worked out from the loop
        // and the edges of its blocks alone: the groupings of the
        // header do not touch the exits' predecessors, and the
        // blocks they make are in the loop and branch to its header.
        let mut plan: Vec<(i64, BTreeSet<i64>)> = Vec::new();
        let outside = _outside(function, &original);
        let Some(&parent) = outside.first() else { continue };
        if outside.len() != 1 || function.successors(cfg::block(parent)) != [cfg::block(original.header)] {
            plan.push((original.header, outside));
        }
        if original.latches.len() != 1 {
            plan.push((original.header, original.latches.clone()));
        }
        for target in _exits(function, &original) {
            if !_reached_only_from(function, target, &original.body) {
                let sources = function
                    .predecessors(cfg::block(target))
                    .into_iter()
                    .map(cfg::id)
                    .filter(|from| original.body.contains(from))
                    .collect::<BTreeSet<_>>();
                plan.push((target, sources));
            }
        }
        // All or none: a grouping that cannot be made leaves the loop as it is.
        if plan.is_empty() || !plan.iter().all(|(target, sources)| groupable(function, *target, sources)) {
            continue;
        }
        let mut bridges: Vec<(i64, i64, BTreeSet<i64>)> = Vec::new();
        for (target, sources) in plan {
            let Some(bridge) = grouped(function, target, &sources) else {
                unreachable!("groupable said it could be made");
            };
            bridges.push((cfg::id(bridge), target, sources));
        }
        changed = true;
        for loop_ in &mut shape.loops {
            for (bridge, target, sources) in &bridges {
                _bridged(loop_, *bridge, *target, sources);
            }
        }
        if cfg!(test) || llrm_support::env_flag!("LLRM_CHECK_SHAPE") {
            _agrees(&shape, function);
        }
    }
    changed
}

/// What `loop_` is once `bridge` stands on the edges from `sources` to
/// `target`: the block joins it where `target` and every source are in it (it
/// reaches the latch through `target`, and what reaches it came from inside),
/// and becomes its only latch where `target` is its header.
fn _bridged(
    loop_: &mut loops::Loop,
    bridge: i64,
    target: i64,
    sources: &BTreeSet<i64>,
) {
    if !loop_.body.contains(&target) || !sources.iter().all(|at| loop_.body.contains(at)) {
        return;
    }
    loop_.body.insert(bridge);
    if loop_.header == target {
        loop_.latches = BTreeSet::from([bridge]);
    }
}

/// `LLRM_CHECK_SHAPE`: the loops kept up to date are those of a new shape of
/// the function.
fn _agrees(
    kept: &cfg::Shape,
    function: &Function,
) {
    let fresh = cfg::Shape::of(function);
    let by_header = |loops: &[loops::Loop]| {
        loops
            .iter()
            .map(|one| (one.header, (one.body.clone(), one.latches.clone())))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    assert!(
        by_header(&kept.loops) == by_header(&fresh.loops),
        "the loops kept through loopsimplify differ from a new shape's"
    );
}

#[cfg(test)]
#[path = "loopsimplify_tests.rs"]
mod tests;
