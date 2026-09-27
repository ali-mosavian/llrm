//! A loop's control chosen once what it computes has settled: llrm-core's
//! `optimize/indvars.rs`, the port of `qbopt/optimize/indvars.py`, adapted
//! to the rich MIR.
//! LLVM: IndVarSimplify's linear function test replace (`simplified`, `zeroed`), and its exit rewriting reused across an outer loop (`rewound`).
//!
//! - `simplified` ends a loop on another recurrence, so a counter read
//!   only by its control, or by equalities an injective map carries, dies.
//! - `zeroed` (`CountToZero`) moves control onto a recurrence counted up
//!   to zero, rebasing its reads by its final value, then rotates the loop
//!   to end on the step, behind a skip guard where it may run no trip.
//! - `rewound` carries an exact inner recurrence's exit value around the
//!   outer loop, less its distance, in place of its saved start.
//!
//! What changed with the IR:
//! - A use is an instruction's operand, not a memory reference's base: an
//!   address the recurrence indexes is a `getelementptr`, rebased by
//!   moving its base, in the preheader, by the bias -- never an add in the
//!   loop, whether the bias is constant or not.
//! - An equality is an `icmp eq` or `ne`, whoever reads it; the old one
//!   was flags only a branch might read.
//! - The zero test is an `icmp`; ending the loop on the step's flags is
//!   isel's. Rotation is `rotate`'s, and exit values past a skip guard are
//!   `counting::leaving`'s.
//! - Values after the loop take the final value by use, which the rich
//!   MIR's use lists name; the old `loopexit::_substituted_exits` rebuilt
//!   every block the exit dominates.
//! - The old `loop_trip_counts` side table, and clearing a private start,
//!   are gone: induction proves the count again, Dead takes what is unused.
//!
//! llrm-mir's `indvars` settles a compare of a counter with a constant
//! where the counter's range decides it; that is ranges' and Decide's here.
//! It never replaces a loop's control or rebases a recurrence.

use std::collections::BTreeSet;

use llrm_analysis::consts::{Known, masked};
use llrm_analysis::induction::{self, Affine, AffineMap, AffineOperand, CountedLoop};
use llrm_analysis::manager::Registers;
use llrm_analysis::{cfg, liveness, memory};
use llrm_graph::loops::Loop;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::{Signed, Zero};

use crate::counting::{self, Seeds};
use crate::lcssa::{arms, from_arms};
use crate::profit::OperationCosts;
use crate::{dead, rotate};

/// Counting to zero, once every other pass, strength reduction included,
/// has settled: a loop's control is chosen last, once what it computes is
/// known.
pub struct CountToZero;

impl FunctionPass for CountToZero {
    fn name(&self) -> &'static str {
        "zeroed"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        match zeroed(unit.context, unit.layout, unit.function, analyses) {
            Ok(true) => {
                dead::dead(unit.context, analyses.outer().callees(), unit.function);
                PreservedAnalyses::none()
            }
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("zeroed: {error}"),
        }
    }
}

