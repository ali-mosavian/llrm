//! Settles what a loop's counters are worth: a counter's value after its loop
//! becomes a number, and a narrow counter read as a wider index becomes a
//! counter of that width. LLVM's IndVarSimplify (`rewriteLoopExitValues`,
//! `widenIV`) plus an exit-value rewind across an outer loop. Which counters a
//! loop keeps, and which ends it, is `lsr`'s.
//!
//! # How it works
//! One run is these steps, each followed by a refresh of what is known where it
//! changed the body.
//!
//! 0. `exitvalues::rewritten`: a value read after a loop becomes an expression
//!    of the loop's count where the loop is left (`rewriteLoopExitValues`,
//!    ScalarEvolution's exit values, from every exit of the loop, a rotated
//!    loop's latch included); `LLRM_NO_SCEV_EXITVALUES` turns it off.
//! 1. `loopexit::evaluated`: values after a counted loop computed from its
//!    trips.
//! 2. `exitfold::folded`: exits their counts decide are folded.
//! 4. `exitsink::sunk`: final updates only exit phis read move to the exits.
//! 5. `widened`: a counter read through `zext` (or, where it is signed and
//!    tested by `slt`, `sext`) to the address's index width, where `induction`
//!    proves its values never pass its own (`rises_unsigned`, `rises_signed`),
//!    becomes a counter of that width. C's `unsigned short i` reads `a[zext
//!    i]`, and `short k` reads `a[sext k]`: a conversion per trip (`inc cx;
//!    movsx esi,cx; cmp cx,bx`), which hides the loop from idioms and strength
//!    reduction. A signed counter of a loop of at most `unroll`'s trip limit
//!    (`IndVars::unrolls`) is left for `unroll` until the run with
//!    `exit_values`: one operation fewer in the rolled loop made a loop that
//!    `unroll` took whole no longer fit its own size. The wide counter ends the
//!    loop against the bound extended once in the preheader; an unsigned or
//!    equality compare of the narrow one with a value the loop leaves alone
//!    compares the wide one with that value extended
//!    (`WidenIV::widenLoopCompare`); other readers of the narrow one read its
//!    low part.
//! 6. `congruent`: header counters with one start and one step are one counter
//!    (`SCEVExpander::replaceCongruentIVs`); the later ones' readers read the
//!    first, and `dead` removes them.
//! 7. `rewound`: see below.
//!
//! `rewound` carries an exact inner recurrence's exit value around the outer
//! loop. An inner loop running exactly `count` trips leaves as
//! `start + count * step`; subtracting that distance on the outer back edge
//! rebuilds the next start, so the saved start is dead across the hot inner
//! loop.
//!
//! ```text
//!  outer:                                 outer:
//!    inner_pre: br inner                    %st = phi [%start,pre],[%r,latch]
//!  inner:                                 inner:
//!    %p = phi [%start,inner_pre],..  =>     %p = phi [%st,inner_pre],..
//!  exit:                                  exit:
//!    ...                                    %r = add %p_exit, -distance
//! ```
//!
//! It applies only for a pair of single-latch loops whose inner loop has one
//! exit, a known trip count, a counter with constant nonzero step, and a start
//! defined outside the outer loop and read in it only by that phi. It is
//! profitable when inner-loop register pressure reaches the target's registers
//! and the target's `add` costs no more than a `move` and its `memory_update`
//! no more than a `load` plus `store`: in a register the old copy and the
//! rewind are equal work; in frame cells the old form is a load and a store,
//! the new a memory update.
//!
//! # Example
//! Step 5, the widening, on `for (unsigned short i = 0; i < n; ++i) a[i] = 1`
//! with 32-bit addresses. Before:
//!
//! ```mir
//! target datalayout = "e-p:32:32-n8:16:32"
//!
//! define i32 @f(i16 %n) {
//! b0:
//!   %buf = alloca [64 x i8]
//!   br label %b1
//!
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
//!   %go = icmp ult i16 %i, %n
//!   br i1 %go, label %b2, label %b3
//!
//! b2:
//!   %w = zext i16 %i to i32
//!   %p = getelementptr inbounds i8, ptr %buf, i32 %w
//!   store i8 1, ptr %p
//!   %next = add i16 %i, 1
//!   br label %b1
//!
//! b3:
//!   ret i32 0
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! b0:
//!   %buf = alloca [64 x i8]
//!   %0 = zext i16 %n to i32
//!   br label %b1
//!
//! b1:
//!   %widen.iv = phi i32 [ 0, %b0 ], [ %widen.iv.next, %b2 ]
//!   %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
//!   %1 = icmp ult i32 %widen.iv, %0
//!   br i1 %1, label %b2, label %b3
//!
//! b2:
//!   %p = getelementptr inbounds i8, ptr %buf, i32 %widen.iv
//!   store i8 1, ptr %p
//!   %widen.iv.next = add i32 %widen.iv, 1
//!   %next = add i16 %i, 1
//!   br label %b1
//! ; ... b3 unchanged
//! ```
//!
//! The loop now ends against `%n` extended once in the preheader, and the
//! address reads the wide counter: the `zext` a trip is gone.
//!
//! From unit test `a_counter_extended_to_the_index_width_is_that_wide` in
//! `indvars_tests.rs`.
//!
//! # Runs
//! In the body pipeline after `promote`, with `exit_values: false`. With
//! `-ftree-ch` (not at -Os) it runs again with `exit_values: true` after the
//! loop-header copy, where the copy made a counted exit's test the one that
//! leaves (a number is a `mov` where the counter's register already holds it).
//!
//! # Needs
//! `Registers`, `Bounded` (for a `sext` counter), `induction`, `liveness`,
//! [`crate::loopexit`], [`crate::exitfold`], [`crate::exitsink`],
//! [`crate::profit`].
//!
//! # Keeps
//! Nothing after steps 1 to 3; dominators and loops after the others.
//!
//! # See also
//! LLVM `IndVarSimplify.cpp`. Tests: `indvars_tests.rs`.

