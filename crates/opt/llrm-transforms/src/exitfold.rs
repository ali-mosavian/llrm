//! Exits their counts decide, folded: LLVM IndVarSimplify's
//! `optimizeLoopExits`. Of a loop's exits the latch follows, each counted
//! (`induction::exits`), in dominance order:
//!
//! - one whose count an earlier exit shares never leaves: the earlier one
//!   leaves on that trip first;
//! - one whose count is above the loop's most, the least of the others',
//!   as the constants or the guards on entry prove, never leaves;
//! - one whose count is zero leaves on the first trip.
//!
//! Its branch is given the condition that decides it; `decide` makes it a
//! jump. Nothing else changes, so no effect or value between the exits
//! matters.
//!
//! An exit no count decides, whose compare tests a counter stepping by one
//! against an invariant, is tested once instead, before the loop, where
//! the guards prove it holds on every trip up to the loop's most once it
//! holds on the first: LLVM's `optimizeLoopExitWithUnknownExitCount`. The
//! test is of the counter's start; failing it, the loop leaves on the first
//! trip, as it did. That test is itself a fact the proof may use, with the
//! ranges the program states (`guards::holds_given`).
//!
//! Exits that leave to the same place with the same values, nothing seen
//! or trapping between them, become one: the first leaves on the trip the
//! least of all their counts ends, testing its counter against its value
//! then, and the others never leave. Whichever would have left first, the
//! first leaves on that trip, earlier in it, to the same place: nothing in
//! between could be seen. Neither LLVM nor GCC merges live exits; this is
//! what lets a loop bounded by several lengths count one.
//!
//! A loop that writes nothing, or only plain stores where each exit it
//! leaves by early crashes at once touching no memory the program sees,
//! and whose exits carry no values out, leaves on its first trip through
//! the exit it would have left by: each exit, in dominance order, leaves
//! where its count is the loop's. LLVM's `predicateLoopExits`, with its
//! traps.

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, AffineOperand, ExitCount, Scev};
use llrm_analysis::{cfg, guards, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Outer;
use num_bigint::BigInt;

use crate::counting;
use crate::expand::{self, Expander};

/// How an exit's branch is decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decided {
    Stays,
    Leaves,
}

/// A compare an exit branches on, tested once before its loop: `left
/// predicate right`, where the branch stays while it holds.
struct Hoisted {
    branch: InstId,
    exit: i64,
    predicate: IntPredicate,
    left: AffineOperand,
    right: Operand,
    before: InstId,
    preheader: BlockId,
    /// The branch can be taken in the preheader: the first trip reaches it and nothing seen runs on the way.
    early: bool,
}

