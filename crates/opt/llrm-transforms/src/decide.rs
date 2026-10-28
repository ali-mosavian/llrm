//! Branches whose way is known, decided: LLVM's JumpThreading and SimplifyCFG
//! on known conditions.
//!
//! # How it works
//! `decided` runs two steps. Threading is `jumpthreading`'s and
//! `simplifycfg`'s.
//!
//! 1. Deciding. For each block ending in a `br i1` or a `switch`, the way is
//!    found from, in order: `consts`' answer for the condition, or for the two
//!    operands of the `icmp` it is, carried across loops by `constant_cycles`
//!    along only the edges that run; a pointer compared with null that
//!    points-to facts say is non-null; the intervals `ranges` has on each edge,
//!    if exactly one successor is possible; the guards that hold at the block
//!    and the loops around it (`guards::Given`, over `induction` terms). A
//!    switch is decided by a known condition with distinct cases.
//! 2. The decided terminator is replaced by a `br` to the chosen block; blocks
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
//!   %c = icmp slt i16 5, 10
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
//!   %c = icmp slt i16 5, 10
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
//! Once per pass of the function fixed point, after `fold` and `ports`.
//! `Options::decide` turns it off (no `-f` flag); nothing runs
//! at -O0. `jumpthreading` and `dfajumpthreading` are the separate passes that
//! thread edges.
//!
//! # ScalarEvolution
//! None. A branch only a loop's trip count settles is `indvars`'s
//! (`eliminateIVComparison`, on the loops an unroll made too: `indvars@753`),
//! as in LLVM; the `guards` engine answers what it can from the guards over
//! the branch (the partition index: not an add recurrence).
//!
//! # Needs
//! `consts`, `constant_cycles`, LazyValueInfo (a branch the dominating
//! branches, assumes and `!range`s settle), `ranges` (the counted loops' facts,
//! until indvars decides them as LLVM does), `guards`, `induction`, points-to
//! (`pointers`), the CFG shape.
//!
//! # Keeps
//! Blocks and edges change: every analysis is invalidated when anything was
//! decided or merged.
//!
//! # See also
//! [`crate::cfg`], [`crate::fold`], [`crate::jumpthreading`]; LLVM
//! JumpThreading, SimplifyCFG.

use std::collections::BTreeSet;

use llrm_analysis::alias;
use llrm_analysis::cfg;
use llrm_analysis::constant_cycles::{self, State};
use llrm_analysis::consts::{self, Known, masked};
use llrm_analysis::guards;
use llrm_analysis::induction::{self, Scev};
use llrm_analysis::manager::Held;
use llrm_analysis::memory::Unit;
use llrm_analysis::num::Num as BigInt;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::footprint::Footprint;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Outer, PreservedAnalyses};
use llrm_support::hash::IndexMap;

use crate::transform;

/// `decided`, then its chains merged, as the old Decide.
pub struct Decide {
    /// Branches on known numbers, switches and non-null pointers are decided
    /// too (`consts`, `constant_cycles`). Off in LLVM's order: SimplifyCFG,
    /// CVP and SCCP fold those there, and nothing in the rows needs them here.
    pub constants: bool,
}

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
        let decided = _decided(unit.context, unit.layout, unit.function, analyses, self.constants)
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
    _decided(context, layout, function, &mut Analyses::new(std::rc::Rc::new(outer.clone())), true)
}

/// `decided`, `analyses` holding what is known of `function`.
fn _decided(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    constants: bool,
) -> Result<bool, String> {
    // What the values say, as LLVM's SimplifyCFG decides a branch: a number
    // memory gave is `fold`'s to put in the condition first. Solving memory
    // here decided none of 574 branches at d_faces -O2 and was a quarter of
    // the pass.
    let registers = constants.then(|| analyses.get::<llrm_analysis::manager::Registers>(context, layout, function));
    let held = Held::of_without_memory(context, layout, function, analyses);
    let outer = std::rc::Rc::clone(analyses.outer());
    let unit = held.unit(context, layout, function, &outer);
    let decisions = match registers.as_deref() {
        Some(facts) => _decisions_known(&unit, facts)?,
        None => _decisions_implied(&unit),
    };
    if decisions.is_empty() {
        return Ok(false);
    }
    for (block, target) in decisions {
        let last = function.terminator(block).expect("a terminator");
        _jump(context, function, last, target);
    }
    cfg::_unreachable(context, function);
    transform::_trivial_phis(function)?;
    Ok(true)
}

