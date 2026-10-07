//! The range of each integer parameter of a body only the program calls,
//! from what its callers pass: LLVM's IPSCCP, which states an argument's
//! range as a `range` attribute where it keeps no single constant.
//!
//! A call's actual is read in its caller by `ranges`, with the ranges
//! already stamped on the caller's own parameters, so a recursion that
//! passes `row + 1` under `row != 7` is found by iterating: each round
//! stamps what the last round's callers pass, joined with it, widened
//! after the second to the numbers the callee compares the parameter
//! with. A range that has not stopped growing after `ROUNDS` is withdrawn
//! with every other this pass stamped: a stamp is a promise, and only a
//! fixed point keeps it.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::callgraph::Defined;
use llrm_mir::facts::{Bounds, Fact};
use llrm_mir::module::{GlobalKind, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::program::Program;
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use crate::cfg;
use crate::consts::Known;
use crate::effects;
use crate::memory::Unit;
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
    fn joined(&self, other: &Interval) -> Self {
        match self {
            Seen::Nothing => Seen::Within(other.clone()),
            Seen::Within(one) if one.width == other.width => Seen::Within(Interval { low: one.low.clone().min(other.low.clone()), high: one.high.clone().max(other.high.clone()), width: one.width }),
            _ => Seen::Anything,
        }
    }
}

/// `operand` in `unit`'s function where `scope` holds: its interval, or one
/// computed from its operands' a level or two down.
fn actual(unit: &Unit, scope: &IndexMap<ValueId, Interval>, facts: &IndexMap<ValueId, Known>, operand: Operand, depth: u32) -> Option<Interval> {
    if let Some(found) = ranges::_operand(unit, operand, scope, facts) {
        return Some(found);
    }
    let Operand::Value(value) = operand else { return None };
    let ValueDef::Instruction(inst) = unit.function.value(value).def else { return None };
    if depth == 0 {
        return None;
    }
    let mut known = scope.clone();
    for &one in &unit.function.instruction(inst).operands {
        if let Operand::Value(from) = one
            && !known.contains_key(&from)
            && let Some(found) = actual(unit, scope, facts, one, depth - 1)
        {
            known.insert(from, found);
        }
    }
    ranges::_computed(unit, inst, &known, facts)
}

/// The numbers `parameter` of the body at `defined` is compared with, one either side of each,
/// then the type's own bounds: where a growing range stops.
fn thresholds(program: &Program, defined: Defined, index: usize, width: u32) -> Vec<BigInt> {
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

/// The `range` attribute stating `interval` of a parameter of type `ty`, where it says something.
fn attribute(context: &llrm_mir::Context, ty: llrm_mir::types::TypeId, interval: &Interval) -> Option<Attribute> {
    let bits = context.types.int_bits(ty)?;
    let half = BigInt::from(1) << (bits - 1);
    if interval.low <= -half.clone() && interval.high >= half - 1 {
        return None;
    }
    let (lo, hi) = (i64::try_from(&interval.low).ok()?, i64::try_from(&interval.high).ok()?);
    Fact::Range(Bounds { lo, hi }).typed_attribute(ty, bits)
}

/// Stamps each `eligible` body's integer parameters with the range its callers pass, where they all are
/// known; the bodies whose parameters it stamped.
pub fn stamp(program: &mut Program, eligible: &BTreeSet<Defined>) -> BTreeSet<Defined> {
    // The parameters worth following: an integer one of a body nothing but the program calls.
    let mut seen: BTreeMap<(Defined, usize), Seen> = BTreeMap::new();
    for &(at, id) in eligible {
        let module = &program.modules[at];
        let Some(function) = module.global(id).function() else { continue };
        for (index, &parameter) in function.parameters().iter().enumerate() {
            let declared = function.parameter_attrs.get(index).is_some_and(|one| one.iter().any(|attribute| matches!(attribute, Attribute::Range { .. })));
            if !declared && !function.users(parameter).is_empty() && module.context.types.int_bits(function.value(parameter).ty).is_some() {
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
        for (at, module) in program.modules.iter().enumerate() {
            for (own, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
                // A body none has called yet, or only itself: what it passes is not known, so not counted, until it is.
                let waiting = seen.iter().any(|(&(one, _), now)| one == (at, own) && *now == Seen::Nothing);
                let calls: Vec<_> = function
                    .walk()
                    .filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_)))
                    .filter_map(|(block, inst)| {
                        let target = effects::callee(&module.context, function, inst).and_then(|callee| program.definition(at, callee))?;
                        (0..function.instruction(inst).operands.len()).any(|index| seen.contains_key(&(target, index))).then_some((block, inst, target))
                    })
                    .collect();
                if calls.is_empty() {
                    continue;
                }
                let unit = Unit::of(module, &program.layout, function);
                // Found once, for the body as it is: its ranges and the arguments' proofs ask the same.
                let registers = crate::consts::known(&unit, None, None, None);
                let unit = unit.with_registers(&registers);
                let scoped = ranges::bounded(&unit).unwrap_or_default();
                for (block, inst, target) in calls {
                    let scope = scoped.get(&cfg::id(block)).cloned().unwrap_or_default();
                    let parameters = program.modules[target.0].global(target.1).function().expect("a procedure").parameters().len();
                    for index in 0..parameters {
                        let Some(entry) = next.get_mut(&(target, index)) else { continue };
                        let found = function.instruction(inst).operands.get(index).and_then(|&operand| actual(&unit, &scope, &registers, operand, 2));
                        *entry = match found {
                            Some(found) => entry.joined(&found),
                            None if waiting => continue,
                            None => Seen::Anything,
                        };
                    }
                }
            }
        }
        // A range still growing after the second round goes to the next number the callee compares it with.
        if round >= 2 {
            for (&(target, index), now) in next.iter_mut() {
                let (Seen::Within(new), Some(Seen::Within(old))) = (&*now, seen.get(&(target, index))) else { continue };
                if new == old {
                    continue;
                }
                let steps = thresholds(program, target, index, new.width);
                let high = if new.high > old.high { steps.iter().find(|one| **one >= new.high).cloned().unwrap_or_else(|| new.high.clone()) } else { new.high.clone() };
                let low = if new.low < old.low { steps.iter().rev().find(|one| **one <= new.low).cloned().unwrap_or_else(|| new.low.clone()) } else { new.low.clone() };
                *now = Seen::Within(Interval { low, high, width: new.width });
            }
        }
        if next == seen {
            settled = true;
            break;
        }
        seen = next;
        // What this round found is what the next round reads of each callee's parameters.
        for (&((at, id), index), now) in &seen {
            let Seen::Within(interval) = now else {
                // Seen to be anything after all: what an earlier round stamped goes.
                if let Some(old) = stamped.remove(&((at, id), index))
                    && let GlobalKind::Function(function) = &mut program.modules[at].globals[id.0 as usize].kind
                {
                    function.parameter_attrs[index].retain(|one| *one != old);
                }
                continue;
            };
            let module = &mut program.modules[at];
            let ty = module.global(id).function().expect("a procedure").value(module.global(id).function().expect("a procedure").parameters()[index]).ty;
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
