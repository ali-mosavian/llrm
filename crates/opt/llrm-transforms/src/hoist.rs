//! Moves work whose operands do not change in a loop, and loads nothing in it
//! writes, to before the loop, so it runs once and not every trip: LLVM's LICM
//! hoisting and gcc's `-fmove-loop-invariants`.
//!
//! # How it works
//! Loops are taken inner first, so what leaves one may leave the next. A loop
//! is skipped unless exactly one block outside it enters its header (its
//! preheader). For each loop the pass grows an invariant run to a fixed point:
//! an instruction joins the run when every operand is defined outside the
//! loop or earlier in the run, and it is movable. Movable are pure arithmetic,
//! casts, compares, `gep`, `select`, `freeze`, aggregate extract/insert, and a
//! non-volatile load that nothing in the loop may write (`memoryssa::Accesses`;
//! a load from a `noalias readonly` parameter or marked invariant needs no such
//! proof). A call in the loop blocks only the loads it may write.
//!
//! What may fault moves only where the loop certainly runs it, as the preheader
//! would otherwise run it on a path that did not. That is a load of what is not
//! known dereferenceable and that the target says may trap at its alignment
//! (`Machine::load_may_trap`), or a division by anything but a constant that is
//! neither 0 nor -1 (`_cannot_fault`). It is certain when it is in the header
//! behind an unconditional entry and before any call that cannot return
//! (`_guaranteed`), or when `ranges` shows its index inside the object on the
//! edge into the loop (`_bounded_inside`). Floating arithmetic is pure here.
//!
//! Moving values out lengthens their live ranges. Where the target's registers
//! are priced and the function is in SSA, the moved function is priced against
//! the kept one (`profit::motion_price`: work by block frequency plus the spill
//! forecast). While it prices higher, the values crossing the loop that the
//! spill model spills, and what reads them, stay; a displacement the accesses
//! fold, or a value rebuilt at each read, is not charged. A floating value held
//! across the loop is charged its release. At -Os the run is priced in bytes,
//! every block once. The moved function's liveness is the kept one's with the
//! values the run moved solved again (`profit::spill_forecast_after`;
//! `LLRM_CHECK_VIEW` holds it to the forecast of the function on its own).
//!
//! A hoisted load is priced by what it saves each trip. A floating load its one
//! reader takes from memory (x87 `fmul m32`) is not an instruction of the loop
//! (`OperationCosts::float_load_folded`, the fold's own extra cost), so
//! hoisting it saves that and costs a load and a release before the loop: it
//! stays unless the loop's trips pay for both. And the class holds only so many
//! floating values (`Machine::float_registers`): of those the run holds across
//! the loop, what the loop's own values leave room for are hoisted and the rest
//! stay. A 4x4 matrix read through a pointer in a loop of six trips was hoisted
//! whole, 13 of its 16 values spilled to the frame as doubles, 40 instructions
//! each entry for a loop gcc runs with the matrix in memory operands.
//!
//! # Example
//! Before:
//!
//! ```mir
//! define i16 @f(i16 %n, i16 %a) {
//! b0:
//!   br label %b1
//!
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
//!   %s = phi i16 [ 0, %b0 ], [ %s1, %b2 ]
//!   %c = icmp slt i16 %i, %n
//!   br i1 %c, label %b2, label %b3
//!
//! b2:
//!   %k = mul i16 %a, 3
//!   %v = add i16 %k, 1
//!   %s1 = add i16 %s, %v
//!   %i1 = add i16 %i, 1
//!   br label %b1
//!
//! b3:
//!   ret i16 %s
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! b0:
//!   %k = mul i16 %a, 3
//!   %v = add i16 %k, 1
//!   br label %b1
//!
//! b2:
//!   %s1 = add i16 %s, %v
//!   %i1 = add i16 %i, 1
//!   br label %b1
//! ; ... b1 and b3 unchanged
//! ```
//!
//! `%k` and `%v` read only `%a`, which the loop does not define; they leave in
//! order into the preheader `b0`.
//!
//! From unit test `test_invariant_arithmetic_leaves_its_loop_in_order` in
//! `hoist_tests.rs`.
//!
//! # Runs
//! In `llvm_order`, first without speculation (only what the loop certainly
//! runs moves), then after `rotate`'s guard with it. Otherwise early, after
//! `floatloop` and before `loopmotion`; and again after `jumpthread`, at the
//! end of the loop passes, when a loop entered at its body runs it at least
//! once and what it loads unchanged may leave. On from
//! -O1; `-fno-move-loop-invariants` turns it and `loopmotion` off.
//!
//! # Needs
//! `memoryssa::Accesses`, `Summaries`, [`crate::profit`], `cfg::Shape`,
//! `noreturn::TerminalSites`, `ranges` and `consts` facts.
//!
//! # Keeps
//! Blocks, edges, dominators and loops; instructions keep their ids.
//!
//! # See also
//! LLVM `LICM.cpp`; gcc `loop-invariant.cc`. Stores sink in
//! [`crate::loopmotion`]. Tests: `hoist_tests.rs`.

