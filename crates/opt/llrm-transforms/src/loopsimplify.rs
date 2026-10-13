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
//! The loops are processed one at a time on a copy of the function, kept only
//! when every grouping the loop needed went through, so a refused one leaves
//! the body as it was. A loop already in the form makes no copy. The loops of
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

/// The edges from `sources` into `target` routed through one new block,
/// which `target`'s phis read; the new block, or `None`, changing nothing,
/// where a source's terminator or a phi cannot be regrouped.
pub fn grouped(
    function: &mut Function,
    target: i64,
    sources: &BTreeSet<i64>,
) -> Option<BlockId> {
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let entry = cfg::id(function.entry()?);
    let reaching = predecessors.get(&target)?;
    if sources.is_empty() || !sources.is_subset(reaching) || target == entry {
        return None;
    }
    let destination = cfg::block(target);
    for &parent in sources {
        let last = function.terminator(cfg::block(parent))?;
        match function.instruction(last).operands.len() {
            _ if function.instruction(last).opcode != Opcode::Br => return None,
            3 if !edges::conditional(function, cfg::block(parent), destination) => return None,
            1 if function.successors(cfg::block(parent)) != [destination] => return None,
            _ => {}
        }
    }
    let phis = edges::phis(function, destination);
    let incoming = |function: &Function, phi| {
        function.instruction(phi).operands.chunks(2).map(|pair| (pair[0], pair[1])).collect::<Vec<_>>()
    };
    let at = |block: Operand| match block {
        Operand::Block(block) => cfg::id(block),
        _ => unreachable!("a phi's block operand"),
    };
    if phis
        .iter()
        .any(|&phi| incoming(function, phi).iter().map(|&(_, block)| at(block)).collect::<BTreeSet<_>>() != *reaching)
    {
        return None;
    }
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

thread_local! {
    static COPIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many copies of a function this thread has made to try grouping a loop
/// on, for a test that a loop already in the form makes none.
pub fn copies() -> usize {
    COPIES.with(std::cell::Cell::get)
}

/// Whether `loop_` has a block to group in `function`, whose graph and
/// predecessors are given: its entries from outside are not one block that only
/// enters it, it has more than one latch, or an exit is reached from outside
/// the loop too. What the rest of `_simplified` finds on a copy, found without
/// one.
fn _needs_grouping(
    function: &Function,
    graph: &[cfg::Block],
    predecessors: &std::collections::BTreeMap<i64, BTreeSet<i64>>,
    loop_: &loops::Loop,
) -> bool {
    let Some(entering) = predecessors.get(&loop_.header) else { return false };
    let outside = entering.difference(&loop_.body).copied().collect::<BTreeSet<_>>();
    let Some(&parent) = outside.first() else { return false };
    if outside.len() != 1
        || function.successors(cfg::block(parent)) != [cfg::block(loop_.header)]
        || loop_.latches.len() != 1
    {
        return true;
    }
    let empty = BTreeSet::new();
    graph
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .flat_map(|block| block.succ.iter().copied())
        .filter(|at| !loop_.body.contains(at))
        .any(|target| !predecessors.get(&target).unwrap_or(&empty).is_subset(&loop_.body))
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
    let mut changed = false;
    let headers = shape.loops.iter().map(|loop_| loop_.header).collect::<Vec<_>>();
    // The function's graph, which only a loop that changes it makes stale.
    let mut whole = cfg::graph(function);
    let mut entered = loops::predecessors(&whole);
    for header in headers {
        let original = shape.loops.iter().find(|loop_| loop_.header == header).expect("StopIteration").clone();
        // A loop that is already in the form has nothing to group: neither it
        // nor the function is copied.
        if !_needs_grouping(function, &whole, &entered, &original) {
            continue;
        }
        COPIES.with(|copies| copies.set(copies.get() + 1));
        let mut candidate = function.clone();
        let mut grouping = false;
        // The blocks `grouped` makes, as (block, target, sources): what the
        // loops of the shape gain from them, without a new shape of the
        // function.
        let mut bridges: Vec<(i64, i64, BTreeSet<i64>)> = Vec::new();
        let predecessors = loops::predecessors(&cfg::graph(&candidate));
        let outside = predecessors[&original.header].difference(&original.body).copied().collect::<BTreeSet<_>>();
        let Some(&parent) = outside.first() else { continue };
        if outside.len() != 1 || candidate.successors(cfg::block(parent)) != [cfg::block(original.header)] {
            let Some(bridge) = grouped(&mut candidate, original.header, &outside) else { continue };
            bridges.push((cfg::id(bridge), original.header, outside));
            grouping = true;
        }
        if original.latches.len() != 1 {
            let Some(bridge) = grouped(&mut candidate, original.header, &original.latches) else { continue };
            bridges.push((cfg::id(bridge), original.header, original.latches.clone()));
            grouping = true;
        }
        let graph = cfg::graph(&candidate);
        let mut current = original.clone();
        for (bridge, target, sources) in &bridges {
            _bridged(&mut current, *bridge, *target, sources);
        }
        let predecessors = loops::predecessors(&graph);
        let exits = graph
            .iter()
            .filter(|block| current.body.contains(&block.at))
            .flat_map(|block| block.succ.iter().copied())
            .filter(|at| !current.body.contains(at))
            .collect::<BTreeSet<_>>();
        let empty = BTreeSet::new();
        let mut broke = false;
        for target in exits {
            let reaching = predecessors.get(&target).unwrap_or(&empty);
            let sources = reaching.intersection(&current.body).copied().collect::<BTreeSet<_>>();
            if !reaching.is_subset(&current.body) {
                let Some(bridge) = grouped(&mut candidate, target, &sources) else {
                    broke = true;
                    break;
                };
                bridges.push((cfg::id(bridge), target, sources));
                grouping = true;
            }
        }
        if !broke && grouping {
            *function = candidate;
            changed = true;
            for loop_ in &mut shape.loops {
                for (bridge, target, sources) in &bridges {
                    _bridged(loop_, *bridge, *target, sources);
                }
            }
            if cfg!(test) || llrm_support::env_set("LLRM_CHECK_SHAPE") {
                _agrees(&shape, function);
            }
            whole = cfg::graph(function);
            entered = loops::predecessors(&whole);
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
    if !loop_.body.contains(&target) || !sources.is_subset(&loop_.body) {
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