fn _phis(function: &Function, block: BlockId) -> Vec<InstId> {
    function.block(block).instructions().iter().copied().filter(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect()
}

/// The instruction defining `value`.
fn _defining(function: &Function, value: ValueId) -> Option<InstId> {
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// The value `phi` takes from `from`.
fn _from(function: &Function, phi: InstId, from: BlockId) -> Option<Operand> {
    arms(function, phi).into_iter().find(|&(_, source)| source == from).map(|(value, _)| value)
}

fn _block_of(function: &Function, inst: InstId) -> i64 {
    cfg::id(function.parent(inst).expect("a placed instruction"))
}

/// The loop's one outside predecessor of its header, going only there.
fn _preheader(function: &Function, loop_: &Loop) -> Option<BlockId> {
    let header = cfg::block(loop_.header);
    let [preheader] = function.predecessors(header).into_iter().filter(|block| !loop_.body.contains(&cfg::id(*block))).collect::<Vec<_>>()[..] else {
        return None;
    };
    (function.successors(preheader) == [header]).then_some(preheader)
}

/// The header's one successor outside the loop, reached from nowhere else.
fn _exit(function: &Function, loop_: &Loop) -> Option<BlockId> {
    let header = cfg::block(loop_.header);
    let [exit] = function.successors(header).into_iter().filter(|block| !loop_.body.contains(&cfg::id(*block))).collect::<Vec<_>>()[..] else {
        return None;
    };
    (function.predecessors(exit) == [header]).then_some(exit)
}

/// The continuing test of `branch` as an `icmp` of `left` and `right`: `ne`
/// where its true arm stays in the loop, `eq` where it leaves.
fn _test(context: &mut Context, function: &mut Function, loop_: &Loop, branch: InstId, left: Operand, right: Operand) -> Result<InstId, String> {
    let stays = matches!(function.instruction(branch).operands[1], Operand::Block(target) if loop_.body.contains(&cfg::id(target)));
    let bit = context.types.int(1);
    let predicate = if stays { IntPredicate::Ne } else { IntPredicate::Eq };
    let test = function.create_instruction(Opcode::ICmp(predicate), bit, vec![left, right], Flags::default(), None);
    function.insert(test, Position::Before(branch))?;
    Ok(test)
}

/// `compare`, which only `branch` reads, replaced by `test`.
fn _retest(function: &mut Function, branch: InstId, compare: InstId, test: InstId) -> Result<(), String> {
    function.set_operand(branch, 0, Operand::Value(function.instruction(test).result.expect("a bit")));
    function.erase(compare)
}

/// Whether only `branch` reads `compare`.
fn _only_branch_reads(function: &Function, compare: InstId, branch: InstId) -> bool {
    function.instruction(compare).result.is_some_and(|value| function.users(value).iter().all(|one| one.user == branch))
}

// ---------------------------------------------------------------- simplified

/// Every loop whose counter another recurrence can end, ended on it;
/// whether any was. The counter is left to Dead.
pub fn simplified(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &Analyses) -> Result<bool, String> {
    let mut changed = false;
    while _simplified(context, layout, function, analyses)? {
        changed = true;
    }
    Ok(changed)
}

/// One loop's control moved onto `alternative`, decided before anything changes.
struct Replaced {
    loop_: Loop,
    proof: CountedLoop,
    preheader: BlockId,
    alternative: Affine,
    stride: BigInt,
    width: u32,
    /// Equalities read through the alternative: each compare, and its new operands.
    rebased: Vec<(InstId, Vec<Operand>)>,
}

fn _simplified(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &Analyses) -> Result<bool, String> {
    let facts = analyses.fresh().get::<Registers>(context, layout, function);
    let found = {
        let unit = memory::Unit::within(context, layout, function, analyses.outer());
        _replacement(&unit, &facts)
    };
    let Some(plan) = found else {
        return Ok(false);
    };
    let proof = &plan.proof;
    let count = proof.count.clone().expect("a span has a count");
    let entering = function.terminator(plan.preheader).expect("a terminated preheader");
    let bound = {
        let mut seeds = Seeds { context: &mut *context, function: &mut *function, at: entering, width: plan.width };
        let distance = AffineOperand::constant(&plan.stride * &count, plan.width);
        let bound = seeds.computed(BinaryOp::Add, vec![plan.alternative.start.clone(), distance]);
        seeds.operand(&bound)
    };
    let loop_ = &plan.loop_;
    let test = _test(context, function, loop_, proof.branch, Operand::Value(plan.alternative.value), bound)?;
    _retest(function, proof.branch, proof.compare, test)?;
    for (compare, operands) in &plan.rebased {
        function.set_operands(*compare, operands.clone());
    }
    // After the loop the counter is the first value failing its test.
    let last = proof.last.clone().expect("a span has a last");
    let finished = counting::constant(context, &(last + &proof.step), proof.width());
    for one in function.users(proof.counter.value).to_vec() {
        if !loop_.body.contains(&_block_of(function, one.user)) {
            function.set_operand(one.user, one.index as usize, finished);
        }
    }
    Ok(true)
}

/// The first loop another recurrence can end, and how.
fn _replacement(unit: &memory::Unit, facts: &IndexMap<ValueId, Known>) -> Option<Replaced> {
    let function = unit.function;
    let shape = unit.shape();
    let dominance = &shape.dominance;
    let recurrences = _recurrences(unit, facts);
    for loop_ in shape.loops.iter().cloned() {
        let (Some(preheader), [latch]) = (_preheader(function, &loop_), &loop_.latches.iter().copied().collect::<Vec<_>>()[..]) else {
            continue;
        };
        let Some(exit) = _exit(function, &loop_) else {
            continue;
        };
        let counters = induction::basics(unit, &loop_);
        for proof in induction::counted(unit, &loop_, Some(facts), false) {
            if proof.posttested || proof.first.is_none() || proof.width() != proof.counter.start.width() {
                continue;
            }
            let Some(count) = proof.count.clone() else { continue };
            let counter = proof.counter.value;
            let Some(Operand::Value(update)) = _from(function, proof.phi, cfg::block(*latch)) else {
                continue;
            };
            if function.users(update).iter().any(|one| one.user != proof.phi) || !_only_branch_reads(function, proof.compare, proof.branch) {
                continue;
            }
            let Some(stepping) = _defining(function, update) else { continue };
            // After the loop, the counter is read only where the exit dominates.
            let leaves = function.users(counter).iter().any(|one| {
                let at = _block_of(function, one.user);
                !loop_.body.contains(&at) && !dominance.dominates(cfg::id(exit), at)
            });
            if leaves {
                continue;
            }
            for alternative in counters.values() {
                let width = alternative.start.width();
                let Some(stride) = induction::_signed(&alternative.step, facts, width).filter(|stride| !stride.is_zero()) else {
                    continue;
                };
                let modulus = BigInt::from(1) << width;
                if alternative.value == counter || count >= &modulus / induction::gcd(stride.abs(), modulus.clone()) {
                    continue;
                }
                let Some(phi) = _defining(function, alternative.value) else { continue };
                let sides = arms(function, phi).into_iter().map(|(_, from)| from).collect::<BTreeSet<_>>();
                if sides != BTreeSet::from([preheader, cfg::block(*latch)]) {
                    continue;
                }
                let own = match _from(function, phi, cfg::block(*latch)) {
                    Some(Operand::Value(next)) => _defining(function, next),
                    _ => None,
                };
                let read = function.users(alternative.value).iter().any(|one| loop_.body.contains(&_block_of(function, one.user)) && Some(one.user) != own && one.user != phi);
                if !read {
                    continue;
                }
                let control = BTreeSet::from([proof.compare, stepping]);
                let Some(rebased) = _rebased_equalities(unit, &loop_, counter, &control, alternative.value, &recurrences, facts) else {
                    continue;
                };
                return Some(Replaced { loop_: loop_.clone(), proof: proof.clone(), preheader, alternative: alternative.clone(), stride, width, rebased });
            }
        }
    }
    None
}

/// Every basic recurrence, with its proven domain when finite.
fn _recurrences(unit: &memory::Unit, facts: &IndexMap<ValueId, Known>) -> IndexMap<ValueId, (Affine, Option<(BigInt, BigInt)>)> {
    let mut out = IndexMap::default();
    for loop_ in &unit.shape().loops {
        for affine in induction::basics(unit, loop_).values() {
            out.insert(affine.value, (affine.clone(), induction::domain(unit, loop_, affine, facts)));
        }
    }
    out
}

/// Equality-only reads of `counter` in the loop, besides `control`, read
/// through `alternative`: each compare with its new operands; None where
/// one cannot be.
///
/// Strength reduction commonly leaves both `i` and `i * element_size`
/// live. The scaled one may control the loop only if every other read of
/// `i` can use it too. An equality with another finite recurrence is such a
/// read when both sides have one affine map and their joint domain is
/// shorter than its period.
fn _rebased_equalities(
    unit: &memory::Unit,
    loop_: &Loop,
    counter: ValueId,
    control: &BTreeSet<InstId>,
    alternative: ValueId,
    recurrences: &IndexMap<ValueId, (Affine, Option<(BigInt, BigInt)>)>,
    facts: &IndexMap<ValueId, Known>,
) -> Option<Vec<(InstId, Vec<Operand>)>> {
    let function = unit.function;
    let reads = function.users(counter).iter().map(|one| one.user).filter(|&user| loop_.body.contains(&_block_of(function, user)) && !control.contains(&user)).collect::<BTreeSet<_>>();
    if reads.is_empty() {
        // The original IndVarSimplify case: any long enough recurrence can
        // end a loop whose counter has no other purpose.
        return Some(Vec::new());
    }
    let (source, Some((source_low, source_high))) = recurrences.get(&counter)? else {
        return None;
    };
    let (target, _) = recurrences.get(&alternative)?;
    let relation = induction::relation(source, target, facts)?;
    let mut out = Vec::new();
    for inst in reads {
        let op = function.instruction(inst);
        if !matches!(op.opcode, Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne)) {
            return None;
        }
        let at = op.operands.iter().position(|one| *one == Operand::Value(counter))?;
        let Operand::Value(other) = op.operands[1 - at] else {
            return None;
        };
        let (other_affine, Some((other_low, other_high))) = recurrences.get(&other)? else {
            return None;
        };
        if other_affine.start.width() != source.start.width() {
            return None;
        }
        let partner = recurrences.iter().find(|(value, (candidate, _))| **value != other && candidate.header == other_affine.header && induction::relation(other_affine, candidate, facts).as_ref() == Some(&relation)).map(|(value, _)| *value)?;
        if !relation.injective(source_low.min(other_low), source_high.max(other_high)) {
            return None;
        }
        let mut operands = op.operands.clone();
        operands[at] = Operand::Value(alternative);
        operands[1 - at] = Operand::Value(partner);
        out.push((inst, operands));
    }
    Some(out)
}

// ------------------------------------------------------------------ rewound

/// An exact inner recurrence carried around its outer loop, where the
/// target makes that pay; whether one was.
///
/// An inner recurrence running exactly `count` trips leaves through its
/// sole exit as `start + count * step`. Where that loop is itself repeated,
/// subtracting the distance on the outer back edge rebuilds the next
/// start, and the saved start is dead across the hot inner loop. In a
/// register the old copy and the rewind are equal work; in frame cells the
/// old form is a load and a store, the new a memory update, which `costs`
/// prices. Nothing here names either form.
pub fn rewound(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &Analyses, registers: i64, costs: &OperationCosts) -> bool {
    if registers == 0 || costs.add > costs.r#move || costs.memory_update > costs.load + costs.store {
        return false;
    }
    let facts = analyses.fresh().get::<Registers>(context, layout, function);
    let plan = {
        let unit = memory::Unit::within(context, layout, function, analyses.outer());
        _rewinding(&unit, &facts, registers)
    };
    let Some(plan) = plan else {
        return false;
    };
    let ty = function.value(plan.start).ty;
    let exit_value = if plan.sources.len() == 1 {
        plan.exit_value
    } else {
        let closed = function.create_instruction(Opcode::Phi, ty, from_arms(&plan.sources.iter().map(|&source| (plan.exit_value, source)).collect::<Vec<_>>()), Flags::default(), None);
        let first = function.block(plan.exit).instructions()[0];
        function.insert(closed, Position::Before(first)).expect("a placed block");
        Operand::Value(function.instruction(closed).result.expect("a phi's value"))
    };
    let width = context.types.int_bits(ty).expect("an integer");
    let back = counting::constant(context, &-&plan.distance, width);
    let reset = function.create_instruction(Opcode::Binary(BinaryOp::Add), ty, vec![exit_value, back], Flags::default(), None);
    let after = function.block(plan.exit).instructions().iter().copied().find(|&inst| function.instruction(inst).opcode != Opcode::Phi).expect("a terminator");
    function.insert(reset, Position::Before(after)).expect("a placed block");
    let reset = Operand::Value(function.instruction(reset).result.expect("a value"));
    let carried = function.create_instruction(Opcode::Phi, ty, from_arms(&[(Operand::Value(plan.start), plan.parent_preheader), (reset, plan.parent_latch)]), Flags::default(), None);
    let first = function.block(plan.parent_header).instructions()[0];
    function.insert(carried, Position::Before(first)).expect("a placed block");
    let carried = Operand::Value(function.instruction(carried).result.expect("a phi's value"));
    let mut incoming = arms(function, plan.phi);
    for arm in &mut incoming {
        if arm.1 == plan.inner_preheader {
            arm.0 = carried;
        }
    }
    function.set_operands(plan.phi, from_arms(&incoming));
    true
}