/// Every loop's exits their counts decide, folded, and those tested once
/// hoisted; whether any was.
pub fn folded(context: &mut Context, layout: &DataLayout, function: &mut Function, outer: &Outer) -> bool {
    let (decided, hoisted, predicated, merged) = {
        let unit = memory::Unit::within(context, layout, function, outer);
        let facts = unit.registers();
        let loops = unit.shape().loops.clone();
        let mut decided = Vec::new();
        let mut hoisted = Vec::new();
        let mut predicated = Vec::new();
        let mut merged = Vec::new();
        for loop_ in &loops {
            let exits = induction::exits(&unit, loop_, Some(&facts), false);
            let folding = _decided(&unit, loop_, &exits);
            let lifting = _hoisted(&unit, outer, loop_, &exits, &folding);
            // One rewrite of a loop's exits a round: each reads them as they were.
            if folding.is_empty() && lifting.is_empty() {
                match _merged(&unit, outer, loop_, &exits) {
                    Some(merge) => merged.push(merge),
                    None => predicated.extend(_predicated(&unit, outer, loop_, &exits)),
                }
            }
            decided.extend(folding);
            hoisted.extend(lifting);
        }
        (decided, hoisted, predicated, merged)
    };
    for one in &merged {
        let mut expander = Expander::new(one.before);
        let trips = expander.least(context, function, &one.counts);
        let width = one.counts[0].width;
        let at = Position::Before(one.before);
        let ty = context.types.int(width);
        let magnitude = BigInt::from(one.step.magnitude().clone());
        let walked = expand::scaled(context, function, trips, &magnitude, width, at);
        let start = expander.int(context, function, &one.start);
        let kind = if one.step > BigInt::from(0) { BinaryOp::Add } else { BinaryOp::Sub };
        let end = expand::placed(context, function, Opcode::Binary(kind), ty, vec![start, walked], at);
        let stays_on_true = !matches!(function.instruction(one.first).operands[..], [_, Operand::Block(yes), _] if cfg::id(yes) == one.exit);
        let predicate = if stays_on_true { IntPredicate::Ne } else { IntPredicate::Eq };
        let bit = context.types.int(1);
        let test = function.create_instruction(Opcode::ICmp(predicate), bit, vec![Operand::Value(one.counter), end], Flags::default(), None);
        function.insert(test, Position::Before(one.first)).expect("the first exit's branch");
        _replaced(function, one.first, Operand::Value(function.instruction(test).result.expect("a value")));
        for &(branch, exit) in &one.others {
            let stays_on_true = !matches!(function.instruction(branch).operands[..], [_, Operand::Block(yes), _] if cfg::id(yes) == exit);
            _replaced(function, branch, counting::constant(context, &BigInt::from(u8::from(stays_on_true)), 1));
        }
    }
    for one in &predicated {
        // Each exit leaves where its count is the loop's, tested once, in the
        // preheader, in the order the loop tests them; the loop never leaves by them.
        let loop_count = Expander::new(function.terminator(one.preheader).expect("a preheader's branch")).least(context, function, &one.loop_count);
        let mut current = one.preheader;
        for (branch, exit, count) in &one.exits {
            let terminator = function.terminator(current).expect("a block's branch");
            let condition = if *count == one.loop_count {
                counting::constant(context, &BigInt::from(1), 1)
            } else {
                let own = Expander::new(terminator).least(context, function, count);
                let bit = context.types.int(1);
                expand::placed(context, function, Opcode::ICmp(IntPredicate::Eq), bit, vec![own, loop_count], Position::Before(terminator))
            };
            let next = function.create_block(None);
            function.insert_block(next, Some(current)).expect("a block after the preheader");
            let void = function.instruction(terminator).ty;
            let onward = function.create_instruction(Opcode::Br, void, vec![Operand::Block(one.header)], Flags::default(), None);
            function.insert(onward, Position::End(next)).expect("a new block");
            function.erase(terminator).expect("a branch");
            let leaving = function.create_instruction(Opcode::Br, void, vec![condition, Operand::Block(cfg::block(*exit)), Operand::Block(next)], Flags::default(), None);
            function.insert(leaving, Position::End(current)).expect("a block");
            for phi in crate::edges::phis(function, one.header) {
                let operands = function.instruction(phi).operands.iter().map(|&arm| if arm == Operand::Block(current) { Operand::Block(next) } else { arm }).collect();
                function.set_operands(phi, operands);
            }
            current = next;
            let stays_on_true = !matches!(function.instruction(*branch).operands[..], [_, Operand::Block(yes), _] if cfg::id(yes) == *exit);
            _replaced(function, *branch, counting::constant(context, &BigInt::from(u8::from(stays_on_true)), 1));
        }
    }
    for one in &hoisted {
        let stays_on_true = !matches!(function.instruction(one.branch).operands[..], [_, Operand::Block(yes), _] if cfg::id(yes) == one.exit);
        let predicate = if stays_on_true { one.predicate } else { one.predicate.inverse() };
        let left = match &one.left {
            AffineOperand::Value(value, _) => Operand::Value(*value),
            AffineOperand::Const(known) => counting::constant(context, &known.n, known.width),
        };
        let bit = context.types.int(1);
        let test = function.create_instruction(Opcode::ICmp(predicate), bit, vec![left, one.right], Flags::default(), None);
        function.insert(test, Position::Before(one.before)).expect("a preheader's branch");
        let condition = Operand::Value(function.instruction(test).result.expect("a value"));
        if !one.early {
            _replaced(function, one.branch, condition);
            continue;
        }
        // Tested once, in the preheader: the way out, or on to the loop, which then never leaves here.
        let preheader = one.preheader;
        let header = match function.instruction(one.before).operands[..] {
            [Operand::Block(header)] => header,
            _ => unreachable!("a preheader branches to its header"),
        };
        let next = function.create_block(None);
        function.insert_block(next, Some(preheader)).expect("a block after the preheader");
        let void = function.instruction(one.before).ty;
        let onward = function.create_instruction(Opcode::Br, void, vec![Operand::Block(header)], Flags::default(), None);
        function.insert(onward, Position::End(next)).expect("a new block");
        function.erase(one.before).expect("a branch");
        let (on, off) = if stays_on_true { (next, cfg::block(one.exit)) } else { (cfg::block(one.exit), next) };
        let leaving = function.create_instruction(Opcode::Br, void, vec![condition, Operand::Block(on), Operand::Block(off)], Flags::default(), None);
        function.insert(leaving, Position::End(preheader)).expect("a block");
        for phi in crate::edges::phis(function, header) {
            let operands = function.instruction(phi).operands.iter().map(|&arm| if arm == Operand::Block(preheader) { Operand::Block(next) } else { arm }).collect();
            function.set_operands(phi, operands);
        }
        _replaced(function, one.branch, counting::constant(context, &BigInt::from(u8::from(stays_on_true)), 1));
    }
    for &(branch, exit, way) in &decided {
        let stays_on_true = !matches!(function.instruction(branch).operands[..], [_, Operand::Block(yes), _] if cfg::id(yes) == exit);
        let holds = (way == Decided::Stays) == stays_on_true;
        let condition = counting::constant(context, &BigInt::from(u8::from(holds)), 1);
        function.set_operand(branch, 0, condition);
    }
    !decided.is_empty() || !hoisted.is_empty() || !predicated.is_empty() || !merged.is_empty()
}

