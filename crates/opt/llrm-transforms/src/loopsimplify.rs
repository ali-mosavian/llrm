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
//! (`LLRM_CHECK_SHAPE` compares them with a new shape). The loops are the
//! natural ones, as LoopInfo finds them: an irreducible cycle is no loop, and
//! its function's natural loops are simplified all the same.
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
//! [`crate::lsr_pass`] after it guards a loop. On at every level above -O0.
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
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_support::hash::HashMap;

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
    // DEVIATE: LLVM splits a landing pad in two
    // (`SplitLandingPadPredecessors`); `ehprepare` takes one pad per
    // function, so a pad is left as it is.
    if sources.is_empty()
        || !sources.is_subset(&reaching)
        || target == entry
        || edges::starts_with_landing_pad(function, destination)
    {
        return false;
    }
    for &parent in sources {
        let Some(last) = function.terminator(cfg::block(parent)) else { return false };
        if matches!(function.instruction(last).opcode, Opcode::Invoke(_)) {
            if (_normal_destination(function, last) == Operand::Block(destination))
                == (_unwind_destination(function, last) == Operand::Block(destination))
            {
                return false;
            }
            continue;
        }
        // `replaceUsesOfWith` takes every case that names the target.
        if function.instruction(last).opcode == Opcode::Switch {
            continue;
        }
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
    // Right before `destination`, as `SplitBlockPredecessors` lays it out.
    function.insert_block_before(bridge, destination).expect("a new block");
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
        if matches!(function.instruction(last).opcode, Opcode::Invoke(_)) {
            let at = function.instruction(last).operands.len() - 3;
            let at = if _normal_destination(function, last) == Operand::Block(destination) { at } else { at + 1 };
            function.set_operand(last, at, Operand::Block(bridge));
        } else {
            edges::retarget(function, last, destination, bridge);
        }
    }
    Some(bridge)
}

/// The normal destination of the invoke `last`; its unwind destination is the
/// operand after it.
fn _normal_destination(
    function: &Function,
    last: InstId,
) -> Operand {
    let operands = &function.instruction(last).operands;
    operands[operands.len() - 3]
}