struct Rewinding {
    phi: InstId,
    start: ValueId,
    exit_value: Operand,
    distance: BigInt,
    sources: Vec<BlockId>,
    exit: BlockId,
    inner_preheader: BlockId,
    parent_header: BlockId,
    parent_preheader: BlockId,
    parent_latch: BlockId,
}

fn _rewinding(unit: &memory::Unit, facts: &IndexMap<ValueId, Known>, registers: i64) -> Option<Rewinding> {
    let function = unit.function;
    let shape = unit.shape();
    let found = &shape.loops;
    if found.len() < 2 {
        return None;
    }
    let dominance = &shape.dominance;
    let live = liveness::live(function);
    let graph = cfg::graph(function);
    for inner in found {
        let Some(parent) = found.iter().filter(|parent| inner.body.is_subset(&parent.body) && inner.body != parent.body).min_by_key(|parent| parent.body.len()) else {
            continue;
        };
        let ([inner_latch], [parent_latch]) = (&inner.latches.iter().copied().collect::<Vec<_>>()[..], &parent.latches.iter().copied().collect::<Vec<_>>()[..]) else {
            continue;
        };
        let (Some(inner_preheader), Some(parent_preheader)) = (_preheader(function, inner), _preheader(function, parent)) else {
            continue;
        };
        let parent_header = cfg::block(parent.header);
        let preds = function.predecessors(parent_header).into_iter().collect::<BTreeSet<_>>();
        if !parent.body.contains(&cfg::id(inner_preheader)) || preds != BTreeSet::from([parent_preheader, cfg::block(*parent_latch)]) || (liveness::pressure(function, Some(&live), Some(&inner.body)) as i64) < registers {
            continue;
        }
        let exiting = graph.iter().filter(|block| inner.body.contains(&block.at)).flat_map(|block| block.succ.iter().filter(|at| !inner.body.contains(at)).map(move |at| (block.at, *at))).collect::<Vec<_>>();
        let exits = exiting.iter().map(|(_, target)| *target).collect::<BTreeSet<_>>();
        let [exit] = exits.into_iter().collect::<Vec<_>>()[..] else {
            continue;
        };
        let sources = exiting.iter().map(|(source, _)| *source).collect::<BTreeSet<_>>();
        let exit_preds = function.predecessors(cfg::block(exit)).into_iter().map(cfg::id).collect::<BTreeSet<_>>();
        if !parent.body.contains(&exit) || exit_preds != sources || !dominance.dominates(exit, *parent_latch) {
            continue;
        }
        let Some(count) = induction::trip_count(unit, inner, facts) else { continue };
        for counter in induction::basics(unit, inner).values() {
            let Some(phi) = _defining(function, counter.value) else { continue };
            let sides = arms(function, phi).into_iter().map(|(_, from)| from).collect::<BTreeSet<_>>();
            if sides != BTreeSet::from([inner_preheader, cfg::block(*inner_latch)]) {
                continue;
            }
            let (Some(Operand::Value(start)), Some(Operand::Value(update))) = (_from(function, phi, inner_preheader), _from(function, phi, cfg::block(*inner_latch))) else {
                continue;
            };
            let width = counter.start.width();
            let (Some(step), Some(stepping), Some(defining)) = (induction::_signed(&counter.step, facts, width), _defining(function, update), _defining(function, start)) else {
                continue;
            };
            if step.is_zero() || parent.body.contains(&_block_of(function, defining)) || facts.contains_key(&start) {
                continue;
            }
            // A pre-tested loop leaves from its header, where the phi is
            // already `start + count * step`; a post-tested one from its
            // latch, where the update is.
            let updated = _block_of(function, stepping);
            let exit_value = if sources.iter().all(|&source| dominance.dominates(updated, source)) {
                Operand::Value(update)
            } else if sources == BTreeSet::from([inner.header]) {
                Operand::Value(counter.value)
            } else {
                continue;
            };
            // The saved start must die in the outer loop, or the rewrite is
            // only code size.
            if function.users(start).iter().any(|one| one.user != phi && parent.body.contains(&_block_of(function, one.user))) {
                continue;
            }
            let distance = masked(&(&step * &count), width);
            if distance.is_zero() {
                continue;
            }
            return Some(Rewinding {
                phi,
                start,
                exit_value,
                distance,
                sources: sources.into_iter().map(cfg::block).collect(),
                exit: cfg::block(exit),
                inner_preheader,
                parent_header,
                parent_preheader,
                parent_latch: cfg::block(*parent_latch),
            });
        }
    }
    None
}

