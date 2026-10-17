//! Branches whose way is known, decided: LLVM's JumpThreading and SimplifyCFG
//! on known conditions.
//!
//! # How it works
//! `decided` runs three steps, then `cfg::merged` joins the chains that result.
//!
//! 1. Threading. A block holding only a `br` is bypassed (`_threaded`): each
//!    branch or switch edge to it goes to what it jumps to, where that has no
//!    phis to tell the edges apart. A loop's one entry edge, one latch and
//!    dedicated exits are kept, so the loop stays in simplify form. A
//!    conditional branch whose two arms go one way becomes a jump. Separately
//!    (`_phi_threaded`), a predecessor that only jumps to a block holding just
//!    a phi and a branch on it, and gives the phi a constant, goes straight to
//!    where that constant leads (LLVM's `ProcessBranchOnPHI`); a loop header
//!    keeps its edges.
//! 2. Deciding. For each block ending in a `br i1` or a `switch`, the way is
//!    found from, in order: `consts`' answer for the condition, or for the two
//!    operands of the `icmp` it is, carried across loops by `constant_cycles`
//!    along only the edges that run; a pointer compared with null that
//!    points-to facts say is non-null; the intervals `ranges` has on each edge,
//!    if exactly one successor is possible; the guards that hold at the block
//!    and the loops around it (`guards::Given`, over `induction` terms). A
//!    switch is decided by a known condition with distinct cases.
//! 3. The decided terminator is replaced by a `br` to the chosen block; blocks
//!    no longer reachable go and phis left with one value are replaced by it.
//!
//!
//! ```text
//!    b0: %c = icmp ult i16 3, 5          b0: br b1
//!        br i1 %c, b1, b2          ==>   b1: ...
//!    b1: ...                             (b2 and its phi arms are gone)
//!    b2: ...
//! ```
//!
//! # Example
//! Before:
//!
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   %k = add i16 2, 3
//!   %c = icmp slt i16 %k, 10
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   ret i16 %x
//!
//! b2:
//!   ret i16 0
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   %k = add i16 2, 3
//!   %c = icmp slt i16 %k, 10
//!   br label %b1
//!
//! b1:
//!   ret i16 %x
//! }
//! ```
//!
//! `5 < 10` is known, so the branch is a jump to `b1` and `b2` is gone; `dead`
//! removes `%c` later.
//!
//! From unit test `test_a_branch_on_known_numbers_is_a_jump` in
//! `decide_tests.rs`.
//!
//! # Runs
//! Once per pass of the function fixed point, after `fold` and `ports` and
//! before `phiopt`. `Options::decide` turns it off (no `-f` flag); nothing runs
//! at -O0. `jumpthread` is the separate pass that threads a state variable's
//! edges.
//!
//! # Needs
//! `consts`, `constant_cycles`, `ranges`, `guards`, `induction`, points-to
//! (`pointers`), the CFG shape.
//!
//! # Keeps
//! Blocks and edges change: every analysis is invalidated when anything was
//! decided or merged.
//!
//! # See also
//! [`crate::cfg`], [`crate::fold`], [`crate::jumpthread`]; LLVM JumpThreading,
//! SimplifyCFG.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::alias;
use llrm_analysis::cfg;
use llrm_analysis::constant_cycles::{self, State};
use llrm_analysis::consts::{self, Known, masked};
use llrm_analysis::graph::loops;
use llrm_analysis::guards;
use llrm_analysis::induction::{self, Scev};
use llrm_analysis::manager::Held;
use llrm_analysis::memory::Unit;
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::ranges;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::footprint::Footprint;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Outer, PreservedAnalyses};
use llrm_support::hash::IndexMap;

use crate::edges;
use crate::transform;

/// `decided`, then its chains merged, as the old Decide.
pub struct Decide;

