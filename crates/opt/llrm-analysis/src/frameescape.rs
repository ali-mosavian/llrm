//! Which frame objects' addresses reach where, and which are exposed.
//!
//! # How it works
//! A frame object is an `alloca`, named by its result. An address moves
//! through `getelementptr`, `bitcast`, `addrspacecast` and `freeze` (one
//! operand in, one out) and through phis. Everything else that uses an address
//! either accesses through it or exposes it:
//!
//! - a load's pointer and a store's pointer (not the stored value) are
//!   accesses;
//! - a lifetime marker, or a use `marker` says keeps no copy (`nocapture`),
//!   names an object without handing out its address;
//! - any other use, including storing the pointer, passing it to a call,
//!   returning it, `ptrtoint`, or comparing it, exposes it.
//!
//! `exposes(function, alloca, marker)` follows the alloca's uses through moves
//! and answers true at the first exposing use. `exposed_allocas` does it for
//! every alloca in one pass: it marks each value that has an exposing use,
//! then propagates back along the move edges, so an address that reaches an
//! exposing value through moves is exposed too. `analysed` computes the same
//! by fixed point, as `origins` (the allocas each value may hold the address
//! of) and `exposed`; only tests call it.
//!
//! ```text
//!  %a = alloca            %a --gep--> %b --phi--> %c
//!  %b = gep %a, 4         load %b          access: not exposure
//!  %c = phi [%b, ..]      call @f(%c)      exposure: %a is exposed
//! ```
//!
//! `framed(function)` is the other question: which values hold an address
//! inside frame objects on every path, and which objects. C's pointer
//! arithmetic stays inside its object, so an address the body took of a local
//! and moved by an integer still reaches only that local's bytes. It is
//! optimistic: phis and moves start unrefuted, a value is refuted when it has
//! a non-address definition (a parameter, a load, a call) or an input that is
//! a number, and refutation repeats until what remains is stable.
//!
//! # Example
//! Storing a slot's address exposes that slot; storing through a slot does not
//! expose it:
//!
//! ```mir
//! define void @f() {
//! b:
//!   %kept = alloca ptr
//!   %leaked = alloca i16
//!   store ptr %leaked, ptr %kept
//!   ret void
//! }
//! ```
//!
//! `analysed(f).exposed` is `{%leaked}`.
//!
//! From unit test
//! `storing_a_slots_address_exposes_it_but_storing_through_it_does_not` in
//! `frameescape.rs`.
//!
//! # Provides
//! `exposed_allocas`, `exposes`, `framed`, `analysed`. Consumers:
//! [`crate::memory`] (`object_of` marks an object captured when exposed), the
//! manager entry [`crate::manager::ExposedFrames`], [`crate::interprocedural`]
//! (`framed`) and the backend's `isel` (which frame objects are exposed).
//! `scans` counts `exposes` calls, for tests.

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
fn source(
    function: &Function,
    inst: InstId,
) -> Option<Operand> {
    let instruction = function.instruction(inst);
    match instruction.opcode {
        Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) | Opcode::Freeze => {
            instruction.operands.first().copied()
        }
        _ => None,
    }
}

/// The operand positions of `inst` that are accesses through an address.
fn accessed(
    function: &Function,
    inst: InstId,
) -> Option<usize> {
    match function.instruction(inst).opcode {
        Opcode::Load { .. } => Some(0),
        Opcode::Store { .. } => Some(1),
        _ => None,
    }
}

/// Whether `value` may hold an address no fact follows: a parameter, or a
/// result that is neither an alloca nor a move of one such value (a load, a
/// call, an integer cast), or a phi with such an arm. `seen` stops a cycle.
fn unfollowed(
    function: &Function,
    value: ValueId,
    seen: &mut llrm_mir::dense::IdSet<ValueId>,
) -> bool {
    use llrm_mir::module::ValueDef;
    if !seen.insert(value) {
        return false;
    }
    match function.value(value).def {
        ValueDef::Argument(_) => true,
        ValueDef::Instruction(inst) => {
            let instruction = function.instruction(inst);
            match instruction.opcode {
                Opcode::Alloca { .. } => false,
                Opcode::Phi => instruction.operands.chunks(2).any(|pair| {
                    matches!(
                        pair[0],
                        Operand::Value(arm) if unfollowed(function, arm, seen)
                    )
                }),
                _ => match source(function, inst) {
                    Some(Operand::Value(from)) => unfollowed(function, from, seen),
                    Some(_) => false,
                    None => true,
                },
            }
        }
    }
}