// ------------------------------------------------------------------- zeroed

/// A read of a recurrence that counting it to zero rebases: the recurrence
/// moves by its final value, the bias, and each read absorbs that.
#[derive(Clone, Debug)]
enum Use {
    /// Operand `index` of `inst` moves by the bias times `multiplier`: an
    /// invariant added to the recurrence, or the other side of an equality.
    Operand { inst: InstId, index: usize, multiplier: BigInt },
    /// A `getelementptr` indexing by the recurrence times `multiplier` at
    /// operand `index`: its base moves by the bias times `multiplier`.
    Address { inst: InstId, index: usize, multiplier: BigInt },
    /// The recurrence times `multiplier` masked to its low `span`: unchanged
    /// by a bias whose product is a multiple of `span`.
    Masked { inst: InstId, multiplier: BigInt, span: BigInt },
}

/// Every loop's control moved onto a recurrence counted up to zero; whether
/// any was.
///
/// For `n` trips and a recurrence from `r0` by `s`, every read takes the
/// recurrence's final value `r0 + n*s` as a bias, and it starts at `-n*s`,
/// reaching zero on exactly the last trip. When a recurrence other than the
/// counter takes control, the counter dies with its compare. The loop is
/// rotated to end on the step, behind a guard where it may run no trip;
/// one rotation cannot take, proven to run, tests the recurrence at its
/// header.
pub fn zeroed(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &Analyses) -> Result<bool, String> {
    let mut changed = false;
    let mut done = BTreeSet::new();
    while _zeroed(context, layout, function, analyses, &mut done)? {
        changed = true;
    }
    if changed {
        crate::cfg::merged(function);
        crate::canonical::identities(context, function);
    }
    Ok(changed)
}

