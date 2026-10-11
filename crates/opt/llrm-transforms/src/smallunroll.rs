//! gcc's small-loop unroll at -O2 and up: an innermost loop of a few
//! instructions runs two bodies a trip.
//!
//! The x86 backend turns `-funroll-loops` on with `-munroll-only-small-loops`
//! ([`i386-common.cc`
//! 1829-1832](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/common/config/i386/i386-common.cc#L1829-L1832)),
//! and `ix86_loop_unroll_adjust` ([`i386.cc`
//! 24019-24034](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/config/i386/i386.cc#L24019-L24034))
//! lets `loop2_unroll` unroll a loop of at most `small_unroll_ninsns`
//! instructions by `small_unroll_factor`; the 486's costs say 4 and 2
//! ([`x86-tune-costs.h`
//! 361-362](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/config/i386/x86-tune-costs.h#L361-L362)),
//! here the target's `small_unroll_insns` and `small_unroll_factor`. A loop
//! that runs fewer than twice the copies is left (`loop-unroll.cc`
//! [`decide_unroll_constant_iterations`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/loop-unroll.cc#L347),
//! [`decide_unroll_runtime_iterations`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/loop-unroll.cc#L656)).
//!
//! The count the loop makes is peeled to a multiple of the factor: a constant
//! count peels its remainder, a runtime one the `n & 1` iteration behind a
//! test (`unroll_loop_runtime_iterations`), and the loop then chains its
//! bodies (`loopclone::chained`), the test between them gone.
//!
//! Size is `inline::gcc_insns_in` over the loop, gcc's unit; gcc counts RTL
//! instructions after combine, where a compare and its branch are two.
//!
//! A factor of two only, a loop tested in its header and leaving only there.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::graph::loops::{self, Loop};
use llrm_analysis::manager::Registers;
use llrm_analysis::{cfg, induction, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, Analysis, FunctionPass, PreservedAnalyses};
use num_bigint::BigInt;

use crate::expand::Expander;
use crate::lcssa::{arms, definitions, from_arms, operations};
use crate::{edges, inline, lcssa, loopclone, profit};

#[derive(Default)]
pub struct SmallUnroll;

impl FunctionPass for SmallUnroll {
    fn name(&self) -> &'static str {
        "smallunroll"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        match optimized(unit, analyses) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("smallunroll: {error}"),
        }
    }
}

/// Every loop this admits, once each; whether any was.
pub fn optimized(
    unit: &mut passes::Unit,
    analyses: &Analyses,
) -> Result<bool, String> {
    let costs = profit::costs(analyses.outer());
    if costs.small_unroll_factor != 2 || costs.small_unroll_insns < 1 {
        return Ok(false);
    }
    let mut done = BTreeSet::<i64>::new();
    while let Some((candidate, header)) =
        _candidate(unit.context, unit.layout, unit.function, analyses, costs.small_unroll_insns, &done)?
    {
        llrm_support::debug!("smallunroll", "unrolled the loop headed b{header}");
        done.insert(header);
        *unit.function = candidate;
    }
    Ok(!done.is_empty())
}

/// A loop to unroll, tested at its start or its end, and how its count is made
/// a multiple of two.
struct Plan {
    posttested: bool,
    remainder: Remainder,
}

/// How the count of a loop is peeled to a multiple of two.
enum Remainder {
    /// A constant count: its odd iteration is peeled.
    Constant { odd: bool },
    /// A runtime count: its odd iteration, behind a test of the count's low
    /// bit.
    Runtime { trips: induction::Scev, preheader: BlockId },
}

/// `function` closed, with the first loop not in `skip` unrolled, and its
/// header.
fn _candidate(
    context: &mut Context,
    layout: &DataLayout,
    function: &Function,
    analyses: &Analyses,
    limit: i64,
    skip: &BTreeSet<i64>,
) -> Result<Option<(Function, i64)>, String> {
    let mut closed = function.clone();
    lcssa::closed(&mut closed)?;
    let facts = analyses.fresh().get::<Registers>(context, layout, &closed);
    let shape = cfg::Shape::of(&closed);
    for loop_ in shape.loops.clone() {
        if skip.contains(&loop_.header)
            || shape.loops.iter().any(|one| one.header != loop_.header && loop_.body.contains(&one.header))
        {
            continue;
        }
        let plan = {
            let unit = memory::Unit::within(&*context, layout, &closed, analyses.outer())
                .with_registers(&facts)
                .with_shape(&shape);
            _admitted(&unit, &closed, &loop_, &facts, limit)
        };
        let Some(plan) = plan else { continue };
        let Some(header) = _unrolled(context, &mut closed, &loop_, plan)? else { continue };
        return Ok(Some((closed, header)));
    }
    Ok(None)
}

