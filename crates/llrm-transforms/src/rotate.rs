//! Enter a loop proven to run at least once at its body, not at its test:
//! llrm-core's `optimize/rotate.rs`, the port of `qbopt/optimize/rotate.py`,
//! adapted to the rich MIR.
//! LLVM: LoopRotate, entering at the body where the first test is proven to pass.
//!
//! BC writes `FOR` as `jmp test; body: ...; test: cmp; jle body`. Where the
//! first test is proven to pass the entry goes straight to the body, and the
//! test is then reached only from the latch, which it merges into. A dead
//! counter with a symbolic count becomes a guarded countdown first: a skip
//! test before the loop, then trips remaining stepped down to zero.
//!
//! A rotated loop is no longer the pre-tested shape the counted-loop
//! analyses, peel and unroll read, so this goes after them.
//!
//! What changed with the IR: the header's phis move to the body through
//! `ssa::SsaUpdater`, which also rewrites every use, outside the loop
//! included; the countdown's seeds, guard and exit values are `counting`'s.
//! Dropped: `_step_test` and the countdown's branch on the decrement's
//! flags -- an `icmp` is a value, and reusing a step's flags is isel's; the
//! old body's `loop_trip_counts` and `integer_ranges` side tables; clearing
//! a private start (Dead's).

use std::collections::BTreeSet;

use llrm_analysis::induction::{self, ControlReplacement};
use llrm_analysis::manager::Registers;
use llrm_analysis::ssa::SsaUpdater;
use llrm_analysis::{cfg, memory};
use llrm_graph::loops::{self, Loop};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use num_bigint::BigInt;

use crate::counting::{self, Seeds};
use crate::lcssa::{arms, from_arms};

pub struct Rotate;

impl FunctionPass for Rotate {
    fn name(&self) -> &'static str {
        "rotate"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        match entered(unit.context, unit.layout, unit.function, analyses) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("rotate: {error}"),
        }
    }
}

/// Every dead counter with a symbolic count counted down, every proven
/// loop entered at its body, and each test merged into its latch; whether
/// any loop was.
pub fn entered(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &Analyses) -> Result<bool, String> {
    let mut done = BTreeSet::new();
    while _counted_down(context, layout, function, analyses, &mut done)? {}
    while rotated(context, layout, function, analyses, &mut done)? {}
    if done.is_empty() {
        return Ok(false);
    }
    crate::cfg::merged(function);
    crate::canonical::identities(context, function);
    Ok(true)
}

/// A loop this can enter at its body: `preheader` jumps only to `header`,
/// which tests and branches to `first`, reached from nowhere else; the one
/// latch goes only back to `header`.
pub(crate) struct Shape {
    pub preheader: BlockId,
    pub header: BlockId,
    pub first: BlockId,
    pub latch: BlockId,
    pub exit: BlockId,
    /// The loop's blocks but its header: where a moved phi is read.
    pub body: BTreeSet<BlockId>,
}

fn _phis(function: &Function, block: BlockId) -> Vec<InstId> {
    function.block(block).instructions().iter().copied().filter(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect()
}

/// Whether an instruction may be skipped on entry: it computes, and nothing
/// outside `header` reads it.
fn _test_only(function: &Function, header: BlockId, inst: InstId) -> bool {
    let op = function.instruction(inst);
    let computes = match op.opcode {
        Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::Cast(_) | Opcode::GetElementPtr { .. } | Opcode::Select | Opcode::Freeze => true,
        Opcode::Binary(kind) => !matches!(kind, BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem),
        _ => false,
    };
    computes && op.result.is_none_or(|value| function.users(value).iter().all(|one| function.parent(one.user) == Some(header)))
}

pub(crate) fn _shape(function: &Function, loop_: &Loop) -> Option<Shape> {
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let succ = |at: i64| graph.iter().find(|block| block.at == at).map(|block| block.succ.clone()).unwrap_or_default();
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else {
        return None;
    };
    let [preheader] = predecessors[&loop_.header].difference(&loop_.body).copied().collect::<Vec<_>>()[..] else {
        return None;
    };
    let header = cfg::block(loop_.header);
    let terminator = |at: i64| function.terminator(cfg::block(at)).map(|last| function.instruction(last));
    if succ(preheader) != [loop_.header] || terminator(preheader).is_none_or(|last| last.opcode != Opcode::Br || last.operands.len() != 1) {
        return None;
    }
    if latch == loop_.header || succ(latch) != [loop_.header] {
        return None;
    }
    let branch = terminator(loop_.header)?;
    if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
        return None;
    }
    let (inside, outside): (Vec<i64>, Vec<i64>) = succ(loop_.header).into_iter().partition(|at| loop_.body.contains(at));
    let ([first], [exit]) = (&inside[..], &outside[..]) else {
        return None;
    };
    if predecessors[first] != BTreeSet::from([loop_.header]) || !_phis(function, cfg::block(*first)).is_empty() {
        return None;
    }
    let work = function.block(header).instructions().iter().copied().filter(|&inst| {
        let op = function.instruction(inst);
        op.opcode != Opcode::Phi && !op.opcode.is_terminator()
    });
    if !work.into_iter().all(|inst| _test_only(function, header, inst)) {
        return None;
    }
    let block = cfg::block;
    let body = loop_.body.iter().copied().filter(|&at| at != loop_.header).map(block).collect();
    Some(Shape { preheader: block(preheader), header, first: block(*first), latch: block(latch), exit: block(*exit), body })
}

