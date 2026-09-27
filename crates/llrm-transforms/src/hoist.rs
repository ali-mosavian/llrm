//! Loop-invariant work done once, before the loop: LLVM's LICM hoisting.
//!
//! Adapted from llrm-core's `optimize/transform.rs` `Hoist`: `hoisted`,
//! `_invariant_run`, `_preheader`, `_crossed_values`, `_cannot_fault` and
//! `_guaranteed_float_work`. Loads ask memoryssa's `Accesses`, from the
//! manager, whether the loop writes them (`transform::_unwritten`), so the
//! pipeline requires `Summaries`; induction says whether the loop makes a
//! trip, and noreturn which calls end one.
//!
//! What changed with the IR:
//! - A value is SSA, and an operand defined outside the loop dominates the
//!   preheader's end, so the run goes before its terminator. `_placement`,
//!   `_reparented`, `_pruned`, `_effective`, `_starts` and `_rewritten`
//!   placed, renamed and thinned variables and copies there are none of.
//! - A call keeps only the loads it may write in the loop. The old
//!   `motion_blocked` refused a whole loop holding a call, which clobbered
//!   the machine's registers.
//! - What may fault -- a load of what is not known dereferenceable, a
//!   division by other than a constant neither 0 nor -1 -- moves only from
//!   where the loop certainly runs it: `_guaranteed`, the old
//!   `_guaranteed_float_work` asked of faults rather than of the x87
//!   environment. Floating arithmetic is pure here.
//! - Dropped: flags (`_crossing`'s refusal), volatile published reads (a
//!   volatile access never moves), and `loopmotion::sunk_stores`, which the
//!   old `Hoist` ran after this and loopmotion's port owns.

use std::collections::BTreeSet;

use llrm_analysis::memory::Unit;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::{cfg, induction, noreturn};
use llrm_graph::loops::Loop;
use llrm_mir::context::{ConstantKind, mask};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses};

use crate::transform::_unwritten;

pub struct Hoist;

impl FunctionPass for Hoist {
    fn name(&self) -> &'static str {
        "hoist"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        // Blocks and edges are as they were.
        if hoisted(unit, analyses) { PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>() } else { PreservedAnalyses::all() }
    }
}

/// Each loop's invariant run moved to its preheader, inner loops first so
/// what leaves one may leave the next. Whether anything moved.
pub fn hoisted(unit: &mut passes::Unit, analyses: &mut Analyses) -> bool {
    let graph = cfg::graph(unit.function);
    let found = cfg::Shape::of(unit.function).loops;
    if found.is_empty() {
        return false;
    }
    // Asked before anything moves: an instruction keeps its id where it goes.
    let Ok(accesses) = Accesses::managed(unit.context, unit.layout, unit.function, analyses) else { return false };
    let outer = std::rc::Rc::clone(analyses.outer());
    let terminal = noreturn::terminal_sites(unit.context, &outer.globals, unit.function, &BTreeSet::new());
    let mut changed = false;
    for one in &found {
        let Some(into) = _preheader(&graph, one) else { continue };
        let run = _invariant_run(unit, &outer, one, into, &accesses, &terminal);
        if _crossed_values(unit.function, &run).is_empty() {
            continue;
        }
        let before = unit.function.terminator(cfg::block(into)).expect("a terminator");
        for inst in run {
            unit.function.move_to(inst, Position::Before(before)).expect("a placed instruction");
        }
        changed = true;
    }
    changed
}

/// The one block entering `loop_` from outside it.
pub fn _preheader(graph: &[cfg::Block], loop_: &Loop) -> Option<i64> {
    let outside: Vec<i64> = graph.iter().filter(|block| block.succ.contains(&loop_.header) && !loop_.body.contains(&block.at)).map(|block| block.at).collect();
    match outside[..] {
        [one] => Some(one),
        _ => None,
    }
}

/// The loop's instructions whose results never change, in an order each
/// reads only what is outside the loop or earlier in it. Grown, to a fixed
/// point.
pub fn _invariant_run(unit: &passes::Unit, outer: &Outer, loop_: &Loop, into: i64, accesses: &Accesses, terminal: &BTreeSet<InstId>) -> Vec<InstId> {
    let function = &*unit.function;
    let inside = |block: BlockId| loop_.body.contains(&cfg::id(block));
    let insts: Vec<InstId> = function.layout().iter().filter(|&&block| inside(block)).flat_map(|&block| function.block(block).instructions().to_vec()).collect();
    let outside = |value: ValueId| match function.value(value).def {
        ValueDef::Argument(_) => true,
        ValueDef::Instruction(def) => function.parent(def).is_some_and(|block| !inside(block)),
    };
    let mut certain: Option<BTreeSet<InstId>> = None;
    let mut run: Vec<InstId> = Vec::new();
    let mut made: BTreeSet<ValueId> = BTreeSet::new();
    loop {
        let mut grew = false;
        for &inst in &insts {
            if run.contains(&inst) || !_movable(unit, inst, &insts, accesses, outer.target.as_deref()) {
                continue;
            }
            let ready = function.instruction(inst).operands.iter().all(|&operand| match operand {
                Operand::Value(value) => made.contains(&value) || outside(value),
                Operand::Constant(_) => true,
                Operand::Block(_) => false,
            });
            if !ready || (_may_fault(unit, inst) && !certain.get_or_insert_with(|| _guaranteed(unit, outer, loop_, into, terminal)).contains(&inst)) {
                continue;
            }
            run.push(inst);
            made.extend(function.instruction(inst).result);
            grew = true;
        }
        if !grew {
            return run;
        }
    }
}

