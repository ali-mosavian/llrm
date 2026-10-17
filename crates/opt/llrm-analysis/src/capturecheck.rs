//! `LLRM_CHECK_CAPTURES`: the frame objects the pointer facts say never escape,
//! checked against LLVM's capture tracking read straight off the uses, with no
//! fact followed: a pointer is captured when any use of it, or of what is made
//! of it by a gep, a cast, a phi or a select (every arm), stores it, returns
//! it, turns it into an integer or passes it where the callee keeps no copy is
//! not promised. A fact that misses one of these is an under-escape: a store to
//! the object may be dropped while a later load can still read it.

use llrm_mir::dense::IdSet;
use llrm_mir::module::{InstId, ValueId};
use llrm_mir::opcode::CastOp;
use llrm_mir::opcode::Opcode;

use crate::alias::PointsTo;
use crate::memory::{MemoryKind, Unit, is_lifetime_marker};

/// The alloca an address is made of by geps and casts.
fn frame_base(
    unit: &Unit,
    address: llrm_mir::module::Operand,
) -> Option<ValueId> {
    let function = unit.function;
    let mut at = address;
    loop {
        let llrm_mir::module::Operand::Value(value) = at else { return None };
        let llrm_mir::module::ValueDef::Instruction(inst) = function.value(value).def else { return None };
        let op = function.instruction(inst);
        match &op.opcode {
            Opcode::Alloca { .. } => return Some(value),
            Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => at = op.operands[0],
            _ => return None,
        }
    }
}

/// Whether a join's arm is a pointer nothing identifies: read from memory,
/// passed in, returned, or made from an integer.
fn unknown_arm(
    unit: &Unit,
    arm: llrm_mir::module::Operand,
) -> bool {
    let function = unit.function;
    let mut at = arm;
    loop {
        let llrm_mir::module::Operand::Value(value) = at else { return false };
        let llrm_mir::module::ValueDef::Instruction(inst) = function.value(value).def else { return true };
        let op = function.instruction(inst);
        match &op.opcode {
            Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => at = op.operands[0],
            Opcode::Load { .. } | Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Cast(CastOp::IntToPtr) => return true,
            _ => return false,
        }
    }
}

/// The first instruction that captures the address `root`, if any.
fn captured_by(
    unit: &Unit,
    root: ValueId,
    held_in: &mut Vec<ValueId>,
) -> Option<InstId> {
    let function = unit.function;
    let mut seen: IdSet<ValueId> = IdSet::new();
    let mut work = vec![root];
    while let Some(value) = work.pop() {
        if !seen.insert(value) {
            continue;
        }
        for used in function.users(value) {
            let op = function.instruction(used.user);
            let at = used.index as usize;
            match &op.opcode {
                Opcode::GetElementPtr { .. } if at == 0 => work.extend(op.result),
                Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => work.extend(op.result),
                // A join with an arm no object is followed through is itself
                // none: the objects of its other arms are lost (a load through
                // it reads them).
                Opcode::Phi | Opcode::Select if at != 0 || matches!(op.opcode, Opcode::Phi) => {
                    if op.operands.iter().enumerate().any(|(index, one)| {
                        (matches!(op.opcode, Opcode::Phi) || index != 0) && index != at && unknown_arm(unit, *one)
                    }) {
                        return Some(used.user);
                    }
                    work.extend(op.result);
                }
                Opcode::Load { .. } | Opcode::ICmp(_) => {}
                Opcode::Store { .. } if at == 1 => {}
                // Kept in another frame object: captured if that one is.
                Opcode::Store { .. } if frame_base(unit, op.operands[1]).is_some() => {
                    held_in.extend(frame_base(unit, op.operands[1]));
                }
                Opcode::Call(_) | Opcode::Invoke(_) => {
                    let args = match op.opcode {
                        Opcode::Call(_) => op.operands.len() - 1,
                        _ => op.operands.len() - 3,
                    };
                    // A unit holds the other bodies as declarations and the
                    // facts follow the summaries of those
                    // the module defines: only a call through a
                    // pointer is read here.
                    let defined = llrm_mir::memory::callee(unit.context, function, used.user).is_some();
                    if at < args
                        && !defined
                        && !is_lifetime_marker(unit, used.user)
                        && !crate::alias::borrowed(unit, used.user, at)
                    {
                        return Some(used.user);
                    }
                }
                _ => return Some(used.user),
            }
        }
    }
    None
}

