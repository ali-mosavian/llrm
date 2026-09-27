//! Adapted from llrm-core's `analysis/frameescape.rs`: which frame objects'
//! addresses reach where.
//!
//! A frame object is an `alloca`, named by its result; the old frame
//! offsets and extents were the raise's names for the same thing, and an
//! alloca is its own extent. So `reach` is `exposed` and is gone, and the
//! old `opaque_addresses` (an address the raise could not name) cannot
//! occur. The old `Copy` is `bitcast`, `addrspacecast` or `freeze`, and the
//! old `Address`, `Add` and `Sub` of an address are `getelementptr`.
//!
//! A load or store's own pointer is an access, not an exposure: MIR has no
//! frame access that does not go through an address, where the old one
//! named its slot directly.
//!
//! Skipped: `test_opaque_address_is_not_an_empty_escape_proof` (MIR has no
//! opaque address); `test_renderer_exposes_temporary_string_not_counter_address`
//! reads a BC fixture and stays behind.

use std::collections::BTreeSet;

use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_support::hash::IndexMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escapes {
    /// The allocas whose address each value may hold.
    pub origins: IndexMap<ValueId, BTreeSet<ValueId>>,
    pub exposed: BTreeSet<ValueId>,
}

/// The operand an address-preserving instruction moves: the old `Copy` and
/// `Address`.
fn source(function: &Function, inst: InstId) -> Option<Operand> {
    let instruction = function.instruction(inst);
    match instruction.opcode {
        Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) | Opcode::Freeze => instruction.operands.first().copied(),
        _ => None,
    }
}

/// The operand positions of `inst` that are accesses through an address.
fn accessed(function: &Function, inst: InstId) -> Option<usize> {
    match function.instruction(inst).opcode {
        Opcode::Load { .. } => Some(0),
        Opcode::Store { .. } => Some(1),
        _ => None,
    }
}