impl FunctionPass for Decide {
    fn name(&self) -> &'static str {
        "decide"
    }

    /// A branch is decided from what is known of its condition (the values it
    /// is made of, and the memory they were loaded from) along the edges
    /// that run: not from a value nothing above reads.
    fn reads(&self) -> Footprint {
        Footprint::CONTROL | Footprint::MEMORY | Footprint::STRUCTURE
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let decided = _decided(unit.context, unit.layout, unit.function, analyses)
            .unwrap_or_else(|error| panic!("decide: {error}"));
        if decided | crate::cfg::merged(unit.function) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// `function` threaded, and each branch whose way is known a jump that
/// way; `outer` is its module and target. Whether anything changed.
pub fn decided(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> Result<bool, String> {
    _decided(context, layout, function, &mut Analyses::new(std::rc::Rc::new(outer.clone())))
}

/// `decided`, `analyses` holding what is known of `function`.
fn _decided(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
) -> Result<bool, String> {
    let threaded = _threaded(context, function) | _phi_threaded(context, function)?;
    if threaded {
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    let held = Held::of(context, layout, function, analyses, true).with_bounded(context, layout, function, analyses);
    // What the values say, as LLVM's SimplifyCFG decides a branch: a number
    // memory gave is `fold`'s to put in the condition first. Solving memory
    // here decided none of 574 branches at d_faces -O2 and was a quarter of
    // the pass.
    let registers = analyses.get::<llrm_analysis::manager::Registers>(context, layout, function);
    let decisions = _decisions(&held.unit(context, layout, function, analyses.outer()), &registers)?;
    if std::env::var_os("LLRM_POC_CE").is_some() {
        let sources = SOURCES.with(|one| one.borrow().clone());
        let unit = held.unit(context, layout, function, analyses.outer());
        let w0 = llrm_support::debug::work();
        let found = crate::constraint_elim::decisions(&unit);
        let ce_work = llrm_support::debug::work() - w0;
        // Real decisions as (block, way): true when the first successor is taken.
        let real: std::collections::BTreeMap<i64, bool> = decisions
            .iter()
            .map(|&(block, target)| {
                let last = function.terminator(block).expect("a terminator");
                let first = function.instruction(last).operands.get(1).copied() == Some(Operand::Block(target));
                (cfg::id(block), first)
            })
            .collect();
        let ce: std::collections::BTreeMap<i64, bool> = found.iter().map(|&(block, way)| (cfg::id(block), way)).collect();
        let mut both = 0;
        let mut conflict = 0;
        let mut missed = std::collections::BTreeMap::<&str, usize>::new();
        for (block, way) in &real {
            match ce.get(block) {
                Some(other) if other == way => both += 1,
                Some(_) => conflict += 1,
                None => *missed.entry(sources.get(block).copied().unwrap_or("?")).or_default() += 1,
            }
        }
        let extra = ce.keys().filter(|block| !real.contains_key(block)).count();
        let insts = function.walk().count();
        eprintln!("POCCE insts={insts} real={} ce={} both={both} conflict={conflict} missed={missed:?} extra={extra} ce_work={ce_work}", real.len(), ce.len());
    }
    if decisions.is_empty() {
        return Ok(threaded);
    }
    for (block, target) in decisions {
        let last = function.terminator(block).expect("a terminator");
        _jump(function, last, target);
    }
    cfg::_unreachable(context, function);
    transform::_trivial_phis(function)?;
    Ok(true)
}

/// Each block whose terminator's way is known, with that way.
thread_local! {
    static SOURCES: std::cell::RefCell<std::collections::BTreeMap<i64, &'static str>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

fn _decisions(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
) -> Result<Vec<(BlockId, BlockId)>, String> {
    SOURCES.with(|one| one.borrow_mut().clear());
    let function = unit.function;
    // Points-to only for a pointer compared with null, as LLVM asks
    // isKnownNonZero of one value rather than solving every pointer.
    let pointers = std::cell::OnceCell::new();
    let pointing = alias::may_point(unit);
    let nonnull = |value: ValueId| {
        pointing.contains(&value)
            && alias::nonnull_by_definition(unit, value).unwrap_or_else(|| {
                pointers.get_or_init(|| unit.pointers()).as_ref().is_ok_and(|facts| facts.nonnull(value))
            })
    };
    let successors = |at: i64, values: &IndexMap<ValueId, Known>, states: &IndexMap<ValueId, State>| {
        _executable_successors(unit, at, values, states, Some(&nonnull))
    };
    // What a block's way is read from: its terminator's operands and, for a
    // branch, those of the comparison its condition is.
    let reads = |at: i64| -> Vec<ValueId> {
        let block = cfg::block(at);
        let Some(last) = function.terminator(block) else { return Vec::new() };
        let mut values: Vec<ValueId> = function
            .instruction(last)
            .operands
            .iter()
            .filter_map(|operand| if let Operand::Value(value) = operand { Some(*value) } else { None })
            .collect();
        if let Some(compare) = transform::_comparison(function, block, last) {
            values.extend(
                function
                    .instruction(compare)
                    .operands
                    .iter()
                    .filter_map(|operand| if let Operand::Value(value) = operand { Some(*value) } else { None }),
            );
        }
        values
    };
    let facts = constant_cycles::propagated(
        unit,
        facts.clone(),
        Some(constant_cycles::Successors { choose: &successors, reads: &reads }),
        None,
    );
    let scoped = ranges::bounds(unit)?;
    let mut out = Vec::new();
    for &block in function.layout() {
        let Some(last) = function.terminator(block) else {
            continue;
        };
        let instruction = function.instruction(last);
        if instruction.opcode == Opcode::Switch {
            let target = _switch_target(unit, last, &facts);
            if target.is_some() {
                SOURCES.with(|one| one.borrow_mut().insert(cfg::id(block), "switch"));
            }
            out.extend(target.map(|target| (block, target)));
            continue;
        }
        let [_, Operand::Block(taken), Operand::Block(other)] = instruction.operands[..] else {
            continue;
        };
        let mut answer = _outcome(unit, block, &facts, Some(&nonnull));
        let mut source = if answer.is_some() { "consts" } else { "" };
        if answer.is_none() {
            let scope = scoped.scope(unit, cfg::id(block));
            let mut possible = Vec::new();
            for target in function.successors(block) {
                if ranges::on_edge_over(unit, block, target, &scope, Some(&facts))?.is_some() {
                    possible.push(target);
                }
            }
            if let [one] = possible[..] {
                answer = Some(one == taken);
                source = "ranges";
            }
        }
        if answer.is_none() {
            answer = _implied(unit, block, last);
            if answer.is_some() {
                source = "guards";
            }
        }
        if answer.is_some() {
            SOURCES.with(|one| one.borrow_mut().insert(cfg::id(block), source));
        }
        out.extend(answer.map(|answer| (block, if answer { taken } else { other })));
    }
    Ok(out)
}

/// Whether the compare `last` branches on holds or fails there, by what the
/// guards over `block` and the loops holding it prove of its two sides.
fn _implied(
    unit: &Unit,
    block: BlockId,
    last: InstId,
) -> Option<bool> {
    let function = unit.function;
    let Some(Operand::Value(condition)) = function.instruction(last).operands.first().copied() else { return None };
    let (_, compare) = unit.defining(Operand::Value(condition))?;
    let (Opcode::ICmp(predicate), [left, right]) = (&compare.opcode, &compare.operands[..]) else { return None };
    let width = unit.int_bits(*left)?;
    let (left, right) = (induction::term(unit, *left)?, induction::term(unit, *right)?);
    let (left, right) = (Scev::of(&left, width), Scev::of(&right, width));
    let at = cfg::id(block);
    let given = guards::Given::at(unit, at, &[]);
    if given.holds(unit, *predicate, &left, &right) {
        return Some(true);
    }
    given.holds(unit, predicate.inverse(), &left, &right).then_some(false)
}

/// `last`, a terminator, replaced by a jump to `target`.
fn _jump(
    function: &mut Function,
    last: InstId,
    target: BlockId,
) {
    let jump = function.create_instruction(
        Opcode::Br,
        function.instruction(last).ty,
        vec![Operand::Block(target)],
        Flags::default(),
        None,
    );
    function.insert(jump, Position::Before(last)).expect("a placed terminator");
    function.erase(last).expect("a terminator defines nothing");
}

/// Whether `block`'s conditional branch is taken: its condition is known,
/// the compare that makes it has two known operands, or it compares with
/// null a pointer `nonnull` says is not.
pub fn _outcome(
    unit: &Unit,
    block: BlockId,
    facts: &IndexMap<ValueId, Known>,
    nonnull: Option<&dyn Fn(ValueId) -> bool>,
) -> Option<bool> {
    let branch = unit.function.terminator(block)?;
    let instruction = unit.function.instruction(branch);
    if instruction.opcode != Opcode::Br || instruction.operands.len() != 3 {
        return None;
    }
    if let Some(condition) = consts::_operand(unit, instruction.operands[0], facts, None) {
        return Some(masked(&condition.n, 1) != BigInt::from(0));
    }
    let compare = unit.function.instruction(transform::_comparison(unit.function, block, branch)?);
    let Opcode::ICmp(predicate) = compare.opcode else { unreachable!("_comparison finds an icmp") };
    let (left, right) = (
        consts::_operand(unit, compare.operands[0], facts, None),
        consts::_operand(unit, compare.operands[1], facts, None),
    );
    if let (Some(left), Some(right)) = (&left, &right) {
        return Some(consts::holds(predicate, left, right));
    }
    if !matches!(predicate, IntPredicate::Eq | IntPredicate::Ne) {
        return None;
    }
    let null = |one: Operand| {
        matches!(
            one,
            Operand::Constant(id) if unit.context.get(id).kind == ConstantKind::Null
        )
    };
    let pointer = match compare.operands[..] {
        [Operand::Value(pointer), other] | [other, Operand::Value(pointer)] if null(other) => pointer,
        _ => return None,
    };
    nonnull?(pointer).then_some(predicate == IntPredicate::Ne)
}

/// Where the switch `inst` goes, where its condition is known and its
/// cases distinct.
pub fn _switch_target(
    unit: &Unit,
    inst: InstId,
    facts: &IndexMap<ValueId, Known>,
) -> Option<BlockId> {
    let op = unit.function.instruction(inst);
    if op.opcode != Opcode::Switch {
        return None;
    }
    let value = consts::_operand(unit, op.operands[0], facts, None)?;
    let cases = op.operands[2..]
        .chunks(2)
        .map(|pair| match pair {
            [case, Operand::Block(target)] => Some((consts::_operand(unit, *case, facts, None)?.n, *target)),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if cases.iter().map(|(number, _)| number).collect::<BTreeSet<_>>().len() != cases.len() {
        return None;
    }
    let Operand::Block(default) = op.operands[1] else {
        return None;
    };
    Some(cases.into_iter().find(|(number, _)| *number == value.n).map_or(default, |(_, target)| target))
}

/// The successors of block `at` that can run, given what is known so far;
/// `None` while its condition is still pending.
pub fn _executable_successors(
    unit: &Unit,
    at: i64,
    facts: &IndexMap<ValueId, Known>,
    states: &IndexMap<ValueId, State>,
    nonnull: Option<&dyn Fn(ValueId) -> bool>,
) -> Option<Vec<i64>> {
    let function = unit.function;
    let block = cfg::block(at);
    let all = function.successors(block).into_iter().map(cfg::id).collect::<Vec<_>>();
    let pending = |operands: &[Operand]| {
        operands
            .iter()
            .any(
                |operand| matches!(
                    operand,
                    Operand::Value(value) if states.get(value) == Some(&State::Pending)
                ),
            )
    };
    let Some(last) = function.terminator(block) else {
        return Some(all);
    };
    let instruction = function.instruction(last);
    if instruction.opcode == Opcode::Switch {
        if let Some(target) = _switch_target(unit, last, facts) {
            return Some(vec![cfg::id(target)]);
        }
        return if pending(&instruction.operands[..1]) { None } else { Some(all) };
    }
    let [condition, Operand::Block(taken), Operand::Block(other)] = instruction.operands[..] else {
        return Some(all);
    };
    if let Some(answer) = _outcome(unit, block, facts, nonnull) {
        return Some(vec![cfg::id(if answer { taken } else { other })]);
    }
    let compared = transform::_comparison(function, block, last)
        .map(|compare| function.instruction(compare).operands.clone())
        .unwrap_or_default();
    if pending(&[condition]) || pending(&compared) {
        return None;
    }
    Some(all)
}

/// `value` as a constant in `from`, a block whose one predecessor branches on
/// it: false where `from` is the branch's false side, true where it is the true
/// side (LLVM JumpThreading's `getKnownConstant` of an edge).
fn known_on_edge(
    context: &mut Context,
    function: &Function,
    value: ValueId,
    from: BlockId,
) -> Option<Operand> {
    let [only] = function.predecessors(from)[..] else { return None };
    let branch = function.instruction(function.terminator(only)?);
    let [Operand::Value(condition), Operand::Block(taken), Operand::Block(other)] = branch.operands[..] else {
        return None;
    };
    if branch.opcode != Opcode::Br || condition != value || taken == other {
        return None;
    }
    let ty = function.value(value).ty;
    let bit = if from == taken {
        1
    } else if from == other {
        0
    } else {
        return None;
    };
    Some(Operand::Constant(context.int(ty, bit)))
}

/// A predecessor that only jumps to a block holding just a phi and a
/// branch on it, and gives the phi a constant, goes straight where that
/// constant leads: LLVM JumpThreading's `ProcessBranchOnPHI`. A loop's
/// header keeps its edges. Whether anything changed.
pub fn _phi_threaded(
    context: &mut Context,
    function: &mut Function,
) -> Result<bool, String> {
    let headers = cfg::Shape::of(function).loops.iter().map(|one| one.header).collect::<BTreeSet<_>>();
    let mut changed = false;
    for block in function.layout().to_vec() {
        let instructions = function.block(block).instructions().to_vec();
        let [phi, last] = instructions[..] else { continue };
        let branch = function.instruction(last);
        let [Operand::Value(condition), Operand::Block(taken), Operand::Block(other)] = branch.operands[..] else {
            continue;
        };
        let reads_only = function.users(condition).len() == 1;
        let is_phi = function.instruction(phi).opcode == Opcode::Phi;
        if headers.contains(&cfg::id(block))
            || branch.opcode != Opcode::Br
            || !is_phi
            || function.instruction(phi).result != Some(condition)
            || !reads_only
        {
            continue;
        }
        for (value, from) in crate::lcssa::arms(function, phi) {
            // A condition is its own answer on the edge that its branch
            // settles.
            let value = match value {
                Operand::Value(one) => known_on_edge(context, function, one, from).unwrap_or(value),
                other => other,
            };
            let Operand::Constant(id) = value else { continue };
            let ConstantKind::Int(bits) = context.get(id).kind else { continue };
            let target = if bits & 1 == 1 { taken } else { other };
            let jump = function.terminator(from).expect("a terminator");
            let only_jumps = function.instruction(jump).operands == [Operand::Block(block)];
            if !only_jumps || target == block || function.predecessors(target).contains(&from) {
                continue;
            }
            // What the target's phis took from `block` they now take from
            // `from` too.
            for one in edges::phis(function, target) {
                let mut incoming = crate::lcssa::arms(function, one);
                let (carried, _) =
                    *incoming.iter().find(|(_, source)| *source == block).expect("an arm from the block");
                incoming.push((if carried == Operand::Value(condition) { value } else { carried }, from));
                function.set_operands(one, crate::lcssa::from_arms(&incoming));
            }
            let kept =
                crate::lcssa::arms(function, phi).into_iter().filter(|(_, source)| *source != from).collect::<Vec<_>>();
            function.set_operands(phi, crate::lcssa::from_arms(&kept));
            function.set_operands(jump, vec![Operand::Block(target)]);
            changed = true;
        }
    }
    if changed {
        cfg::_unreachable(context, function);
        transform::_trivial_phis(function)?;
    }
    Ok(changed)
}

/// Blocks holding only a jump bypassed, where what they jump to has no
/// phis to tell the edges apart. Whether anything changed.
///
/// Loop-simplify form keeps a loop's one entry edge, its one back edge and
/// its dedicated exits as blocks of their own.
pub fn _threaded(
    context: &mut Context,
    function: &mut Function,
) -> bool {
    if function.entry().is_none() {
        return false;
    }
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let known = graph.iter().map(|block| (block.at, &block.succ)).collect::<BTreeMap<_, _>>();
    let none: &[i64] = &[];
    let mut loop_edges = BTreeSet::new();
    for loop_ in cfg::Shape::of(function).loops {
        let outside = predecessors
            .get(&loop_.header)
            .unwrap_or(none)
            .iter()
            .copied()
            .filter(|at| !loop_.body.contains(at))
            .collect::<Vec<_>>();
        if let [parent] = outside[..]
            && *known[&parent] == [loop_.header]
        {
            loop_edges.insert(parent);
        }
        if let Some(&latch) = loop_.latches.iter().next().filter(|_| loop_.latches.len() == 1)
            && *known[&latch] == [loop_.header]
        {
            loop_edges.insert(latch);
        }
        loop_edges.extend(
            graph
                .iter()
                .filter(|block| {
                    let parents = predecessors.get(&block.at).unwrap_or(none);
                    !loop_.body.contains(&block.at)
                        && !parents.is_empty()
                        && parents.iter().all(|at| loop_.body.contains(at))
                        && block.succ.len() == 1
                        && !loop_.body.contains(&block.succ[0])
                })
                .map(|block| block.at),
        );
    }
    let mut redirects = BTreeMap::new();
    for &block in function.layout() {
        let at = cfg::id(block);
        let [only] = function.block(block).instructions()[..] else {
            continue;
        };
        let instruction = function.instruction(only);
        if let (Opcode::Br, [Operand::Block(next)]) = (&instruction.opcode, &instruction.operands[..])
            && !loop_edges.contains(&at)
            && edges::phis(function, *next).is_empty()
        {
            redirects.insert(at, cfg::id(*next));
        }
    }
    let destination = |start: i64, source: i64| {
        let (mut target, mut seen) = (start, BTreeSet::from([source]));
        while let Some(&next) = redirects.get(&target) {
            if !seen.insert(target) {
                break;
            }
            target = next;
        }
        if seen.contains(&target) { start } else { target }
    };

    let mut changed = false;
    for block in function.layout().to_vec() {
        let Some(last) = function.terminator(block) else {
            continue;
        };
        if !matches!(function.instruction(last).opcode, Opcode::Br | Opcode::Switch) {
            continue;
        }
        for (index, operand) in function.instruction(last).operands.clone().into_iter().enumerate() {
            if let Operand::Block(target) = operand {
                let to = destination(cfg::id(target), cfg::id(block));
                if to != cfg::id(target) {
                    function.set_operand(last, index, Operand::Block(cfg::block(to)));
                    changed = true;
                }
            }
        }
        // Both arms one way: the condition chooses nothing.
        if let [_, Operand::Block(taken), Operand::Block(other)] = function.instruction(last).operands[..]
            && taken == other
        {
            _jump(function, last, taken);
            changed = true;
        }
    }
    if changed {
        cfg::_unreachable(context, function);
    }
    changed
}

#[cfg(test)]
#[path = "decide_tests.rs"]
mod tests;