/// Panics on a frame object some use captures that `facts.escaped` lacks.
pub fn check(
    unit: &Unit,
    facts: &PointsTo,
) {
    let function = unit.function;
    let mut direct: Vec<(ValueId, Option<InstId>, Vec<ValueId>)> = Vec::new();
    for (_, inst) in function.walk() {
        let op = function.instruction(inst);
        let (Opcode::Alloca { .. }, Some(value)) = (&op.opcode, op.result) else { continue };
        let mut held_in = Vec::new();
        let by = captured_by(unit, value, &mut held_in);
        direct.push((value, by, held_in));
    }
    // Held in a captured object is captured, to a fixed point.
    let mut by: Vec<Option<InstId>> = direct.iter().map(|one| one.1).collect();
    loop {
        let mut changed = false;
        for (at, one) in direct.iter().enumerate() {
            if by[at].is_some() {
                continue;
            }
            if let Some(found) = one
                .2
                .iter()
                .find_map(|holder| direct.iter().position(|other| other.0 == *holder).and_then(|index| by[index]))
            {
                by[at] = Some(found);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut missed = Vec::new();
    for (at, (value, ..)) in direct.iter().enumerate() {
        let Some(by) = by[at] else { continue };
        let said = facts
            .escaped
            .iter()
            .any(
                |one| one.kind == MemoryKind::Frame
                    && matches!(
                        one.key,
                        crate::memory::Key::Value(number) if number == value.0
                    ),
            );
        if !said {
            missed.push((
                *value,
                by,
                format!(
                    "{:?} {:?}",
                    function.instruction(by).opcode,
                    llrm_mir::memory::callee(unit.context, function, by)
                        .and_then(|one| unit.globals.get(one.0 as usize).and_then(|global| global.name.clone()))
                ),
            ));
        }
    }
    assert!(
        missed.is_empty(),
        "LLRM_CHECK_CAPTURES: frame objects captured by a use that the facts say never escape: {:?}",
        missed
    );
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::alias::points_to;
    use crate::memory::Unit;
    use crate::testing::{DOS, function, layout, parsed};

    const KEEPS: &str =
        "define void @f(ptr %keep) {\nentry:\n  %s = alloca i32\n  call void %keep(ptr %s)\n  ret void\n}\n";

    /// A frame object handed to a function called through a pointer escapes.
    /// The check passes on the facts the solve gives, and stops on facts
    /// that forgot it: the under-escape that dropped a store (a join with
    /// an unknown arm).
    #[test]
    fn a_frame_object_passed_to_a_pointer_callee_must_be_in_the_escaped_set() {
        let module = parsed(&format!("{DOS}{KEEPS}"));
        let layout = layout(&module);
        let unit = crate::testing::with_registers(Unit::of(&module, &layout, function(&module, "f")));
        let mut facts = points_to(&unit, None, None).unwrap();
        check(&unit, &facts);
        facts.escaped.clear();
        let forgotten = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&unit, &facts)));
        assert!(forgotten.is_err(), "facts that forgot the escape were accepted");
    }

    /// `phi [%slot, %b0], [%q, %b1]` with `%q` read from memory is a pointer no
    /// object is followed through: the slot behind it is lost, and the store
    /// to it was dropped (cgscc-poc's miscompile on main). The check names it.
    #[test]
    fn a_frame_object_behind_a_join_with_an_unknown_arm_must_be_in_the_escaped_set() {
        let module = parsed(&format!(
            "{DOS}define i32 @f(i1 %c, ptr %cell) {{\nentry:\n  %s = alloca i32\n  %q = load ptr, ptr %cell\n  br i1 %c, label %a, label %b\na:\n  br label %j\nb:\n  br label %j\nj:\n  %p = phi ptr [ %s, %a ], [ %q, %b ]\n  %v = load i32, ptr %p\n  ret i32 %v\n}}\n"
        ));
        let layout = layout(&module);
        let unit = crate::testing::with_registers(Unit::of(&module, &layout, function(&module, "f")));
        let mut facts = points_to(&unit, None, None).unwrap();
        facts.escaped.clear();
        let forgotten = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&unit, &facts)));
        assert!(forgotten.is_err(), "facts that lost the slot behind the join were accepted");
    }
}
