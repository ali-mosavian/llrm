//! A counted loop of exact float work in memory, run once: its last trip,
//! from the memory the trips before leave, which floatfacts proves. Adapted
//! from llrm-core's `optimize/floatloop.rs`, the port of
//! `qbopt/optimize/floatloop.py`. LLVM has no one counterpart; its
//! IndVarSimplify rewrites exit values and its SCCP folds a computable
//! loop's effects.
//!
//! What changed with the IR:
//! - The seeds go first in the latch, where the old ones followed its first
//!   float load, the x87 checkpoint.
//! - The counter's value after the loop is its final number, placed by
//!   `SsaUpdater` for every use outside the loop, a phi's included.
//! - Header phis lose the latch's arm and, left with one, are that value. What
//!   the loop no longer reads is Dead's.
//! - A seed's pointer must be there before the latch: a constant or a value
//!   from outside it.
//!
//! Dropped, no rich MIR analogue: the checkpoint and `floatfacts::checkpoint`
//! (the rich MIR observes no FP exception), `strength::_made` (the final
//! counter is a constant, no copy), `transform::_leaving` (every use is an
//! operand). With no exception to observe the last trip could go too; it
//! stays, as the old pass kept it.
//!
//! floatbounds has no consumer here, nor elsewhere on the rich MIR.
//!
//! Tests, in `floatloop_tests.rs`:
//! `test_exact_loop_retains_checkpoint_and_final_iteration`,
//! `test_emitted_final_answer_has_the_correct_symbol` and
//! `test_unproved_or_observable_iterations_remain`, as MIR. Skipped:
//! `test_checkpoint_with_additional_effects_is_not_ignored` (`Fcheck`),
//! `test_floatloop_can_be_disabled_for_stage_bisection` (the pipeline's
//! options) and
//! `test_checkpoint_keeps_initial_memory_and_counter_stores_for_an_error_handler`
//! (loopmotion's ON ERROR stores).

use std::collections::BTreeSet;

use llrm_analysis::consts::{self, Calls};
use llrm_analysis::floatfacts::{self, LoopExit, Solved};
use llrm_analysis::manager::{self, FloatFacts};
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::ssa::SsaUpdater;
use llrm_analysis::{cfg, induction, regions};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Outer, PreservedAnalyses};
use llrm_mir::types::TypeId;
use num_bigint::BigInt;

use crate::lcssa::{arms, from_arms};
use crate::transform::live;

pub struct FloatLoop;

impl FunctionPass for FloatLoop {
    fn name(&self) -> &'static str {
        "floatloop"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        // With no float in the body no loop is a float loop.
        // LLRM_CHECK_FLOATSKIP runs the pass anyway and says if it
        // changed anything.
        let none = !floatfacts::touches(unit.context, unit.function);
        if none && !llrm_support::env_set("LLRM_CHECK_FLOATSKIP") {
            return PreservedAnalyses::all();
        }
        let before = none.then(|| unit.function.clone());
        let preserved = self.floated(unit, analyses);
        if let Some(before) = before {
            assert!(
                preserved.are_all_preserved()
                    && llrm_mir::print::body(unit.context, unit.function)
                        == llrm_mir::print::body(unit.context, &before),
                "LLRM_CHECK_FLOATSKIP: floatloop changed a body with no float in it"
            );
        }
        preserved
    }
}