/// Exits made one: the first, testing `counter` against `start + step *
/// least(counts)`, and the others, never leaving.
struct Merged {
    before: InstId,
    first: InstId,
    exit: i64,
    counter: ValueId,
    start: Scev,
    step: BigInt,
    counts: Vec<Scev>,
    others: Vec<(InstId, i64)>,
}

/// The exits of `loop_` that may become one with its first counted exit.
fn _merged(unit: &memory::Unit, outer: &Outer, loop_: &Loop, exits: &[ExitCount]) -> Option<Merged> {
    let function = unit.function;
    let header = cfg::block(loop_.header);
    let outside = function.predecessors(header).into_iter().filter(|&one| !loop_.body.contains(&cfg::id(one))).collect::<Vec<_>>();
    let [preheader] = outside[..] else { return None };
    let before = function.terminator(preheader)?;
    let innermost = |at: i64| unit.shape().loops.iter().filter(|one| one.body.contains(&at)).all(|one| one.body.len() >= loop_.body.len());
    let single = |exit: &ExitCount| match (&exit.taken, &exit.proofs[..]) {
        (Some(taken), [proof]) if taken.len() == 1 && innermost(exit.block) => Some(proof.clone()),
        _ => None,
    };
    let (first_at, first) = exits.iter().enumerate().find(|(_, one)| single(one).is_some())?;
    let proof = single(first)?;
    // The counter tested as the trip begins, stepping without wrapping up to its count.
    if proof.stepped || proof.posttested || proof.test == IntPredicate::Ne {
        return None;
    }
    let width = proof.width();
    let leaving = _leaving(function, first.exit, first.block)?;
    let mut counts = first.taken.clone()?;
    let mut others = Vec::new();
    for other in &exits[first_at + 1..] {
        let Some(taken) = other.taken.clone().filter(|taken| taken.len() == 1 && taken[0].width == width && innermost(other.block)) else { continue };
        if _leaving(function, other.exit, other.block).as_ref() != Some(&leaving) || !_quiet(unit, outer, loop_, first.block, other.block) {
            continue;
        }
        counts.extend(taken);
        others.push((other.branch, other.exit));
    }
    if others.is_empty() {
        return None;
    }
    Some(Merged {
        before,
        first: first.branch,
        exit: first.exit,
        counter: proof.counter.value,
        start: Scev::of(&proof.start, width),
        step: proof.step.clone(),
        counts,
        others,
    })
}

