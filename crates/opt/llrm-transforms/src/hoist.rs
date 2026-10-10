//! Loop-invariant work done once, before the loop: LLVM's LICM hoisting.
//!
//! Adapted from llrm-core's `optimize/transform.rs` `Hoist`: `hoisted`,
//! `_invariant_run`, `_preheader`, `_crossed_values`, `_cannot_fault` and
//! `_guaranteed_float_work`. Loads ask memoryssa's `Accesses`, from the
//! manager, whether the loop writes them (`transform::_undisturbed`), so the
//! pipeline requires `Summaries`; induction says whether the loop makes a
//! trip, and noreturn which calls end one.
//!
//! What changed with the IR:
//! - A value is SSA, and an operand defined outside the loop dominates the
//!   preheader's end, so the run goes before its terminator. `_placement`,
//!   `_reparented`, `_pruned`, `_effective`, `_starts` and `_rewritten` placed,
//!   renamed and thinned variables and copies there are none of.
//! - A call keeps only the loads it may write in the loop. The old
//!   `motion_blocked` refused a whole loop holding a call, which clobbered the
//!   machine's registers.
//! - What may fault -- a load of what is not known dereferenceable and the
//!   target says may trap at its alignment (`Machine::load_may_trap`), a
//!   division by other than a constant neither 0 nor -1 -- moves only from
//!   where the loop certainly runs it: `_guaranteed`, the old
//!   `_guaranteed_float_work` asked of faults rather than of the x87
//!   environment. Floating arithmetic is pure here.
//! - Dropped: flags (`_crossing`'s refusal), volatile published reads (a
//!   volatile access never moves), and `loopmotion::sunk_stores`, which the old
//!   `Hoist` ran after this and loopmotion's port owns.

use std::collections::BTreeSet;

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::memory::Unit;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::{cfg, induction, noreturn, ranges};
use llrm_mir::context::{ConstantKind, mask};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses};
use llrm_mir::types::Type;

use crate::profit::{self, OperationCosts};

/// `size`: price a run in bytes, every block once (-Os), not in executed work.
pub struct Hoist {
    pub size: bool,
}

