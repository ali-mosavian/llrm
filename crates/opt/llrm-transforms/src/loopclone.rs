//! Clones loop iterations as CFG, for the passes that peel a loop: `peeled`
//! copies whole iterations ahead of it.
//!
//! # How it works
//! This is the mechanism for peeling; it makes no profitability decision. It
//! returns each copy's map from an original block to its clone, or `None`
//! having changed nothing when the loop is not one it can clone. It needs
//! - a header that is not the entry, one latch that ends in a jump to it, and
//!   one outside predecessor that falls into the header (a preheader);
//! - no block but the header entered from outside the loop;
//! - only `br` and `switch` as multi-way terminators in the body;
//! - loop live-outs in LCSSA: no instruction outside the loop reads a value the
//!   loop defines, and no phi outside takes one from outside the loop.
//!
//! `peeled(function, loop, count)` clones the loop's blocks `count` times, in
//! order, and puts them between the preheader and the loop:
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   pre["<b>pre</b>"]:::mid
//!   h["<b>H</b>"]:::mid
//!   body["<b>body</b>"]:::mid
//!   ex["<b>exit</b>"]:::mid
//!   pre --> h
//!   h --> body
//!   body --> h
//!   h --> ex
//! ```
//!
//! *Before*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   pre["<b>pre</b>"]:::mid
//!   h1["<b>H1</b>"]:::mid
//!   b1["<b>body1</b>"]:::mid
//!   h2["<b>H2</b>"]:::mid
//!   b2["<b>body2</b>"]:::mid
//!   h["<b>H</b>"]:::mid
//!   body["<b>body</b>"]:::mid
//!   ex["<b>exit</b>"]:::mid
//!   pre --> h1
//!   h1 --> b1
//!   b1 --> h2
//!   h2 --> b2
//!   b2 --> h
//!   h --> body
//!   body --> h
//!   h1 --> ex
//!   h2 --> ex
//!   h --> ex
//! ```
//!
//! *After peeled(.., 2)*
//!
//! The first copy's header phis take the preheader's arm, each later copy's
//! the previous copy's latch value, and the residual loop's header the last
//! copy's latch value. Every exit phi gains an arm for each copy's exit edge.
//! The residual loop stays so the result is right whatever the trip count;
//! the caller proves it unreachable where the count is exact.
//!
//! # Example
//! `peeled(.., 1)` on a loop whose body is a diamond with an early exit.
//! Before:
//!
//! ```mir
//! define i16 @f(i16 %seed, i16 %n, i16 %limit) {
//! b0:
//!   br label %b1
//!
//! b1:
//!   %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b5 ]
//!   %go = icmp ult i16 %carried, %n
//!   br i1 %go, label %b2, label %b6
//! ; ... b2 to b5: the diamond, leaving from b3 to b6 or going on to b5
//! b6:
//!   %answer = phi i16 [ %carried, %b1 ], [ %left, %b3 ]
//!   ret i16 %answer
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define i16 @f(i16 %seed, i16 %n, i16 %limit) {
//! b0:
//!   br label %0
//!
//! b1:
//!   %carried = phi i16 [ %stepped, %b5 ], [ %13, %11 ]
//!   ; ...
//! b6:
//!   %answer = phi i16 [ %carried, %b1 ], [ %left, %b3 ], [ %1, %0 ], [ %7, %6 ]
//!   ret i16 %answer
//!
//! 0:
//!   %1 = phi i16 [ %seed, %b0 ]
//!   %2 = icmp ult i16 %1, %n
//!   br i1 %2, label %3, label %b6
//! ; ... 3 to 9: the copy of b2 to b4
//! 11:
//!   %12 = phi i16 [ %7, %6 ], [ %10, %9 ]
//!   %13 = add i16 %12, 1
//!   br label %b1
//! }
//! ```
//!
//! `b0` enters the copy `0`, whose last block `11` enters the residual loop at
//! `b1`; `b6` gains an arm for each of the copy's two exits.
//!
//! From the fixture of unit test
//! `test_peeling_clones_diamond_and_early_exit_phis` in `loopclone_tests.rs`,
//! peeled once where the test peels twice.
//!
//! # Runs
//! Called by [`crate::loopunrollpass`] (`peeled_any_latch`); not a pass.
//!
//! # Needs
//! LCSSA ([`crate::lcssa`]) and a simplified loop ([`crate::loopsimplify`]).
//!
//! # See also
//! LLVM's `peelLoop` (`LoopPeel.cpp`) and `UnrollLoop`.

use std::collections::BTreeMap;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::{self, Loop};
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;

use crate::edges;
use crate::lcssa::{arms, definitions, from_arms, operations};

/// What cloning a loop starts from: the graph, the one block that enters the
/// loop, its latch and its blocks in layout order.
struct Plan {
    graph: Vec<cfg::Block>,
    entry: i64,
    latch: i64,
    originals: Vec<i64>,
}

/// `loop_`'s `Plan`, or `None` where it is not a loop these functions clone;
/// `leaves_from_latch` lets its latch also leave it, as a loop tested after
/// each trip does.
fn plan(
    function: &Function,
    loop_: &Loop,
    leaves_from_latch: bool,
) -> Option<Plan> {
    let graph = cfg::graph(function);
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(&graph);
    if Some(loop_.header) == function.entry().map(cfg::id) || loop_.latches.len() != 1 {
        return None;
    }
    let outside = predecessors[&loop_.header].iter().copied().filter(|at| !loop_.body.contains(at)).collect::<Vec<_>>();
    if outside.len() != 1 {
        return None;
    }
    let entry = outside[0];
    let latch = *loop_.latches.first().expect("one latch");
    let back =
        blocks[&latch].succ.contains(&loop_.header) && (leaves_from_latch || blocks[&latch].succ == [loop_.header]);
    if blocks[&entry].succ != [loop_.header] || !back {
        return None;
    }
    if loop_.body.iter().any(|at| *at != loop_.header && !predecessors[at].iter().all(|at| loop_.body.contains(at))) {
        return None;
    }
    let originals =
        graph.iter().filter(|block| loop_.body.contains(&block.at)).map(|block| block.at).collect::<Vec<_>>();
    if originals.iter().any(|&at| {
        blocks[&at].succ.len() > 1
            && function
                .terminator(cfg::block(at))
                .is_none_or(|last| !matches!(function.instruction(last).opcode, Opcode::Br | Opcode::Switch))
    }) {
        return None;
    }
    let defined = definitions(function, &loop_.body);
    for block in graph.iter().filter(|block| !loop_.body.contains(&block.at)) {
        let at = cfg::block(block.at);
        let reads = |inst: InstId| {
            function
                .instruction(inst)
                .operands
                .iter()
                .any(|operand| matches!(operand, Operand::Value(value) if defined.contains_key(value)))
        };
        if operations(function, at).into_iter().any(reads) {
            return None;
        }
        if edges::phis(function, at).into_iter().any(|phi| {
            arms(function, phi)
                .into_iter()
                .any(
                    |(value, source)| matches!(value, Operand::Value(value) if defined.contains_key(&value))
                        && !loop_.body.contains(&cfg::id(source)),
                )
        }) {
            return None;
        }
    }
    Some(Plan { graph, entry, latch, originals })
}

/// `count` iterations of `loop_` cloned ahead of it, or `None`, changing
/// nothing, where the loop is not one this can clone. Each iteration's map
/// from an original block to its copy.
pub fn peeled(
    function: &mut Function,
    loop_: &Loop,
    count: i64,
) -> Result<Option<Vec<BTreeMap<i64, BlockId>>>, String> {
    peeled_from(function, loop_, count, false)
}

/// `peeled` for a loop that may also leave from its latch, as `peelLoop` takes
/// any loop in simplified form.
pub fn peeled_any_latch(
    function: &mut Function,
    loop_: &Loop,
    count: i64,
) -> Result<Option<Vec<BTreeMap<i64, BlockId>>>, String> {
    peeled_from(function, loop_, count, true)
}

fn peeled_from(
    function: &mut Function,
    loop_: &Loop,
    count: i64,
    leaves_from_latch: bool,
) -> Result<Option<Vec<BTreeMap<i64, BlockId>>>, String> {
    if count < 1 {
        return Err("peeling needs a positive iteration count".to_owned());
    }
    let count = count as usize;
    let Some(Plan { graph, entry, latch, originals }) = plan(function, loop_, leaves_from_latch) else {
        return Ok(None);
    };
    let mut labels: Vec<BTreeMap<i64, BlockId>> = Vec::new();
    let mut copies: Vec<BTreeMap<ValueId, ValueId>> = Vec::new();
    let mut cloned = Vec::new();
    for iteration in 0..count {
        let map = originals.iter().map(|&at| (at, function.create_block(None))).collect::<BTreeMap<_, _>>();
        let mut values = BTreeMap::new();
        for &at in &originals {
            function.insert_block(map[&at], None)?;
            for inst in function.iter_block(cfg::block(at)).collect::<Vec<_>>() {
                let copy = function.clone_instruction(inst);
                function.insert(copy, Position::End(map[&at]))?;
                if let (Some(original), Some(result)) =
                    (function.instruction(inst).result, function.instruction(copy).result)
                {
                    values.insert(original, result);
                }
                cloned.push((iteration, at, inst, copy));
            }
        }
        labels.push(map);
        copies.push(values);
    }

    let value = |original: Operand, iteration: usize| match original {
        Operand::Value(one) => Operand::Value(copies[iteration].get(&one).copied().unwrap_or(one)),
        other => other,
    };
    let destination = |at: i64, source: i64, iteration: usize| {
        if source == latch && at == loop_.header {
            return if iteration + 1 < count { labels[iteration + 1][&at] } else { cfg::block(at) };
        }
        labels[iteration].get(&at).copied().unwrap_or(cfg::block(at))
    };
    let from = |function: &Function, phi: InstId, source: i64| {
        arms(function, phi)
            .into_iter()
            .find(|&(_, block)| cfg::id(block) == source)
            .map(|(value, _)| value)
            .ok_or("KeyError")
    };

    for &(iteration, at, inst, copy) in &cloned {
        let operands = if function.instruction(inst).opcode == Opcode::Phi {
            let incoming = if at == loop_.header {
                if iteration == 0 {
                    vec![(from(function, inst, entry)?, cfg::block(entry))]
                } else {
                    vec![(value(from(function, inst, latch)?, iteration - 1), labels[iteration - 1][&latch])]
                }
            } else {
                arms(function, inst)
                    .into_iter()
                    .map(|(incoming, source)| (value(incoming, iteration), labels[iteration][&cfg::id(source)]))
                    .collect()
            };
            from_arms(&incoming)
        } else {
            function
                .instruction(inst)
                .operands
                .iter()
                .map(|&operand| match operand {
                    Operand::Block(target) => Operand::Block(destination(cfg::id(target), at, iteration)),
                    other => value(other, iteration),
                })
                .collect()
        };
        function.set_operands(copy, operands);
    }

    let first = labels[0][&loop_.header];
    let jump = function.terminator(cfg::block(entry)).expect("a terminated block");
    edges::retarget(function, jump, cfg::block(loop_.header), first);
    for block in graph.iter().map(|block| block.at) {
        for phi in edges::phis(function, cfg::block(block)) {
            let mut incoming = arms(function, phi);
            if block == loop_.header {
                let residual = from(function, phi, latch)?;
                incoming.retain(|&(_, source)| cfg::id(source) != entry);
                incoming.push((value(residual, count - 1), labels[count - 1][&latch]));
            } else if !loop_.body.contains(&block) {
                for (original, source) in arms(function, phi) {
                    if loop_.body.contains(&cfg::id(source)) {
                        for (iteration, label) in labels.iter().enumerate() {
                            incoming.push((value(original, iteration), label[&cfg::id(source)]));
                        }
                    }
                }
            } else {
                continue;
            }
            function.set_operands(phi, from_arms(&incoming));
        }
    }
    Ok(Some(labels))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/CloneFunction.cpp#L1020
/// `cloneLoopWithPreheader(Before, .., OrigLoop, VMap, NameSuffix, ..)`: the
/// loop's preheader and blocks cloned, named with `suffix`, and placed before
/// `before`; the preheader's clone first, then the blocks in the loop's order.
/// `vmap` maps each original block and result to its clone, and the copies
/// still read the originals' values until `remap_instruction` runs over them.
/// The loop info and the dominator tree are not updated (DEVIATE 30): the
/// caller finds them again.
pub fn clone_loop_with_preheader(
    function: &mut Function,
    before: BlockId,
    orig_loop: llrm_mir::loopinfo::Loop,
    suffix: &str,
    vmap: &mut ValueMap,
) -> Vec<BlockId> {
    let orig_ph = orig_loop.get_loop_preheader(&*function).expect("No preheader");
    let mut blocks = Vec::new();
    for orig in std::iter::once(orig_ph).chain(orig_loop.get_blocks().iter().copied()) {
        let new = function.clone_block(orig, vmap);
        if let Some(name) = function.block(orig).name.clone() {
            function.set_block_name(new, &format!("{name}{suffix}"));
        }
        for (from, to) in
            function.iter_block(orig).collect::<Vec<_>>().into_iter().zip(function.iter_block(new).collect::<Vec<_>>())
        {
            if let (Some(from), Some(to)) = (function.instruction(from).result, function.instruction(to).result)
                && let Some(name) = function.value(from).name.clone()
            {
                function.set_name(to, &format!("{name}{suffix}"));
            }
        }
        function.insert_block_before(new, before).expect("a new block");
        blocks.push(new);
    }
    blocks
}

#[cfg(test)]
#[path = "loopclone_tests.rs"]
mod tests;