/// How `loop_` is made a multiple of two trips, where gcc unrolls it.
fn _admitted(
    unit: &memory::Unit,
    function: &Function,
    loop_: &Loop,
    facts: &<Registers as Analysis>::Result,
    limit: i64,
) -> Option<Plan> {
    let proofs = induction::counted(unit, loop_, Some(facts), false);
    llrm_support::debug!(
        "smallunroll",
        "loop b{}: {} proofs {:?}",
        loop_.header,
        proofs.len(),
        proofs.iter().map(|one| (one.posttested, one.trips_linear().is_some())).collect::<Vec<_>>()
    );
    // Counters that agree on the count are one proof of it.
    let proof = proofs.first()?;
    if proofs.iter().any(|one| one.posttested != proof.posttested || one.trips_linear() != proof.trips_linear()) {
        return None;
    }
    let posttested = proof.posttested;
    let size = inline::gcc_insns_in(
        function,
        loop_.body.iter().flat_map(|&at| function.block(cfg::block(at)).instructions().to_vec()),
    );
    // One more than gcc's RTL count, since the compare and branch are two
    // there.
    let rtl = size;
    llrm_support::debug!(
        "smallunroll",
        "counted loop b{}: {size} gcc insns, {rtl} counted, limit {limit}, count {:?}",
        loop_.header,
        induction::trip_count(unit, loop_, facts)
    );
    if rtl > limit {
        return None;
    }
    let preheader = {
        let outside = cfg::graph(function)
            .iter()
            .filter(|block| block.succ.contains(&loop_.header) && !loop_.body.contains(&block.at))
            .map(|block| block.at)
            .collect::<Vec<_>>();
        let [only] = outside[..] else { return None };
        cfg::block(only)
    };
    // gcc leaves a loop that rolls fewer than twice the copies twice over.
    if let Some(count) = induction::trip_count(unit, loop_, facts) {
        return (count >= BigInt::from(4))
            .then(|| Plan { posttested, remainder: Remainder::Constant { odd: count.bit(0) } });
    }
    if induction::trip_bound(unit, loop_, facts).is_some_and(|bound| bound < BigInt::from(4)) {
        return None;
    }
    Some(Plan { posttested, remainder: Remainder::Runtime { trips: proof.trips_linear()?, preheader } })
}

/// The conditional branches of `loop_`: each is a compare and a jump to gcc's
/// RTL, one statement to its GIMPLE.
fn _branches(
    function: &Function,
    loop_: &Loop,
) -> usize {
    loop_
        .body
        .iter()
        .flat_map(|&at| function.block(cfg::block(at)).instructions())
        .filter(|&&inst| {
            let op = function.instruction(inst);
            op.opcode == Opcode::Br && op.operands.len() == 3
        })
        .count()
}