/// Whether `inst` computes only from its operands, or is a load nothing in
/// the loop of `insts` may write.
fn _movable(unit: &passes::Unit, inst: InstId, insts: &[InstId], accesses: &Accesses, machine: Option<&dyn llrm_mir::target::Machine>) -> bool {
    match unit.function.instruction(inst).opcode {
        Opcode::Binary(_)
        | Opcode::Cast(_)
        | Opcode::ICmp(_)
        | Opcode::FCmp(_)
        | Opcode::FNeg
        | Opcode::GetElementPtr { .. }
        | Opcode::Select
        | Opcode::Freeze
        | Opcode::ExtractValue(_)
        | Opcode::InsertValue(_) => true,
        Opcode::Load { volatile: false, .. } => _unwritten(unit.function, inst, insts, accesses, machine),
        _ => false,
    }
}

/// Whether `inst` may fault where the loop would not have run it: a load of
/// what is not known dereferenceable, or a division that may trap.
fn _may_fault(unit: &passes::Unit, inst: InstId) -> bool {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { .. } => {
            let bytes = unit.layout.store_size(&unit.context.types, instruction.ty);
            !llrm_mir::valuetracking::dereferenceable(unit.context, unit.layout, unit.sizes, unit.function, instruction.operands[0], bytes)
        }
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem | BinaryOp::SDiv | BinaryOp::SRem) => !_cannot_fault(unit, inst),
        _ => false,
    }
}

/// Whether this divide can be performed where it might not have been: by a
/// constant other than 0, and than -1 where it is signed.
pub fn _cannot_fault(unit: &passes::Unit, inst: InstId) -> bool {
    let instruction = unit.function.instruction(inst);
    let Operand::Constant(id) = instruction.operands[1] else { return false };
    let ConstantKind::Int(bits) = unit.context.get(id).kind else { return false };
    let width = unit.context.types.int_bits(instruction.ty).expect("an integer division");
    let signed = matches!(instruction.opcode, Opcode::Binary(BinaryOp::SDiv | BinaryOp::SRem));
    bits != 0 && !(signed && bits == mask(width))
}

/// The instructions of `loop_` that run whenever control enters it from
/// `into`: the header's, and once induction proves a trip, those of every
/// block each path from the header's one successor in the loop reaches
/// before leaving or coming back. A call that cannot return ends its block.
pub fn _guaranteed(unit: &passes::Unit, outer: &Outer, loop_: &Loop, into: i64, terminal: &BTreeSet<InstId>) -> BTreeSet<InstId> {
    let function = &*unit.function;
    if function.successors(cfg::block(into)) != [cfg::block(loop_.header)] {
        return BTreeSet::new();
    }
    let ends = |at: i64| function.block(cfg::block(at)).instructions().iter().any(|inst| terminal.contains(inst));
    let before_terminal = |at: i64| {
        let instructions = function.block(cfg::block(at)).instructions();
        let stop = instructions.iter().position(|inst| terminal.contains(inst)).map_or(instructions.len(), |at| at + 1);
        instructions[..stop].to_vec()
    };
    let mut out: BTreeSet<InstId> = before_terminal(loop_.header).into_iter().collect();
    let successors = |at: i64| function.successors(cfg::block(at)).into_iter().map(cfg::id).collect::<Vec<_>>();
    let starts: Vec<i64> = successors(loop_.header).into_iter().filter(|at| loop_.body.contains(at) && *at != loop_.header).collect();
    let [start] = starts[..] else { return out };
    if ends(loop_.header) || !induction::nonempty(&Unit::within(unit.context, unit.layout, function, outer), loop_) {
        return out;
    }
    for &target in &loop_.body {
        // Least fixed point: a path round an inner loop may never arrive.
        let mut sure = BTreeSet::from([target]);
        loop {
            let more: Vec<i64> = loop_
                .body
                .iter()
                .copied()
                .filter(|&at| at != loop_.header && !sure.contains(&at) && !ends(at))
                .filter(|&at| {
                    let next = successors(at);
                    !next.is_empty() && next.iter().all(|one| sure.contains(one))
                })
                .collect();
            if more.is_empty() {
                break;
            }
            sure.extend(more);
        }
        if target != loop_.header && sure.contains(&start) {
            out.extend(before_terminal(target));
        }
    }
    out
}

/// The values the run computes that something outside it reads.
pub fn _crossed_values(function: &Function, run: &[InstId]) -> BTreeSet<ValueId> {
    run.iter()
        .filter_map(|&inst| function.instruction(inst).result)
        .filter(|&value| function.users(value).iter().any(|one| !run.contains(&one.user)))
        .collect()
}

#[cfg(test)]
#[path = "hoist_tests.rs"]
mod tests;
