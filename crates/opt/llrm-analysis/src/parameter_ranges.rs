//! Bounds each integer parameter of a body only the program calls by what its
//! callers pass, so the body's own checks on it can be decided.
//!
//! # How it works
//! `stamp` takes the bodies that are neither exported nor address-taken. For
//! each integer parameter that has users and no `range` attribute, it collects
//! what every direct call (`call` or `invoke`) passes. A call's actual is read
//! in its caller by [`crate::ranges`] under the interval scope of the call's
//! block, with the caller's own parameters' stamps, and for an operand with no
//! interval, computed from its operands two levels down. A call in a block
//! `ranges` finds unreachable passes nothing. The actuals are joined into
//! `Seen`: `Nothing`, `Within(interval)` or `Anything`. A caller that has not
//! itself been seen yet is `waiting` and does not turn an unknown actual into
//! `Anything`.
//!
//! Rounds repeat, each stamping what the last round saw as a `range` attribute
//! on the callee, so the next round's callers read it. A recursion that passes
//! `row + 1` under `row != 7` is found this way.
//!
//! ```text
//!  f(row) called with 0, and by f itself as f(row + 1) under row != 7
//!  round 0   callers outside pass 0            row in [0,0]
//!  round 1   f(row + 1) under row != 7         row in [0,1]
//!  round 2   still growing: high jumps to the next number f compares row
//!            with (6, 7 or 8)                  row in [0,6]
//!  round 3   6 + 1 is 7: high jumps to 7       row in [0,7]
//!  round 4   no change: fixed point, stamps stay
//! ```
//!
//! From the third round (`round >= 2`) a bound that still moves jumps to the
//! next number the callee compares the parameter with (one either side of each
//! `icmp` constant, then the type's bounds). After `ROUNDS` (8) with no fixed
//! point every stamp of the call is withdrawn and nothing is returned: a stamp
//! is a promise, and only a fixed point keeps it. A parameter seen to be
//! `Anything` loses an earlier stamp. A range covering the whole type states
//! nothing and is not stamped.
//!
//! # Example
//! `@place` is called from `@f` with 0 and from itself with `%row + 1`:
//!
//! ```mir
//! define internal i16 @place(i16 %row, i16 %n) {
//! b:
//!   %done = icmp eq i16 %row, %n
//!   br i1 %done, label %leaf, label %more
//! more:
//!   %fits = icmp ult i16 %row, 12
//!   br i1 %fits, label %next, label %crash
//! next:
//!   %up = add nsw i16 %row, 1
//!   %r = call i16 @place(i16 %up, i16 %n)
//!   ret i16 %r
//! leaf:
//!   ret i16 1
//! crash:
//!   ret i16 99
//! }
//!
//! define i16 @f(i16 %a) {
//! b:
//!   %x = call i16 @place(i16 0, i16 7)
//!   ret i16 %x
//! }
//! ```
//!
//! The printed body then declares the parameter as `range(i16 0, 8) %row`.
//!
//! From unit test
//! `test_what_the_callers_pass_bounds_a_parameter_the_body_checks` in
//! `crates/opt/llrm-transforms/src/interprocedural_tests.rs`.
//!
//! # Provides
//! `stamp`: a `range` attribute on each integer parameter whose callers all
//! pass a known range, and the set of bodies stamped. Consumer: the
//! `interprocedural` program pass in llrm-transforms, which calls it up to 4
//! times after its constant and inlining rounds when `Interprocedural::ranges`
//! is set (gcc's `-fipa-vrp`, -O2 and up), re-optimises the bodies it returns
//! and stops when none.
//!
//! # Needs
//! [`crate::ranges`] (`bounds`, interval of an operand), [`crate::consts`],
//! [`crate::effects`] for `callee`, `llrm_mir::program::Program`.
//!
//! # See also
//! LLVM's IPSCCP, which states an argument's range as a `range` attribute where
//! it has no single constant.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::callgraph::Defined;
use llrm_mir::context::ConstantId;
use llrm_mir::facts::{Bounds, Fact};
use llrm_mir::module::{GlobalKind, Mark, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::program::Program;
use llrm_support::hash::IndexMap;

use crate::cfg;
use crate::consts::Known;
use crate::effects;
use crate::memory::Unit;
use crate::num::Num as BigInt;
use crate::ranges::{self, Interval};

const ROUNDS: usize = 8;

/// What the calls of a body pass for one parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Seen {
    Nothing,
    Within(Interval),
    Anything,
}