impl FunctionPass for Hoist {
    fn name(&self) -> &'static str {
        "hoist"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        // Blocks and edges are as they were.
        if hoisted(unit, analyses, self.size) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Each loop's invariant run moved to its preheader, inner loops first so
/// what leaves one may leave the next, where the function does not price
/// higher for it: a run's values live across the loop may be more than
/// the target's registers hold, and spill (`profit::motion_price`). Whether
/// anything moved.
pub fn hoisted(
    unit: &mut passes::Unit,
    analyses: &mut Analyses,
    size: bool,
) -> bool {
    let graph = cfg::graph(unit.function);
    let shape = analyses.get::<cfg::Shape>(unit.context, unit.layout, unit.function);
    let found = &shape.loops;
    if found.is_empty() {
        return false;
    }
    // Asked before anything moves: an instruction keeps its id where it goes.
    let Ok(accesses) = Accesses::managed(unit.context, unit.layout, unit.function, analyses) else { return false };
    let outer = std::rc::Rc::clone(analyses.outer());
    let terminal = analyses.get::<noreturn::TerminalSites>(unit.context, unit.layout, unit.function);
    let room = profit::registers(&outer);
    let costs = if size { outer.target().size_costs() } else { profit::costs(&outer) };
    // Moving instructions leaves every block and loop as they are.
    // The registers are priced where the function is in SSA; before that, with
    // scalar slots still in memory, only what a held value costs to release
    // is.
    let pressure = room.priced() && !_slots_remain(unit);
    let frequency = (pressure || room.priced() && costs.float_release > 0)
        .then(|| _frequency(unit, analyses, &outer, size))
        .flatten();
    let room = if pressure { room } else { crate::spill::Room { registers: 0, ..room } };
    // What is known without memory holds as instructions move: no block, edge
    // or value changes.
    let registers = analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
    let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
    // The price of the function as it stands, when the last motion priced it.
    let mut held = None;
    let mut changed = false;
    for one in found {
        let Some(into) = _preheader(&graph, one) else { continue };
        let mut bounds = || analyses.get::<llrm_analysis::manager::Bounded>(unit.context, unit.layout, unit.function);
        let run = _invariant_run(unit, &outer, one, into, &accesses, &terminal, &registers, &shape, &mut bounds);
        if _crossed_values(unit.function, &run).is_empty() {
            continue;
        }
        let before = unit.function.terminator(cfg::block(into)).expect("a terminator");
        let run = match &frequency {
            Some(frequency) => _affordable(unit, &outer, run, before, into, &costs, room, frequency, &mut held),
            None => run,
        };
        if run.is_empty() {
            continue;
        }
        for inst in run {
            unit.function.move_to(inst, Position::Before(before)).expect("a placed instruction");
        }
        changed = true;
    }
    changed
}

/// `run` less what the function prices higher for: while moving it before
/// `before` costs more than leaving it, the values crossing the loop that the
/// spill model spills, and the instructions reading them, stay in the loop.
/// What the model charges for spilling a value that needs no register, a
/// displacement the accesses fold or a value made again at each read, is not
/// counted.
fn _affordable(
    unit: &passes::Unit,
    outer: &Outer,
    mut run: Vec<InstId>,
    before: InstId,
    into: i64,
    costs: &OperationCosts,
    room: crate::spill::Room,
    frequency: &std::collections::BTreeMap<i64, i64>,
    held: &mut Option<i64>,
) -> Vec<InstId> {
    let price =
        |function: &Function| profit::motion_price(unit.context, unit.layout, outer, function, costs, room, frequency);
    // A run that is let go leaves the function as the clone priced it, whose
    // price is then the next loop's `kept`.
    let Some(kept) = held.take().or_else(|| price(unit.function)) else { return run };
    // Where no register is priced the price is the work alone, a sum over the
    // instructions each at its block's frequency: moving `run` changes it by
    // what each costs at the preheader's frequency less at its own. The
    // function is not cloned and priced again for each loop (the square of the
    // loops: 2.3 G of hoist on `branches` at N=1024, 4x a doubling).
    let unpriced = !room.priced();
    while !run.is_empty() {
        let moved = (!unpriced || cfg!(test)).then(|| {
            let mut hoisted = unit.function.clone();
            for &inst in &run {
                hoisted.move_to(inst, Position::Before(before)).expect("a placed instruction");
            }
            hoisted
        });
        // The price of the moved function is its work and the forecast below,
        // which is found once (it was found twice: 41% of hoist on a 16-deep
        // nest, where the loops' passes cost 6 G).
        let work = if unpriced {
            let at = frequency.get(&into).copied().unwrap_or(1);
            let by: Option<i64> = run
                .iter()
                .map(|&inst| {
                    let from = unit.function.parent(inst).map_or(1, |block| frequency[&cfg::id(block)]);
                    profit::operation(unit.context, unit.layout, unit.function, outer.callees(), inst, costs)
                        .map(|price| price * (at - from))
                })
                .sum();
            let Some(by) = by else { return run };
            #[cfg(test)]
            assert_eq!(
                profit::weighted(
                    unit.context,
                    unit.layout,
                    moved.as_ref().expect("cloned under test"),
                    outer.callees(),
                    costs,
                    frequency
                ),
                Some(kept + by),
                "the work of the moved function is not the kept work and what each instruction's move changes"
            );
            kept + by
        } else {
            let Some(work) = profit::weighted(
                unit.context,
                unit.layout,
                moved.as_ref().expect("cloned where priced"),
                outer.callees(),
                costs,
                frequency,
            ) else {
                return run;
            };
            work
        };
        let hoisted = moved.as_ref().unwrap_or(unit.function);
        // A floating value held across the loop is released after it, once for
        // each time the loop is entered.
        let floats: BTreeSet<ValueId> = _crossed_values(hoisted, &run)
            .into_iter()
            .filter(|&value| matches!(
                unit.context.types.get(hoisted.value(value).ty),
                Type::Float(_)
            ))
            .collect();
        let Some(forecast) = profit::spill_forecast(
            unit.context,
            unit.layout,
            hoisted,
            costs,
            room,
            &|inst| crate::spill::kept_across(outer, unit.context, hoisted, inst),
            frequency,
        ) else {
            return run;
        };
        let moved = work
            + forecast.cost
            + floats.len() as i64 * costs.float_release * frequency.get(&into).copied().unwrap_or(1);
        // Asked of the values the spill model spills that cross the loop, which
        // are none in most loops: the traffic of every cell of the
        // function is not found until one is asked.
        let traffic = std::cell::OnceCell::new();
        let traffic = || {
            traffic.get_or_init(|| {
                let cells = crate::spill::cells(hoisted);
                crate::spill::traffic(hoisted, frequency, &cells, costs, &|_| true, &|value| {
                    crate::spill::words(unit.context, unit.layout, hoisted, value)
                })
            })
        };
        let free = |value: ValueId| {
            _displacement(hoisted, value) || traffic().get(&value).is_some_and(|one| one.rebuild.is_some())
        };
        let crossing = _crossed_values(hoisted, &run)
            .into_iter()
            .filter(|value| forecast.spilled.contains(value))
            .collect::<BTreeSet<_>>();
        let uncounted: i64 = crossing
            .iter()
            .filter(|&&value| free(value))
            .filter_map(|value| traffic().get(value))
            .map(|one| one.price(costs))
            .sum();
        for value in forecast.spilled.iter() {
            if let ValueDef::Instruction(def) = hoisted.value(value).def {
                llrm_support::debug!(
                    "hoist",
                    "  spilled {value:?} = {:?} {:?}",
                    hoisted.instruction(def).opcode,
                    hoisted.instruction(def).operands
                );
            }
        }
        llrm_support::debug!(
            "hoist",
            "kept {kept}, moved {moved}, uncounted {uncounted}, floats {}, crossing {:?}, spilled {:?}",
            floats.len(),
            crossing,
            forecast.spilled
        );
        if moved - uncounted <= kept {
            *held = Some(work + forecast.cost);
            return run;
        }
        let mut stay: BTreeSet<ValueId> = crossing.into_iter().filter(|&value| !free(value)).collect();
        // What costs its release more than it saves stays, with what reads it.
        if costs.float_release > 0 {
            stay.extend(floats);
        }
        if stay.is_empty() {
            // Nothing moves: the function is as it was priced.
            *held = Some(kept);
            return Vec::new();
        }
        // What reads a value that stays cannot leave before it.
        let mut grew = true;
        while grew {
            grew = false;
            for &inst in &run {
                let instruction = unit.function.instruction(inst);
                if instruction
                    .operands
                    .iter()
                    .any(|operand| matches!(operand, Operand::Value(value) if stay.contains(value)))
                {
                    grew |= instruction.result.is_some_and(|result| stay.insert(result));
                }
            }
        }
        // And what only they read has nothing to do before the loop.
        let mut grew = true;
        while grew {
            grew = false;
            for &inst in &run {
                let Some(result) = unit.function.instruction(inst).result else { continue };
                let users = unit.function.users(result);
                if !stay.contains(&result)
                    && !users.is_empty()
                    && users
                        .iter()
                        .all(|one| unit.function.instruction(one.user).result.is_some_and(|read| stay.contains(&read)))
                {
                    grew |= stay.insert(result);
                }
            }
        }
        run.retain(|&inst| unit.function.instruction(inst).result.is_none_or(|result| !stay.contains(&result)));
    }
    // Everything stayed: the function is as it was priced.
    *held = Some(kept);
    run
}

/// Whether `value` is a `getelementptr` by constants that only memory accesses,
/// or such `getelementptr`s, read.
fn _displacement(
    function: &Function,
    value: ValueId,
) -> bool {
    let ValueDef::Instruction(def) = function.value(value).def else { return false };
    let op = function.instruction(def);
    matches!(op.opcode, Opcode::GetElementPtr { .. })
        && op.operands[1..].iter().all(|one| matches!(one, Operand::Constant(_)))
        && crate::spill::address_only(function, value, 3)
}

/// Whether a scalar local is still held in a slot only loaded and stored:
/// `promote` will make it a value, and what is priced before that counts
/// loads that will not be there.
fn _slots_remain(unit: &passes::Unit) -> bool {
    let function = &*unit.function;
    function
        .walk()
        .any(
            |(_, inst)| {
                let Opcode::Alloca { allocated, .. } = function.instruction(inst).opcode else { return false };
                let scalar = matches!(
                    unit.context.types.get(allocated),
                    Type::Int(_) | Type::Float(_) | Type::Pointer(_)
                );
                scalar
                    && function.instruction(inst).result.is_some_and(|slot| {
                        function
                            .users(slot)
                            .iter()
                            .all(
                                |one| match function.instruction(one.user).opcode {
                                    Opcode::Load { .. } => one.index == 0,
                                    Opcode::Store { .. } => one.index == 1,
                                    _ => false,
                                },
                            )
                    })
            },
        )
}

/// Each block's executions per entry: as `_frequencies` finds them from the
/// loops' proven trips, or once for all where `size` counts bytes.
fn _frequency(
    unit: &passes::Unit,
    analyses: &mut Analyses,
    outer: &Outer,
    size: bool,
) -> Option<std::collections::BTreeMap<i64, i64>> {
    if size {
        return Some(unit.function.layout().iter().map(|&block| (cfg::id(block), profit::UNIT)).collect());
    }
    let facts = analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
    let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
    let counted = analyses.get::<llrm_analysis::manager::Counted>(unit.context, unit.layout, unit.function);
    let trips = profit::proven_trips(
        &Unit::within(unit.context, unit.layout, unit.function, outer)
            .with_shape(&shape)
            .with_registers(&facts)
            .with_counted(&counted),
        &facts,
    );
    profit::_frequencies(unit.context, unit.metadata, &outer.globals, unit.function, Some(&trips))
}

/// The one block entering `loop_` from outside it.
pub fn _preheader(
    graph: &[cfg::Block],
    loop_: &Loop,
) -> Option<i64> {
    let outside: Vec<i64> = graph
        .iter()
        .filter(|block| block.succ.contains(&loop_.header) && !loop_.body.contains(&block.at))
        .map(|block| block.at)
        .collect();
    match outside[..] {
        [one] => Some(one),
        _ => None,
    }
}

/// The loop's instructions whose results never change, in an order each
/// reads only what is outside the loop or earlier in it. Grown, to a fixed
/// point.
pub fn _invariant_run(
    unit: &passes::Unit,
    outer: &Outer,
    loop_: &Loop,
    into: i64,
    accesses: &Accesses,
    terminal: &BTreeSet<InstId>,
    registers: &llrm_support::hash::IndexMap<ValueId, llrm_analysis::consts::Known>,
    shape: &llrm_analysis::cfg::Shape,
    bounds: &mut dyn FnMut() -> std::rc::Rc<Result<ranges::Bounds, String>>,
) -> Vec<InstId> {
    let function = &*unit.function;
    let inside = |block: BlockId| loop_.body.contains(&cfg::id(block));
    let insts: Vec<InstId> = function
        .layout()
        .iter()
        .filter(|&&block| inside(block))
        .flat_map(|&block| function.block(block).instructions().to_vec())
        .collect();
    let outside = |value: ValueId| match function.value(value).def {
        ValueDef::Argument(_) => true,
        ValueDef::Instruction(def) => function.parent(def).is_some_and(|block| !inside(block)),
    };
    let mut certain: Option<BTreeSet<InstId>> = None;
    let mut bounded: Option<std::rc::Rc<Result<ranges::Bounds, String>>> = None;
    let mut run: Vec<InstId> = Vec::new();
    let mut taken: llrm_mir::dense::IdSet<InstId> = llrm_mir::dense::IdSet::new();
    // Whether an instruction may move is a fact of the loop, not of how much of
    // it has moved: asked of each once, not once a round.
    let mut movable: llrm_mir::dense::IdMap<InstId, bool> = llrm_mir::dense::IdMap::new();
    let mut made: BTreeSet<ValueId> = BTreeSet::new();
    let writers = llrm_analysis::memoryssa::Writers::of(accesses, &insts);
    loop {
        let mut grew = false;
        for &inst in &insts {
            if taken.contains(&inst)
                || !*movable
                    .get_or_insert_with(inst, || _movable(unit, inst, &writers, accesses, Some(outer.program())))
            {
                continue;
            }
            let ready = function
                .instruction(inst)
                .operands
                .iter()
                .all(
                    |&operand| match operand {
                        Operand::Value(value) => made.contains(&value) || outside(value),
                        Operand::Constant(_) => true,
                        Operand::Block(_) => false,
                    },
                );
            if !ready
                || (_may_fault(unit, outer, inst)
                    && !certain
                        .get_or_insert_with(|| _guaranteed(unit, outer, loop_, into, terminal, registers, shape))
                        .contains(&inst)
                    && !_bounded_inside(unit, outer, inst, into, loop_.header, &mut bounded, bounds, registers, shape))
            {
                continue;
            }
            run.push(inst);
            taken.insert(inst);
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
fn _movable(
    unit: &passes::Unit,
    inst: InstId,
    writers: &llrm_analysis::memoryssa::Writers,
    accesses: &Accesses,
    program: Option<&llrm_mir::program::ProgramProxy>,
) -> bool {
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
        // Memory in a `noalias readonly` parameter is written by no one: a
        // slice's length, read past a store through its data pointer.
        Opcode::Load { volatile: false, .. } => {
            llrm_mir::memory::invariant_load(unit.context, unit.layout, unit.function, inst)
                || accesses.references.get(&inst).is_some_and(|read| writers.spare(accesses, program, read))
        }
        _ => false,
    }
}

/// Whether `inst` may fault where the loop would not have run it: a load of
/// what is not known dereferenceable, at an alignment the target says may
/// trap; or a division that may trap.
fn _may_fault(
    unit: &passes::Unit,
    outer: &Outer,
    inst: InstId,
) -> bool {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { .. } => {
            let (pointer, bytes) =
                (instruction.operands[0], unit.layout.store_size(&unit.context.types, instruction.ty));
            !llrm_mir::valuetracking::dereferenceable(
                unit.context,
                unit.layout,
                outer.sizes(),
                unit.function,
                pointer,
                bytes,
            ) && outer.target().load_may_trap(
                bytes,
                llrm_mir::valuetracking::alignment(unit.context, unit.layout, &outer.globals, unit.function, pointer),
            )
        }
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem | BinaryOp::SDiv | BinaryOp::SRem) => !_cannot_fault(unit, inst),
        _ => false,
    }
}

/// Whether `inst` loads only bytes inside its object wherever the
/// preheader `into` enters the loop at `header`, its index's bounds on
/// that edge as `ranges` finds them.
fn _bounded_inside(
    unit: &passes::Unit,
    outer: &Outer,
    inst: InstId,
    into: i64,
    header: i64,
    bounded: &mut Option<std::rc::Rc<Result<ranges::Bounds, String>>>,
    bounds: &mut dyn FnMut() -> std::rc::Rc<Result<ranges::Bounds, String>>,
    registers: &llrm_support::hash::IndexMap<ValueId, llrm_analysis::consts::Known>,
    shape: &llrm_analysis::cfg::Shape,
) -> bool {
    if !matches!(unit.function.instruction(inst).opcode, Opcode::Load { .. }) {
        return false;
    }
    let memory =
        Unit::within(unit.context, unit.layout, unit.function, outer).with_registers(registers).with_shape(shape);
    let Some(reference) = memory.reference(inst) else { return false };
    // What the counted loops bound is the manager's, asked when first needed.
    let held = bounded.get_or_insert_with(|| bounds());
    // What holds on the edge into the loop, as the preheader's branch narrows
    // it.
    let scope = held.as_ref().as_ref().ok().and_then(|facts| facts.at(into)).cloned().unwrap_or_default();
    let Ok(Some(known)) = ranges::on_edge(&memory, cfg::block(into), cfg::block(header), &scope, None) else {
        return false;
    };
    ranges::inside_object(&memory, &reference, &known.into_iter().collect())
}

/// Whether this divide can be performed where it might not have been: by a
/// constant other than 0, and than -1 where it is signed.
pub fn _cannot_fault(
    unit: &passes::Unit,
    inst: InstId,
) -> bool {
    let instruction = unit.function.instruction(inst);
    let Operand::Constant(id) = instruction.operands[1] else { return false };
    let ConstantKind::Int(bits) = unit.context.get(id).kind else { return false };
    let width = unit.context.types.int_bits(instruction.ty).expect("an integer division");
    let signed = matches!(
        instruction.opcode,
        Opcode::Binary(BinaryOp::SDiv | BinaryOp::SRem)
    );
    bits != 0 && !(signed && bits == mask(width))
}

/// The instructions of `loop_` that run whenever control enters it from
/// `into`: the header's, and once induction proves a trip, those of every
/// block each path from the header's one successor in the loop reaches
/// before leaving or coming back. A call that cannot return ends its block.
pub fn _guaranteed(
    unit: &passes::Unit,
    outer: &Outer,
    loop_: &Loop,
    into: i64,
    terminal: &BTreeSet<InstId>,
    registers: &llrm_support::hash::IndexMap<ValueId, llrm_analysis::consts::Known>,
    shape: &llrm_analysis::cfg::Shape,
) -> BTreeSet<InstId> {
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
    let starts: Vec<i64> =
        successors(loop_.header).into_iter().filter(|at| loop_.body.contains(at) && *at != loop_.header).collect();
    let [start] = starts[..] else { return out };
    if ends(loop_.header)
        || !induction::nonempty(
            &Unit::within(unit.context, unit.layout, function, outer).with_registers(registers).with_shape(shape),
            loop_,
        )
    {
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
pub fn _crossed_values(
    function: &Function,
    run: &[InstId],
) -> BTreeSet<ValueId> {
    run.iter()
        .filter_map(|&inst| function.instruction(inst).result)
        .filter(|&value| function.users(value).iter().any(|one| !run.contains(&one.user)))
        .collect()
}

#[cfg(test)]
#[path = "hoist_tests.rs"]
mod tests;