impl FloatLoop {
    fn floated(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let calls = manager::writes(unit.context, unit.layout, unit.function, analyses);
        let solved = analyses.get::<FloatFacts>(unit.context, unit.layout, unit.function);
        if specialized(
            unit.context,
            unit.layout,
            analyses.outer().callees(),
            unit.function,
            analyses.outer(),
            &calls,
            &solved,
        ) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// A store to put before the last trip, or after the loop.
struct _Store {
    pointer: Operand,
    ty: TypeId,
    constant: ConstantKind,
}

/// What `_rewritten` does to one loop.
struct _Plan {
    header: BlockId,
    latch: BlockId,
    exit: BlockId,
    counter: ValueId,
    seeds: Vec<_Store>,
    finals: Vec<_Store>,
    last: u128,
}

/// The first counted exact float loop run as its last trip alone, given
/// `solved`, floatfacts' solve of `function` with `calls`. Whether one was.
#[allow(clippy::too_many_arguments)]
pub fn specialized(
    context: &mut Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    outer: &Outer,
    calls: &Calls,
    solved: &Solved,
) -> bool {
    let plan = {
        let shape = llrm_analysis::cfg::Shape::of(function);
        let unit = Unit::within(context, layout, function, outer).with_shape(&shape);
        let alive = live(context, callees, function);
        floatfacts::exits(&unit, calls, solved)
            .into_iter()
            .filter(|proof| proof.count > BigInt::from(1))
            .find_map(|proof| _planned(&unit, calls, solved, &alive, &proof))
    };
    let Some(plan) = plan else {
        return false;
    };
    _rewritten(context, function, plan);
    true
}

/// Whether `reference` is read in `insts` before a store covers it.
fn _carried(
    unit: &Unit,
    reference: &MemRef,
    insts: &[InstId],
) -> bool {
    for &inst in insts {
        let Some(access) = MemRef::of(unit, inst) else {
            continue;
        };
        match unit.function.instruction(inst).opcode {
            Opcode::Load { .. }
                if regions::overlapping(reference, &access, None, None, unit.program).unwrap_or(true) =>
            {
                return true;
            }
            Opcode::Store { .. }
                if access.addr().is_some() && access.addr() == reference.addr() && access.width == reference.width =>
            {
                return false;
            }
            _ => {}
        }
    }
    false
}

fn _planned(
    unit: &Unit,
    calls: &Calls,
    solved: &Solved,
    alive: &BTreeSet<ValueId>,
    proof: &LoopExit,
) -> Option<_Plan> {
    let function = unit.function;
    let loop_ = unit.shape().loops.iter().find(|one| one.header == proof.header)?.clone();
    let (header, latch) = (cfg::block(loop_.header), cfg::block(*loop_.latches.first()?));
    let [exit] =
        function.successors(header).into_iter().filter(|one| !loop_.body.contains(&cfg::id(*one))).collect::<Vec<_>>()
            [..]
    else {
        return None;
    };
    if function.predecessors(exit) != [header]
        || function.block(exit).instructions().iter().any(|&inst| function.instruction(inst).opcode == Opcode::Phi)
    {
        return None;
    }
    let header_insts = function.block(header).instructions();
    let latch_insts = function.block(latch).instructions();
    let phis = header_insts
        .iter()
        .copied()
        .filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .collect::<Vec<_>>();
    let [phi] = phis
        .iter()
        .copied()
        .filter(|&inst| alive.contains(&function.instruction(inst).result.expect("a phi's value")))
        .collect::<Vec<_>>()[..]
    else {
        return None;
    };
    let counter = function.instruction(phi).result.expect("a phi's value");
    let affine = induction::basics(unit, &loop_).shift_remove(&counter)?;
    let width = unit.int_bits(Operand::Value(counter))?;
    let start = induction::_signed(&affine.start, &solved.integers, width)?;
    let step = induction::_signed(&affine.step, &solved.integers, width)?;
    let (update, _) = arms(function, phi).into_iter().find(|&(_, from)| from == latch)?;
    // The latch: float work that never reads the counter, the update, and its
    // branch.
    for &inst in latch_insts {
        let op = function.instruction(inst);
        let reads_counter = op.operands.contains(&Operand::Value(counter));
        let fine = match op.opcode {
            Opcode::Br => op.operands.len() == 1,
            _ if floatfacts::rule(unit, inst).is_some() => !reads_counter,
            Opcode::Binary(_) => op.result.map(Operand::Value) == Some(update),
            _ => false,
        };
        if !fine {
            return None;
        }
    }
    // The header stores nothing but the counter.
    let mut finals = Vec::new();
    for &inst in header_insts {
        let op = function.instruction(inst);
        if let Opcode::Store { .. } = op.opcode {
            if op.operands[0] != Operand::Value(counter) {
                return None;
            }
            finals.push((op.operands[1], function.value(counter).ty));
        }
    }
    // Nothing the loop computes but its counter is read outside it.
    let inside = |inst: InstId| function.parent(inst).is_some_and(|block| loop_.body.contains(&cfg::id(block)));
    let leaves = header_insts
        .iter()
        .chain(latch_insts)
        .filter_map(|&inst| function.instruction(inst).result)
        .filter(|&value| value != counter);
    for value in leaves.collect::<Vec<_>>() {
        if function.users(value).iter().any(|one| !inside(one.user)) {
            return None;
        }
    }
    let [preheader] = function
        .predecessors(header)
        .into_iter()
        .filter(|one| !loop_.body.contains(&cfg::id(*one)))
        .collect::<Vec<_>>()[..]
    else {
        return None;
    };
    let before = function.terminator(preheader)?;
    let mut asked = consts::memory_queries(*unit, &solved.integers);
    let mut references = latch_insts.iter().filter_map(|&inst| MemRef::of(unit, inst)).collect::<Vec<_>>();
    references.extend(proof.stores.iter().map(|(reference, _)| reference.clone()));
    let initial = consts::_kills(
        floatfacts::cells_before(unit, calls, solved, cfg::id(preheader), &references, &mut asked),
        before,
        &solved.integers,
        calls,
        None,
        None,
        false,
        &mut asked,
    );
    let before_last = floatfacts::repeated(
        unit,
        latch_insts,
        &(&proof.count - 1),
        &initial,
        Some(&solved.integers),
        Some(&mut asked),
    )?;
    let mut seeds = Vec::new();
    for (reference, _) in &proof.stores {
        if !_carried(unit, reference, latch_insts) {
            continue;
        }
        let fact = consts::_cell(&before_last, &asked.resolve(reference))?;
        let owner = latch_insts
            .iter()
            .copied()
            .find(
                |&inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. })
                    && MemRef::of(unit, inst).as_ref() == Some(reference),
            )?;
        let (value, pointer) = (function.instruction(owner).operands[0], function.instruction(owner).operands[1]);
        let outside = match pointer {
            Operand::Value(one) => match function.value(one).def {
                ValueDef::Instruction(defining) => function.parent(defining) != Some(latch),
                ValueDef::Argument(_) => true,
            },
            _ => true,
        };
        if !outside {
            return None;
        }
        let bits = u64::try_from(&fact.n).ok()?;
        seeds.push(_Store { pointer, ty: unit.operand_type(value)?, constant: ConstantKind::Float(bits) });
    }
    let last = consts::masked(&(start + step * &proof.count), width);
    let last = u128::try_from(&last).expect("a masked number");
    let finals =
        finals.into_iter().map(|(pointer, ty)| _Store { pointer, ty, constant: ConstantKind::Int(last) }).collect();
    Some(_Plan { header, latch, exit, counter, seeds, finals, last })
}