use std::collections::BTreeSet;

use llrm_analysis::cfg::Around;
use llrm_analysis::consts::{Known, masked};
use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, AffineOperand};
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::{cfg, liveness, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_support::hash::IndexMap;
use num_traits::Zero;

use crate::counting;
use crate::dead;
use crate::lcssa::{arms, from_arms};
use crate::profit::OperationCosts;

/// What IndVarSimplify leaves a loop's counters: values after a loop
/// computed from its trips (`loopexit::evaluated`), final updates sunk to
/// the exits (`exitsink`), and an inner recurrence's exit value carried
/// round the loop outside (`rewound`). Which counters the loop keeps is
/// `lsr`'s.
pub struct IndVars {
    /// Off before the header copy: a signed counter of a loop `unrolls` takes
    /// is left for it until this run.
    pub exit_values: bool,
    /// The trips up to which `unroll` takes a loop whole (its
    /// `max_unroll_iterations`, 0 for any number), none where it is off: a
    /// signed counter of such a loop is left for it until `exit_values`.
    pub unrolls: Option<i64>,
}

impl FunctionPass for IndVars {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "indvars"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        // What is known without memory is the manager's until a step changes
        // the body.
        let held = analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
        // Only a `sext` of a counter asks what the loops bound.
        let widens =
            unit.function.walk().any(|(_, inst)| unit.function.instruction(inst).opcode == Opcode::Cast(CastOp::SExt));
        let bounded =
            widens.then(|| analyses.get::<llrm_analysis::manager::Bounded>(unit.context, unit.layout, unit.function));
        let shape = analyses.get::<cfg::Shape>(unit.context, unit.layout, unit.function);
        // Once the body has changed, the manager brings what it holds up to
        // date from the changes logged, where `Standing` would derive
        // it again (13 times at d_faces -O2, 25 M).
        let mut standing = match bounded.as_deref() {
            Some(Ok(bounds)) => llrm_analysis::memory::Standing::held_with(&held, bounds),
            _ => llrm_analysis::memory::Standing::held(&held),
        }
        .with_shape(shape)
        .renewing(|context, layout, function, reshaped| {
            let kept = if reshaped {
                PreservedAnalyses::none()
            } else {
                PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>()
            };
            analyses.invalidate(function, &kept);
            analyses.get::<llrm_analysis::manager::Registers>(context, layout, function)
        });
        let extending = crate::simplifyindvar::widening_enabled();
        let compared = if extending {
            crate::simplifyindvar::simplified_and_extended(
                unit.context,
                unit.layout,
                unit.function,
                unit.declared,
                &outer,
            )
        } else {
            crate::simplifyindvar::enabled()
                && crate::simplifyindvar::simplified(unit.context, unit.layout, unit.function, &outer)
        };
        if compared {
            standing.changed();
        }
        let rewrote = crate::exitvalues::enabled()
            && crate::exitvalues::rewritten(unit.context, unit.layout, unit.function, unit.declared, &outer);
        if rewrote {
            standing.changed();
        }
        let evaluated =
            crate::loopexit::evaluated_with(unit.context, unit.layout, unit.function, &outer, &mut standing)
                .unwrap_or_else(|error| panic!("indvars: {error}"));
        if evaluated {
            standing.reshaped();
        }
        let folded = crate::exitfold::folded_with(unit.context, unit.layout, unit.function, &outer, &mut standing);
        if folded {
            standing.reshaped();
        }
        let shape = standing.shape(unit.function);
        let sunk = crate::exitsink::sunk(unit.function, &shape);
        if sunk {
            standing.changed();
        }
        let widened = !extending
            && widened_with(
                unit.context,
                unit.layout,
                unit.function,
                &outer,
                &mut standing,
                if self.exit_values { None } else { self.unrolls },
            );
        let merged = congruent(unit.context, unit.layout, unit.function, &outer, &mut standing);
        let rewound = rewound_with(
            unit.context,
            unit.layout,
            unit.function,
            &outer,
            &mut standing,
            crate::profit::registers(&outer).registers,
            &crate::profit::costs(&outer),
        );
        let dead = (compared || rewrote || sunk || rewound || widened || merged)
            && dead::dead(unit.context, outer.callees(), unit.function);
        if evaluated || folded {
            PreservedAnalyses::none()
        } else if compared || rewrote || sunk || rewound || widened || merged || dead {
            // Blocks and edges are as they were.
            PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Header counters that start at one value and step by one value are one
/// counter: each later one's readers read the first (`SCEVExpander::
/// replaceCongruentIVs`). Two loops fused, or one written with a counter twice,
/// leave such a pair, and what is indexed by either is then the same address.
fn congruent(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &passes::Outer,
    standing: &mut llrm_analysis::memory::Standing,
) -> bool {
    if !alike_phis(function) {
        return false;
    }
    let shape = standing.shape(function);
    let mut merged = Vec::new();
    {
        let unit = llrm_analysis::memory::Unit::within(context, layout, function, outer).with_shape(&shape);
        let facts = standing.of(&unit);
        let unit = unit.with_registers(facts);
        for loop_ in shape.loops.iter() {
            let counters = induction::basics(&unit, loop_);
            let mut kept = IndexMap::default();
            for (&value, counter) in &counters {
                let first = *kept.entry((&counter.start, &counter.step)).or_insert(value);
                if first != value {
                    merged.push((value, first));
                }
            }
        }
    }
    for &(later, first) in &merged {
        function.replace_all_uses_with(later, Operand::Value(first));
    }
    !merged.is_empty()
}

/// Two phis of one block take the same value from each predecessor, or the
/// same operation of one operand on themselves: the cheap look, over the
/// phis alone, before the loops and their counters are asked of.
fn alike_phis(function: &Function) -> bool {
    #[derive(Eq, Hash, PartialEq)]
    enum Arm {
        Value(Operand),
        Stepped(&'static str, Vec<Operand>),
    }
    let mut seen = llrm_support::hash::IndexSet::default();
    llrm_analysis::occurrence::phis(function).any(|(phi, block, _)| {
        let result = function.instruction(phi).result.map(Operand::Value);
        let mut key = arms(function, phi)
            .into_iter()
            .map(|(value, from)| {
                let stepped = match value {
                    Operand::Value(one) => _defining(function, one).map(|inst| function.instruction(inst)),
                    _ => None,
                }
                .filter(|step| step.operands.iter().any(|&operand| Some(operand) == result));
                let arm = match stepped {
                    Some(step) => Arm::Stepped(
                        step.opcode.mnemonic(),
                        step.operands.iter().copied().filter(|&operand| Some(operand) != result).collect(),
                    ),
                    None => Arm::Value(value),
                };
                (from, arm)
            })
            .collect::<Vec<_>>();
        key.sort_by_key(|(from, _)| from.0);
        !seen.insert((block, key))
    })
}

/// The instruction defining `value`.
fn _defining(
    function: &Function,
    value: ValueId,
) -> Option<InstId> {
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// The value `phi` takes from `from`.
fn _from(
    function: &Function,
    phi: InstId,
    from: BlockId,
) -> Option<Operand> {
    arms(function, phi).into_iter().find(|&(_, source)| source == from).map(|(value, _)| value)
}

fn _block_of(
    function: &Function,
    inst: InstId,
) -> i64 {
    cfg::id(function.parent(inst).expect("a placed instruction"))
}

/// The loop's one outside predecessor of its header, going only there.
fn _preheader(
    function: &Function,
    loop_: &Loop,
) -> Option<BlockId> {
    let header = cfg::block(loop_.header);
    let [preheader] = function
        .predecessors(header)
        .into_iter()
        .filter(|block| !loop_.body.contains(&cfg::id(*block)))
        .collect::<Vec<_>>()[..]
    else {
        return None;
    };
    (function.successors(preheader) == [header]).then_some(preheader)
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
pub fn rewound(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &Analyses,
    registers: i64,
    costs: &OperationCosts,
) -> bool {
    rewound_with(context, layout, function, analyses.outer(), &mut memory::Standing::underived(), registers, costs)
}

/// `rewound`, what is known of the body as `standing` says: the manager's
/// where the run before it changed nothing, found once for the body where it
/// did.
pub fn rewound_with(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &passes::Outer,
    standing: &mut memory::Standing,
    registers: i64,
    costs: &OperationCosts,
) -> bool {
    if registers == 0 || costs.add > costs.r#move || costs.memory_update > costs.load + costs.store {
        return false;
    }
    let shape = standing.shape(function);
    let plan = {
        let unit = memory::Unit::within(context, layout, function, outer).with_shape(&shape);
        let facts = standing.of(&unit);
        _rewinding(&unit, facts, registers)
    };
    let Some(plan) = plan else {
        return false;
    };
    let ty = function.value(plan.start).ty;
    let exit_value = if plan.sources.len() == 1 {
        plan.exit_value
    } else {
        let closed = function.create_instruction(
            Opcode::Phi,
            ty,
            from_arms(&plan.sources.iter().map(|&source| (plan.exit_value, source)).collect::<Vec<_>>()),
            Flags::default(),
            None,
        );
        let first = function.block(plan.exit).instructions()[0];
        function.insert(closed, Position::Before(first)).expect("a placed block");
        Operand::Value(function.instruction(closed).result.expect("a phi's value"))
    };
    let width = context.types.int_bits(ty).expect("an integer");
    let back = counting::constant(context, &-&plan.distance, width);
    let reset =
        function.create_instruction(Opcode::Binary(BinaryOp::Add), ty, vec![exit_value, back], Flags::default(), None);
    let after = function
        .block(plan.exit)
        .instructions()
        .iter()
        .copied()
        .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
        .expect("a terminator");
    function.insert(reset, Position::Before(after)).expect("a placed block");
    let reset = Operand::Value(function.instruction(reset).result.expect("a value"));
    let carried = function.create_instruction(
        Opcode::Phi,
        ty,
        from_arms(&[(Operand::Value(plan.start), plan.parent_preheader), (reset, plan.parent_latch)]),
        Flags::default(),
        None,
    );
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

fn _rewinding(
    unit: &memory::Unit,
    facts: &IndexMap<ValueId, Known>,
    registers: i64,
) -> Option<Rewinding> {
    let function = unit.function;
    let shape = unit.shape();
    let found = &shape.loops;
    if found.len() < 2 {
        return None;
    }
    let dominance = &shape.dominance;
    let live = liveness::live(function);
    for inner in found {
        let Some(parent) =
            inner.body.parent_header().and_then(|header| found.iter().find(|parent| parent.header == header))
        else {
            continue;
        };
        let ([inner_latch], [parent_latch]) = (
            &inner.latches.iter().copied().collect::<Vec<_>>()[..],
            &parent.latches.iter().copied().collect::<Vec<_>>()[..],
        ) else {
            continue;
        };
        let (Some(inner_preheader), Some(parent_preheader)) =
            (_preheader(function, inner), _preheader(function, parent))
        else {
            continue;
        };
        let parent_header = cfg::block(parent.header);
        let preds = function.predecessors(parent_header).into_iter().collect::<BTreeSet<_>>();
        if !parent.body.contains(&cfg::id(inner_preheader))
            || preds != BTreeSet::from([parent_preheader, cfg::block(*parent_latch)])
            || (liveness::pressure(function, Some(&live), Some(&inner.body)) as i64) < registers
        {
            continue;
        }
        let exiting = inner.exits(function);
        let exits = exiting.iter().map(|(_, target)| *target).collect::<BTreeSet<_>>();
        let [exit] = exits.into_iter().collect::<Vec<_>>()[..] else {
            continue;
        };
        let sources = exiting.iter().map(|(source, _)| *source).collect::<BTreeSet<_>>();
        let exit_preds = function.predecessors(cfg::block(exit)).into_iter().map(cfg::id).collect::<BTreeSet<_>>();
        if !parent.body.contains(&exit)
            || exit_preds != sources
            || !dominance.dominates(unit.function, exit, *parent_latch)
        {
            continue;
        }
        let Some(count) = induction::trip_count(unit, inner, facts) else { continue };
        for counter in induction::basics(unit, inner).values() {
            let Some(phi) = _defining(function, counter.value) else { continue };
            let sides = arms(function, phi).into_iter().map(|(_, from)| from).collect::<BTreeSet<_>>();
            if sides != BTreeSet::from([inner_preheader, cfg::block(*inner_latch)]) {
                continue;
            }
            let (Some(Operand::Value(start)), Some(Operand::Value(update))) =
                (_from(function, phi, inner_preheader), _from(function, phi, cfg::block(*inner_latch)))
            else {
                continue;
            };
            let width = counter.start.width();
            let (Some(step), Some(stepping), Some(defining)) = (
                induction::_signed(&counter.step, facts, width),
                _defining(function, update),
                _defining(function, start),
            ) else {
                continue;
            };
            if step.is_zero() || parent.body.contains(&_block_of(function, defining)) || facts.contains_key(&start) {
                continue;
            }
            // A pre-tested loop leaves from its header, where the phi is
            // already `start + count * step`; a post-tested one from its
            // latch, where the update is.
            let updated = _block_of(function, stepping);
            let exit_value = if sources.iter().all(|&source| dominance.dominates(unit.function, updated, source)) {
                Operand::Value(update)
            } else if sources == BTreeSet::from([inner.header]) {
                Operand::Value(counter.value)
            } else {
                continue;
            };
            // The saved start must die in the outer loop, or the rewrite is
            // only code size.
            if function
                .users(start)
                .iter()
                .any(|one| one.user != phi && parent.body.contains(&_block_of(function, one.user)))
            {
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

/// A counter read through `zext` to the default address's index width is a
/// counter of that width, where `induction` proves its values never pass its
/// own (`rises_unsigned`): C's `unsigned short i` reads `a[zext i]`, a
/// conversion per trip, and its counted-loop proof is narrower than the
/// address, so the idioms and strength reduction that want the address's width
/// find none. The wide counter ends the loop against the bound extended once,
/// before it, and what else reads the narrow one reads its low part. Every
/// counter widened; whether any was.
pub fn widened(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &passes::Outer,
) -> bool {
    widened_with(context, layout, function, outer, &mut llrm_analysis::memory::Standing::underived(), None)
}

/// `widened`, what is known of the body without memory given as `standing`
/// says: derived once for each state of the body, not once for each loop.
/// With `unrolls`, a signed counter of a loop that runs that many times or
/// fewer is left for `unroll`, which takes a loop whole by what it comes to
/// rolled: one operation fewer in the widened loop was a loop that no longer
/// fit its own size (bench/lru's three-trip fill).
pub fn widened_with(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &passes::Outer,
    standing: &mut llrm_analysis::memory::Standing,
    unrolls: Option<i64>,
) -> bool {
    let mut changed = false;
    'again: loop {
        let shape = standing.shape(function);
        for loop_ in shape.loops.clone() {
            let found = {
                let unit = llrm_analysis::memory::Unit::within(context, layout, function, outer).with_shape(&shape);
                let facts = standing.of(&unit);
                _find(&unit.with_registers(facts), &loop_, unrolls)
            };
            if let Some(found) = found {
                let mark = function.mark();
                _widen(context, function, &found);
                match function.changes_since(mark) {
                    Some(changes) => standing.edited(function, changes),
                    None => standing.changed(),
                }
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
    /// Where the counter starts: a number, or a value the loop does not define.
    start: AffineOperand,
    bound: Operand,
    wide: u32,
    extensions: Vec<InstId>,
    /// Every other reader: its instruction and operand.
    rest: Vec<(InstId, u32)>,
    /// The readers that compare it, unsigned, with a value the loop does not
    /// define: they compare the wide counter with that value extended.
    compares: Vec<(InstId, u32)>,
    /// The loop is tested after its trips: the compare reads the counter
    /// stepped.
    after: bool,
    /// The counter is signed and read through `sext`: its wide counter is
    /// tested by `slt` against the bound sign-extended.
    signed: bool,
}

fn _find(
    unit: &llrm_analysis::memory::Unit,
    loop_: &Loop,
    unrolls: Option<i64>,
) -> Option<_Found> {
    let function = unit.function;
    let header = cfg::block(loop_.header);
    let outside = function
        .predecessors(header)
        .into_iter()
        .filter(|&one| !loop_.body.contains(&cfg::id(one)))
        .collect::<Vec<_>>();
    let [preheader] = outside[..] else { return None };
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    let latch = cfg::block(latch);
    let still = induction::invariant(function, &loop_.body);
    let facts = unit.registers();
    for proof in induction::counted(unit, loop_, Some(&facts), false) {
        let width = proof.width();
        let signed = proof.rises_signed() || proof.rises_signed_after();
        let after = proof.rises_unsigned_after() || proof.rises_signed_after();
        if !(proof.rises_unsigned() || signed || after) || width > 64 {
            continue;
        }
        let for_unroll = |count: &BigInt| unrolls.is_some_and(|most| most == 0 || *count <= BigInt::from(most));
        if signed && proof.count.as_ref().is_some_and(for_unroll) {
            continue;
        }
        let ext = if signed { CastOp::SExt } else { CastOp::ZExt };
        let compare = function.instruction(proof.compare);
        let (Operand::Value(tested), bound) = (compare.operands[0], compare.operands[1]) else { continue };
        let Some(counter) = function.instruction(proof.phi).result else { continue };
        // Tested before its trips the compare reads the counter, after them the
        // counter stepped.
        if !still.operand(bound) || function.parent(proof.phi) != Some(header) || (tested != counter && !after) {
            continue;
        }
        let arms =
            function.instruction(proof.phi).operands.chunks(2).map(|pair| (pair[0], pair[1])).collect::<Vec<_>>();
        // The arms in either order: a loop entered behind a copy of its test
        // lists its latch first.
        let [(one, Operand::Block(one_from)), (other, Operand::Block(other_from))] = arms[..] else { continue };
        let next = if other_from == latch && one_from == preheader {
            other
        } else if one_from == latch && other_from == preheader {
            one
        } else {
            continue;
        };
        let Operand::Value(next_value) = next else { continue };
        let ValueDef::Instruction(next_inst) = function.value(next_value).def else { continue };
        let adds = function.instruction(next_inst);
        if adds.opcode != Opcode::Binary(BinaryOp::Add)
            || adds.operands[0] != Operand::Value(counter)
            || unit.int_constant(adds.operands[1]) != Some(1)
        {
            continue;
        }
        if function.users(next_value).iter().any(|one| one.user != proof.phi && !(after && one.user == proof.compare)) {
            continue;
        }
        if after && tested != next_value {
            continue;
        }
        // The default space's index width, which a use already extends to.
        let mut wide = None;
        for one in function.users(counter) {
            let user = function.instruction(one.user);
            if user.opcode != Opcode::Cast(ext) {
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
        let mut compares = Vec::new();
        for one in function.users(counter) {
            let user = function.instruction(one.user);
            if one.user == proof.compare || one.user == next_inst {
                continue;
            }
            if user.opcode == Opcode::Cast(ext) && unit.int_bits(Operand::Value(user.result?)) == Some(wide) {
                extensions.push(one.user);
            } else if _compares_extended(user, one.index, &still, signed) {
                compares.push((one.user, one.index));
            } else {
                rest.push((one.user, one.index));
            }
        }
        return Some(_Found {
            phi: proof.phi,
            next: next_inst,
            compare: proof.compare,
            preheader,
            latch,
            start: proof.start.clone(),
            bound,
            wide,
            extensions,
            rest,
            compares,
            after,
            signed,
        });
    }
    None
}

/// `user` compares the counter, read as its `index`th operand, as unsigned
/// numbers or for equality with a value `still` says the loop leaves alone:
/// the counter never passes the narrow width, so its extension compares as it
/// does (LLVM's `WidenIV::widenLoopCompare`).
fn _compares_extended(
    user: &llrm_mir::module::Instruction,
    index: u32,
    still: &induction::Invariant,
    signed: bool,
) -> bool {
    let fits = if signed {
        matches!(
            user.opcode,
            Opcode::ICmp(
                IntPredicate::Eq
                    | IntPredicate::Ne
                    | IntPredicate::Slt
                    | IntPredicate::Sle
                    | IntPredicate::Sgt
                    | IntPredicate::Sge
            )
        )
    } else {
        matches!(
            user.opcode,
            Opcode::ICmp(
                IntPredicate::Eq
                    | IntPredicate::Ne
                    | IntPredicate::Ult
                    | IntPredicate::Ule
                    | IntPredicate::Ugt
                    | IntPredicate::Uge
            )
        )
    };
    fits && still.operand(user.operands[1 - index as usize])
}

/// `operand` zero- (or, where `signed`, sign-) extended to `ty` at the end of
/// `preheader`: a number is its own extension.
fn _extended(
    context: &mut Context,
    function: &mut Function,
    preheader: BlockId,
    operand: Operand,
    ty: llrm_mir::types::TypeId,
    signed: bool,
) -> Operand {
    match operand {
        Operand::Constant(constant) => match context.get(constant).kind {
            llrm_mir::context::ConstantKind::Int(bits) => {
                let narrow = context.types.int_bits(context.get(constant).ty).unwrap_or(64);
                let value = if signed && narrow < 128 && bits >> (narrow - 1) & 1 == 1 {
                    bits as i128 - (1_i128 << narrow)
                } else {
                    bits as i128
                };
                Operand::Constant(context.int(ty, value))
            }
            _ => unreachable!("an integer operand"),
        },
        value => {
            let ext = if signed { CastOp::SExt } else { CastOp::ZExt };
            let cast = function.create_instruction(Opcode::Cast(ext), ty, vec![value], Flags::default(), None);
            let end = function.terminator(preheader).expect("a terminated preheader");
            function.insert(cast, Position::Before(end)).expect("a preheader");
            Operand::Value(function.instruction(cast).result.expect("a value"))
        }
    }
}

fn _widen(
    context: &mut Context,
    function: &mut Function,
    found: &_Found,
) {
    let ty = context.types.int(found.wide);
    let counter = function.instruction(found.phi).result.expect("a phi's value");
    let header = function.parent(found.phi).expect("a placed phi");
    // The bound and the start, extended once before the loop.
    let bound = _extended(context, function, found.preheader, found.bound, ty, found.signed);
    let first = function.block(header).instructions()[0];
    let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), Some("widen.iv"));
    function.insert(phi, Position::Before(first)).expect("a header");
    let value = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
    let one = Operand::Constant(context.int(ty, 1));
    let next = function.create_instruction(
        Opcode::Binary(BinaryOp::Add),
        ty,
        vec![value, one],
        Flags::default(),
        Some("widen.iv.next"),
    );
    function.insert(next, Position::Before(found.next)).expect("a latch");
    let next = Operand::Value(function.instruction(next).result.expect("a value"));
    let start = match &found.start {
        AffineOperand::Const(start) if found.signed => {
            // The narrow start's value, sign-extended.
            let width = function.value(counter).ty;
            let bits = context.types.int_bits(width).unwrap_or(64);
            let modulus = BigInt::from(1) << bits;
            let mut n = ((&start.n % &modulus) + &modulus) % &modulus;
            if n >= (BigInt::from(1) << (bits - 1)) {
                n -= &modulus;
            }
            counting::constant(context, &n, found.wide)
        }
        AffineOperand::Const(start) => counting::constant(context, &start.n, found.wide),
        AffineOperand::Value(value, _) => {
            _extended(context, function, found.preheader, Operand::Value(*value), ty, found.signed)
        }
    };
    function.set_operands(phi, vec![start, Operand::Block(found.preheader), next, Operand::Block(found.latch)]);
    let bit = context.types.int(1);
    let tested = if found.after { next } else { value };
    let order = if found.signed { IntPredicate::Slt } else { IntPredicate::Ult };
    let test = function.create_instruction(Opcode::ICmp(order), bit, vec![tested, bound], Flags::default(), None);
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
    for &(user, index) in &found.compares {
        let other = function.instruction(user).operands[1 - index as usize];
        let other = _extended(context, function, found.preheader, other, ty, found.signed);
        function.set_operand(user, index as usize, value);
        function.set_operand(user, 1 - index as usize, other);
    }
    // What else reads the counter reads its low part, cut where it is read.
    let narrow = function.value(counter).ty;
    for &(user, index) in &found.rest {
        let at = match function.instruction(user).opcode {
            Opcode::Phi => {
                let Operand::Block(from) = function.instruction(user).operands[index as usize + 1] else {
                    unreachable!("a phi arm names its block")
                };
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