/// `loop_` with its count made even and two bodies chained; its header.
fn _unrolled(
    context: &mut Context,
    function: &mut Function,
    loop_: &Loop,
    Plan { posttested, remainder }: Plan,
) -> Result<Option<i64>, String> {
    let header = cfg::block(loop_.header);
    // What the header's phis take from the way in, for the way that skips the
    // peeled trip.
    let entry = cfg::graph(function)
        .iter()
        .find(|block| block.succ.contains(&loop_.header) && !loop_.body.contains(&block.at))
        .map(|block| cfg::block(block.at))
        .ok_or("a loop with no way in")?;
    let phis = edges::phis(function, header);
    let starts = phis
        .iter()
        .map(|&phi| {
            arms(function, phi)
                .into_iter()
                .find(|&(_, source)| source == entry)
                .map(|(value, _)| value)
                .ok_or_else(|| "KeyError".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let latch = cfg::block(*loop_.latches.first().ok_or("a loop with no latch")?);
    let odd = match &remainder {
        Remainder::Constant { odd } => *odd,
        Remainder::Runtime { .. } => true,
    };
    if odd {
        let peeled = if posttested {
            _peeled_after(function, loop_, entry)?
        } else {
            loopclone::peeled(function, loop_, 1)?.map(|mut one| one.remove(0))
        };
        let Some(labels) = peeled else { return Ok(None) };
        if let Remainder::Runtime { trips, preheader } = &remainder {
            let peeled_latch = labels[&cfg::id(latch)];
            _guarded(context, function, header, *preheader, peeled_latch, &phis, &starts, trips);
        }
    }
    let shape = cfg::Shape::of(function);
    let Some(again) = shape.loops.iter().find(|one| one.header == loop_.header) else { return Ok(None) };
    if posttested {
        return Ok(_chained_after(function, again)?.then_some(loop_.header));
    }
    Ok(loopclone::chained(function, again, 1)?.map(|_| loop_.header))
}

/// The peeled trip behind a test that the count is odd: the way in tests it
/// and goes round the peeled trip otherwise, into a block that joins the two
/// ways to the loop.
#[allow(clippy::too_many_arguments)]
fn _guarded(
    context: &mut Context,
    function: &mut Function,
    header: BlockId,
    preheader: BlockId,
    peeled_latch: BlockId,
    phis: &[InstId],
    starts: &[Operand],
    trips: &induction::Scev,
) {
    let jump = function.terminator(preheader).expect("a terminated preheader");
    let peeled = match function.instruction(jump).operands[..] {
        [Operand::Block(first)] => first,
        _ => unreachable!("a peeled loop is entered by a jump"),
    };
    let ty = context.types.int(trips.width);
    let count = Expander::new(jump).int(context, function, trips);
    let one = Operand::Constant(context.int(ty, 1));
    let low = placed(function, Opcode::Binary(BinaryOp::And), ty, vec![count, one], jump);
    let bit = context.types.int(1);
    let zero = Operand::Constant(context.int(ty, 0));
    let odd = placed(function, Opcode::ICmp(IntPredicate::Ne), bit, vec![low, zero], jump);
    let join = function.create_block(None);
    function.insert_block(join, Some(peeled_latch)).expect("a block after the peeled latch");
    let void = function.instruction(jump).ty;
    let onward = function.create_instruction(Opcode::Br, void, vec![Operand::Block(header)], Flags::default(), None);
    function.insert(onward, Position::End(join)).expect("a new block");
    let latch_jump = function.terminator(peeled_latch).expect("a terminated latch");
    edges::retarget(function, latch_jump, header, join);
    let branch = function.create_instruction(
        Opcode::Br,
        void,
        vec![odd, Operand::Block(peeled), Operand::Block(join)],
        Flags::default(),
        None,
    );
    function.insert(branch, Position::Before(jump)).expect("the preheader's branch");
    function.erase(jump).expect("a branch");
    for (&phi, &start) in phis.iter().zip(starts) {
        let ty = function.instruction(phi).ty;
        let after = arms(function, phi)
            .into_iter()
            .find(|&(_, source)| source == peeled_latch)
            .map(|(value, _)| value)
            .expect("the peeled trip's value");
        let merged = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), None);
        function.insert(merged, Position::Before(onward)).expect("the join");
        function.set_operands(merged, from_arms(&[(after, peeled_latch), (start, preheader)]));
        let value = Operand::Value(function.instruction(merged).result.expect("a phi's value"));
        let incoming = arms(function, phi)
            .into_iter()
            .map(|(arm, source)| if source == peeled_latch { (value, join) } else { (arm, source) })
            .collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&incoming));
    }
}

