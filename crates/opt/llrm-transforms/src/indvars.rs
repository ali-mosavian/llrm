//! IndVarSimplify's exit rewriting reused across an outer loop: llrm-core's
//! `optimize/indvars.rs`, the port of `qbopt/optimize/indvars.py`, adapted
//! to the rich MIR. `widened` is `widenIV` for a counter that needs no sign proof.
//! `rewound` carries an exact inner recurrence's exit value
//! around the outer loop, less its distance, in place of its saved start.
//! Which counters a loop keeps, and which ends it, is `lsr`'s.

use std::collections::BTreeSet;

use llrm_analysis::consts::{Known, masked};
use llrm_analysis::induction::{self, AffineOperand};
use llrm_analysis::manager::Registers;
use llrm_analysis::{cfg, liveness, memory};
use llrm_analysis::graph::loops::Loop;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::Zero;

use crate::counting;
use crate::lcssa::{arms, from_arms};
use crate::profit::OperationCosts;
use crate::dead;

/// What IndVarSimplify leaves a loop's counters: values after a loop
/// computed from its trips (`loopexit::evaluated`), final updates sunk to
/// the exits (`exitsink`), and an inner recurrence's exit value carried
/// round the loop outside (`rewound`). Which counters the loop keeps is
/// `lsr`'s.
pub struct IndVars;