impl Seen {
    fn joined(
        &self,
        other: &Interval,
    ) -> Self {
        match self {
            Seen::Nothing => Seen::Within(other.clone()),
            Seen::Within(one) if one.width == other.width => Seen::Within(Interval {
                low: one.low.clone().min(other.low.clone()),
                high: one.high.clone().max(other.high.clone()),
                width: one.width,
            }),
            _ => Seen::Anything,
        }
    }
}

/// `operand` in `unit`'s function where `scope` holds: its interval, or one
/// computed from its operands' a level or two down.
fn actual(
    unit: &Unit,
    scope: &dyn crate::ranges::Lookup,
    facts: &IndexMap<ValueId, Known>,
    operand: Operand,
    depth: u32,
) -> Option<Interval> {
    if let Some(found) = ranges::_operand(unit, operand, scope, facts) {
        return Some(found);
    }
    let Operand::Value(value) = operand else { return None };
    let ValueDef::Instruction(inst) = unit.function.value(value).def else { return None };
    if depth == 0 {
        return None;
    }
    let mut found_here = crate::ranges::Intervals::default();
    for &one in &unit.function.instruction(inst).operands {
        if let Operand::Value(from) = one
            && scope.lookup(&from).is_none()
            && !found_here.contains_key(&from)
            && let Some(found) = actual(unit, scope, facts, one, depth - 1)
        {
            found_here.insert(from, found);
        }
    }
    ranges::_computed(unit, inst, &crate::ranges::Over { delta: found_here, base: scope }, facts)
}

/// The numbers `parameter` of the body at `defined` is compared with, one
/// either side of each, then the type's own bounds: where a growing range
/// stops.
fn thresholds(
    program: &Program,
    defined: Defined,
    index: usize,
    width: u32,
) -> Vec<BigInt> {
    let (at, id) = defined;
    let module = &program.modules[at];
    let function = module.global(id).function().expect("a procedure");
    let unit = Unit::of(module, &program.layout, function);
    let half = BigInt::from(1) << (width - 1);
    let mut found = BTreeSet::from([-half.clone(), half - 1]);
    let parameter = function.parameters()[index];
    for (_, inst) in function.walk() {
        let op = function.instruction(inst);
        if !matches!(op.opcode, Opcode::ICmp(_)) || !op.operands.contains(&Operand::Value(parameter)) {
            continue;
        }
        for &one in &op.operands {
            if let Some(bits) = unit.int_constant(one) {
                let number = BigInt::from(llrm_mir::context::signed(bits, width));
                found.extend([&number - 1, number.clone(), number + 1]);
            }
        }
    }
    found.into_iter().collect()
}

/// The `range` attribute stating `interval` of a parameter of type `ty`, where
/// it says something.
fn attribute(
    context: &llrm_mir::Context,
    ty: llrm_mir::types::TypeId,
    interval: &Interval,
) -> Option<Attribute> {
    let bits = context.types.int_bits(ty)?;
    let half = BigInt::from(1) << (bits - 1);
    if interval.low <= -half.clone() && interval.high >= half - 1 {
        return None;
    }
    let (lo, hi) = (i64::try_from(&interval.low).ok()?, i64::try_from(&interval.high).ok()?);
    Fact::Range(Bounds { lo, hi }).typed_attribute(ty, bits)
}

/// What a body's solve reads outside the body: the attributes of every function
/// and the kind of every variable (`ranges` reads a callee's `return_attrs`,
/// `consts` a global's constness and initializer). A callee's body and its
/// parameter attributes are not in it: stamping changes the second each round.
#[derive(Clone, Debug, PartialEq)]
enum GlobalFact {
    Function { attrs: Vec<Attribute>, returns: Vec<Attribute> },
    Variable { constant: bool, initializer: Option<ConstantId> },
}