/// What counting one loop to zero needs, decided before anything changes.
struct Zeroing {
    proof: CountedLoop,
    candidate: Affine,
    phi: InstId,
    stepping: InstId,
    step: BigInt,
    uses: Vec<Use>,
    /// Rotated: the shape, and whether a guard must skip it; tested at the
    /// header: its exit.
    rotated: Option<(rotate::Shape, bool, InstId, ValueId, Vec<InstId>)>,
    exit: Option<BlockId>,
    preheader: BlockId,
    loop_: Loop,
}

fn _zeroed(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &Analyses, done: &mut BTreeSet<i64>) -> Result<bool, String> {
    let facts = analyses.fresh().get::<Registers>(context, layout, function);
    let plan = {
        let unit = memory::Unit::within(context, layout, function, analyses.outer());
        _zeroing(&unit, &facts, done)
    };
    let Some(plan) = plan else {
        return Ok(false);
    };
    done.insert(plan.loop_.header);
    let proof = &plan.proof;
    let width = proof.width();
    let entering = function.terminator(plan.preheader).expect("a terminated preheader");
    let initial = _from(function, plan.phi, plan.preheader).expect("a preheader arm");
    let start = match initial {
        Operand::Value(value) => facts.get(&value).map_or(AffineOperand::Value(value, width), |known| AffineOperand::constant(known.n.clone(), width)),
        Operand::Constant(_) => plan.candidate.start.clone(),
        Operand::Block(_) => unreachable!("a phi's value"),
    };
    let mut seeds = Seeds { context: &mut *context, function: &mut *function, at: entering, width };
    let count = induction::trips(proof, &mut |kind, args| seeds.computed(kind, args)).expect("a pre-tested proof");
    let distance = seeds.computed(BinaryOp::Mul, vec![count, AffineOperand::constant(plan.step.clone(), width)]);
    let bias = seeds.computed(BinaryOp::Add, vec![start, distance.clone()]);
    let rebased = _rebased(&mut seeds, &plan.uses, &bias, width);
    let begun = seeds.computed(BinaryOp::Sub, vec![AffineOperand::constant(0, width), distance]);
    let begun = seeds.operand(&begun);
    let bias = seeds.operand(&bias);
    let (leaving, guard) = match &plan.rotated {
        Some((_, runs, stepping, update, exits)) => {
            let replacement = induction::ControlReplacement { counted: proof, stepping: *stepping, update: *update, exits: exits.clone() };
            let leaving = counting::leaving(&mut seeds, &replacement, !runs);
            let guard = (!runs).then(|| counting::skip_guard(&mut seeds, proof).expect("a pre-tested proof"));
            (leaving, guard)
        }
        None => (Vec::new(), None),
    };
    for (inst, operands) in rebased {
        function.set_operands(inst, operands);
    }
    // The recurrence now runs elsewhere: a promise was about its old range.
    let flags = function.instruction(plan.stepping).flags;
    let mut cleared = Flags::default();
    for (flag, _) in Flags::NAMES {
        if flags.contains(flag) && flag != Flags::NUW && flag != Flags::NSW {
            cleared.insert(flag);
        }
    }
    function.set_flags(plan.stepping, cleared);
    let mut incoming = arms(function, plan.phi);
    for arm in &mut incoming {
        if arm.1 == plan.preheader {
            arm.0 = begun;
        }
    }
    function.set_operands(plan.phi, from_arms(&incoming));
    for (phi, operands) in leaving {
        function.set_operands(phi, operands);
    }
    let zero = counting::constant(context, &BigInt::from(0), width);
    let test = _test(context, function, &plan.loop_, proof.branch, Operand::Value(plan.candidate.value), zero)?;
    _retest(function, proof.branch, proof.compare, test)?;
    match &plan.rotated {
        Some((shape, _, _, _, _)) => rotate::_rotate(context, function, shape, guard.map(Operand::Value))?,
        None => {
            // After the loop the recurrence is its final value.
            let exit = plan.exit.expect("an exit");
            let dominance = cfg::Dominance::of(function);
            for one in function.users(plan.candidate.value).to_vec() {
                let at = _block_of(function, one.user);
                if !plan.loop_.body.contains(&at) && dominance.dominates(cfg::id(exit), at) {
                    function.set_operand(one.user, one.index as usize, bias);
                }
            }
        }
    }
    Ok(true)
}