impl FunctionPass for IndVars {
    fn name(&self) -> &'static str {
        "indvars"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let evaluated = crate::loopexit::evaluated(unit.context, unit.layout, unit.function, &outer).unwrap_or_else(|error| panic!("indvars: {error}"));
        let folded = crate::exitfold::folded(unit.context, unit.layout, unit.function, &outer);
        let sunk = crate::exitsink::sunk(unit.function);
        let widened = widened(unit.context, unit.layout, unit.function, &outer);
        let rewound = rewound(unit.context, unit.layout, unit.function, analyses, crate::profit::registers(&outer).registers, &crate::profit::costs(&outer));
        let dead = (sunk || rewound || widened) && dead::dead(unit.context, outer.callees(), unit.function);
        if evaluated || folded {
            PreservedAnalyses::none()
        } else if sunk || rewound || widened || dead {
            // Blocks and edges are as they were.
            PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>()
        } else {
            PreservedAnalyses::all()
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


#[cfg(test)]
#[path = "indvars_tests.rs"]
mod tests;

/// A counter read through `zext` to the default address's index width is a counter
/// of that width, where `induction` proves its values never pass its own
/// (`rises_unsigned`): C's `unsigned short i` reads `a[zext i]`, a conversion
/// per trip, and its counted-loop proof is narrower than the address, so the
/// idioms and strength reduction that want the address's width find none. The
/// wide counter ends the loop against the bound extended once, before it, and
/// what else reads the narrow one reads its low part. Every counter widened;
/// whether any was.
pub fn widened(context: &mut Context, layout: &DataLayout, function: &mut Function, outer: &passes::Outer) -> bool {
    let mut changed = false;
    'again: loop {
        for loop_ in cfg::Shape::of(function).loops {
            if let Some(found) = _find(&llrm_analysis::memory::Unit::within(context, layout, function, outer), &loop_) {
                _widen(context, function, &found);
                changed = true;
                continue 'again;
            }
        }
        return changed;
    }
}

struct _Found {
    phi: InstId,
    next: InstId,
    compare: InstId,
    preheader: BlockId,
    latch: BlockId,
    start: u128,
    bound: Operand,
    wide: u32,
    extensions: Vec<InstId>,
    /// Every other reader: its instruction and operand.
    rest: Vec<(InstId, u32)>,
}

fn _find(unit: &llrm_analysis::memory::Unit, loop_: &Loop) -> Option<_Found> {
    let function = unit.function;
    let header = cfg::block(loop_.header);
    let outside = function.predecessors(header).into_iter().filter(|&one| !loop_.body.contains(&cfg::id(one))).collect::<Vec<_>>();
    let [preheader] = outside[..] else { return None };
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    let latch = cfg::block(latch);
    let still = induction::invariant(function, &loop_.body);
    let facts = unit.registers();
    for proof in induction::counted(unit, loop_, Some(&facts), false) {
        let AffineOperand::Const(start) = &proof.start else { continue };
        let width = proof.width();
        if !proof.rises_unsigned() || width > 64 {
            continue;
        }
        let compare = function.instruction(proof.compare);
        let (Operand::Value(counter), bound) = (compare.operands[0], compare.operands[1]) else { continue };
        if !still.operand(bound) || function.parent(proof.phi) != Some(header) || function.instruction(proof.phi).result != Some(counter) {
            continue;
        }
        let arms = function.instruction(proof.phi).operands.chunks(2).map(|pair| (pair[0], pair[1])).collect::<Vec<_>>();
        let [(_, Operand::Block(first_from)), (second, Operand::Block(second_from))] = arms[..] else { continue };
        let next = if second_from == latch && first_from == preheader { second } else { continue };
        let Operand::Value(next_value) = next else { continue };
        let ValueDef::Instruction(next_inst) = function.value(next_value).def else { continue };
        let adds = function.instruction(next_inst);
        if adds.opcode != Opcode::Binary(BinaryOp::Add) || adds.operands[0] != Operand::Value(counter) || unit.int_constant(adds.operands[1]) != Some(1) {
            continue;
        }
        if function.users(next_value).iter().any(|one| one.user != proof.phi) {
            continue;
        }
        // The default space's index width, which a use already extends to.
        let mut wide = None;
        for one in function.users(counter) {
            let user = function.instruction(one.user);
            if user.opcode != Opcode::Cast(CastOp::ZExt) {
                continue;
            }
            let to = unit.int_bits(Operand::Value(user.result?))?;
            if to > width && unit.layout.pointer(0).index_bits == to {
                wide = Some(to);
            }
        }
        let Some(wide) = wide else { continue };
        let mut extensions = Vec::new();
        let mut rest = Vec::new();
        for one in function.users(counter) {
            let user = function.instruction(one.user);
            if one.user == proof.compare || one.user == next_inst {
                continue;
            }
            if user.opcode == Opcode::Cast(CastOp::ZExt) && unit.int_bits(Operand::Value(user.result?)) == Some(wide) {
                extensions.push(one.user);
            } else {
                rest.push((one.user, one.index));
            }
        }
        return Some(_Found { phi: proof.phi, next: next_inst, compare: proof.compare, preheader, latch, start: u128::try_from(&start.n).ok()?, bound, wide, extensions, rest });
    }
    None
}

fn _widen(context: &mut Context, function: &mut Function, found: &_Found) {
    let ty = context.types.int(found.wide);
    let counter = function.instruction(found.phi).result.expect("a phi's value");
    let header = function.parent(found.phi).expect("a placed phi");
    // The bound, extended once before the loop.
    let bound = match found.bound {
        Operand::Constant(constant) => match context.get(constant).kind {
            llrm_mir::context::ConstantKind::Int(bits) => Operand::Constant(context.int(ty, bits as i128)),
            _ => unreachable!("an integer bound"),
        },
        value => {
            let cast = function.create_instruction(Opcode::Cast(CastOp::ZExt), ty, vec![value], Flags::default(), None);
            let end = function.terminator(found.preheader).expect("a terminated preheader");
            function.insert(cast, Position::Before(end)).expect("a preheader");
            Operand::Value(function.instruction(cast).result.expect("a value"))
        }
    };
    let first = function.block(header).instructions()[0];
    let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), Some("widen.iv"));
    function.insert(phi, Position::Before(first)).expect("a header");
    let value = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
    let one = Operand::Constant(context.int(ty, 1));
    let next = function.create_instruction(Opcode::Binary(BinaryOp::Add), ty, vec![value, one], Flags::default(), Some("widen.iv.next"));
    function.insert(next, Position::Before(found.next)).expect("a latch");
    let next = Operand::Value(function.instruction(next).result.expect("a value"));
    let start = Operand::Constant(context.int(ty, found.start as i128));
    function.set_operands(phi, vec![start, Operand::Block(found.preheader), next, Operand::Block(found.latch)]);
    let bit = context.types.int(1);
    let test = function.create_instruction(Opcode::ICmp(IntPredicate::Ult), bit, vec![value, bound], Flags::default(), None);
    function.insert(test, Position::Before(found.compare)).expect("a placed compare");
    let test = Operand::Value(function.instruction(test).result.expect("a value"));
    let old = function.instruction(found.compare).result.expect("a value");
    function.replace_all_uses_with(old, test);
    function.erase(found.compare).expect("its uses were replaced");
    for &extension in &found.extensions {
        let old = function.instruction(extension).result.expect("a value");
        function.replace_all_uses_with(old, value);
        function.erase(extension).expect("its uses were replaced");
    }
    // What else reads the counter reads its low part, cut where it is read.
    let narrow = function.value(counter).ty;
    for &(user, index) in &found.rest {
        let at = match function.instruction(user).opcode {
            Opcode::Phi => {
                let Operand::Block(from) = function.instruction(user).operands[index as usize + 1] else { unreachable!("a phi arm names its block") };
                function.terminator(from).expect("a terminated block")
            }
            _ => user,
        };
        let cut = function.create_instruction(Opcode::Cast(CastOp::Trunc), narrow, vec![value], Flags::default(), None);
        function.insert(cut, Position::Before(at)).expect("a placed reader");
        let cut = Operand::Value(function.instruction(cut).result.expect("a value"));
        function.set_operand(user, index as usize, cut);
    }
}
