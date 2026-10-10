//! Enter a loop proven to run at least once at its body, not at its test; and a
//! loop that is not proven, behind a copy of its test (gcc's `-ftree-ch`,
//! `tree-ssa-loop-ch.cc`): llrm-core's `optimize/rotate.rs`, the port of
//! `qbopt/optimize/rotate.py`, adapted to the rich MIR.
//! LLVM: LoopRotate, entering at the body where the first test is proven to
//! pass.
//!
//! BC writes `FOR` as `jmp test; body: ...; test: cmp; jle body`. Where the
//! first test is proven to pass the entry goes straight to the body, and the
//! test is then reached only from the latch, which it merges into. `lsr`
//! enters a loop it counts to zero at its body too, behind a skip guard.
//!
//! A rotated loop is no longer the pre-tested shape the counted-loop
//! analyses, peel and unroll read, so this goes after them.
//!
//! What changed with the IR: the header's phis move to the body through
//! `ssa::SsaUpdater`, which also rewrites every use, outside the loop
//! included. Dropped: `_step_test` -- an `icmp` is a value, and reusing a
//! step's flags is isel's; the old body's `loop_trip_counts` and
//! `integer_ranges` side tables; clearing a private start (Dead's).

use std::collections::BTreeSet;

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction;
use llrm_analysis::manager::Registers;
use llrm_analysis::ssa::SsaUpdater;
use llrm_analysis::{cfg, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};

use crate::lcssa::{arms, from_arms};

/// `copy`: the loops not proven to run are entered behind a copy of their test
/// too. gcc does this at -O1 and above and not at -Os, where it adds code
/// (`optimize_loop_for_size_p`): there only the proven loops are entered at
/// their body.
pub struct Rotate {
    /// Loops proven to run are entered at their body.
    pub proven: bool,
    pub copy: bool,
}

impl FunctionPass for Rotate {
    fn name(&self) -> &'static str {
        "rotate"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if self.proven && llrm_support::debug::enabled("census") {
            census(unit.context, unit.layout, unit.function, unit.id, analyses);
        }
        match entered(unit.context, unit.layout, unit.function, analyses, self.proven, self.copy) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("rotate: {error}"),
        }
    }
}

/// Every proven loop entered at its body, and each test merged into its
/// latch; whether any loop was.
pub fn entered(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &Analyses,
    proven: bool,
    copy: bool,
) -> Result<bool, String> {
    let mut done: llrm_mir::dense::IdSet<BlockId> = llrm_mir::dense::IdSet::new();
    while proven && rotated(context, layout, function, analyses, &mut done)? {}
    while copy && copied(context, layout, function, analyses, &mut done)? {}
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

fn _phis(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function
        .block(block)
        .instructions()
        .iter()
        .copied()
        .filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .collect()
}

/// Whether an instruction may be skipped on entry: it computes, and nothing
/// outside `header` reads it.
fn _test_only(
    function: &Function,
    header: BlockId,
    inst: InstId,
) -> bool {
    let op = function.instruction(inst);
    let computes = match op.opcode {
        Opcode::ICmp(_)
        | Opcode::FCmp(_)
        | Opcode::Cast(_)
        | Opcode::GetElementPtr { .. }
        | Opcode::Select
        | Opcode::Freeze => true,
        Opcode::Binary(kind) => !matches!(
            kind,
            BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem
        ),
        _ => false,
    };
    // A header phi reading it takes it round the back edge: it is the step of a
    // counter, which must run before the trip it starts, not skip the
    // first.
    computes
        && op.result.is_none_or(|value| {
            function.users(value).iter().all(|one| {
                function.parent(one.user) == Some(header) && function.instruction(one.user).opcode != Opcode::Phi
            })
        })
}

pub(crate) fn _shape(
    function: &Function,
    loop_: &Loop,
) -> Option<Shape> {
    // Asked of the blocks the loop's entry and exit are made of, not of a graph
    // of the whole function built for each loop.
    let predecessors =
        |at: i64| -> BTreeSet<i64> { function.predecessors(cfg::block(at)).into_iter().map(cfg::id).collect() };
    let succ = |at: i64| -> Vec<i64> { function.successors(cfg::block(at)).into_iter().map(cfg::id).collect() };
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else {
        return None;
    };
    let [preheader] = predecessors(loop_.header).difference(&loop_.body).copied().collect::<Vec<_>>()[..] else {
        return None;
    };
    let header = cfg::block(loop_.header);
    let terminator = |at: i64| function.terminator(cfg::block(at)).map(|last| function.instruction(last));
    if succ(preheader) != [loop_.header]
        || terminator(preheader).is_none_or(|last| last.opcode != Opcode::Br || last.operands.len() != 1)
    {
        return None;
    }
    if latch == loop_.header || succ(latch) != [loop_.header] {
        return None;
    }
    let branch = terminator(loop_.header)?;
    if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
        return None;
    }
    let (inside, outside): (Vec<i64>, Vec<i64>) =
        succ(loop_.header).into_iter().partition(|at| loop_.body.contains(at));
    let ([first], [exit]) = (&inside[..], &outside[..]) else {
        return None;
    };
    if predecessors(*first) != BTreeSet::from([loop_.header]) || !_phis(function, cfg::block(*first)).is_empty() {
        return None;
    }
    let work = function
        .block(header)
        .instructions()
        .iter()
        .copied()
        .filter(
            |&inst| {
                let op = function.instruction(inst);
                op.opcode != Opcode::Phi && !op.opcode.is_terminator()
            },
        );
    if !work.into_iter().all(|inst| _test_only(function, header, inst)) {
        return None;
    }
    let block = cfg::block;
    let body = loop_.body.iter().copied().filter(|&at| at != loop_.header).map(block).collect();
    Some(Shape {
        preheader: block(preheader),
        header,
        first: block(*first),
        latch: block(latch),
        exit: block(*exit),
        body,
    })
}

