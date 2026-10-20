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
//! 1. `simplifyindvar::simplified_and_extended` (`simplifyAndExtend`): the IV
//!    users simplified and a counter read through `zext` or `sext` at a wider
//!    index becomes a counter of that width (`WidenIV`). C's `unsigned short i`
//!    reads `a[zext i]`: a conversion per trip, which hides the loop from
//!    idioms and strength reduction.
//! 2. `congruentiv::replaced` (`SCEVExpander::replaceCongruentIVs`; the older
//!    `congruent` under `LLRM_NO_SCEV_CONGRUENT`): header phis SCEV proves
//!    equal, a narrow one a truncation of a wide one included, are one, and
//!    `dead` removes the rest.
//! 3. `canonexit::canonicalized` (`canonicalizeExitCondition`;
//!    `LLRM_NO_SCEV_CANONEXIT`), `exitopt::optimized` (`optimizeLoopExits`;
//!    `LLRM_NO_SCEV_EXITOPT`), `exitpredicate::predicated`
//!    (`predicateLoopExits`; `LLRM_NO_SCEV_PREDICATE`), then `exitfold::folded`
//!    (`LLRM_NO_EXITFOLD`): exits their counts decide are folded.
//! 4. `lftr::replaced` (`linearFunctionTestReplace`; `LLRM_NO_SCEV_LFTR`): an
//!    exit tests a counter against the value it has at the last trip.
//! 5. `exitsink::sunk`: final updates only exit phis read move to the exits.
//! 6. `rewound`: see below.
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
//! In the body pipeline after `promote`. With `-ftree-ch` (not at -Os) it runs
//! again after the loop-header copy, where the copy made a counted exit's test
//! the one that leaves (a number is a `mov` where the counter's register
//! already holds it).
//!
//! # Needs
//! `Registers`, `induction`, `liveness`,
//! [`crate::exitvalues`], [`crate::exitfold`], [`crate::exitsink`],
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
use llrm_analysis::induction;
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::{cfg, liveness, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_support::hash::IndexMap;
use num_traits::Zero;

use crate::counting;
use crate::dead;
use crate::lcssa::{arms, from_arms};
use crate::profit::OperationCosts;

/// What IndVarSimplify leaves a loop's counters: values after a loop
/// computed from its trips (`exitvalues`), final updates sunk to
/// the exits (`exitsink`), and an inner recurrence's exit value carried
/// round the loop outside (`rewound`). Which counters the loop keeps is
/// `lsr`'s.
pub struct IndVars {
    /// Whether LFTR runs: opt-in, `LLRM_LFTR` (D22).
    pub lftr: bool,
}

/// `LLRM_LFTR`, read once: LFTR runs in the pipeline (D22). Off by default:
/// the old `lsr` does not reduce the counter LFTR leaves, and every counted
/// loop got a few bytes larger (+0.4% bytes of m32 code, rows pair on #2752).
pub fn lftr_default() -> bool {
    llrm_support::env_flag!("LLRM_LFTR")
}

impl FunctionPass for IndVars {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "indvars"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("indvars")
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
        let shape = analyses.get::<cfg::Shape>(unit.context, unit.layout, unit.function);
        // Once the body has changed, the manager brings what it holds up to
        // date from the changes logged, where `Standing` would derive
        // it again (13 times at d_faces -O2, 25 M).
        let mut standing = llrm_analysis::memory::Standing::held(&held)
            .with_shape(shape)
            .renewing(
                |context, layout, function, reshaped| {
                    let kept = if reshaped {
                        PreservedAnalyses::none()
                    } else {
                        PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>()
                    };
                    analyses.invalidate(function, &kept);
                    analyses.get::<llrm_analysis::manager::Registers>(context, layout, function)
                },
            );
        let compared = crate::simplifyindvar::simplified_and_extended(
            unit.context,
            unit.layout,
            unit.function,
            unit.declared,
            &outer,
        );
        if compared {
            standing.changed();
        }
        let rewrote = crate::exitvalues::enabled()
            && crate::exitvalues::rewritten(unit.context, unit.layout, unit.function, unit.declared, &outer);
        if rewrote {
            standing.changed();
        }
        let merged = twin_phis(unit.function)
            | if crate::congruentiv::enabled() {
                let merged = crate::congruentiv::replaced(unit.context, unit.layout, unit.function, &outer);
                if merged {
                    standing.changed();
                }
                merged
            } else {
                congruent(unit.context, unit.layout, unit.function, &outer, &mut standing)
            };
        let canonical = crate::canonexit::enabled()
            && crate::canonexit::canonicalized(unit.context, unit.layout, unit.function, &outer);
        if canonical {
            standing.changed();
        }
        let optimized = crate::exitopt::enabled()
            && crate::exitopt::optimized(unit.context, unit.layout, unit.function, unit.declared, &outer);
        if optimized {
            standing.changed();
        }
        let predicated = crate::exitpredicate::enabled()
            && crate::exitpredicate::predicated(unit.context, unit.layout, unit.function, unit.declared, &outer);
        if predicated {
            standing.changed();
        }
        let folded = crate::exitfold::enabled()
            && crate::exitfold::folded_with(unit.context, unit.layout, unit.function, &outer, &mut standing);
        if folded {
            standing.reshaped();
        }
        let tested = self.lftr
            && crate::lftr::enabled()
            && crate::lftr::replaced(unit.context, unit.layout, unit.function, unit.declared, &outer);
        if tested {
            standing.changed();
        }
        let shape = standing.shape(unit.function);
        let sunk = crate::exitsink::sunk(unit.function, &shape);
        if sunk {
            standing.changed();
        }
        let rewound = rewound_with(
            unit.context,
            unit.layout,
            unit.function,
            &outer,
            &mut standing,
            crate::profit::registers(&outer).registers,
            &crate::profit::costs(&outer),
        );
        let dead = (compared || rewrote || canonical || optimized || predicated || tested || sunk || rewound || merged)
            && dead::dead(unit.context, outer.callees(), unit.function);
        if folded {
            PreservedAnalyses::none()
        } else if compared
            || rewrote
            || canonical
            || optimized
            || predicated
            || tested
            || sunk
            || rewound
            || merged
            || dead
        {
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
    context: &Context,
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

/// Phis of one block whose arms are, for each predecessor, one value or two
/// pure binary operations of the same operands: the later reads as the first.
/// DEVIATE (IV1, docs/architecture/loop-passes-port.md): `SCEVExpander::
/// replaceCongruentIVs` finds them by their recurrences; this is by shape.
/// LoopRotate leaves a counter twice that way, a phi of its sign-extension
/// stepping by a copy of the counter's own `add`.
fn twin_phis(function: &mut Function) -> bool {
    #[derive(Eq, Hash, PartialEq)]
    enum Arm {
        Value(Operand),
        Made(&'static str, Vec<Operand>),
    }
    let mut first = IndexMap::default();
    let mut merged = Vec::new();
    for (phi, block, _) in llrm_analysis::occurrence::phis(function) {
        let Some(result) = function.instruction(phi).result else { continue };
        let mut key = arms(function, phi)
            .into_iter()
            .map(|(value, from)| {
                let made = match value {
                    Operand::Value(one) => _defining(function, one).map(|inst| function.instruction(inst)),
                    _ => None,
                }
                .filter(|made| matches!(made.opcode, Opcode::Binary(_)));
                let arm = match made {
                    Some(made) => Arm::Made(made.opcode.mnemonic(), made.operands.clone()),
                    None => Arm::Value(value),
                };
                (from.0, arm)
            })
            .collect::<Vec<_>>();
        key.sort_by_key(|(from, _)| *from);
        let ty = function.value(result).ty;
        let kept = *first.entry((block, ty, key)).or_insert(result);
        if kept != result {
            merged.push((result, kept));
        }
    }
    for &(later, kept) in &merged {
        function.replace_all_uses_with(later, Operand::Value(kept));
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
    context: &Context,
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
    context: &Context,
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
        let first = function.block(plan.exit).first().expect("a block with an instruction");
        function.insert(closed, Position::Before(first)).expect("a placed block");
        Operand::Value(function.instruction(closed).result.expect("a phi's value"))
    };
    let width = context.types.int_bits(ty).expect("an integer");
    let back = counting::constant(context, &-&plan.distance, width);
    let reset =
        function.create_instruction(Opcode::Binary(BinaryOp::Add), ty, vec![exit_value, back], Flags::default(), None);
    let after = function
        .iter_block(plan.exit)
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
    let first = function.block(plan.parent_header).first().expect("a block with an instruction");
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
pub(crate) mod tests;