fn _zeroing(unit: &memory::Unit, facts: &IndexMap<ValueId, Known>, done: &BTreeSet<i64>) -> Option<Zeroing> {
    let function = unit.function;
    let shape = unit.shape();
    let dominance = &shape.dominance;
    for loop_ in shape.loops.iter().cloned() {
        if done.contains(&loop_.header) {
            continue;
        }
        let proofs = induction::counted(unit, &loop_, Some(facts), true);
        let [proof] = &proofs[..] else { continue };
        if proof.posttested {
            continue;
        }
        let Some(preheader) = proof.preheader.map(cfg::block) else { continue };
        let width = proof.width();
        // Counting to zero at its header already, it may still be rotated.
        let tested = proof.bound == AffineOperand::constant(0, width) && proof.test == IntPredicate::Ne;
        let runs = proof.count.as_ref().is_some_and(|count| count >= &BigInt::from(1)) || induction::nonempty(unit, &loop_);
        let symbolic = proof.count.is_none();
        let mut ordered = induction::basics(unit, &loop_).into_values().filter_map(|candidate| {
            let itself = candidate == proof.counter;
            let (uses, _) = _uses(unit, &loop_, proof, &candidate, itself)?;
            let unknown = symbolic || induction::_signed(&candidate.start, facts, width).is_none();
            let moved = if unknown { uses.iter().filter(|one| matches!(one, Use::Address { .. })).count() as i64 } else { 0 };
            Some((moved - i64::from(!itself), !itself, candidate))
        }).collect::<Vec<_>>();
        ordered.sort_by(|one, other| (one.0, one.1).cmp(&(other.0, other.1)));
        for (_, _, candidate) in ordered {
            let itself = candidate == proof.counter;
            let Some(phi) = _defining(function, candidate.value) else { continue };
            let sides = arms(function, phi).into_iter().map(|(_, from)| cfg::id(from)).collect::<BTreeSet<_>>();
            if sides != BTreeSet::from([cfg::id(preheader), proof.latch]) || candidate.start.width() != width || candidate.step.width() != width {
                continue;
            }
            let Some(Operand::Value(update)) = _from(function, phi, cfg::block(proof.latch)) else { continue };
            let Some(stepping) = _defining(function, update) else { continue };
            if function.users(update).iter().any(|one| one.user != phi) {
                continue;
            }
            let Some((uses, through)) = _uses(unit, &loop_, proof, &candidate, itself) else { continue };
            if uses.is_empty() && !itself {
                continue;
            }
            let Some(step) = induction::_signed(&candidate.step, facts, width).filter(|step| !step.is_zero()) else { continue };
            // A mask holds only under a bias it cannot see.
            let masked_ok = uses.iter().all(|one| match one {
                Use::Masked { multiplier, span, .. } => {
                    let (Some(count), Some(start)) = (&proof.count, induction::_signed(&candidate.start, facts, width)) else {
                        return false;
                    };
                    let bias = start + count * &step;
                    (bias * multiplier % span).is_zero()
                }
                _ => true,
            });
            if !masked_ok {
                continue;
            }
            let period = AffineMap { scale: step.clone(), offset: BigInt::from(0), width }.period();
            let covered = if itself {
                uses.iter().map(|one| match one {
                    Use::Operand { inst, .. } | Use::Address { inst, .. } | Use::Masked { inst, .. } => *inst,
                }).chain(through.iter().copied()).collect()
            } else {
                BTreeSet::new()
            };
            // Read after the loop, a recurrence other than the counter would
            // need its final value where a guard may have skipped it.
            let outside = function.users(candidate.value).iter().any(|one| !loop_.body.contains(&_block_of(function, one.user)));
            if let Some(control) = induction::zero_terminating_control(unit, &loop_, proof, &candidate, &covered, Some(facts)) {
                let shape = rotate::_shape(function, &loop_);
                if let Some(shape) = shape.filter(|shape| cfg::id(shape.first) == proof.latch && (itself || !outside)) {
                    let replacement = &control.replacement;
                    return Some(Zeroing {
                        proof: proof.clone(),
                        candidate: candidate.clone(),
                        phi,
                        stepping,
                        step,
                        uses,
                        rotated: Some((shape, runs, replacement.stepping, replacement.update, replacement.exits.clone())),
                        exit: None,
                        preheader,
                        loop_: loop_.clone(),
                    });
                }
            }
            // One rotation cannot take tests the recurrence at its header;
            // zero first on the last trip needs fewer trips than the period.
            let header = cfg::block(loop_.header);
            if tested || !runs || proof.maximum.as_ref().is_none_or(|maximum| maximum >= &period) || function.parent(proof.compare) != Some(header) || !_only_branch_reads(function, proof.compare, proof.branch) {
                continue;
            }
            let Some(exit) = _exit(function, &loop_) else { continue };
            let counter_left = function.users(proof.counter.value).iter().any(|one| {
                let at = _block_of(function, one.user);
                !loop_.body.contains(&at) && !dominance.dominates(cfg::id(exit), at)
            });
            let candidate_left = function.users(candidate.value).iter().any(|one| {
                let at = _block_of(function, one.user);
                !loop_.body.contains(&at) && !dominance.dominates(cfg::id(exit), at)
            });
            if counter_left || candidate_left || (!itself && function.users(proof.counter.value).iter().any(|one| !loop_.body.contains(&_block_of(function, one.user)))) {
                continue;
            }
            return Some(Zeroing { proof: proof.clone(), candidate: candidate.clone(), phi, stepping, step, uses, rotated: None, exit: Some(exit), preheader, loop_: loop_.clone() });
        }
    }
    None
}