/// Whether the phi `inst` joins an address with one no fact follows: a pointer
/// loaded from memory, a parameter, a call's result. What the phi holds may
/// then be any of its other arms' objects, reached by a pointer that is no
/// fact, so those objects are exposed (a load through the join may read them).
fn joins_unfollowed(
    function: &Function,
    inst: InstId,
) -> bool {
    function
        .instruction(inst)
        .operands
        .chunks(2)
        .any(
            |pair| matches!(
                pair[0],
                Operand::Value(arm) if unfollowed(function, arm, &mut llrm_mir::dense::IdSet::new())
            ),
        )
}

/// The values that may hold an address no fact follows (`unfollowed`), for
/// every value at once: the parameters and the results that are neither an
/// alloca nor a move, spread to what moves them.
fn unfollowed_values(function: &Function) -> Vec<bool> {
    let mut unfollowed_value = vec![false; function.value_count()];
    let mut spread: Vec<ValueId> = Vec::new();
    for &parameter in function.parameters() {
        unfollowed_value[parameter.0 as usize] = true;
        spread.push(parameter);
    }
    for (_, inst) in function.walk() {
        let instruction = function.instruction(inst);
        let Some(result) = instruction.result else { continue };
        let moving =
            matches!(instruction.opcode, Opcode::Alloca { .. } | Opcode::Phi) || source(function, inst).is_some();
        if !moving {
            unfollowed_value[result.0 as usize] = true;
            spread.push(result);
        }
    }
    while let Some(value) = spread.pop() {
        for (to, _) in function.users(value).iter().map(|one| (one.user, one.index)) {
            let instruction = function.instruction(to);
            let moves = instruction.opcode == Opcode::Phi || source(function, to) == Some(Operand::Value(value));
            if let (true, Some(result)) = (moves, instruction.result)
                && !unfollowed_value[result.0 as usize]
            {
                unfollowed_value[result.0 as usize] = true;
                spread.push(result);
            }
        }
    }
    unfollowed_value
}

pub fn analysed(function: &Function) -> Escapes {
    // Origins are not allocation bounds. Absence here says nothing about
    // runtime frame walking, callbacks, or pointers loaded from memory.
    let mut origins: IndexMap<ValueId, BTreeSet<ValueId>> = IndexMap::default();
    let instructions: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
    let is = |inst: InstId, phi: bool| (function.instruction(inst).opcode == Opcode::Phi) == phi;
    let phis: Vec<InstId> = instructions.iter().copied().filter(|&inst| is(inst, true)).collect();
    let operations: Vec<InstId> = instructions.iter().copied().filter(|&inst| is(inst, false)).collect();
    let moving = |inst: InstId| {
        matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }) || source(function, inst).is_some()
    };

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

    let mut exposed: BTreeSet<ValueId> =
        operations.iter().filter(|&&inst| !moving(inst)).flat_map(|&inst| inputs(inst, &origins)).collect();
    // A join with an arm no fact follows exposes what its other arms hold.
    let unfollowed_value = unfollowed_values(function);
    for &phi in &phis {
        let instruction = function.instruction(phi);
        let joins = instruction
            .operands
            .chunks(2)
            .any(
                |arm| matches!(
                    arm[0],
                    Operand::Value(value) if unfollowed_value[value.0 as usize]
                ),
            );
        if joins {
            for arm in instruction.operands.chunks(2) {
                if let Operand::Value(value) = arm[0] {
                    exposed.extend(origins.get(&value).into_iter().flatten().copied());
                }
            }
        }
    }
    Escapes { origins, exposed }
}