/// Where leaving `from` for `exit` goes, and with what: the block the
/// values arrive at, and each of its phis' value. An exit block of phis
/// only its jump reads, as LCSSA makes, is passed through.
fn _leaving(function: &Function, exit: i64, from: i64) -> Option<(i64, Vec<(InstId, Operand)>)> {
    let block = cfg::block(exit);
    let body = function.block(block).instructions();
    let (phis, rest): (Vec<InstId>, Vec<InstId>) = body.iter().copied().partition(|&inst| function.instruction(inst).opcode == Opcode::Phi);
    let arriving = |phi: InstId, from: llrm_mir::module::BlockId| function.instruction(phi).operands.chunks(2).find(|pair| pair[1] == Operand::Block(from)).map(|pair| pair[0]);
    let own = |to: llrm_mir::module::BlockId, through: llrm_mir::module::BlockId, map: &dyn Fn(Operand) -> Option<Operand>| -> Option<Vec<(InstId, Operand)>> {
        function.block(to).instructions().iter().copied().filter(|&inst| function.instruction(inst).opcode == Opcode::Phi).map(|phi| Some((phi, map(arriving(phi, through)?)?))).collect()
    };
    let jump = match rest[..] {
        [only] => match function.instruction(only).operands[..] {
            [Operand::Block(to)] => Some(to),
            _ => None,
        },
        _ => None,
    };
    let passed = jump.filter(|_| {
        phis.iter().all(|&phi| {
            function.instruction(phi).operands.len() == 2
                && function.instruction(phi).result.is_some_and(|value| function.users(value).iter().all(|one| function.parent(one.user) == jump))
        })
    });
    match passed {
        Some(to) => {
            let through = |operand: Operand| match operand {
                Operand::Value(value) => match phis.iter().find(|&&phi| function.instruction(phi).result == Some(value)) {
                    Some(&phi) => arriving(phi, cfg::block(from)),
                    None => Some(operand),
                },
                other => Some(other),
            };
            Some((cfg::id(to), own(to, block, &through)?))
        }
        None => Some((exit, own(block, cfg::block(from), &Some)?)),
    }
}

/// Whether block `to` is reached from `from` inside the loop short of its header.
fn _reaches(function: &Function, loop_: &Loop, from: i64, to: i64) -> bool {
    let mut seen = std::collections::BTreeSet::from([from]);
    let mut pending = vec![from];
    while let Some(at) = pending.pop() {
        if at == to {
            return true;
        }
        for next in function.successors(cfg::block(at)) {
            let next = cfg::id(next);
            if loop_.body.contains(&next) && next != loop_.header && seen.insert(next) {
                pending.push(next);
            }
        }
    }
    false
}

/// Whether nothing seen or trapping runs in block `at` before its branch.
fn _silent(unit: &memory::Unit, outer: &Outer, at: i64) -> bool {
    let function = unit.function;
    let body = function.block(cfg::block(at)).instructions();
    body[..body.len().saturating_sub(1)].iter().all(|&inst| {
        let op = function.instruction(inst);
        match op.opcode {
            Opcode::Load { volatile: false, align } => {
                let width = op.result.and_then(|value| unit.int_bits(Operand::Value(value))).map_or(8, |bits| u64::from(bits.div_ceil(8)));
                !outer.target().load_may_trap(width, align.map_or(1, u64::from))
            }
            Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) => false,
            _ => llrm_mir::memory::only_value(unit.context, outer.callees(), function, inst),
        }
    })
}

/// Whether nothing seen or trapping runs between the tests of blocks
/// `first` and `later` on a trip: on every way from one to the other
/// short of the header.
fn _quiet(unit: &memory::Unit, outer: &Outer, loop_: &Loop, first: i64, later: i64) -> bool {
    let function = unit.function;
    let between = loop_.body.iter().copied().filter(|&at| at != first && _reaches(function, loop_, first, at) && _reaches(function, loop_, at, later));
    between.chain([later]).all(|at| _silent(unit, outer, at))
}

/// `branch` on `condition`, its old compare gone where nothing else read it.
fn _replaced(function: &mut Function, branch: InstId, condition: Operand) {
    let replaced = function.instruction(branch).operands[0];
    function.set_operand(branch, 0, condition);
    if let Operand::Value(old) = replaced
        && function.users(old).is_empty()
        && let ValueDef::Instruction(inst) = function.value(old).def
    {
        function.erase(inst).expect("an unused compare");
    }
}