/// `shape`'s loop entered at `first`, or, with `guard`, entered there only
/// where the guard sends it to `first` and skipped to the exit otherwise
/// (`true`: the guard is true on the way out).
pub(crate) fn _rotate(
    context: &mut Context,
    function: &mut Function,
    shape: &Shape,
    guard: Option<(Operand, bool)>,
) -> Result<(), String> {
    let entering = function.terminator(shape.preheader).expect("a terminated preheader");
    if let Some((guard, exits_on_true)) = guard {
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
        let (taken, other) = if exits_on_true { (shape.exit, shape.first) } else { (shape.first, shape.exit) };
        function.set_operands(entering, vec![guard, Operand::Block(taken), Operand::Block(other)]);
    } else {
        function.set_operands(entering, vec![Operand::Block(shape.first)]);
    }
    let phis = _phis(function, shape.header);
    let values = phis.iter().map(|&phi| function.instruction(phi).result.expect("a phi's value")).collect::<Vec<_>>();
    let side = |function: &Function, phi: InstId, from: BlockId| {
        arms(function, phi).into_iter().find(|&(_, source)| source == from).expect("a preheader and a latch arm").0
    };
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
            let phi = function.create_instruction(
                Opcode::Phi,
                function.value(value).ty,
                Vec::new(),
                Flags::default(),
                function.value(value).name.clone().as_deref(),
            );
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
        function.set_operands(
            phi,
            vec![starts[index], Operand::Block(shape.preheader), latest(nexts[index]), Operand::Block(shape.header)],
        );
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
pub fn rotated(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &Analyses,
    done: &mut llrm_mir::dense::IdSet<BlockId>,
) -> Result<bool, String> {
    let mut fresh = analyses.fresh();
    let facts = fresh.get::<Registers>(context, layout, function);
    let found = fresh.get::<cfg::Shape>(context, layout, function);
    for loop_ in found.loops.clone() {
        if done.contains(&cfg::block(loop_.header)) {
            continue;
        }
        let Some(shape) = _shape(function, &loop_) else {
            continue;
        };
        let unit = memory::Unit::within(context, layout, function, analyses.outer()).with_shape(&found);
        if induction::trip_count(&unit, &loop_, &facts).is_none() {
            continue;
        }
        _rotate(context, function, &shape, None)?;
        done.insert(shape.first);
        return Ok(true);
    }
    Ok(false)
}

/// `LLRM_DEBUG=census`: per loop, what the trip-count analyses prove of it, as
/// the last loop pass finds it. Comparing a build with and without `-ftree-ch`
/// shows a loop whose symbolic count the copy lost.
fn census(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    id: Option<llrm_mir::context::GlobalId>,
    analyses: &Analyses,
) {
    let mut fresh = analyses.fresh();
    let facts = fresh.get::<Registers>(context, layout, function);
    let found = fresh.get::<cfg::Shape>(context, layout, function);
    let unit = memory::Unit::within(context, layout, function, analyses.outer()).with_shape(&found);
    let name = format!("{id:?}");
    let (mut proofs, mut constant, mut symbolic) = (0, 0, 0);
    for loop_ in found.loops.clone() {
        let found = induction::counted(&unit, &loop_, Some(&facts), false);
        proofs += usize::from(!found.is_empty());
        constant += usize::from(induction::trip_count(&unit, &loop_, &facts).is_some());
        symbolic += usize::from(found.iter().any(|proof| {
            induction::trips(proof, &mut |_, _| induction::AffineOperand::constant(0, proof.width())).is_some()
        }));
    }
    llrm_support::debug!(
        "census",
        "{name}: loops {} proved {proofs} constant {constant} symbolic {symbolic}",
        found.loops.len()
    );
}

/// gcc's `param_max_loop_header_insns`: the statements a copied header may
/// have.
const MAX_HEADER_INSNS: usize = 20;

/// The first loop not in `done` whose test is copied before it, entered at its
/// body behind the copy; whether one was (`copy_headers`,
/// `should_duplicate_loop_header_p`: a header that ends in the exit test,
/// within the limit, calling nothing).
pub fn copied(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &Analyses,
    done: &mut llrm_mir::dense::IdSet<BlockId>,
) -> Result<bool, String> {
    let mut fresh = analyses.fresh();
    let found = fresh.get::<cfg::Shape>(context, layout, function);
    for loop_ in found.loops.clone() {
        if done.contains(&cfg::block(loop_.header)) {
            continue;
        }
        let Some(shape) = _shape(function, &loop_) else {
            continue;
        };
        // A loop proven to run needs no guard: it is entered at its body once
        // the loop passes are done (`rotated`, which unroll, peel and
        // fill wait for), as gcc's value propagation leaves it after its copy.
        let unit = memory::Unit::within(context, layout, function, analyses.outer()).with_shape(&found);
        let facts = fresh.get::<Registers>(context, layout, function);
        if induction::trip_count(&unit, &loop_, &facts).is_some() {
            continue;
        }
        let insns = function
            .block(shape.header)
            .instructions()
            .iter()
            .filter(|&&inst| function.instruction(inst).opcode != Opcode::Phi)
            .count();
        if insns > MAX_HEADER_INSNS {
            continue;
        }
        let Some(guard) = _copy_test(function, &shape) else {
            continue;
        };
        _rotate(context, function, &shape, Some(guard))?;
        done.insert(shape.first);
        return Ok(true);
    }
    Ok(false)
}

/// The header's test computed in the preheader, on what the header's phis read
/// from it: the guard, and whether it is true on the way out. The header keeps
/// its own for the trips that come round.
fn _copy_test(
    function: &mut Function,
    shape: &Shape,
) -> Option<(Operand, bool)> {
    let branch = function.terminator(shape.header).expect("a terminated header");
    let [Operand::Value(cond), Operand::Block(taken), _] = function.instruction(branch).operands[..] else {
        return None;
    };
    let mut known: Vec<(llrm_mir::module::ValueId, Operand)> = Vec::new();
    for phi in _phis(function, shape.header) {
        let value = function.instruction(phi).result.expect("a phi's value");
        let start = arms(function, phi).into_iter().find(|&(_, from)| from == shape.preheader)?.0;
        known.push((value, start));
    }
    let entering = function.terminator(shape.preheader).expect("a terminated preheader");
    let work: Vec<InstId> = function
        .block(shape.header)
        .instructions()
        .iter()
        .copied()
        .filter(|&inst| {
            function.instruction(inst).opcode != Opcode::Phi && !function.instruction(inst).opcode.is_terminator()
        })
        .collect();
    for inst in work {
        let copy = function.clone_instruction(inst);
        for index in 0..function.instruction(copy).operands.len() {
            if let Operand::Value(value) = function.instruction(copy).operands[index] {
                if let Some((_, now)) = known.iter().find(|(old, _)| *old == value) {
                    function.set_operand(copy, index, *now);
                }
            }
        }
        function.insert(copy, Position::Before(entering)).expect("a placed preheader");
        if let (Some(old), Some(new)) = (function.instruction(inst).result, function.instruction(copy).result) {
            known.push((old, Operand::Value(new)));
        }
    }
    let guard = known.iter().find(|(old, _)| *old == cond)?.1;
    Some((guard, taken == shape.exit))
}

#[cfg(test)]
#[path = "rotate_tests.rs"]
mod tests;
