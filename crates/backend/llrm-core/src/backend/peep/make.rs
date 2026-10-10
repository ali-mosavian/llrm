//! What `peephole.peep` builds rewrites from: lane sets, operands, and the
//! metadata a joined instruction carries.

use llrm_lir::registers::Regs;

use crate::backend::lanes::Lanes;
use crate::backend::peephole::{_lanes, _register_operand};
use crate::model::ir::{Imm, Loc, Mem, Reg};
use crate::model::lir::Insn;
use crate::support::hash::{IndexMap, IndexSet};

pub fn lanes(
    regs: Regs,
    one: Reg,
) -> Lanes {
    _lanes(regs, one.register)
}

/// The lanes above a register's low word.
pub fn upper(
    regs: Regs,
    one: Reg,
) -> Lanes {
    _lanes(regs, one.register).into_iter().filter(|lane| lane.1 >= 2).collect()
}

/// The lanes of a register's root above its low word.
pub fn root_upper(
    regs: Regs,
    one: Reg,
) -> Lanes {
    _lanes(regs, crate::model::ir::root(one.register)).into_iter().filter(|lane| lane.1 >= 2).collect()
}

/// The whole register a view belongs to, as a dword operand.
pub fn widen(
    _regs: Regs,
    one: Reg,
) -> Loc {
    Loc::Reg(Reg { register: crate::model::ir::root(one.register), width: 4 })
}

/// A register's low byte.
pub fn low_byte(
    regs: Regs,
    one: Reg,
) -> Loc {
    Loc::Reg(Reg { register: regs.named(one.register, 1), width: 1 })
}

/// Two word immediates, high then low, as the dword they push.
pub fn dword(
    _regs: Regs,
    high: &Imm,
    low: &Imm,
) -> Loc {
    Loc::Imm(Imm { value: ((high.value & 0xFFFF) << 16) | (low.value & 0xFFFF), width: 4, address: None })
}

/// A zero as wide as the cell.
pub fn zero(
    _regs: Regs,
    cell: &Mem,
) -> Loc {
    Loc::Imm(Imm { value: 0, width: cell.width, address: None })
}

/// The dword a far pointer's low word starts.
pub fn dword_cell(
    _regs: Regs,
    low: &Mem,
) -> Loc {
    Loc::Mem(Mem { width: 4, ..low.clone() })
}

/// A register or immediate's low word.
pub trait LowWord {
    fn low_word(
        self,
        regs: Regs,
    ) -> Loc;
}

impl LowWord for Reg {
    fn low_word(
        self,
        regs: Regs,
    ) -> Loc {
        Loc::Reg(Reg { register: regs.named(self.register, 2), width: 2 })
    }
}

impl LowWord for &Imm {
    fn low_word(
        self,
        _regs: Regs,
    ) -> Loc {
        Loc::Imm(Imm { value: self.value & 0xFFFF, width: 2, address: None })
    }
}

/// An immediate's high word.
pub fn high_word(
    _regs: Regs,
    one: &Imm,
) -> Loc {
    Loc::Imm(Imm { value: (one.value >> 16) & 0xFFFF, width: 2, address: None })
}

pub fn low_word(
    regs: Regs,
    one: impl LowWord,
) -> Loc {
    one.low_word(regs)
}

/// An operand with register `from` renamed to `to`.
pub fn renamed(
    regs: Regs,
    one: &Loc,
    from: Reg,
    to: Reg,
) -> Loc {
    _register_operand(regs, one, from.register, to.register)
}

/// The point `one` stands at.
pub fn point(
    _regs: Regs,
    one: &Insn,
) -> Option<(i64, i64)> {
    Some((one.at, one.at))
}

fn deduped(items: impl IntoIterator<Item = u32>) -> Vec<u32> {
    items.into_iter().collect::<IndexSet<u32>>().into_iter().collect()
}

pub fn joined_defines(
    _regs: Regs,
    first: &Insn,
    second: &Insn,
) -> Vec<u32> {
    deduped(first.defines.iter().chain(&second.defines).copied())
}

pub fn joined_uses_all(
    _regs: Regs,
    first: &Insn,
    second: &Insn,
) -> Vec<u32> {
    deduped(first.uses.iter().chain(&second.uses).copied())
}

/// What `first` reads, then what `second` reads that `first` does not define.
pub fn joined_uses(
    _regs: Regs,
    first: &Insn,
    second: &Insn,
) -> Vec<u32> {
    deduped(
        first.uses.iter().copied().chain(second.uses.iter().copied().filter(|value| !first.defines.contains(value))),
    )
}

/// Both instructions' value widths, `second`'s where they name one value.
pub fn joined_widths(
    _regs: Regs,
    first: &Insn,
    second: &Insn,
) -> Vec<(u32, u32)> {
    let mut widths: IndexMap<u32, u32> = IndexMap::default();
    for (value, width) in first.widths.iter().chain(&second.widths) {
        widths.insert(*value, *width);
    }
    widths.into_iter().collect()
}