/// A store of `store`'s constant at `position`.
fn _store(
    context: &mut Context,
    function: &mut Function,
    store: _Store,
    position: Position,
) {
    let value = context.constant(Constant { ty: store.ty, kind: store.constant });
    let void = context.types.void();
    let made = function.create_instruction(
        Opcode::Store { align: None, volatile: false },
        void,
        vec![Operand::Constant(value), store.pointer],
        Flags::default(),
        None,
    );
    function.insert(made, position).expect("a placed block");
}

fn _rewritten(
    context: &mut Context,
    function: &mut Function,
    plan: _Plan,
) {
    let _Plan { header, latch, exit, counter, seeds, finals, last } = plan;
    let first = function.block(latch).instructions()[0];
    for seed in seeds {
        _store(context, function, seed, Position::Before(first));
    }
    let exit_first = function.block(exit).instructions()[0];
    for store in finals {
        _store(context, function, store, Position::Before(exit_first));
    }
    // Header to the latch, the latch out.
    let branch = function.terminator(header).expect("a terminated header");
    function.set_operands(branch, vec![Operand::Block(latch)]);
    let back = function.terminator(latch).expect("a terminated latch");
    function.set_operands(back, vec![Operand::Block(exit)]);
    // After the loop, the counter is its final number.
    let ty = function.value(counter).ty;
    let final_ = Operand::Constant(context.int(ty, last as i128));
    let mut updater = SsaUpdater::new(ty, function.value(counter).name.as_deref());
    updater.add_available_value(latch, final_);
    let within = [header, latch];
    let outside = function
        .users(counter)
        .iter()
        .copied()
        .filter(|one| !function.parent(one.user).is_some_and(|block| within.contains(&block)))
        .collect::<Vec<_>>();
    for one in outside {
        updater.rewrite_use(context, function, one);
    }
    // The header's phis lose the back edge, each then its one input.
    for phi in function.block(header).instructions().to_vec() {
        if function.instruction(phi).opcode != Opcode::Phi {
            continue;
        }
        let kept = arms(function, phi).into_iter().filter(|&(_, from)| from != latch).collect::<Vec<_>>();
        let result = function.instruction(phi).result.expect("a phi's value");
        let [(value, _)] = kept[..] else {
            function.set_operands(phi, from_arms(&kept));
            continue;
        };
        function.replace_all_uses_with(result, value);
        function.erase(phi).expect("its uses were replaced");
    }
}

#[cfg(test)]
#[path = "floatloop_tests.rs"]
mod tests;