thread_local! {
    static SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static WHOLE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has worked out a whole function's exposed
/// allocas, for a test that an edit does not.
pub fn whole_runs() -> usize {
    WHOLE.with(std::cell::Cell::get)
}

/// How many times this thread has asked `exposes` of one alloca, for a test
/// that a function's answers are found once.
pub fn reads() -> usize {
    READS.with(std::cell::Cell::get)
}

/// Counts the operands `ExposedFrames::update` read from the edits it was
/// given.
pub fn count_reads(n: usize) {
    READS.with(|reads| reads.set(reads.get() + n));
}

/// Whether `function` has an alloca, which every answer here is about.
pub fn has_alloca(function: &Function) -> bool {
    function.walk().any(|(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }))
}

pub fn scans() -> usize {
    SCANS.with(std::cell::Cell::get)
}

/// Every alloca of `function` whose address is exposed, as `exposes` says of
/// each, in one pass: a value is exposing where a use of it is neither a move,
/// an access through it, nor a marker, and an address that reaches an exposing
/// value through moves is exposed too.
pub fn exposed_allocas(
    function: &Function,
    marker: impl Fn(InstId, usize) -> bool,
) -> BTreeSet<ValueId> {
    // Nothing to answer for a function with no alloca; the scan below walks
    // every operand of every instruction.
    if !has_alloca(function) {
        return BTreeSet::new();
    }
    WHOLE.with(|runs| runs.set(runs.get() + 1));
    let mut exposing = vec![false; function.value_count()];
    // The values a value moves into, reversed: who feeds each.
    let mut feeds: Vec<Vec<ValueId>> = vec![Vec::new(); function.value_count()];
    let mut allocas = Vec::new();
    for (_, inst) in function.walk() {
        let instruction = function.instruction(inst);
        if let (Opcode::Alloca { .. }, Some(result)) = (&instruction.opcode, instruction.result) {
            allocas.push(result);
        }
        for (index, operand) in instruction.operands.iter().enumerate() {
            let Operand::Value(value) = operand else { continue };
            let moves = instruction.opcode == Opcode::Phi
                || source(function, inst) == Some(Operand::Value(*value)) && index == 0;
            if moves {
                if let Some(result) = instruction.result {
                    feeds[result.0 as usize].push(*value);
                }
            } else if !(accessed(function, inst) == Some(index) || marker(inst, index)) {
                exposing[value.0 as usize] = true;
            }
        }
    }
    // A join with an arm no fact follows exposes its other arms (see
    // `joins_unfollowed`).
    let unfollowed_value = unfollowed_values(function);
    for (_, inst) in function.walk() {
        let instruction = function.instruction(inst);
        if instruction.opcode != Opcode::Phi {
            continue;
        }
        let joins = instruction
            .operands
            .chunks(2)
            .any(
                |arm| matches!(
                    arm[0],
                    Operand::Value(value) if unfollowed_value[value.0 as usize]
                ),
            );
        if joins {
            for arm in instruction.operands.chunks(2) {
                if let Operand::Value(value) = arm[0] {
                    exposing[value.0 as usize] = true;
                }
            }
        }
    }
    let mut work: Vec<ValueId> = (0..exposing.len()).filter(|&at| exposing[at]).map(|at| ValueId(at as u32)).collect();
    while let Some(value) = work.pop() {
        for &feeder in &feeds[value.0 as usize] {
            if !exposing[feeder.0 as usize] {
                exposing[feeder.0 as usize] = true;
                work.push(feeder);
            }
        }
    }
    allocas.into_iter().filter(|one| exposing[one.0 as usize]).collect()
}