/// Every read of `candidate` inside the loop as a `Use`, also through a
/// shift or constant multiple of it read only by such uses, and the reads
/// looked through; None where it is read any other way. Its own step, and
/// the counter's compare where it is the counter, are control.
fn _uses(unit: &memory::Unit, loop_: &Loop, proof: &CountedLoop, candidate: &Affine, itself: bool) -> Option<(Vec<Use>, Vec<InstId>)> {
    let function = unit.function;
    let phi = _defining(function, candidate.value)?;
    let own = match _from(function, phi, cfg::block(proof.latch)) {
        Some(Operand::Value(update)) => _defining(function, update),
        _ => None,
    };
    let inside = |inst: InstId| loop_.body.contains(&_block_of(function, inst));
    let invariant = |operand: Operand| match operand {
        Operand::Value(value) => _defining(function, value).is_none_or(|inst| !inside(inst)),
        _ => true,
    };
    let width = candidate.start.width();
    let same = |operand: Operand| unit.int_bits(operand) == Some(width);
    // One read of `value` by `inst`, as a `Use` at `multiplier`.
    let derived = |inst: InstId, value: ValueId, multiplier: &BigInt| -> Option<Use> {
        let op = function.instruction(inst);
        let at = op.operands.iter().position(|one| *one == Operand::Value(value))?;
        if op.operands.iter().filter(|one| **one == Operand::Value(value)).count() != 1 {
            return None;
        }
        match op.opcode {
            Opcode::Binary(BinaryOp::Add) if invariant(op.operands[1 - at]) && op.result.is_some_and(|one| same(Operand::Value(one))) => {
                Some(Use::Operand { inst, index: 1 - at, multiplier: multiplier.clone() })
            }
            Opcode::Binary(BinaryOp::And) => {
                let mask = unit.int_constant(op.operands[1 - at])?;
                let span = BigInt::from(1) << (128 - mask.leading_zeros());
                Some(Use::Masked { inst, multiplier: multiplier.clone(), span })
            }
            // The invariant side of an equality moves with the recurrence:
            // `r == x` is `r - b == x - b`. An ordered compare would change.
            Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne) if invariant(op.operands[1 - at]) && same(op.operands[1 - at]) => {
                Some(Use::Operand { inst, index: 1 - at, multiplier: -multiplier })
            }
            Opcode::GetElementPtr { .. } if at >= 1 && invariant(op.operands[0]) && op.operands[1..].iter().enumerate().all(|(index, one)| index + 1 == at || unit.int_constant(*one).is_some()) => {
                Some(Use::Address { inst, index: at, multiplier: multiplier.clone() })
            }
            _ => None,
        }
    };
    let (mut out, mut through) = (Vec::new(), Vec::new());
    for one in function.users(candidate.value) {
        let inst = one.user;
        if Some(inst) == own || inst == phi || (itself && inst == proof.compare) || !inside(inst) {
            continue;
        }
        if let Some(form) = derived(inst, candidate.value, &BigInt::from(1)) {
            out.push(form);
            continue;
        }
        let op = function.instruction(inst);
        let scale = match op.opcode {
            Opcode::Binary(BinaryOp::Shl) if op.operands[0] == Operand::Value(candidate.value) => {
                let shift = unit.int_constant(op.operands[1])?;
                (shift < u128::from(width)).then(|| BigInt::from(1) << shift)
            }
            Opcode::Binary(BinaryOp::Mul) => {
                let at = op.operands.iter().position(|one| *one == Operand::Value(candidate.value))?;
                unit.int_constant(op.operands[1 - at]).map(|bits| induction::_signed(&AffineOperand::constant(bits, width), &IndexMap::default(), width).expect("a constant"))
            }
            _ => None,
        }?;
        let result = op.result.filter(|&result| same(Operand::Value(result)))?;
        let forms = function.users(result).iter().map(|reader| derived(reader.user, result, &scale)).collect::<Option<Vec<_>>>()?;
        if forms.is_empty() {
            return None;
        }
        out.extend(forms);
        through.push(inst);
    }
    Some((out, through))
}

