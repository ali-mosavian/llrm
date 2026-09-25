//! Allocated instructions that keep a register an affine sum of registers,
//! and the 32-bit addresses that name such a sum.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::backend::target;
use crate::model::ir::{Address, Imm, Loc, Operation, Reg};
use crate::model::lir::Insn;
use crate::backend::cpu::Profile;

/// What one instruction makes of the register it writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// `mov z,y`.
    Copy(Reg),
    /// `add`/`sub` of a constant, `inc`, `dec`.
    Add(i64),
    /// `shl` by a constant, `add z,z`.
    Scale(i64),
    /// `add z,y`.
    AddRegister(Reg),
}

/// One register's multiple, as an affine chain computes it.
pub type Terms = Vec<(Register, i64)>;

/// Whether `one` has no effect beyond its operands.
pub fn plain(one: &Insn) -> bool {
    one.what.is_some()
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.spread.is_empty()
        && one.group.is_none()
        && !one.frame_adjust
}

/// The register `one` writes, how, and what `cpu` charges for it, where the
/// register stays an affine sum.
pub fn step(one: &Insn, cpu: &Profile) -> Option<(Reg, Step, i64)> {
    if !plain(one) {
        return None;
    }
    let what = one.what.as_ref()?;
    let [Loc::Reg(dest)] = what.dests.as_slice() else {
        return None;
    };
    let register = |one: &Reg| one.width == dest.width && target::WIDTHS.contains_key(&one.register);
    if ![2, 4].contains(&dest.width) || !register(dest) {
        return None;
    }
    let costs = &cpu.operations;
    let (step, cost) = match (what.op, what.name.as_deref(), what.sources.as_slice()) {
        (Operation::Move, Some("mov"), [Loc::Reg(source)]) if register(source) && source.register != dest.register => {
            (Step::Copy(*source), costs.r#move)
        }
        (_, _, [first, ..]) if *first != Loc::Reg(*dest) => return None,
        (Operation::Binary, Some(name @ ("add" | "sub")), [_, Loc::Imm(Imm { value, address: None, .. })]) => {
            (Step::Add(if name == "add" { *value } else { -value }), costs.add)
        }
        (Operation::Unary, Some(name @ ("inc" | "dec")), [_]) => (Step::Add(if name == "inc" { 1 } else { -1 }), costs.add),
        (Operation::Binary, Some("shl" | "sal"), [_, Loc::Imm(Imm { value: count @ 1..=31, address: None, .. })]) => {
            (Step::Scale(1 << count), costs.shift)
        }
        (Operation::Binary, Some("add"), [_, Loc::Reg(other)]) if other == dest => (Step::Scale(2), costs.add),
        (Operation::Binary, Some("add"), [_, Loc::Reg(other)]) if register(other) => (Step::AddRegister(*other), costs.add),
        _ => return None,
    };
    Some((*dest, step, cost))
}

/// The 67h address naming `terms` plus `disp`, if one does.
/// `scales` are the index scales the target's 32-bit address form takes.
pub fn form(terms: &[(Register, i64)], disp: i64, scales: &BTreeSet<i64>) -> Option<Address> {
    let at = |through: Register, index: Register, scale: i64| {
        (index != Register::ESP && scales.contains(&scale))
            .then_some(Address { through, index, scale, offset: disp, ..Address::new(None) })
    };
    match *terms {
        [(only, 1)] => Some(Address { through: only, offset: disp, ..Address::new(None) }),
        [(only, scale)] => at(only, only, scale - 1).or_else(|| at(Register::None, only, scale)),
        [(base, 1), (index, scale)] | [(index, scale), (base, 1)] => {
            at(base, index, scale).or_else(|| at(index, base, 1).filter(|_| scale == 1))
        }
        _ => None,
    }
}
