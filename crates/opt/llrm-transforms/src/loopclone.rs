//! Clones loop iterations as CFG, for the passes that peel, unswitch or jam a
//! loop: `peeled` copies whole iterations ahead of a loop, `chained` copies
//! bodies inside it, keeping the loop and every exit.
//!
//! # How it works
//! This is the mechanism for peeling and unswitching; it makes
//! no profitability decision. Both functions return each copy's map from an
//! original block to its clone, or `None` having changed nothing when the loop
//! is not one they can clone. Both need
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
//! `copied(function, loop, count)` makes `count` whole loops of it, each
//! running every trip, one after another ahead of the original: the copy loop
//! distribution and loop versioning start from.
//!
//! `chained(function, loop, copies)` is the partial unroll LLVM's `UnrollLoop`
//! makes when no iteration is left over. The loop must leave only from its
//! header, by a conditional branch. The copies hold no header test: the
//! original latch runs on into the first copy's header work, each copy's latch
//! into the next, and the last into the header. That is right only where the
//! trip count is a multiple of `copies + 1`, which the caller proves.
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
//! Called by [`crate::peel`] (`peeled`); not a pass.
//!
//! # Needs
//! LCSSA ([`crate::lcssa`]) and a simplified loop ([`crate::loopsimplify`]).
//!
//! # See also
//! LLVM's `peelLoop` (`LoopPeel.cpp`) and `UnrollLoop`.

use std::collections::BTreeMap;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::{self, Loop};
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{Flags, Opcode};

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

/// One copy of a loop `copied` or `versioned` made: its blocks and
/// instructions by their originals'.
#[derive(Clone, Debug)]
pub struct LoopCopy {
    pub blocks: BTreeMap<i64, BlockId>,
    pub instructions: IdMap<InstId, InstId>,
}

/// The loop's blocks cloned, unwired: the copy, and each value the loop
/// defines by its clone.
fn cloned(
    function: &mut Function,
    originals: &[i64],
) -> Result<(LoopCopy, IdMap<ValueId, ValueId>), String> {
    let blocks = originals.iter().map(|&at| (at, function.create_block(None))).collect::<BTreeMap<_, _>>();
    let mut instructions = IdMap::new();
    let mut renamed = IdMap::new();
    for &at in originals {
        function.insert_block(blocks[&at], None)?;
        for inst in function.iter_block(cfg::block(at)).collect::<Vec<_>>() {
            let copy = function.clone_instruction(inst);
            function.insert(copy, Position::End(blocks[&at]))?;
            if let (Some(original), Some(result)) =
                (function.instruction(inst).result, function.instruction(copy).result)
            {
                renamed.insert(original, result);
            }
            instructions.insert(inst, copy);
        }
    }
    Ok((LoopCopy { blocks, instructions }, renamed))
}

/// `copy`'s instructions made to read each other: a value of the loop is its
/// clone, a block of it its clone, and the header's start comes from `start`.
/// What leaves the loop goes to `outside`, or where it went where that is
/// `None`.
fn wired(
    function: &mut Function,
    loop_: &Loop,
    entry: i64,
    copy: &LoopCopy,
    values: &IdMap<ValueId, ValueId>,
    start: BlockId,
    outside: Option<BlockId>,
) {
    let renamed = |operand: Operand| match operand {
        Operand::Value(one) => Operand::Value(values.get(&one).copied().unwrap_or(one)),
        other => other,
    };
    for (original, &cloned) in copy.instructions.iter() {
        let at = cfg::id(function.parent(original).expect("a placed instruction"));
        if function.instruction(original).opcode == Opcode::Phi {
            let incoming = arms(function, original)
                .into_iter()
                .filter(|&(_, source)| at != loop_.header || cfg::id(source) != entry)
                .map(|(value, source)| (renamed(value), copy.blocks[&cfg::id(source)]))
                .chain((at == loop_.header).then(|| {
                    let from = arms(function, original).into_iter().find(|&(_, source)| cfg::id(source) == entry);
                    (from.expect("a value from the entry").0, start)
                }))
                .collect::<Vec<_>>();
            function.set_operands(cloned, from_arms(&incoming));
            continue;
        }
        let operands = function
            .instruction(original)
            .operands
            .iter()
            .map(|&operand| match operand {
                Operand::Block(target) => Operand::Block(match (copy.blocks.get(&cfg::id(target)), outside) {
                    (Some(&inside), _) => inside,
                    (None, Some(outside)) => outside,
                    (None, None) => target,
                }),
                other => renamed(other),
            })
            .collect();
        function.set_operands(cloned, operands);
    }
}