/// `exposed_allocas` of a function as it stands, from `previous`, what it was
/// before edits that touched instructions reading `touched`: an alloca's
/// answer changes only where an instruction that reads one of the values its
/// address moves through is edited, and walking back through moves and phis
/// from the values the edits read finds those allocas. Each is asked again;
/// the rest are as they were.
pub fn exposed_renewed(
    function: &Function,
    previous: impl IntoIterator<Item = ValueId>,
    touched: &[ValueId],
    marker: impl Fn(InstId, usize) -> bool,
) -> Vec<ValueId> {
    use llrm_mir::module::ValueDef;
    let mut affected = llrm_mir::dense::IdSet::<ValueId>::new();
    let mut seen = llrm_mir::dense::IdSet::<ValueId>::new();
    let mut pending = touched.to_vec();
    while let Some(value) = pending.pop() {
        if !seen.insert(value) {
            continue;
        }
        let ValueDef::Instruction(inst) = function.value(value).def else { continue };
        if function.is_erased(inst) {
            continue;
        }
        let instruction = function.instruction(inst);
        if matches!(instruction.opcode, Opcode::Alloca { .. }) {
            affected.insert(value);
        } else if instruction.opcode == Opcode::Phi {
            pending.extend(instruction.operands.iter().filter_map(|one| match one {
                Operand::Value(value) => Some(*value),
                _ => None,
            }));
        } else if let Some(Operand::Value(from)) = source(function, inst) {
            pending.push(from);
        }
    }
    let gone = |value: ValueId| match function.value(value).def {
        ValueDef::Instruction(inst) => function.is_erased(inst),
        ValueDef::Argument(_) => false,
    };
    let mut found: Vec<ValueId> = previous.into_iter().filter(|one| !affected.contains(one) && !gone(*one)).collect();
    found.extend(affected.iter().filter(|&one| exposes(function, one, &marker)));
    found
}