/// A loop's exits, each leaving where its count is the loop's.
struct Predicated {
    preheader: BlockId,
    header: BlockId,
    loop_count: Vec<Scev>,
    exits: Vec<(InstId, i64, Vec<Scev>)>,
}

/// The exits of `loop_` that may leave on its first trip, where it writes
/// nothing, or only plain stores that each of them crashing hides.
fn _predicated(unit: &memory::Unit, outer: &Outer, loop_: &Loop, exits: &[ExitCount]) -> Option<Predicated> {
    let function = unit.function;
    let loop_count = induction::backedges(exits)?;
    let width = loop_count[0].width;
    let header = cfg::block(loop_.header);
    let outside = function.predecessors(header).into_iter().filter(|&one| !loop_.body.contains(&cfg::id(one))).collect::<Vec<_>>();
    let [preheader] = outside[..] else { return None };
    function.terminator(preheader)?;
    let innermost = |at: i64| unit.shape().loops.iter().filter(|one| one.body.contains(&at)).all(|one| one.body.len() >= loop_.body.len());
    // Up to the first exit that cannot be: a later one may not take its trip.
    let mut chosen = Vec::new();
    for exit in exits {
        let condition = function.instruction(exit.branch).operands.first().copied();
        let phis = function.block(cfg::block(exit.exit)).instructions().iter().any(|&inst| function.instruction(inst).opcode == Opcode::Phi);
        let Some(count) = exit.taken.clone().filter(|count| count.iter().all(|one| one.width == width)) else { break };
        if !innermost(exit.block) || matches!(condition, Some(Operand::Constant(_))) || phis {
            break;
        }
        chosen.push((exit.branch, exit.exit, count));
    }
    if chosen.is_empty() {
        return None;
    }
    // Only plain stores, and only where every chosen exit crashes quietly.
    let mut stores = false;
    for &at in &loop_.body {
        for &inst in function.block(cfg::block(at)).instructions() {
            let op = function.instruction(inst);
            let effects = llrm_mir::memory::of(unit.context, outer.callees(), function, inst);
            match op.opcode {
                Opcode::Store { volatile: false, .. } => stores = true,
                Opcode::Call(_) | Opcode::Invoke(_) if effects.writes || !llrm_mir::memory::call_returns(unit.context, outer.callees(), function, inst) => return None,
                Opcode::Load { volatile: true, .. } | Opcode::Store { .. } => return None,
                _ => {}
            }
        }
    }
    // Stores stay unseen only up to the first exit that does not crash.
    if stores {
        let quiet = chosen.iter().take_while(|&&(branch, _, _)| _crashes(unit, outer, branch)).count();
        chosen.truncate(quiet);
    }
    (!chosen.is_empty()).then_some(Predicated { preheader, header, loop_count, exits: chosen })
}

/// Whether one way out of `branch` crashes at once, touching no memory the
/// program sees: calls touching none it can name, then `unreachable`.
fn _crashes(unit: &memory::Unit, outer: &Outer, branch: InstId) -> bool {
    let function = unit.function;
    function.instruction(branch).operands[1..].iter().any(|to| {
        let Operand::Block(to) = *to else { return false };
        let body = function.block(to).instructions();
        body.last().is_some_and(|&last| function.instruction(last).opcode == Opcode::Unreachable)
            && body[..body.len() - 1].iter().all(|&inst| {
                matches!(function.instruction(inst).opcode, Opcode::Call(_)) && llrm_mir::memory::accessible(unit.context, outer.callees(), function, inst) == llrm_mir::memory::Effects::NONE
            })
    })
}