/// `count` whole copies of `loop_`, each a loop of its own that runs every
/// trip, in order ahead of `loop_`, which runs last: each copy's one exit
/// enters the next, through a block of its own. `None`, changing nothing,
/// where the loop is not one `peeled` can clone or has other than one exit.
/// The copies' values are theirs: what the loop defines is not read outside it
/// (LCSSA, as `peeled` needs), so no exit phi gains an arm.
pub fn copied(
    function: &mut Function,
    loop_: &Loop,
    count: usize,
) -> Result<Option<Vec<LoopCopy>>, String> {
    if count < 1 {
        return Err("copying needs a positive number of copies".to_owned());
    }
    let Some(Plan { graph, entry, originals, .. }) = plan(function, loop_, true) else { return Ok(None) };
    if exits(&graph, loop_) != 1 {
        return Ok(None);
    }
    let header = cfg::block(loop_.header);
    // Where each copy's exit goes: a block that enters the next copy, the last
    // one the loop itself.
    let joints = (0..count).map(|_| function.create_block(None)).collect::<Vec<_>>();
    for &joint in &joints {
        function.insert_block(joint, None)?;
    }
    let mut copies = Vec::new();
    for (k, &joint) in joints.iter().enumerate() {
        let (copy, values) = cloned(function, &originals)?;
        let start = if k == 0 { cfg::block(entry) } else { joints[k - 1] };
        wired(function, loop_, entry, &copy, &values, start, Some(joint));
        copies.push(copy);
    }
    let void = function.instruction(function.terminator(cfg::block(entry)).expect("a terminated entry")).ty;
    for (k, &joint) in joints.iter().enumerate() {
        let next = if k + 1 < count { copies[k + 1].blocks[&loop_.header] } else { header };
        let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(next)], Flags::default(), None);
        function.insert(jump, Position::End(joint))?;
    }
    let jump = function.terminator(cfg::block(entry)).expect("a terminated block");
    edges::retarget(function, jump, header, copies[0].blocks[&loop_.header]);
    for phi in edges::phis(function, header) {
        let incoming = arms(function, phi)
            .into_iter()
            .map(|(value, source)| (value, if cfg::id(source) == entry { joints[count - 1] } else { source }))
            .collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&incoming));
    }
    Ok(Some(copies))
}

/// How many edges leave `loop_`.
fn exits(
    graph: &[cfg::Block],
    loop_: &Loop,
) -> usize {
    graph
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .flat_map(|block| block.succ.iter().filter(|to| !loop_.body.contains(to)))
        .count()
}

/// A second copy of `loop_` that runs where `guard` holds: the entry tests the
/// condition `guard` makes there (before the entry's own branch, which it is
/// given) and goes to the copy or to the loop. The copy leaves to the loop's
/// exit, whose phis take its values. The copy, and the entry's branch, which
/// the caller may weigh. `None`, changing nothing, where the loop is not one
/// `copied` can copy. LLVM's `LoopVersioning::versionLoop`.
pub fn versioned(
    function: &mut Function,
    loop_: &Loop,
    guard: &mut dyn FnMut(&mut Function, InstId) -> Result<Operand, String>,
) -> Result<Option<(LoopCopy, InstId)>, String> {
    let Some(Plan { graph, entry, originals, .. }) = plan(function, loop_, true) else { return Ok(None) };
    if exits(&graph, loop_) != 1 {
        return Ok(None);
    }
    let header = cfg::block(loop_.header);
    let (copy, values) = cloned(function, &originals)?;
    wired(function, loop_, entry, &copy, &values, cfg::block(entry), None);
    // The exit's phis take the copy's values from the copy's blocks.
    for block in graph.iter().filter(|block| !loop_.body.contains(&block.at)) {
        for phi in edges::phis(function, cfg::block(block.at)) {
            let mut incoming = arms(function, phi);
            let added = incoming
                .iter()
                .filter(|(_, source)| loop_.body.contains(&cfg::id(*source)))
                .map(|&(value, source)| {
                    let value = match value {
                        Operand::Value(one) => Operand::Value(values.get(&one).copied().unwrap_or(one)),
                        other => other,
                    };
                    (value, copy.blocks[&cfg::id(source)])
                })
                .collect::<Vec<_>>();
            if !added.is_empty() {
                incoming.extend(added);
                function.set_operands(phi, from_arms(&incoming));
            }
        }
    }
    let jump = function.terminator(cfg::block(entry)).expect("a terminated entry");
    let condition = guard(function, jump)?;
    let dispatch = function.create_instruction(
        Opcode::Br,
        function.instruction(jump).ty,
        vec![condition, Operand::Block(copy.blocks[&loop_.header]), Operand::Block(header)],
        Flags::default(),
        None,
    );
    function.insert(dispatch, Position::Before(jump))?;
    function.erase(jump)?;
    Ok(Some((copy, dispatch)))
}