/// Whether `loop_` is tested at its end and leaves only there, by a conditional
/// branch whose other way is the header; the latch, its branch and the exit.
fn _tested_at_end(
    function: &Function,
    loop_: &Loop,
) -> Option<(BlockId, InstId, BlockId)> {
    let graph = cfg::graph(function);
    let blocks = graph.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let predecessors = loops::predecessors(&graph);
    let [latch_at] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    if Some(loop_.header) == function.entry().map(cfg::id) {
        return None;
    }
    let outside = predecessors[&loop_.header].difference(&loop_.body).count();
    let leaving = loop_.body.iter().filter(|at| blocks[at].succ.iter().any(|to| !loop_.body.contains(to))).count();
    if outside != 1 || leaving != 1 || !blocks[&latch_at].succ.iter().any(|to| !loop_.body.contains(to)) {
        return None;
    }
    if loop_.body.iter().any(|at| *at != loop_.header && !predecessors[at].is_subset(&loop_.body)) {
        return None;
    }
    let (header, latch) = (cfg::block(loop_.header), cfg::block(latch_at));
    let branch = function.terminator(latch)?;
    let [_, Operand::Block(yes), Operand::Block(no)] = function.instruction(branch).operands[..] else { return None };
    let exit = if yes == header { no } else { yes };
    if (yes == header) == (no == header) || loop_.body.contains(&cfg::id(exit)) {
        return None;
    }
    // Nothing outside reads a value of the loop but a phi.
    let defined = definitions(function, &loop_.body);
    for block in graph.iter().filter(|block| !loop_.body.contains(&block.at)) {
        let reads = |inst: InstId| {
            function
                .instruction(inst)
                .operands
                .iter()
                .any(|operand| matches!(operand, Operand::Value(value) if defined.contains_key(value)))
        };
        if operations(function, cfg::block(block.at)).into_iter().any(reads) {
            return None;
        }
    }
    Some((latch, branch, exit))
}

/// A copy of `loop_`'s blocks, its header's phis read as `view` has them; each
/// block's clone and each value's. The copy's branches go to its own blocks
/// but for the way round, which is still the header.
fn _copy(
    function: &mut Function,
    loop_: &Loop,
    view: &IdMap<ValueId, Operand>,
) -> Result<(BTreeMap<i64, BlockId>, IdMap<ValueId, ValueId>), String> {
    let (header, latch_at) = (cfg::block(loop_.header), *loop_.latches.first().expect("one latch"));
    let originals: Vec<i64> =
        cfg::graph(function).iter().filter(|block| loop_.body.contains(&block.at)).map(|block| block.at).collect();
    let mut map = BTreeMap::new();
    let mut values = IdMap::new();
    let mut cloned = Vec::new();
    for &at in &originals {
        let block = function.create_block(None);
        function.insert_block(block, None)?;
        map.insert(at, block);
        for inst in function.block(cfg::block(at)).instructions().to_vec() {
            if at == loop_.header && function.instruction(inst).opcode == Opcode::Phi {
                continue;
            }
            let made = function.clone_instruction(inst);
            function.insert(made, Position::End(block))?;
            if let (Some(original), Some(result)) =
                (function.instruction(inst).result, function.instruction(made).result)
            {
                values.insert(original, result);
            }
            cloned.push((at, inst, made));
        }
    }
    let mapped = |operand: Operand| match operand {
        Operand::Value(one) => match view.get(&one) {
            Some(&supplied) => supplied,
            None => Operand::Value(values.get(&one).copied().unwrap_or(one)),
        },
        other => other,
    };
    for &(at, inst, made) in &cloned {
        let operands = if function.instruction(inst).opcode == Opcode::Phi {
            from_arms(
                &arms(function, inst)
                    .into_iter()
                    .map(|(incoming, source)| (mapped(incoming), map[&cfg::id(source)]))
                    .collect::<Vec<_>>(),
            )
        } else {
            function
                .instruction(inst)
                .operands
                .iter()
                .map(|&operand| match operand {
                    Operand::Block(target) => match map.get(&cfg::id(target)) {
                        Some(&copy) if !(at == latch_at && target == header) => Operand::Block(copy),
                        _ => Operand::Block(target),
                    },
                    other => mapped(other),
                })
                .collect()
        };
        function.set_operands(made, operands);
    }
    Ok((map, values))
}

/// What a header phi takes from `from`.
fn _taken(
    function: &Function,
    header: BlockId,
    from: BlockId,
) -> Result<IdMap<ValueId, Operand>, String> {
    edges::phis(function, header)
        .into_iter()
        .map(|phi| {
            arms(function, phi)
                .into_iter()
                .find(|&(_, source)| source == from)
                .map(|(value, _)| (function.instruction(phi).result.expect("a phi's value"), value))
                .ok_or_else(|| "KeyError".to_owned())
        })
        .collect()
}