/// Whether `alloca`'s address is exposed, as `analysed` finds it: it
/// reaches, through moves and phis, an operand that is not an access's
/// own pointer. A lifetime marker (`marker` says which instructions are
/// one) names an object without handing out its address.
pub fn exposes(
    function: &Function,
    alloca: ValueId,
    marker: impl Fn(InstId, usize) -> bool,
) -> bool {
    SCANS.with(|scans| scans.set(scans.get() + 1));
    let mut seen = BTreeSet::from([alloca]);
    let mut pending = vec![alloca];
    while let Some(value) = pending.pop() {
        for one in function.users(value) {
            let instruction = function.instruction(one.user);
            let moves = instruction.opcode == Opcode::Phi
                || source(function, one.user) == Some(Operand::Value(value)) && one.index == 0;
            if !moves {
                if accessed(function, one.user) == Some(one.index as usize) || marker(one.user, one.index as usize) {
                    continue;
                }
                return true;
            }
            // A join with an arm no fact follows (`alias.rs`: it publishes
            // the other arms' objects).
            if instruction.opcode == Opcode::Phi && joins_unfollowed(function, one.user) {
                return true;
            }
            if let Some(result) = instruction.result
                && seen.insert(result)
            {
                pending.push(result);
            }
        }
    }
    false
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
    let phis: Vec<InstId> =
        instructions.iter().copied().filter(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect();
    let mut moving: IndexMap<ValueId, InstId> = IndexMap::default();
    // A parameter is defined by nothing here: the old incoming value no op
    // defines.
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
        function
            .instruction(phi)
            .operands
            .iter()
            .copied()
            .filter(|operand| !matches!(operand, Operand::Block(_)))
            .collect()
    };
    let result = |inst: InstId| function.instruction(inst).result.expect("a phi has a result");
    let mut state: IndexMap<ValueId, BTreeSet<ValueId>> = IndexMap::default();

    let side =
        |operand: Operand, state: &IndexMap<ValueId, BTreeSet<ValueId>>, refuted: &BTreeSet<ValueId>| match operand {
            Operand::Value(value) if refuted.contains(&value) => Side::Number,
            Operand::Value(value) if state.contains_key(&value) => Side::Address,
            Operand::Value(_) => Side::Unknown,
            _ => Side::Number,
        };
    // The allocas `inst` leaves its result in, None where it cannot, `...`
    // while unknown.
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
        // Optimism settles cycles; anything still unproven is refuted and the
        // rest looked at again.
        let unproven: BTreeSet<ValueId> = moving
            .keys()
            .copied()
            .chain(phis.iter().map(|&phi| result(phi)))
            .filter(|value| {
                !refuted.contains(value)
                    && (!state.contains_key(value)
                        || moving
                            .get(value)
                            .is_some_and(|&inst| !matches!(moved(inst, &state, &refuted), Moved::Extents(_)))
                        || (!moving.contains_key(value)
                            && phis.iter().filter(|&&phi| result(phi) == *value).any(|&phi| {
                                incoming(phi)
                                    .iter()
                                    .any(|one| !matches!(one, Operand::Value(one) if state.contains_key(one)))
                            })))
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
        let sinks = [
            "call void @sink(ptr %copied)",
            "store ptr %copied, ptr @g",
            "%n = ptrtoint ptr %copied to i16",
            "ret ptr %copied",
        ];
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

    /// `exposed_allocas` walked every operand of every instruction, asking
    /// `marker` of each, in a function with no alloca: an empty answer for
    /// O(function) per GVN run (`analysis exposed-frames` 0.44 G of chain512's
    /// 34.9 G).
    #[test]
    fn a_function_with_no_alloca_is_answered_without_a_scan() {
        let module = parsed(
            "declare void @sink(ptr)
define void @f(ptr %p, i16 %x) {
entry:
  call void @sink(ptr %p)
  %a = add i16 %x, 1
  store i16 %a, ptr %p
  ret void
}
",
        );
        let asked = std::cell::Cell::new(0);
        let found = exposed_allocas(function(&module, "f"), |_, _| {
            asked.set(asked.get() + 1);
            false
        });
        assert!(found.is_empty());
        assert_eq!(asked.get(), 0, "operands asked about with no alloca to answer for");
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

    /// The names of `@f`'s exposed allocas.
    fn exposed(text: &str) -> BTreeSet<String> {
        let module = parsed(text);
        let f = function(&module, "f");
        analysed(f).exposed.iter().map(|&one| f.value(one).name.clone().unwrap()).collect()
    }

    fn names(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|one| (*one).to_owned()).collect()
    }

    /// A slot's address joined with a pointer loaded from memory, then read
    /// through the join, was taken to be reached by no pointer a load could
    /// give: LICM promoted the slot's store out of its loop and the read saw
    /// the slot unwritten (`phi_slot_unknown_arm`, 2158 for 5736113).
    #[test]
    fn a_slot_joined_with_a_loaded_pointer_is_exposed() {
        let text = "@t = global ptr null

define i16 @f(i1 %c) {
b:
  %slot = alloca i16
  %other = alloca i16
  %q = load ptr, ptr @t
  br label %loop

loop:
  %p = phi ptr [ %slot, %b ], [ %q, %loop ]
  %v = load i16, ptr %p
  br i1 %c, label %loop, label %out

out:
  ret i16 %v
}
";
        assert_eq!(exposed(text), names(&["slot"]));
        let module = parsed(text);
        let f = function(&module, "f");
        let by_pass = exposed_allocas(f, |_, _| false);
        assert_eq!(by_pass, BTreeSet::from([value(f, "slot")]));
        assert!(exposes(f, value(f, "slot"), |_, _| false));
        assert!(!exposes(f, value(f, "other"), |_, _| false));
    }

    #[test]
    fn a_slot_only_loaded_and_stored_through_does_not_escape() {
        let found = exposed(
            "define i16 @f(i16 %x) {
b:
  %slot = alloca i16
  %high = getelementptr inbounds i8, ptr %slot, i16 1
  store i16 %x, ptr %slot
  store i8 0, ptr %high
  %y = load i16, ptr %slot
  ret i16 %y
}
",
        );
        assert_eq!(found, names(&[]));
    }

    #[test]
    fn storing_a_slots_address_exposes_it_but_storing_through_it_does_not() {
        let found = exposed(
            "define void @f() {
b:
  %kept = alloca ptr
  %leaked = alloca i16
  store ptr %leaked, ptr %kept
  ret void
}
",
        );
        assert_eq!(found, names(&["leaked"]));
    }

    #[test]
    fn only_the_slot_passed_to_a_call_escapes() {
        let found = exposed(
            "declare void @sink(ptr addrspace(1))

define void @f() {
b:
  %private = alloca i16
  %passed = alloca i16
  store i16 1, ptr %private
  %far = addrspacecast ptr %passed to ptr addrspace(1)
  call void @sink(ptr addrspace(1) %far)
  ret void
}
",
        );
        assert_eq!(found, names(&["passed"]));
    }

    #[test]
    fn a_select_or_compare_of_slots_exposes_every_slot_it_reads() {
        let found = exposed(
            "define i1 @f(i1 %c) {
b:
  %one = alloca i16
  %two = alloca i16
  %three = alloca i16
  %either = select i1 %c, ptr %one, ptr %two
  %same = icmp eq ptr %three, null
  ret i1 %same
}
",
        );
        assert_eq!(found, names(&["one", "two", "three"]));
    }

    #[test]
    fn a_phi_of_two_slots_is_framed_in_both_but_a_phi_with_a_parameter_is_not() {
        let module = parsed(
            "define void @f(i1 %c, ptr %p) {
top:
  %one = alloca i16
  %two = alloca i16
  br i1 %c, label %left, label %join

left:
  br label %join

join:
  %both = phi ptr [ %one, %top ], [ %two, %left ]
  %mixed = phi ptr [ %one, %top ], [ %p, %left ]
  store i16 0, ptr %both
  store i16 0, ptr %mixed
  ret void
}
",
        );
        let f = function(&module, "f");
        let found = framed(f);
        assert_eq!(found.get(&value(f, "both")), Some(&BTreeSet::from([value(f, "one"), value(f, "two")])));
        assert!(!found.contains_key(&value(f, "mixed")));
        // Accessing through a phi of slots exposes nothing; a phi that joins a
        // slot with a parameter holds a pointer no fact follows, which may be
        // the slot: the slot is exposed.
        assert_eq!(analysed(f).exposed, BTreeSet::from([value(f, "one")]));
    }

    #[test]
    fn a_pointer_loaded_from_memory_is_not_framed() {
        let module = parsed(
            "define void @f() {
b:
  %box = alloca ptr
  %slot = alloca i16
  store ptr %slot, ptr %box
  %back = load ptr, ptr %box
  store i16 1, ptr %back
  ret void
}
",
        );
        let f = function(&module, "f");
        assert!(!framed(f).contains_key(&value(f, "back")), "memory hides where it points");
        assert_eq!(analysed(f).exposed, BTreeSet::from([value(f, "slot")]));
    }

    #[test]
    fn a_body_without_slots_has_no_origins() {
        let module = parsed(
            "define ptr @f(ptr %p) {
b:
  %q = getelementptr inbounds i8, ptr %p, i16 1
  ret ptr %q
}
",
        );
        let f = function(&module, "f");
        assert_eq!(analysed(f), Escapes { origins: IndexMap::default(), exposed: BTreeSet::new() });
        assert!(framed(f).is_empty());
    }

    /// A scope's lifetime markers name a local; they hand its address to no
    /// one. As a call they exposed every local with a scope, and its bytes
    /// were never private.
    #[test]
    fn test_a_lifetime_marker_does_not_expose_the_local() {
        let module = parsed(
            "declare void @llvm.lifetime.start.p0(i64, ptr)
define i16 @f(i16 %a) {
b0:
  %x = alloca i16
  call void @llvm.lifetime.start.p0(i64 2, ptr %x)
  store i16 %a, ptr %x
  %v = load i16, ptr %x
  ret i16 %v
}
",
        );
        let f = function(&module, "f");
        let x = value(f, "x");
        let marker = |inst: InstId, _: usize| matches!(&f.instruction(inst).opcode, Opcode::Call(_));
        assert!(exposes(f, x, |_, _| false));
        assert!(!exposes(f, x, marker));
    }
}