/// `shape`'s loop entered at `first`, or, with `guard`, entered there only
/// where the guard is false and skipped to the exit where it is true.
pub(crate) fn _rotate(context: &mut Context, function: &mut Function, shape: &Shape, guard: Option<Operand>) -> Result<(), String> {
    let entering = function.terminator(shape.preheader).expect("a terminated preheader");
    if let Some(guard) = guard {
        // The exit is now also reached before the loop: each of its phis reads
        // there what it read from the header, which the updater below makes
        // the value from before the loop.
        for phi in _phis(function, shape.exit) {
            let mut incoming = arms(function, phi);
            if incoming.iter().any(|&(_, from)| from == shape.preheader) {
                continue;
            }
            let (value, _) = *incoming.iter().find(|&&(_, from)| from == shape.header).expect("an arm from the header");
            incoming.push((value, shape.preheader));
            function.set_operands(phi, from_arms(&incoming));
        }
        function.set_operands(entering, vec![guard, Operand::Block(shape.exit), Operand::Block(shape.first)]);
    } else {
        function.set_operands(entering, vec![Operand::Block(shape.first)]);
    }
    let phis = _phis(function, shape.header);
    let values = phis.iter().map(|&phi| function.instruction(phi).result.expect("a phi's value")).collect::<Vec<_>>();
    let side = |function: &Function, phi: InstId, from: BlockId| arms(function, phi).into_iter().find(|&(_, source)| source == from).expect("a preheader and a latch arm").0;
    let starts = phis.iter().map(|&phi| side(function, phi, shape.preheader)).collect::<Vec<_>>();
    let nexts = phis.iter().map(|&phi| side(function, phi, shape.latch)).collect::<Vec<_>>();
    for &phi in &phis {
        function.set_operands(phi, Vec::new());
    }
    // Each header phi `p(preheader: a, latch: b)` moves to `first` as
    // `p'(preheader: a, header: b')`, a `b` that is another header phi read
    // as its moved `b'`. Phis read in parallel, so a swap needs no order.
    let moved = values
        .iter()
        .map(|&value| {
            let phi = function.create_instruction(Opcode::Phi, function.value(value).ty, Vec::new(), Flags::default(), function.value(value).name.clone().as_deref());
            let top = function.block(shape.first).instructions().first().copied();
            function.insert(phi, top.map_or(Position::End(shape.first), Position::Before)).expect("a placed block");
            (phi, Operand::Value(function.instruction(phi).result.expect("a phi's value")))
        })
        .collect::<Vec<_>>();
    let latest = |read: Operand| match read {
        Operand::Value(value) => values.iter().position(|&one| one == value).map_or(read, |index| moved[index].1),
        other => other,
    };
    for (index, &(phi, _)) in moved.iter().enumerate() {
        function.set_operands(phi, vec![starts[index], Operand::Block(shape.preheader), latest(nexts[index]), Operand::Block(shape.header)]);
    }
    // The body reads the moved phi; the header and what follows the loop
    // read `a` as it left the preheader and `b'` as it left the latch.
    for (index, &phi) in phis.iter().enumerate() {
        let value = values[index];
        let mut updater = SsaUpdater::new(function.value(value).ty, function.value(value).name.clone().as_deref());
        updater.add_available_value(shape.preheader, starts[index]);
        updater.add_available_value(shape.latch, latest(nexts[index]));
        for one in function.users(value).to_vec() {
            let user = function.instruction(one.user);
            let at = match (user.opcode == Opcode::Phi).then(|| user.operands[one.index as usize + 1]) {
                Some(Operand::Block(from)) => from,
                _ => function.parent(one.user).expect("a placed user"),
            };
            if shape.body.contains(&at) {
                function.set_operand(one.user, one.index as usize, moved[index].1);
            } else {
                updater.rewrite_use(context, function, one);
            }
        }
        function.erase(phi)?;
    }
    Ok(())
}

