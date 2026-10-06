//! What `peephole.peep` builds rewrites from: lane sets, operands, and the
//! metadata a joined instruction carries.

use crate::backend::lanes::Lanes;
use crate::backend::peephole::{_lanes, _register_operand};
use crate::backend::target;
use crate::model::ir::{Imm, Loc, Mem, Reg};
use crate::model::lir::Insn;
use crate::support::hash::{IndexMap, IndexSet};

pub fn lanes(one: Reg) -> Lanes {
    _lanes(one.register)
}

/// The lanes above a register's low word.
pub fn upper(one: Reg) -> Lanes {
    _lanes(one.register).into_iter().filter(|lane| lane.1 >= 2).collect()
}

/// The lanes of a register's root above its low word.
pub fn root_upper(one: Reg) -> Lanes {
    _lanes(crate::model::ir::root(one.register)).into_iter().filter(|lane| lane.1 >= 2).collect()
}

/// The whole register a view belongs to, as a dword operand.
pub fn widen(one: Reg) -> Loc {
    Loc::Reg(Reg { register: crate::model::ir::root(one.register), width: 4 })
}

/// Two word immediates, high then low, as the dword they push.
pub fn dword(high: &Imm, low: &Imm) -> Loc {
    Loc::Imm(Imm { value: ((high.value & 0xFFFF) << 16) | (low.value & 0xFFFF), width: 4, address: None })
}

/// A zero as wide as the cell.
pub fn zero(cell: &Mem) -> Loc {
    Loc::Imm(Imm { value: 0, width: cell.width, address: None })
}

/// The dword a far pointer's low word starts.
pub fn dword_cell(low: &Mem) -> Loc {
    Loc::Mem(Mem { width: 4, ..low.clone() })
}

/// A register or immediate's low word.
pub trait LowWord {
    fn low_word(self) -> Loc;
}

impl LowWord for Reg {
    fn low_word(self) -> Loc {
        Loc::Reg(Reg { register: target::named(self.register, 2), width: 2 })
    }
}

impl LowWord for &Imm {
    fn low_word(self) -> Loc {
        Loc::Imm(Imm { value: self.value & 0xFFFF, width: 2, address: None })
    }
}

/// An immediate's high word.
pub fn high_word(one: &Imm) -> Loc {
    Loc::Imm(Imm { value: (one.value >> 16) & 0xFFFF, width: 2, address: None })
}

pub fn low_word(one: impl LowWord) -> Loc {
    one.low_word()
}

/// An operand with register `from` renamed to `to`.
pub fn renamed(one: &Loc, from: Reg, to: Reg) -> Loc {
    _register_operand(one, from.register, to.register)
}

/// The point `one` stands at.
pub fn point(one: &Insn) -> Option<(i64, i64)> {
    Some((one.at, one.at))
}

fn deduped(items: impl IntoIterator<Item = u32>) -> Vec<u32> {
    items.into_iter().collect::<IndexSet<u32>>().into_iter().collect()
}

pub fn joined_defines(first: &Insn, second: &Insn) -> Vec<u32> {
    deduped(first.defines.iter().chain(&second.defines).copied())
}

pub fn joined_uses_all(first: &Insn, second: &Insn) -> Vec<u32> {
    deduped(first.uses.iter().chain(&second.uses).copied())
}

/// What `first` reads, then what `second` reads that `first` does not define.
pub fn joined_uses(first: &Insn, second: &Insn) -> Vec<u32> {
    deduped(first.uses.iter().copied().chain(second.uses.iter().copied().filter(|value| !first.defines.contains(value))))
}

/// Both instructions' value widths, `second`'s where they name one value.
pub fn joined_widths(first: &Insn, second: &Insn) -> Vec<(u32, u32)> {
    let mut widths: IndexMap<u32, u32> = IndexMap::default();
    for (value, width) in first.widths.iter().chain(&second.widths) {
        widths.insert(*value, *width);
    }
    widths.into_iter().collect()
}