pub fn analysed(function: &Function) -> Escapes {
    // Origins are not allocation bounds. Absence here says nothing about
    // runtime frame walking, callbacks, or pointers loaded from memory.
    let mut origins: IndexMap<ValueId, BTreeSet<ValueId>> = IndexMap::default();
    let instructions: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
    let is = |inst: InstId, phi: bool| (function.instruction(inst).opcode == Opcode::Phi) == phi;
    let phis: Vec<InstId> = instructions.iter().copied().filter(|&inst| is(inst, true)).collect();
    let operations: Vec<InstId> = instructions.iter().copied().filter(|&inst| is(inst, false)).collect();
    let moving = |inst: InstId| matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }) || source(function, inst).is_some();

    let inputs = |inst: InstId, origins: &IndexMap<ValueId, BTreeSet<ValueId>>| -> BTreeSet<ValueId> {
        let instruction = function.instruction(inst);
        let mut direct = BTreeSet::new();
        if let (Opcode::Alloca { .. }, Some(result)) = (&instruction.opcode, instruction.result) {
            direct.insert(result);
        }
        for (index, operand) in instruction.operands.iter().enumerate() {
            if let Operand::Value(value) = operand
                && accessed(function, inst) != Some(index)
            {
                direct.extend(origins.get(value).into_iter().flatten().copied());
            }
        }
        direct
    };

    loop {
        let mut changed = false;
        for &phi in &phis {
            let instruction = function.instruction(phi);
            let Some(result) = instruction.result else { continue };
            let incoming: BTreeSet<ValueId> = instruction
                .operands
                .iter()
                .filter_map(|operand| match operand {
                    Operand::Value(value) => origins.get(value),
                    _ => None,
                })
                .flatten()
                .copied()
                .collect();
            let previous = origins.get(&result).cloned().unwrap_or_default();
            if !incoming.is_subset(&previous) {
                origins.insert(result, previous.union(&incoming).copied().collect());
                changed = true;
            }
        }
        for &inst in &operations {
            if !moving(inst) {
                continue;
            }
            let incoming = inputs(inst, &origins);
            let Some(value) = function.instruction(inst).result else { continue };
            let previous = origins.get(&value).cloned().unwrap_or_default();
            if !incoming.is_subset(&previous) {
                origins.insert(value, previous.union(&incoming).copied().collect());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let exposed: BTreeSet<ValueId> = operations.iter().filter(|&&inst| !moving(inst)).flat_map(|&inst| inputs(inst, &origins)).collect();
    Escapes { origins, exposed }
}

/// `side`'s three answers.
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Number,
    Address,
    Unknown,
}

/// `moved`'s answers: allocas, None where it cannot, `...` while unknown.
enum Moved {
    Extents(BTreeSet<ValueId>),
    None,
    Ellipsis,
}

/// Values that hold an address inside frame objects on every path, with
/// those objects.
///
/// C's pointer arithmetic stays inside its object, so an address the body took
/// of a local, moved by an integer, still reaches only that local's bytes.
pub fn framed(function: &Function) -> IndexMap<ValueId, BTreeSet<ValueId>> {
    let instructions: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
    let phis: Vec<InstId> = instructions.iter().copied().filter(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect();
    let mut moving: IndexMap<ValueId, InstId> = IndexMap::default();
    // A parameter is defined by nothing here: the old incoming value no op defines.
    let mut refuted: BTreeSet<ValueId> = function.parameters().iter().copied().collect();
    for &inst in &instructions {
        let instruction = function.instruction(inst);
        let Some(value) = instruction.result else { continue };
        if instruction.opcode == Opcode::Phi {
            continue;
        }
        if matches!(instruction.opcode, Opcode::Alloca { .. }) || source(function, inst).is_some() {
            moving.insert(value, inst);
        } else {
            refuted.insert(value);
        }
    }
    let incoming = |phi: InstId| -> Vec<Operand> {
        function.instruction(phi).operands.iter().copied().filter(|operand| !matches!(operand, Operand::Block(_))).collect()
    };
    let result = |inst: InstId| function.instruction(inst).result.expect("a phi has a result");
    let mut state: IndexMap<ValueId, BTreeSet<ValueId>> = IndexMap::default();

    let side = |operand: Operand, state: &IndexMap<ValueId, BTreeSet<ValueId>>, refuted: &BTreeSet<ValueId>| match operand {
        Operand::Value(value) if refuted.contains(&value) => Side::Number,
        Operand::Value(value) if state.contains_key(&value) => Side::Address,
        Operand::Value(_) => Side::Unknown,
        _ => Side::Number,
    };
    // The allocas `inst` leaves its result in, None where it cannot, `...` while unknown.
    let moved = |inst: InstId, state: &IndexMap<ValueId, BTreeSet<ValueId>>, refuted: &BTreeSet<ValueId>| -> Moved {
        let instruction = function.instruction(inst);
        if let (Opcode::Alloca { .. }, Some(value)) = (&instruction.opcode, instruction.result) {
            return Moved::Extents(BTreeSet::from([value]));
        }
        let Some(source) = source(function, inst) else { return Moved::None };
        match (side(source, state, refuted), source) {
            (Side::Address, Operand::Value(value)) => Moved::Extents(state[&value].clone()),
            (Side::Unknown, _) => Moved::Ellipsis,
            _ => Moved::None,
        }
    };

    loop {
        let mut changed = true;
        while changed {
            changed = false;
            for &phi in &phis {
                let value = result(phi);
                if refuted.contains(&value) {
                    continue;
                }
                if incoming(phi).iter().any(|one| !matches!(one, Operand::Value(one) if !refuted.contains(one))) {
                    refuted.insert(value);
                    changed = true;
                    continue;
                }
                let union: BTreeSet<ValueId> = incoming(phi)
                    .iter()
                    .filter_map(|one| match one {
                        Operand::Value(one) => state.get(one),
                        _ => None,
                    })
                    .flatten()
                    .copied()
                    .collect();
                let previous = state.get(&value).cloned().unwrap_or_default();
                if !union.is_subset(&previous) {
                    state.insert(value, previous.union(&union).copied().collect());
                    changed = true;
                }
            }
            for (&value, &inst) in &moving {
                if refuted.contains(&value) {
                    continue;
                }
                match moved(inst, &state, &refuted) {
                    Moved::None => {
                        refuted.insert(value);
                        changed = true;
                    }
                    Moved::Extents(got) => {
                        let previous = state.get(&value).cloned().unwrap_or_default();
                        if !got.is_subset(&previous) {
                            state.insert(value, previous.union(&got).copied().collect());
                            changed = true;
                        }
                    }
                    Moved::Ellipsis => {}
                }
            }
        }
        // Optimism settles cycles; anything still unproven is refuted and the rest looked at again.
        let unproven: BTreeSet<ValueId> = moving
            .keys()
            .copied()
            .chain(phis.iter().map(|&phi| result(phi)))
            .filter(|value| {
                !refuted.contains(value)
                    && (!state.contains_key(value)
                        || moving.get(value).is_some_and(|&inst| !matches!(moved(inst, &state, &refuted), Moved::Extents(_)))
                        || (!moving.contains_key(value)
                            && phis
                                .iter()
                                .filter(|&&phi| result(phi) == *value)
                                .any(|&phi| incoming(phi).iter().any(|one| !matches!(one, Operand::Value(one) if state.contains_key(one))))))
            })
            .collect();
        if unproven.is_empty() {
            return state.into_iter().filter(|(value, _)| !refuted.contains(value)).collect();
        }
        refuted.extend(unproven);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{function, parsed, value};

    #[test]
    fn test_frame_origin_reaches_use_through_copy_and_loop_phi() {
        let sinks = ["call void @sink(ptr %copied)", "store ptr %copied, ptr @g", "%n = ptrtoint ptr %copied to i16", "ret ptr %copied"];
        for sink in sinks {
            let tail = if sink.starts_with("ret") { "" } else { "\n  ret ptr null" };
            let text = format!(
                "@g = global ptr null
declare void @sink(ptr)

define ptr @f(i1 %c) {{
entry:
  %root = alloca [16 x i8]
  br label %loop

loop:
  %joined = phi ptr [ %root, %entry ], [ %copied, %loop ]
  %copied = freeze ptr %joined
  br i1 %c, label %loop, label %out

out:
  {sink}{tail}
}}
"
            );
            let module = parsed(&text);
            let f = function(&module, "f");
            let result = analysed(f);
            let root = BTreeSet::from([value(f, "root")]);
            assert_eq!(result.origins[&value(f, "copied")], root, "{sink}");
            assert_eq!(result.exposed, root, "{sink}");
        }
    }

    #[test]
    fn framed_follows_steps_and_phis_but_not_a_parameter() {
        let module = parsed(
            "define void @f(i1 %c, ptr %p) {
entry:
  %a = alloca [4 x i16]
  br label %loop

loop:
  %q = phi ptr [ %a, %entry ], [ %n, %loop ]
  %n = getelementptr inbounds i16, ptr %q, i16 1
  br i1 %c, label %loop, label %out

out:
  %m = phi ptr [ %n, %loop ]
  %either = select i1 %c, ptr %m, ptr %p
  %r = getelementptr inbounds i16, ptr %p, i16 1
  ret void
}
",
        );
        let f = function(&module, "f");
        let found = framed(f);
        let a = BTreeSet::from([value(f, "a")]);
        for name in ["a", "q", "n", "m"] {
            assert_eq!(found.get(&value(f, name)), Some(&a), "{name}");
        }
        for name in ["either", "r", "p"] {
            assert!(!found.contains_key(&value(f, name)), "{name}");
        }
    }
}