/// The compare `exit`'s branch tests, as `counter predicate invariant` where the loop stays.
fn _tested(unit: &memory::Unit, exit: &ExitCount, counters: &llrm_support::hash::IndexMap<ValueId, induction::Affine>, still: &induction::Invariant) -> Option<(ValueId, Operand, IntPredicate)> {
    let function = unit.function;
    let [Operand::Value(condition), Operand::Block(yes), _] = function.instruction(exit.branch).operands[..] else { return None };
    let (_, compare) = unit.defining(Operand::Value(condition))?;
    let (Opcode::ICmp(predicate), [one, other]) = (&compare.opcode, &compare.operands[..]) else { return None };
    if function.users(condition).len() != 1 || matches!(predicate, IntPredicate::Eq | IntPredicate::Ne) {
        return None;
    }
    let stays = cfg::id(yes) != exit.exit;
    let continuing = if stays { *predicate } else { predicate.inverse() };
    // The counter on the left, the invariant on the right.
    let (counter, right, predicate) = match (*one, *other) {
        (Operand::Value(value), right) if counters.contains_key(&value) => (value, right, continuing),
        (left, Operand::Value(value)) if counters.contains_key(&value) => (value, left, continuing.swapped()),
        _ => return None,
    };
    if matches!(right, Operand::Value(value) if !still.contains(value)) {
        return None;
    }
    Some((counter, right, predicate))
}

/// Whether the first trip reaches `exit`'s branch, nothing seen or trapping
/// on the way, so a test of it made before the loop fails where the branch
/// would on that trip: each exit on the way passes on it, as the guards on
/// entry prove, and the way out has no phis to carry values from the loop.
fn _early(unit: &memory::Unit, outer: &Outer, loop_: &Loop, exits: &[ExitCount], exit: &ExitCount, counters: &llrm_support::hash::IndexMap<ValueId, induction::Affine>, still: &induction::Invariant, preheader: BlockId) -> bool {
    let function = unit.function;
    let header = loop_.header;
    let innermost = |at: i64| unit.shape().loops.iter().filter(|one| one.body.contains(&at)).all(|one| one.body.len() >= loop_.body.len());
    if !innermost(exit.block) || function.block(cfg::block(exit.exit)).instructions().iter().any(|&inst| function.instruction(inst).opcode == Opcode::Phi) {
        return false;
    }
    if !_silent(unit, outer, header) || !_quiet(unit, outer, loop_, header, exit.block) {
        return false;
    }
    let way = loop_.body.iter().copied().filter(|&at| at != exit.block && (at == header || _reaches(function, loop_, header, at)) && _reaches(function, loop_, at, exit.block));
    for at in way.collect::<Vec<_>>() {
        if function.successors(cfg::block(at)).iter().all(|&to| loop_.body.contains(&cfg::id(to))) {
            continue;
        }
        // An exiting block on the way: it passes on the first trip.
        let Some(earlier) = exits.iter().find(|one| one.block == at) else { return false };
        let Some((counter, right, predicate)) = _tested(unit, earlier, counters, still) else { return false };
        let affine = &counters[&counter];
        let (Some(width), Some(term)) = (unit.int_bits(Operand::Value(counter)), induction::term(unit, right)) else { return false };
        if !guards::holds(unit, cfg::id(preheader), predicate, &Scev::of(&affine.start, width), &Scev::of(&term, width)) {
            return false;
        }
    }
    true
}

