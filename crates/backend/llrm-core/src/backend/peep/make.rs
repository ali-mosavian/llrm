//! What `peephole.peep` builds rewrites from: lane sets, operands, and the
//! metadata a joined instruction carries.
//!
//! # How it works
//! A rule's `rewrite` calls these makers. They come in three kinds.
//!
//! ```text
//!   lanes      lanes, upper, root_upper     lane sets of a register
//!   operands   widen, low_byte, dword,      the operand (or source point)
//!              zero, dword_cell, renamed,   a fused or narrowed
//!              high_word, low_word, point   instruction takes
//!   metadata   joined_defines,              the defined values, used values
//!              joined_uses(_all),           and value widths of an
//!              joined_widths                instruction made of two
//! ```
//!
//! `joined_uses` is what the first instruction reads, then what the second
//! reads that the first does not define; `joined_widths` takes the second's
//! width where both name one value. Together they keep the value numbering
//! of the fused instruction consistent for later phases.
//!
//! # Example
//! None: the makers are helpers that build pieces of a rewrite, and only a
//! whole rule gives LIR to show. See the `# Example` of [`super`] and of
//! `crate::backend::peephole`.
//! # Used by
//! The code generated from `peephole.peep`, through [`super::walk`].

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

/// The lanes of a register's root that the register itself does not cover.
pub fn rest_of_root(
    regs: Regs,
    one: Reg,
) -> Lanes {
    let own = _lanes(regs, one.register);
    _lanes(regs, crate::model::ir::root(one.register)).into_iter().filter(|lane| !own.contains(lane)).collect()
}

/// The dword register a byte or word view belongs to (`eax` for `al` and `ax`).
pub fn dword_of(
    regs: Regs,
    one: Reg,
) -> Loc {
    Loc::Reg(Reg { register: regs.named(regs.root(one.register), 4), width: 4 })
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