/// `loop_` with `copies` more bodies chained in it: the latch of each runs on
/// into the header's work of the next, whose test is not made, and the last
/// into the header. Right where the loop leaves only from its header and its
/// trip count is a multiple of `copies + 1`: the test of a later copy would
/// always continue. Each copy's map from an original block to its clone, the
/// first copy's first; `None`, changing nothing, where the loop is not one
/// this can chain. The loop's partial unrolling, as LLVM's `UnrollLoop` makes
/// it when no iteration is left over.
pub fn chained(
    function: &mut Function,
    loop_: &Loop,
    copies: usize,
) -> Result<Option<Vec<BTreeMap<i64, BlockId>>>, String> {
    if copies < 1 {
        return Err("chaining needs a positive number of copies".to_owned());
    }
    let graph = cfg::graph(function);
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(&graph);
    if Some(loop_.header) == function.entry().map(cfg::id) || loop_.latches.len() != 1 {
        return Ok(None);
    }
    let outside = predecessors[&loop_.header].iter().copied().filter(|at| !loop_.body.contains(at)).collect::<Vec<_>>();
    let [entry] = outside[..] else { return Ok(None) };
    let latch = *loop_.latches.first().expect("one latch");
    if blocks[&entry].succ != [loop_.header] || blocks[&latch].succ != [loop_.header] || latch == loop_.header {
        return Ok(None);
    }
    if loop_.body.iter().any(|at| *at != loop_.header && !predecessors[at].iter().all(|at| loop_.body.contains(at))) {
        return Ok(None);
    }
    // The header alone leaves, by a conditional branch.
    let leaving = loop_.body.iter().filter(|at| blocks[at].succ.iter().any(|to| !loop_.body.contains(to))).count();
    let header_branch = function
        .terminator(cfg::block(loop_.header))
        .filter(
            |&last| {
                let op = function.instruction(last);
                op.opcode == Opcode::Br && op.operands.len() == 3
            },
        );
    let (1, Some(header_branch)) = (leaving, header_branch) else { return Ok(None) };
    if !blocks[&loop_.header].succ.iter().any(|to| loop_.body.contains(to)) {
        return Ok(None);
    }
    let originals =
        graph.iter().filter(|block| loop_.body.contains(&block.at)).map(|block| block.at).collect::<Vec<_>>();
    if originals.iter().any(|&at| {
        blocks[&at].succ.len() > 1
            && function
                .terminator(cfg::block(at))
                .is_none_or(|last| !matches!(function.instruction(last).opcode, Opcode::Br | Opcode::Switch))
    }) {
        return Ok(None);
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
            return Ok(None);
        }
        if edges::phis(function, at).into_iter().any(|phi| {
            arms(function, phi)
                .into_iter()
                .any(
                    |(value, source)| matches!(value, Operand::Value(value) if defined.contains_key(&value))
                        && !loop_.body.contains(&cfg::id(source)),
                )
        }) {
            return Ok(None);
        }
    }
    let header_phis = edges::phis(function, cfg::block(loop_.header));
    let inside_target =
        blocks[&loop_.header].succ.iter().copied().find(|to| loop_.body.contains(to)).expect("a successor in the loop");

    let mut labels: Vec<BTreeMap<i64, BlockId>> = Vec::new();
    let mut copied: Vec<IdMap<ValueId, ValueId>> = Vec::new();
    let mut cloned = Vec::new();
    for copy in 0..copies {
        let map = originals.iter().map(|&at| (at, function.create_block(None))).collect::<BTreeMap<_, _>>();
        let mut values = IdMap::new();
        for &at in &originals {
            function.insert_block(map[&at], None)?;
            for inst in function.iter_block(cfg::block(at)).collect::<Vec<_>>() {
                // A later header's phis take the value the copy before flows
                // in; its branch is the way into the body.
                if at == loop_.header && function.instruction(inst).opcode == Opcode::Phi {
                    continue;
                }
                let made = function.clone_instruction(inst);
                function.insert(made, Position::End(map[&at]))?;
                if let (Some(original), Some(result)) =
                    (function.instruction(inst).result, function.instruction(made).result)
                {
                    values.insert(original, result);
                }
                cloned.push((copy, at, inst, made));
            }
        }
        labels.push(map);
        copied.push(values);
    }

    // What an original value is in copy `copy`: copy 0 is the original.
    let mut latch_values = Vec::new();
    for &phi in &header_phis {
        let arm = arms(function, phi)
            .into_iter()
            .find(|&(_, source)| cfg::id(source) == latch)
            .map(|(value, _)| value)
            .ok_or("KeyError")?;
        latch_values.push((function.instruction(phi).result.expect("a phi's value"), arm));
    }
    // `values[k]` reads copy k + 1 (k + 1 of 1..=copies); the header phis of
    // copy k + 1 are what copy k's latch supplied.
    let mut phi_values: Vec<IdMap<ValueId, Operand>> = vec![IdMap::new(); copies];
    let value = |phi_values: &[IdMap<ValueId, Operand>], original: Operand, copy: usize| match original {
        Operand::Value(one) => match phi_values[copy].get(&one) {
            Some(&supplied) => supplied,
            None => Operand::Value(copied[copy].get(&one).copied().unwrap_or(one)),
        },
        other => other,
    };
    // Copy c (0-based among the clones) is iteration c + 1: its header phis are
    // the latch's values of iteration c, itself a clone but for the first.
    for copy in 0..copies {
        for &(phi, arm) in &latch_values {
            let supplied = if copy == 0 { arm } else { value(&phi_values, arm, copy - 1) };
            phi_values[copy].insert(phi, supplied);
        }
    }
    let destination = |at: i64, source: i64, copy: usize| {
        if source == latch && at == loop_.header {
            return if copy + 1 < copies { labels[copy + 1][&at] } else { cfg::block(at) };
        }
        labels[copy].get(&at).copied().unwrap_or(cfg::block(at))
    };

    for &(copy, at, inst, made) in &cloned {
        let op = function.instruction(inst).clone();
        let operands = if op.opcode == Opcode::Phi {
            from_arms(
                &arms(function, inst)
                    .into_iter()
                    .map(|(incoming, source)| (value(&phi_values, incoming, copy), labels[copy][&cfg::id(source)]))
                    .collect::<Vec<_>>(),
            )
        } else if at == loop_.header && inst == header_branch {
            vec![Operand::Block(labels[copy][&inside_target])]
        } else {
            op.operands
                .iter()
                .map(|&operand| match operand {
                    Operand::Block(target) => Operand::Block(destination(cfg::id(target), at, copy)),
                    other => value(&phi_values, other, copy),
                })
                .collect()
        };
        function.set_operands(made, operands);
    }

    // The original latch runs on into the first copy; the header's phis take
    // what the last copy's latch supplies.
    let jump = function.terminator(cfg::block(latch)).expect("a terminated latch");
    edges::retarget(function, jump, cfg::block(loop_.header), labels[0][&loop_.header]);
    for &phi in &header_phis {
        let original = function.instruction(phi).result.expect("a phi's value");
        let supplied = value(&phi_values, latch_values.iter().find(|(one, _)| *one == original).unwrap().1, copies - 1);
        let mut incoming = arms(function, phi);
        incoming.retain(|&(_, source)| cfg::id(source) != latch);
        incoming.push((supplied, labels[copies - 1][&latch]));
        function.set_operands(phi, from_arms(&incoming));
    }
    Ok(Some(labels))
}

#[cfg(test)]
#[path = "loopclone_tests.rs"]
mod tests;