fn module_facts(module: &llrm_mir::module::Module) -> Vec<GlobalFact> {
    module
        .globals
        .iter()
        .map(|global| match &global.kind {
            GlobalKind::Function(function) => {
                GlobalFact::Function { attrs: function.attrs.clone(), returns: function.return_attrs.clone() }
            }
            GlobalKind::Variable(variable) => {
                GlobalFact::Variable { constant: variable.constant, initializer: variable.initializer }
            }
        })
        .collect()
}

/// What a calling body's solve found, and what it was found over.
struct Solved {
    mark: Mark,
    parameters: Vec<Vec<Attribute>>,
    facts: Vec<GlobalFact>,
    registers: IndexMap<ValueId, Known>,
    scoped: ranges::Bounds,
}

/// The solves of the calling bodies, kept between rounds of `stamp_with` and
/// between calls of it: a body's solve is a function of the body, its own
/// parameters' attributes and the module's facts, so it stands until one of
/// them moves.
#[derive(Default)]
pub struct Cache {
    solved: BTreeMap<Defined, Solved>,
    /// Bodies solved so far (a hit is not one).
    pub solves: usize,
}

/// `stamp_with` with a fresh cache.
pub fn stamp(
    program: &mut Program,
    eligible: &BTreeSet<Defined>,
) -> BTreeSet<Defined> {
    stamp_with(program, eligible, &mut Cache::default())
}