fn _unwind_destination(
    function: &Function,
    last: InstId,
) -> Operand {
    let operands = &function.instruction(last).operands;
    operands[operands.len() - 2]
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

/// The blocks outside `loop_` that its blocks branch to.
fn _exits(
    function: &Function,
    loop_: &loops::Loop,
) -> BTreeSet<i64> {
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

/// `placeSplitBlockCarefully`
/// ([LoopSimplify.cpp:80](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopSimplify.cpp#L80)):
/// a preheader stands right after an outside predecessor, so the jump into
/// the loop falls through and the block is not inside the loop's run.
fn place_carefully(
    function: &mut Function,
    new: BlockId,
    entries: &[BlockId],
    body: &llrm_mir::forest::Body,
) {
    let layout = function.layout();
    let at = layout.iter().position(|&one| one == new).expect("a placed block");
    if at > 0 && entries.contains(&layout[at - 1]) {
        return;
    }
    let next_in_loop = |pred: &BlockId| {
        let at = layout.iter().position(|one| one == pred).expect("a placed block");
        layout.get(at + 1).is_some_and(|next| body.contains(&cfg::id(*next)))
    };
    let after = entries.iter().find(|pred| next_in_loop(pred)).unwrap_or(&entries[0]);
    function.move_block_after(new, *after).expect("placed blocks");
}

thread_local! {
    static PARENT_PROBES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static BRIDGE_PROBES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The blocks looked at to find the loops' parents on this thread: a test
/// bounds it by the size of the function.
pub fn parent_probes() -> usize {
    PARENT_PROBES.with(std::cell::Cell::get)
}

/// The loops looked at to keep them up to date through the bridges made on this
/// thread: a test bounds it by the loops around the blocks a bridge joins.
pub fn bridge_probes() -> usize {
    BRIDGE_PROBES.with(std::cell::Cell::get)
}

/// The headers in the order `simplifyLoop` works through them: the top-level
/// loops in `LoopInfo`'s order (reverse program order, program order being
/// the header's place in reverse postorder), and in each nest its loops from
/// the back of a breadth-first list, so the deepest and latest first.
fn visit_order(
    function: &Function,
    loops: &[loops::Loop],
) -> Vec<i64> {
    let Some(entry) = function.entry() else { return Vec::new() };
    let program: Vec<i64> = loops::reverse_postorder(&cfg::graph(function), cfg::id(entry));
    let places: HashMap<i64, usize> = program.iter().enumerate().map(|(at, &block)| (block, at)).collect();
    let place = |header: i64| places.get(&header).copied().unwrap_or(usize::MAX);
    // A loop's parent is the smallest other loop holding its header: the
    // loops from the largest down each claim their blocks, so the owner of a
    // header when its loop comes up is its parent.
    let mut by_size: Vec<&loops::Loop> = loops.iter().collect();
    by_size.sort_by_key(|loop_| std::cmp::Reverse(loop_.body.len()));
    let mut owner: HashMap<i64, i64> = Default::default();
    let mut parents: HashMap<i64, Option<i64>> = Default::default();
    for loop_ in by_size {
        parents.insert(loop_.header, owner.get(&loop_.header).copied());
        for &block in &loop_.body {
            PARENT_PROBES.with(|one| one.set(one.get() + 1));
            owner.insert(block, loop_.header);
        }
    }
    let mut children: std::collections::BTreeMap<Option<i64>, Vec<i64>> = Default::default();
    for loop_ in loops {
        children.entry(parents[&loop_.header]).or_default().push(loop_.header);
    }
    for siblings in children.values_mut() {
        siblings.sort_by_key(|&header| place(header));
    }
    let mut order = Vec::new();
    let tops = children.get(&None).cloned().unwrap_or_default();
    for &top in tops.iter().rev() {
        let mut worklist = vec![top];
        let mut at = 0;
        while at < worklist.len() {
            let own = children.get(&Some(worklist[at])).cloned().unwrap_or_default();
            worklist.extend(own);
            at += 1;
        }
        order.extend(worklist.into_iter().rev());
    }
    order
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
    if function.entry().is_none() || !shape.loops.iter().any(|loop_| _needs_grouping(function, loop_)) {
        return false;
    }
    let mut changed = false;
    let headers = visit_order(function, &shape.loops);
    let at: HashMap<i64, usize> = shape.loops.iter().enumerate().map(|(index, loop_)| (loop_.header, index)).collect();
    // The loops each block is in, kept through the bridges: a bridge touches
    // only the loops around its target.
    let mut around: HashMap<i64, Vec<usize>> = HashMap::default();
    for (index, loop_) in shape.loops.iter().enumerate() {
        for &block in &loop_.body {
            around.entry(block).or_default().push(index);
        }
    }
    for header in headers {
        let original = shape.loops[at[&header]].clone();
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
        // The order LLVM visits the header's predecessors in places the new
        // blocks, so it is taken before any is grouped.
        let header = cfg::block(original.header);
        let (entries, latches): (Vec<BlockId>, Vec<BlockId>) =
            function.predecessors(header).into_iter().partition(|&one| !original.body.contains(&cfg::id(one)));
        let mut bridges: Vec<(i64, i64, BTreeSet<i64>)> = Vec::new();
        for (target, sources) in plan {
            let Some(bridge) = grouped(function, target, &sources) else {
                unreachable!("groupable said it could be made");
            };
            if target == original.header {
                if sources.contains(&cfg::id(entries[0])) {
                    place_carefully(function, bridge, &entries, &original.body);
                } else if let Some(&last) = latches.last() {
                    // `insertUniqueBackedgeBlock`: right after the last
                    // backedge block.
                    function.move_block_after(bridge, last).expect("placed blocks");
                }
            }
            bridges.push((cfg::id(bridge), target, sources));
        }
        changed = true;
        for (bridge, target, sources) in &bridges {
            for index in around.get(target).cloned().unwrap_or_default() {
                BRIDGE_PROBES.with(|one| one.set(one.get() + 1));
                if _bridged(&mut shape.loops[index], *bridge, *target, sources) {
                    around.entry(*bridge).or_default().push(index);
                }
            }
        }
        if cfg!(test) || llrm_support::env_flag!("LLRM_CHECK_SHAPE") {
            _agrees(&shape, function);
        }
    }
    changed
}

/// What `loop_` is once `bridge` stands on the edges from `sources` to
/// `target`: the block joins it where `target` and a source are in it (it
/// reaches the latch through `target`, and some of what reaches it came from
/// inside), and becomes its only latch where `target` is its header.
fn _bridged(
    loop_: &mut loops::Loop,
    bridge: i64,
    target: i64,
    sources: &BTreeSet<i64>,
) -> bool {
    if !loop_.body.contains(&target) || !sources.iter().any(|at| loop_.body.contains(at)) {
        return false;
    }
    loop_.body.insert(bridge);
    if loop_.header == target {
        loop_.latches = BTreeSet::from([bridge]);
    }
    true
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