/// The exits of `loop_` that no count decides, and that one test before it may.
fn _hoisted(unit: &memory::Unit, outer: &Outer, loop_: &Loop, exits: &[ExitCount], decided: &[(InstId, i64, Decided)]) -> Vec<Hoisted> {
    let function = unit.function;
    let shape = unit.shape();
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return Vec::new() };
    let header = cfg::block(loop_.header);
    let outside = function.predecessors(header).into_iter().filter(|&one| !loop_.body.contains(&cfg::id(one))).collect::<Vec<_>>();
    let [preheader] = outside[..] else { return Vec::new() };
    let Some(before) = function.terminator(preheader) else { return Vec::new() };
    let counters = induction::basics(unit, loop_);
    let still = induction::invariant(function, &loop_.body);
    let most = induction::most_backedges(exits);
    let mut found = Vec::new();
    let mut earlier: Vec<Scev> = Vec::new();
    for exit in exits {
        // A counted exit nothing decided is tested once like an uncounted one.
        let taken = exit.taken.iter().flatten().cloned().collect::<Vec<_>>();
        if exit.taken.is_some() && decided.iter().any(|one| one.0 == exit.branch) {
            earlier.extend(taken);
            continue;
        }
        'candidate: {
            if !shape.dominance.dominates(exit.block, latch) {
                break 'candidate;
            }
            let Some((counter, right, predicate)) = _tested(unit, exit, &counters, &still) else { break 'candidate };
            let affine = &counters[&counter];
            let (Some(width), Some(right_term)) = (unit.int_bits(Operand::Value(counter)), induction::term(unit, right)) else { break 'candidate };
            let step = Scev::of(&affine.step, width);
            let Some(by) = step.known().filter(|by| by.magnitude() == &num_bigint::BigUint::from(1_u8)) else { break 'candidate };
            let start = Scev::of(&affine.start, width);
            let bound = Scev::of(&right_term, width);
            let signed = matches!(predicate, IntPredicate::Slt | IntPredicate::Sle | IntPredicate::Sgt | IntPredicate::Sge);
            let rising = by > BigInt::from(0);
            let no_wrap = match (signed, rising) {
                (true, true) => IntPredicate::Sle,
                (true, false) => IntPredicate::Sge,
                (false, true) => IntPredicate::Ule,
                (false, false) => IntPredicate::Uge,
            };
            // Taken only where the start passes its test, as it does on the first trip, so the test is a fact in the proof.
            let first = [guards::Guard { predicate, left: start.clone(), right: bound.clone() }];
            let holds_up_to = |trips: &Scev| {
                let last = start.plus(&trips.times(&by));
                guards::holds_given(unit, latch, &first, predicate, &last, &bound) && guards::holds_given(unit, exit.block, &first, no_wrap, &start, &last)
            };
            // An earlier exit leaving on the loop's last trip spares the later ones it.
            let invariant = most.iter().filter(|one| one.width == width).any(|trips| holds_up_to(trips) || (earlier.contains(trips) && holds_up_to(&trips.minus(&Scev::constant(1, width)))));
            if invariant {
                let early = matches!(function.instruction(before).operands[..], [Operand::Block(_)]) && _early(unit, outer, loop_, exits, exit, &counters, &still, preheader);
                found.push(Hoisted { branch: exit.branch, exit: exit.exit, predicate, left: affine.start.clone(), right, before, preheader, early });
            }
        }
        earlier.extend(taken);
    }
    found
}

/// The branches of `loop_`'s exits their counts decide, and their exits.
fn _decided(unit: &memory::Unit, loop_: &Loop, exits: &[ExitCount]) -> Vec<(InstId, i64, Decided)> {
    let function = unit.function;
    // Only an exit of this loop and no inner one decides this loop's trips.
    let innermost = |at: i64| unit.shape().loops.iter().filter(|one| one.body.contains(&at)).all(|one| one.body.len() >= loop_.body.len());
    let mut decided = Vec::new();
    let mut earlier: Vec<Vec<Scev>> = Vec::new();
    for (index, exit) in exits.iter().enumerate() {
        let Some(taken) = &exit.taken else { continue };
        let condition = function.instruction(exit.branch).operands.first().copied();
        if !innermost(exit.block) || matches!(condition, Some(Operand::Constant(_))) {
            earlier.push(taken.clone());
            continue;
        }
        let others = exits.iter().enumerate().filter(|(at, _)| *at != index).filter_map(|(_, one)| one.taken.clone()).flatten().collect::<Vec<_>>();
        let way = if taken.iter().any(|one| one.known() == Some(BigInt::from(0))) {
            Some(Decided::Leaves)
        } else if earlier.iter().any(|one| one == taken) {
            Some(Decided::Stays)
        } else if others.iter().any(|most| taken.iter().all(|count| _below(unit, loop_.header, most, count))) {
            Some(Decided::Stays)
        } else {
            None
        };
        if let Some(way) = way {
            decided.push((exit.branch, exit.exit, way));
        }
        earlier.push(taken.clone());
    }
    decided
}

/// Whether `most` is below `count` as unsigned numbers on entry to `header`'s loop.
fn _below(unit: &memory::Unit, header: i64, most: &Scev, count: &Scev) -> bool {
    if most.width != count.width {
        return false;
    }
    if let (Some(one), Some(other)) = (most.known(), count.known()) {
        return guards::evaluated(IntPredicate::Ult, &one, &other, most.width);
    }
    guards::holds(unit, header, IntPredicate::Ult, most, count)
}

#[cfg(test)]
#[path = "exitfold_tests.rs"]
mod tests;