/// The first proven loop not in `done` entered at its body; whether one was.
pub fn rotated(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &Analyses, done: &mut BTreeSet<BlockId>) -> Result<bool, String> {
    let facts = analyses.fresh().get::<Registers>(context, layout, function);
    for loop_ in cfg::Shape::of(function).loops {
        if done.contains(&cfg::block(loop_.header)) {
            continue;
        }
        let Some(shape) = _shape(function, &loop_) else {
            continue;
        };
        let unit = memory::Unit::within(context, layout, function, analyses.outer());
        if induction::trip_count(&unit, &loop_, &facts).is_none() {
            continue;
        }
        _rotate(context, function, &shape, None)?;
        done.insert(shape.first);
        return Ok(true);
    }
    Ok(false)
}

/// Rotate a dead counted counter into a guarded countdown; whether one was.
///
/// A dynamic bound cannot prove that the loop is entered, so the ordinary
/// rotation leaves its first test in place. Where the counter is otherwise
/// dead, its only meaning is the trips remaining:
///
/// ```text
/// i = start; while (i < n) { body; ++i; }
/// ```
///
/// becomes a zero-trip guard, then `body; --trips` until `trips` is zero.
/// The guard makes a zero count exact, and refusing an observed counter
/// makes replacing its values sound. Only the one-body-block form
/// `induction::control_replacement` proves.
pub fn _counted_down(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &Analyses, done: &mut BTreeSet<BlockId>) -> Result<bool, String> {
    let facts = analyses.fresh().get::<Registers>(context, layout, function);
    for loop_ in cfg::Shape::of(function).loops {
        if done.contains(&cfg::block(loop_.header)) {
            continue;
        }
        let Some(shape) = _shape(function, &loop_) else {
            continue;
        };
        let (proof, stepping, update, exits) = {
            let unit = memory::Unit::within(context, layout, function, analyses.outer());
            let proofs = induction::counted(&unit, &loop_, Some(&facts), true);
            let [proof] = &proofs[..] else {
                continue;
            };
            // A constant count is the finite-domain transforms' to take.
            if proof.count.is_some() || proof.preheader != Some(cfg::id(shape.preheader)) || proof.exit != cfg::id(shape.exit) {
                continue;
            }
            let Some(replacement) = induction::control_replacement(&unit, &loop_, proof, &BTreeSet::new()) else {
                continue;
            };
            (proof.clone(), replacement.stepping, replacement.update, replacement.exits.clone())
        };
        let width = proof.width();
        let entering = function.terminator(shape.preheader).expect("a terminated preheader");
        let mut seeds = Seeds { context: &mut *context, function: &mut *function, at: entering, width };
        let Some(count) = induction::trips(&proof, &mut |kind, args| seeds.computed(kind, args)) else {
            unreachable!("control_replacement proved a pre-tested loop")
        };
        let count = seeds.operand(&count);
        let guard = counting::skip_guard(&mut seeds, &proof).expect("a pre-tested loop");
        let replacement = ControlReplacement { counted: &proof, stepping, update, exits };
        let leaving = counting::leaving(&mut seeds, &replacement, true);

        let ty = context.types.int(width);
        let one = counting::constant(context, &BigInt::from(1), width);
        let zero = counting::constant(context, &BigInt::from(0), width);
        let trips = Operand::Value(proof.counter.value);
        let back = function.terminator(shape.latch).expect("a terminated latch");
        let decrement = function.create_instruction(Opcode::Binary(BinaryOp::Sub), ty, vec![trips, one], Flags::default(), Some("trips.next"));
        function.insert(decrement, Position::Before(back))?;
        let next = Operand::Value(function.instruction(decrement).result.expect("a value"));
        function.set_operands(proof.phi, from_arms(&[(count, shape.preheader), (next, shape.latch)]));
        function.erase(stepping)?;
        for (phi, operands) in leaving {
            function.set_operands(phi, operands);
        }
        // The header tests trips remaining, the branch's way round.
        let branch = proof.branch;
        let continues = function.instruction(branch).operands[1] == Operand::Block(shape.first);
        let predicate = if continues { IntPredicate::Ne } else { IntPredicate::Eq };
        let bit = context.types.int(1);
        let test = function.create_instruction(Opcode::ICmp(predicate), bit, vec![trips, zero], Flags::default(), None);
        function.insert(test, Position::Before(branch))?;
        function.set_operand(branch, 0, Operand::Value(function.instruction(test).result.expect("a value")));
        function.erase(proof.compare)?;
        _rotate(context, function, &shape, Some(Operand::Value(guard)))?;
        done.insert(shape.first);
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
#[path = "rotate_tests.rs"]
mod tests;