/// Stamps each `eligible` body's integer parameters with the range its callers
/// pass, where they all are known; the bodies whose parameters it stamped. The
/// solves of the calling bodies are kept in `cache`.
pub fn stamp_with(
    program: &mut Program,
    eligible: &BTreeSet<Defined>,
    cache: &mut Cache,
) -> BTreeSet<Defined> {
    // The parameters worth following: an integer one of a body nothing but the
    // program calls.
    let mut seen: BTreeMap<(Defined, usize), Seen> = BTreeMap::new();
    for &(at, id) in eligible {
        let module = &program.modules[at];
        let Some(function) = module.global(id).function() else { continue };
        for (index, &parameter) in function.parameters().iter().enumerate() {
            let declared = function
                .parameter_attrs
                .get(index)
                .is_some_and(|one| one.iter().any(|attribute| matches!(attribute, Attribute::Range { .. })));
            if !declared
                && !function.users(parameter).is_empty()
                && module.context.types.int_bits(function.value(parameter).ty).is_some()
            {
                seen.insert(((at, id), index), Seen::Nothing);
            }
        }
    }
    if seen.is_empty() {
        return BTreeSet::new();
    }
    let mut stamped: BTreeMap<(Defined, usize), Attribute> = BTreeMap::new();
    let mut settled = false;
    for round in 0..ROUNDS {
        let mut next = seen.clone();
        let facts: Vec<Vec<GlobalFact>> = program.modules.iter().map(module_facts).collect();
        for (at, module) in program.modules.iter().enumerate() {
            for (own, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
                // A body none has called yet, or only itself: what it passes is
                // not known, so not counted, until it is.
                let waiting =
                    seen.range(((at, own), 0)..=((at, own), usize::MAX)).any(|(_, now)| *now == Seen::Nothing);
                let calls: Vec<_> = function
                    .walk()
                    .filter(|&(_, inst)| {
                        matches!(
                            function.instruction(inst).opcode,
                            Opcode::Call(_) | Opcode::Invoke(_)
                        )
                    })
                    .filter_map(|(block, inst)| {
                        let target = effects::callee(&module.context, function, inst)
                            .and_then(|callee| program.definition(at, callee))?;
                        (0..function.instruction(inst).operands.len())
                            .any(|index| seen.contains_key(&(target, index)))
                            .then_some((block, inst, target))
                    })
                    .collect();
                if calls.is_empty() {
                    continue;
                }
                let shape = cfg::Shape::of(function);
                let key = (at, own);
                // Found once for the body as it is, and again only where the
                // body, its own parameters' attributes or the module's facts
                // moved: its ranges and the arguments' proofs ask the same.
                let kept = cache.solved.get(&key).is_some_and(|one| {
                    one.mark == function.mark() && one.parameters == function.parameter_attrs && one.facts == facts[at]
                });
                if !kept {
                    let unit = Unit::of(module, &program.layout, function).with_shape(&shape);
                    let registers = crate::consts::known(&unit);
                    let scoped = {
                        let unit = unit.with_registers(&registers);
                        ranges::bounds(&unit).map(std::borrow::Cow::into_owned).unwrap_or_default()
                    };
                    cache.solves += 1;
                    cache
                        .solved
                        .insert(
                            key,
                            Solved {
                                mark: function.mark(),
                                parameters: function.parameter_attrs.clone(),
                                facts: facts[at].clone(),
                                registers,
                                scoped,
                            },
                        );
                }
                let solved = &cache.solved[&key];
                let (registers, scoped) = (&solved.registers, &solved.scoped);
                let unit = Unit::of(module, &program.layout, function).with_shape(&shape).with_registers(registers);
                for (block, inst, target) in calls {
                    // A round assumes a range before it proves it: where the
                    // loops' facts contradict each other under
                    // the assumption the block is not reached, and a call in it
                    // passes nothing.
                    if scoped.unreachable(cfg::id(block)) {
                        continue;
                    }
                    let scope = scoped.scope(&unit, cfg::id(block));
                    let parameters =
                        program.modules[target.0].global(target.1).function().expect("a procedure").parameters().len();
                    for index in 0..parameters {
                        let Some(entry) = next.get_mut(&(target, index)) else { continue };
                        let found = function
                            .instruction(inst)
                            .operands
                            .get(index)
                            .and_then(|&operand| actual(&unit, &scope, registers, operand, 2));
                        *entry = match found {
                            Some(found) => entry.joined(&found),
                            None if waiting => continue,
                            None => Seen::Anything,
                        };
                    }
                }
            }
        }
        // A range still growing after the second round goes to the next number
        // the callee compares it with.
        if round >= 2 {
            for (&(target, index), now) in next.iter_mut() {
                let (Seen::Within(new), Some(Seen::Within(old))) = (&*now, seen.get(&(target, index))) else {
                    continue;
                };
                if new == old {
                    continue;
                }
                let steps = thresholds(program, target, index, new.width);
                let high = if new.high > old.high {
                    steps.iter().find(|one| **one >= new.high).cloned().unwrap_or_else(|| new.high.clone())
                } else {
                    new.high.clone()
                };
                let low = if new.low < old.low {
                    steps.iter().rev().find(|one| **one <= new.low).cloned().unwrap_or_else(|| new.low.clone())
                } else {
                    new.low.clone()
                };
                *now = Seen::Within(Interval { low, high, width: new.width });
            }
        }
        if next == seen {
            settled = true;
            break;
        }
        seen = next;
        // What this round found is what the next round reads of each callee's
        // parameters.
        for (&((at, id), index), now) in &seen {
            let Seen::Within(interval) = now else {
                // Seen to be anything after all: what an earlier round stamped
                // goes.
                if let Some(old) = stamped.remove(&((at, id), index))
                    && let GlobalKind::Function(function) = &mut program.modules[at].globals[id.0 as usize].kind
                {
                    function.parameter_attrs[index].retain(|one| *one != old);
                }
                continue;
            };
            let module = &mut program.modules[at];
            let ty = module
                .global(id)
                .function()
                .expect("a procedure")
                .value(module.global(id).function().expect("a procedure").parameters()[index])
                .ty;
            let Some(stated) = attribute(&module.context, ty, interval) else { continue };
            let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else { continue };
            if function.parameter_attrs.len() <= index {
                function.parameter_attrs.resize(index + 1, Vec::new());
            }
            if let Some(old) = stamped.insert(((at, id), index), stated.clone()) {
                function.parameter_attrs[index].retain(|one| *one != old);
            }
            function.parameter_attrs[index].push(stated);
        }
    }
    let done: BTreeSet<Defined> = stamped.keys().map(|&(one, _)| one).collect();
    if !settled {
        for (&((at, id), index), stated) in &stamped {
            if let GlobalKind::Function(function) = &mut program.modules[at].globals[id.0 as usize].kind {
                function.parameter_attrs[index].retain(|one| one != stated);
            }
        }
        return BTreeSet::new();
    }
    done
}