/// `uses` moved by `bias`, each instruction with its new operands. A move
/// is placed by `seeds` before the loop: an invariant plus the bias, or a
/// base moved by it.
fn _rebased(seeds: &mut Seeds, uses: &[Use], bias: &AffineOperand, width: u32) -> Vec<(InstId, Vec<Operand>)> {
    let mut out = Vec::new();
    for one in uses {
        match one {
            Use::Masked { .. } => {}
            Use::Operand { inst, index, multiplier } => {
                let moved = seeds.computed(BinaryOp::Mul, vec![bias.clone(), AffineOperand::constant(multiplier.clone(), width)]);
                let operand = seeds.function.instruction(*inst).operands[*index];
                let term = match operand {
                    Operand::Value(value) => AffineOperand::Value(value, width),
                    other => {
                        let bits = match other {
                            Operand::Constant(id) => match seeds.context.get(id).kind {
                                llrm_mir::ConstantKind::Int(bits) => bits,
                                _ => unreachable!("an integer invariant"),
                            },
                            _ => unreachable!("an integer invariant"),
                        };
                        AffineOperand::constant(bits, width)
                    }
                };
                let adjusted = seeds.computed(BinaryOp::Add, vec![term, moved]);
                let mut operands = seeds.function.instruction(*inst).operands.clone();
                operands[*index] = seeds.operand(&adjusted);
                out.push((*inst, operands));
            }
            Use::Address { inst, index, multiplier } => {
                let moved = seeds.computed(BinaryOp::Mul, vec![bias.clone(), AffineOperand::constant(multiplier.clone(), width)]);
                let moved = seeds.operand(&moved);
                let op = seeds.function.instruction(*inst).clone();
                // Each other index is zero, so the base moves by the bias's
                // stride alone.
                let mut indices = Vec::new();
                for (at, &original) in op.operands.iter().enumerate().skip(1) {
                    indices.push(if at == *index {
                        moved
                    } else {
                        let ty = seeds.function.operand_type(seeds.context, original).expect("an index");
                        let bits = seeds.context.types.int_bits(ty).expect("an integer index");
                        counting::constant(seeds.context, &BigInt::from(0), bits)
                    });
                }
                // The moved base may point outside the object: no `inbounds`.
                let mut operands = vec![op.operands[0]];
                operands.extend(indices);
                let base = seeds.function.create_instruction(op.opcode.clone(), op.ty, operands, Flags::default(), None);
                seeds.function.insert(base, Position::Before(seeds.at)).expect("`at` is placed");
                let mut operands = op.operands.clone();
                operands[0] = Operand::Value(seeds.function.instruction(base).result.expect("an address"));
                seeds.function.set_flags(*inst, Flags::default());
                out.push((*inst, operands));
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "indvars_tests.rs"]
mod tests;