/// `loop_` tested at its end with a second body chained in it: the latch of
/// the first runs on into the second, whose test leaves or goes round, and
/// the first's test, which would always go on, is gone. Right where the loop
/// makes an even number of trips. `false`, changing nothing, where
/// `_tested_at_end` does not hold.
fn _chained_after(
    function: &mut Function,
    loop_: &Loop,
) -> Result<bool, String> {
    let Some((latch, branch, exit)) = _tested_at_end(function, loop_) else { return Ok(false) };
    let header = cfg::block(loop_.header);
    // What the second body reads of the first's header phis: what the latch
    // supplied.
    let flowing = _taken(function, header, latch)?;
    let (map, values) = _copy(function, loop_, &flowing)?;
    let mapped = |operand: Operand| match operand {
        Operand::Value(one) => match flowing.get(&one) {
            Some(&supplied) => supplied,
            None => Operand::Value(values.get(&one).copied().unwrap_or(one)),
        },
        other => other,
    };
    // The first body runs on into the second; the header takes what the second
    // supplies, and the exit what the second leaves with.
    let second = map[&cfg::id(latch)];
    let void = function.instruction(branch).ty;
    let onward =
        function.create_instruction(Opcode::Br, void, vec![Operand::Block(map[&loop_.header])], Flags::default(), None);
    function.insert(onward, Position::Before(branch))?;
    function.erase(branch).expect("a branch");
    for phi in edges::phis(function, header).into_iter().chain(edges::phis(function, exit)) {
        let incoming = arms(function, phi)
            .into_iter()
            .map(|(value, source)| if source == latch { (mapped(value), second) } else { (value, source) })
            .collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&incoming));
    }
    Ok(true)
}

/// One trip of `loop_`, tested at its end, ahead of it: the copy's test leaves
/// or goes on into the loop, which the header's phis take from the copy's
/// latch. The copy's blocks; `None`, changing nothing, where `_tested_at_end`
/// does not hold.
fn _peeled_after(
    function: &mut Function,
    loop_: &Loop,
    entry: BlockId,
) -> Result<Option<BTreeMap<i64, BlockId>>, String> {
    let Some((latch, _, exit)) = _tested_at_end(function, loop_) else { return Ok(None) };
    let header = cfg::block(loop_.header);
    let starting = _taken(function, header, entry)?;
    let (map, values) = _copy(function, loop_, &starting)?;
    let mapped = |operand: Operand| match operand {
        Operand::Value(one) => match starting.get(&one) {
            Some(&supplied) => supplied,
            None => Operand::Value(values.get(&one).copied().unwrap_or(one)),
        },
        other => other,
    };
    let first = map[&loop_.header];
    let jump = function.terminator(entry).expect("a terminated entry");
    edges::retarget(function, jump, header, first);
    let copy_latch = map[&cfg::id(latch)];
    for phi in edges::phis(function, header) {
        let value = arms(function, phi).into_iter().find(|&(_, source)| source == latch).map(|(value, _)| value);
        let mut incoming = arms(function, phi);
        incoming.retain(|&(_, source)| source != entry);
        incoming.push((mapped(value.ok_or("KeyError")?), copy_latch));
        function.set_operands(phi, from_arms(&incoming));
    }
    for phi in edges::phis(function, exit) {
        let mut incoming = arms(function, phi);
        if let Some(value) = incoming.iter().find(|&&(_, source)| source == latch).map(|&(value, _)| value) {
            incoming.push((mapped(value), copy_latch));
        }
        function.set_operands(phi, from_arms(&incoming));
    }
    Ok(Some(map))
}

fn placed(
    function: &mut Function,
    opcode: Opcode,
    ty: llrm_mir::types::TypeId,
    operands: Vec<Operand>,
    before: InstId,
) -> Operand {
    let inst = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(inst, Position::Before(before)).expect("a placed instruction");
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

#[cfg(test)]
#[path = "smallunroll_tests.rs"]
mod tests;