/// Each block whose terminator's way is known, with that way: only what the
/// guards prove of a branch's compare.
fn _decisions_implied(unit: &Unit) -> Vec<(BlockId, BlockId)> {
    let function = unit.function;
    let mut out = Vec::new();
    for &block in function.layout() {
        let Some(last) = function.terminator(block) else { continue };
        let [_, Operand::Block(taken), Operand::Block(other)] = function.instruction(last).operands[..] else {
            continue;
        };
        let answer = _implied_by_guards(unit, block, last);
        out.extend(answer.map(|answer| (block, if answer { taken } else { other })));
    }
    out
}

/// Each block whose terminator's way is known, with that way, from the numbers
/// the values and the cycles of the constants say.
fn _decisions_known(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
) -> Result<Vec<(BlockId, BlockId)>, String> {
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
    let mut out = Vec::new();
    for &block in function.layout() {
        let Some(last) = function.terminator(block) else {
            continue;
        };
        let instruction = function.instruction(last);
        if instruction.opcode == Opcode::Switch {
            out.extend(_switch_target(unit, last, &facts).map(|target| (block, target)));
            continue;
        }
        let [_, Operand::Block(taken), Operand::Block(other)] = instruction.operands[..] else {
            continue;
        };
        let answer = _outcome(unit, block, &facts, Some(&nonnull)).or_else(|| _implied_by_guards(unit, block, last));
        out.extend(answer.map(|answer| (block, if answer { taken } else { other })));
    }
    Ok(out)
}

/// Whether the compare `last` branches on holds or fails there, by what the
/// guards over `block` and the loops holding it prove of its two sides.
fn _implied_by_guards(
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

/// `last`, a terminator, replaced by a jump to `target`. `br i1 %c, T, T` has
/// two edges to `T` and a phi of `T` two inputs from the block: a jump has one
/// edge, so the phi loses one.
fn _jump(
    context: &Context,
    function: &mut Function,
    last: InstId,
    target: BlockId,
) {
    let block = function.parent(last).expect("a placed terminator");
    let edges = function.instruction(last).operands.iter().filter(|one| **one == Operand::Block(target)).count();
    for _ in 1..edges {
        function.predecessor_removed(context, target, block, true);
    }
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

/// The blocks that hold a loop's simple form up: a jump alone in the block
/// before the header (its one entry), the one latch, or an exit block only the
/// loop reaches. A pass that takes them out meets `loopsimplify` putting them
/// back, round after round.
pub(crate) fn loop_form_blocks(
    function: &Function,
    loops: Option<Vec<llrm_analysis::graph::loops::Loop>>,
) -> BTreeSet<i64> {
    loop_form_blocks_of(loops.unwrap_or_else(|| cfg::loops_of(function)), function)
}

/// Each loop's blocks, asked one at a time (`Around`, `successors_of`,
/// `predecessors_of`): a whole-body graph and predecessor map per ask cost
/// 88 K Ir a function in d_faces -O2, 2.9 M over 33 asks. A block's successors
/// are read once for all the loops that hold it, and an exit once for each
/// loop (`Around::exits` read them again for every nested loop: 1.5 M Ir).
fn loop_form_blocks_of(
    all_loops: Vec<llrm_analysis::graph::loops::Loop>,
    function: &Function,
) -> BTreeSet<i64> {
    use llrm_analysis::cfg::{Around, predecessors_of, successors_of};
    let mut successors: llrm_support::hash::IndexMap<i64, llrm_analysis::cfg::Successors> = Default::default();
    let mut loop_edges = BTreeSet::new();
    for loop_ in all_loops {
        if let Some(parent) = loop_.preheader(function) {
            loop_edges.insert(parent);
        }
        if let Some(&latch) = loop_.latches.iter().next().filter(|_| loop_.latches.len() == 1)
            && successors_of(function, latch)[..] == [loop_.header]
        {
            loop_edges.insert(latch);
        }
        // A successor of the body whose predecessors are all in it.
        let mut seen = BTreeSet::new();
        for &from in &loop_.body {
            let after_from = successors.entry(from).or_insert_with(|| successors_of(function, from)).clone();
            for exit in after_from {
                if loop_.body.contains(&exit) || !seen.insert(exit) {
                    continue;
                }
                let after = successors.entry(exit).or_insert_with(|| successors_of(function, exit));
                if after.len() == 1
                    && !loop_.body.contains(&after[0])
                    && predecessors_of(function, exit).iter().all(|at| loop_.body.contains(at))
                {
                    loop_edges.insert(exit);
                }
            }
        }
    }
    loop_edges
}

#[cfg(test)]
#[path = "decide_tests.rs"]
mod tests;