use std::collections::BTreeSet;

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::memory::Unit;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::{cfg, induction, noreturn, ranges};
use llrm_mir::context::{ConstantKind, mask};
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses};
use llrm_mir::types::Type;

use crate::profit::{self, OperationCosts};

/// `size`: price a run in bytes, every block once (-Os), not in executed work.
/// `speculate`: LICM's `AllowSpeculation` (`licm<allowspeculation>`): an
/// instruction that is not certain to run in the loop may still go to the
/// preheader when it cannot fault. Without it (`licm<no-allowspeculation>`,
/// LPM1's first run, before `loop-rotate`) only what the loop certainly runs
/// goes.
pub struct Hoist {
    pub size: bool,
    pub speculate: bool,
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
        if hoisted_with(unit, analyses, self.size, self.speculate) {
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
    hoisted_with(unit, analyses, size, true)
}

/// [`hoisted`] with LICM's `AllowSpeculation` as `speculate`.
pub fn hoisted_with(
    unit: &mut passes::Unit,
    analyses: &mut Analyses,
    size: bool,
    speculate: bool,
) -> bool {
    let graph = cfg::graph(unit.function);
    let shape = analyses.get::<cfg::Shape>(unit.context, unit.layout, unit.function);
    let found = &shape.loops;
    if found.is_empty() {
        return false;
    }
    // A run starts from an instruction of a kind that may move whose operands
    // are all outside the loop (LICM's scan before anything is asked): with
    // none in any loop nothing moves, and the accesses, the invariant runs
    // and the prices are not made. `LLRM_CHECK_HOISTSKIP` makes them anyway
    // and fails if they find a run.
    let check = llrm_support::env_flag!("LLRM_CHECK_HOISTSKIP");
    if !check && !found.iter().any(|one| _seeded(unit, one)) {
        return false;
    }
    // Asked before anything moves: an instruction keeps its id where it goes.
    let Ok(accesses) = Accesses::managed(unit.context, unit.layout, unit.function, analyses) else { return false };
    let outer = std::rc::Rc::clone(analyses.outer());
    let terminal = analyses.get::<noreturn::TerminalSites>(unit.context, unit.layout, unit.function);
    let room = profit::registers(&outer);
    let costs =
        OperationCosts { folds_loads: true, ..if size { outer.target().size_costs() } else { profit::costs(&outer) } };
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
    let mut moved = false;
    let mut asked_after_moving = false;
    for one in found {
        let Some(into) = _preheader(&graph, one) else { continue };
        // The intervals of the loops, asked once a loop has been moved out of:
        // the edges they stand on follow where a value is defined, and a
        // bound moved out of a loop makes it counted, so those and what is
        // made of them are let go first and the rest stands
        // (`LLRM_CHECK_REPLAY`, `LLRM_CHECK_UNREPORTED`).
        let mut bounds = || {
            if moved && !asked_after_moving {
                analyses.invalidate(
                    unit.function,
                    &PreservedAnalyses::all()
                        .except::<llrm_analysis::manager::DominatedEdges>()
                        .except::<llrm_analysis::manager::Counted>()
                        .except::<llrm_analysis::manager::Bounded>(),
                );
            }
            asked_after_moving = moved;
            analyses.get::<llrm_analysis::manager::Bounded>(unit.context, unit.layout, unit.function)
        };
        let seeded = _seeded(unit, one);
        if !seeded && !check {
            continue;
        }
        let run =
            _invariant_run(unit, &outer, one, into, &accesses, &terminal, &registers, &shape, speculate, &mut bounds);
        assert!(seeded || run.is_empty(), "LLRM_CHECK_HOISTSKIP: a loop with no seed has a run of {}", run.len());
        if _crossed_values(unit.function, &run).is_empty() {
            continue;
        }
        let before = unit.function.terminator(cfg::block(into)).expect("a terminator");
        let run = match &frequency {
            Some(frequency) => {
                _affordable(unit, analyses, moved, &outer, run, before, into, one, &costs, room, frequency, &mut held)
            }
            None => run,
        };
        if run.is_empty() {
            continue;
        }
        for inst in run {
            unit.function.move_to(inst, Position::Before(before)).expect("a placed instruction");
        }
        changed = true;
        moved = true;
        asked_after_moving = false;
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
    analyses: &mut llrm_mir::passes::Analyses,
    moved_before: bool,
    outer: &Outer,
    mut run: Vec<InstId>,
    before: InstId,
    into: i64,
    loop_: &Loop,
    costs: &OperationCosts,
    room: crate::spill::Room,
    frequency: &std::collections::BTreeMap<i64, i64>,
    held: &mut Option<i64>,
) -> Vec<InstId> {
    // Each clone is priced from this, its liveness solved again for the values
    // the run moves alone (the whole function was solved for each of 28
    // forecasts: 63% of the spill model on d_faces at -Os).
    // The cache holds the pressure of the function before the motions this pass
    // has made, which are not told to it until it is let go.
    if moved_before && room.priced() {
        analyses.invalidate(unit.function, &PreservedAnalyses::all().except::<crate::spill::Pressure>());
    }
    let base =
        (room.priced()).then(|| analyses.get::<crate::spill::Pressure>(unit.context, unit.layout, unit.function));
    #[cfg(test)]
    if let Some(base) = &base {
        assert!(
            **base == crate::spill::Pressure::of(unit.context, unit.function, room.index_scales),
            "the pressure a clone is priced from is not the function's, which a motion has moved things in"
        );
    }
    // A run that is let go leaves the function as the clone priced it, whose
    // price is then the next loop's `kept`.
    let kept = match held.take() {
        Some(kept) => kept,
        None => {
            let work = profit::weighted(unit.context, unit.layout, unit.function, outer.callees(), costs, frequency);
            let forecast = match &base {
                Some(base) => profit::spill_forecast_over(
                    unit.context,
                    unit.layout,
                    unit.function,
                    base,
                    costs,
                    room,
                    &|inst| crate::spill::kept_across(outer, unit.context, unit.function, inst),
                    frequency,
                ),
                None => Some(crate::spill::Forecast { cost: 0, spilled: Default::default(), peak: 0 }),
            };
            let Some(kept) = work.zip(forecast).map(|(work, forecast)| work + forecast.cost) else { return run };
            kept
        }
    };
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
            // The clone's log holds what the run made of it, and nothing else.
            let changes = hoisted.take_changes();
            (hoisted, changes)
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
                    &moved.as_ref().expect("cloned under test").0,
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
                &moved.as_ref().expect("cloned where priced").0,
                outer.callees(),
                costs,
                frequency,
            ) else {
                return run;
            };
            work
        };
        let hoisted = moved.as_ref().map_or(&*unit.function, |(function, _)| function);
        // A floating value held across the loop is released after it, once for
        // each time the loop is entered.
        let floats: IdSet<ValueId> = _crossed_values(hoisted, &run)
            .iter()
            .filter(|&value| matches!(
                unit.context.types.get(hoisted.value(value).ty),
                Type::Float(_)
            ))
            .collect();
        let across = |inst| crate::spill::kept_across(outer, unit.context, hoisted, inst);
        let forecast = match (&moved, &base) {
            (Some((_, changes)), Some(base)) => profit::spill_forecast_after(
                unit.context,
                unit.layout,
                hoisted,
                base,
                changes,
                costs,
                room,
                &across,
                frequency,
            ),
            _ => profit::spill_forecast(unit.context, unit.layout, hoisted, costs, room, &across, frequency),
        };
        let Some(forecast) = forecast else { return run };
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
            .iter()
            .filter(|value| forecast.spilled.contains(value))
            .collect::<IdSet<_>>();
        let uncounted: i64 = crossing
            .iter()
            .filter(|&value| free(value))
            .filter_map(|value| traffic().get(&value))
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
        // The class holds only so many floating values: of those the run holds
        // across the loop, what the loop's own values leave room for are held,
        // and the rest stay where they are read (a register stack that is
        // full spills what is hoisted to a cell the loop then reads, and
        // the hoist is the spill).
        // A load the loop folds into its reader saves what the fold costs more
        // than the register form, each trip; hoisted, it is a load of its own
        // and a release, once per entry.
        let unworthy: IdSet<ValueId> = floats
            .iter()
            .filter(|&value| {
                let ValueDef::Instruction(def) = unit.function.value(value).def else { return false };
                let Some(home) = unit.function.parent(def) else { return false };
                let Some(price) =
                    profit::operation(unit.context, unit.layout, unit.function, outer.callees(), def, costs)
                else {
                    return false;
                };
                price < costs.float_load
                    && price * frequency[&cfg::id(home)]
                        <= (costs.float_load + costs.float_release) * frequency.get(&into).copied().unwrap_or(1)
            })
            .collect();
        let beyond: IdSet<ValueId> = match outer.target().float_registers() {
            0 => unworthy,
            class => {
                let left = (class - _float_working_set(unit, loop_, &run)).max(0) as usize;
                floats.iter().skip(left).chain(unworthy.iter()).collect()
            }
        };
        if beyond.is_empty() && moved - uncounted <= kept {
            *held = Some(work + forecast.cost);
            return run;
        }
        let costly = moved - uncounted > kept;
        let mut stay: IdSet<ValueId> =
            if costly { crossing.iter().filter(|&value| !free(value)).collect() } else { IdSet::new() };
        stay.extend(beyond.iter());
        // What costs its release more than it saves stays, with what reads it.
        if costly && costs.float_release > 0 {
            stay.extend(floats.iter());
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

/// The most floating values the loop holds at once besides the `run`'s, which
/// are held across it: those defined outside it and read inside it, and in a
/// block those defined and not yet read for the last time.
fn _float_working_set(
    unit: &passes::Unit,
    loop_: &Loop,
    run: &[InstId],
) -> i64 {
    let function = &*unit.function;
    let floating = |value: ValueId| matches!(
        unit.context.types.get(function.value(value).ty),
        Type::Float(_)
    );
    let inside = |inst: InstId| function.parent(inst).is_some_and(|home| loop_.body.contains(&cfg::id(home)));
    let moving: IdSet<InstId> = run.iter().copied().collect();
    let mut held: IdSet<ValueId> = IdSet::new();
    let mut peak = 0;
    for &block in function.layout() {
        if !loop_.body.contains(&cfg::id(block)) {
            continue;
        }
        let list = function.block(block).instructions();
        let mut ends: Vec<usize> = Vec::new();
        for (at, &inst) in list.iter().enumerate() {
            let one = function.instruction(inst);
            for operand in &one.operands {
                let Operand::Value(value) = operand else { continue };
                let defined_inside = matches!(
                    function.value(*value).def,
                    ValueDef::Instruction(def) if inside(def)
                );
                if floating(*value) && !defined_inside {
                    held.insert(*value);
                }
            }
            if moving.contains(&inst) {
                continue;
            }
            let Some(result) = one.result.filter(|&value| floating(value)) else { continue };
            let end = function
                .users(result)
                .iter()
                .map(|user| {
                    if function.parent(user.user) == Some(block) {
                        list.iter().position(|&other| other == user.user).unwrap_or(at)
                    } else {
                        list.len()
                    }
                })
                .max()
                .unwrap_or(at);
            ends.push(end);
            peak = peak.max(ends.iter().filter(|&&end| end > at).count() as i64);
        }
    }
    peak + held.len() as i64
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
pub(crate) fn _frequency(
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
    speculate: bool,
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
    let mut made: IdSet<ValueId> = IdSet::new();
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
            if !ready {
                continue;
            }
            // `isSafeToExecuteUnconditionally`
            // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LICM.cpp#L1507:
            // with
            // `AllowSpeculation` an instruction that cannot fault moves
            // wherever it is; without it, or when it may fault, only one the
            // loop certainly runs (`isGuaranteedToExecute`), or a load in
            // bounds on the edge in (speculating).
            let in_loop_for_sure = |certain: &mut Option<_>| {
                certain
                    .get_or_insert_with(|| _guaranteed(unit, outer, loop_, into, terminal, registers, shape))
                    .contains(&inst)
            };
            if !speculate {
                if !in_loop_for_sure(&mut certain) {
                    continue;
                }
            } else if _may_fault(unit, outer, inst)
                && !in_loop_for_sure(&mut certain)
                && !_bounded_inside(unit, outer, inst, into, loop_.header, &mut bounded, bounds, registers, shape)
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
    let Ok(facts) = held.as_ref().as_ref() else { return false };
    let scope = facts.scope(&memory, into);
    let Ok(Some(known)) = ranges::on_edge_over(&memory, cfg::block(into), cfg::block(header), &scope, None) else {
        return false;
    };
    ranges::inside_object(&memory, &reference, &known)
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
) -> IdSet<ValueId> {
    let inside: IdSet<InstId> = run.iter().copied().collect();
    run.iter()
        .filter_map(|&inst| function.instruction(inst).result)
        .filter(|&value| function.users(value).iter().any(|one| !inside.contains(&one.user)))
        .collect()
}

#[cfg(test)]
#[path = "hoist_tests.rs"]
mod tests;

/// Whether some instruction of `loop_` could start a run: one of the kinds
/// `_movable` accepts whose operands are all constants or defined outside the
/// loop. Every run begins with one, so a loop with none has no run.
fn _seeded(
    unit: &passes::Unit,
    loop_: &Loop,
) -> bool {
    let function = &*unit.function;
    let inside = |block: BlockId| loop_.body.contains(&cfg::id(block));
    let outside = |value: ValueId| match function.value(value).def {
        ValueDef::Argument(_) => true,
        ValueDef::Instruction(def) => function.parent(def).is_some_and(|block| !inside(block)),
    };
    function
        .layout()
        .iter()
        .filter(|&&block| inside(block))
        .any(
            |&block| function
                .block(block)
                .instructions()
                .iter()
                .any(
                    |&inst| {
                        let op = function.instruction(inst);
                        matches!(
                            op.opcode,
                            Opcode::Binary(_)
                                | Opcode::Cast(_)
                                | Opcode::ICmp(_)
                                | Opcode::FCmp(_)
                                | Opcode::FNeg
                                | Opcode::GetElementPtr { .. }
                                | Opcode::Select
                                | Opcode::Freeze
                                | Opcode::ExtractValue(_)
                                | Opcode::InsertValue(_)
                                | Opcode::Load { volatile: false, .. }
                        ) && op.operands.iter().all(|operand| match *operand {
                            Operand::Value(value) => outside(value),
                            Operand::Constant(_) => true,
                            Operand::Block(_) => false,
                        })
                    },
                ),
        )
}
