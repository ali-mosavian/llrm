//! The guards `peephole.peep` names. Each answers one question about the
//! matched instructions, their operands or the block around them.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;

use super::walk::Cx;
use super::{Set, field};
use crate::backend::lanes::Lanes;
use crate::backend::peephole::{_lanes, _register_effects};
use crate::backend::{select, target};
use crate::model::ir::{self, Held, Imm, Loc, Mem, Operation, Reg};
use crate::model::lir::Insn;

/// Anything with a width in bytes; zero for an operand that has none.
pub trait Wide {
    fn bytes(&self) -> u32;
}

impl Wide for Reg {
    fn bytes(&self) -> u32 {
        self.width
    }
}

impl Wide for Held {
    fn bytes(&self) -> u32 {
        self.width
    }
}

impl Wide for &Mem {
    fn bytes(&self) -> u32 {
        self.width
    }
}

impl Wide for &Imm {
    fn bytes(&self) -> u32 {
        self.width
    }
}

impl Wide for &Loc {
    fn bytes(&self) -> u32 {
        match self {
            Loc::Reg(one) => one.width,
            Loc::Mem(one) => one.width,
            Loc::Imm(one) => one.width,
            Loc::Held(one) => one.width,
            Loc::Address(_) | Loc::St(_) => 0,
        }
    }
}

/// `one` carries none of the metadata `fields` names (`peep::field`).
pub fn free(_: &Cx, one: &Insn, fields: u32) -> bool {
    let has = |bit: u32| fields & bit != 0;
    !(has(field::CLOBBERS) && !one.clobbers.is_empty()
        || has(field::CLOBBERS_HIGH) && !one.clobbers_high.is_empty()
        || has(field::REQUIRES) && !one.requires.is_empty()
        || has(field::DELIVERS) && !one.delivers.is_empty()
        || has(field::SPREAD) && !one.spread.is_empty()
        || has(field::GROUP) && one.group.is_some()
        || has(field::SYMBOL) && one.symbol == Some(true)
        || has(field::FRAME_ADJUST) && one.frame_adjust
        || has(field::SPILL_RELOAD) && one.spill_reload
        || has(field::SPILL_STORE) && one.spill_store
        || has(field::DEFINES) && !one.defines.is_empty()
        || has(field::USES) && !one.uses.is_empty()
        || has(field::POINT) && one.covers.is_none_or(|(start, end)| start != end)
        || has(field::UNOWNED) && one.covers.is_some_and(|(start, end)| start != end))
}

/// Every lane in `lanes` is dead after `one`.
pub fn dead(cx: &Cx, one: &Arc<Insn>, lanes: Lanes) -> bool {
    lanes.is_subset(&cx.dead_after(one))
}

/// Nothing after the block may read these flags.
pub fn flags_dead_out(cx: &Cx, lanes: Lanes) -> bool {
    cx.flags_out().is_disjoint(&lanes)
}

pub fn empty(_: &Cx, lanes: Lanes) -> bool {
    lanes.is_empty()
}

pub fn disjoint(_: &Cx, one: Lanes, other: Lanes) -> bool {
    one.is_disjoint(&other)
}

fn emitted(one: &Insn) -> Option<select::Emitted> {
    select::emit(one.what.as_ref()?, 0, None, false, false, None)
}

pub fn encodable(_: &Cx, one: &Arc<Insn>) -> bool {
    emitted(one).is_some()
}

/// Both encode, `after` in no more bytes than `before`.
pub fn no_longer(_: &Cx, before: &Arc<Insn>, after: &Arc<Insn>) -> bool {
    matches!((emitted(before), emitted(after)), (Some(before), Some(after)) if after.code.len() <= before.code.len())
}

/// Both encode, in as many bytes.
pub fn same_length(_: &Cx, before: &Arc<Insn>, after: &Arc<Insn>) -> bool {
    matches!((emitted(before), emitted(after)), (Some(before), Some(after)) if after.code.len() == before.code.len())
}

/// The target prices `add r,r` below `shl r,1`.
pub fn doubles_by_add(cx: &Cx) -> bool {
    cx.cpu().doubling().is_ok_and(|form| form == "alu_rr")
}

pub fn same_width(_: &Cx, one: impl Wide, other: impl Wide) -> bool {
    one.bytes() == other.bytes()
}

pub fn narrower(_: &Cx, one: impl Wide, other: impl Wide) -> bool {
    one.bytes() < other.bytes()
}

/// The width, in bits, is one of these.
pub fn width_in(_: &Cx, one: impl Wide, bits: &[u32]) -> bool {
    bits.contains(&(one.bytes() * 8))
}

pub fn same_root(_: &Cx, one: Reg, other: Reg) -> bool {
    ir::root(one.register) == ir::root(other.register)
}

/// A general register at its own width.
pub fn register_width(_: &Cx, one: Reg) -> bool {
    target::WIDTHS.get(&one.register) == Some(&i64::from(one.width))
}

/// A general register.
pub fn register_named(_: &Cx, one: Reg) -> bool {
    target::WIDTHS.contains_key(&one.register)
}

pub fn stack_or_frame(_: &Cx, one: Reg) -> bool {
    [Register::ESP, Register::EBP].contains(&one.register.full_register32())
}

pub fn segment(_: &Cx, one: Reg) -> bool {
    target::SEGMENTS.contains(&one.register)
}

/// `one` is the fixed register of a member of `set`.
pub fn fixed_by(_: &Cx, set: &Set, one: Reg) -> bool {
    set.by_fixed(one.register).is_some()
}

/// Its `covers` is the point it stands at.
pub fn placed(_: &Cx, one: &Insn) -> bool {
    one.covers == Some((one.at, one.at))
}

/// `one`'s bytes end where `next` stands.
pub fn ends_at(_: &Cx, one: &Insn, next: &Insn) -> bool {
    one.covers.is_some_and(|(_, end)| end == next.at)
}

/// `second` reads `first`'s one value, which nothing else reads.
pub fn feeds_once(cx: &Cx, first: &Insn, second: &Insn) -> bool {
    first.defines.len() == 1
        && second.uses == first.defines
        && cx.facts.users().get(&first.defines[0]).copied().unwrap_or(0) == 1
}

/// `one` neither reads nor redefines what `first` defines.
pub fn independent(_: &Cx, first: &Insn, one: &Arc<Insn>) -> bool {
    first.defines.is_empty() || !first.defines.iter().any(|value| one.uses.contains(value) || one.defines.contains(value))
}

/// Writing `made`'s wider destination where `first` stood crosses no
/// instruction in `crossed` that observes or replaces a lane it newly
/// writes. Source frontends batch their entry loads, so independent
/// parameter loads commonly stand between a load and its extension.
pub fn hoistable(_: &Cx, made: &Arc<Insn>, first: &Insn, crossed: &[Arc<Insn>]) -> bool {
    if crossed.is_empty() {
        // Nothing to cross. Asking anyway refused every unrolled clone,
        // whose effects `_register_effects` will not read.
        return true;
    }
    let (Some(original), Some(combined)) = (_register_effects(first, false, true), _register_effects(made, false, true)) else {
        return false;
    };
    let newly_written: Lanes = combined.1.minus(&original.1);
    crossed.iter().all(|one| match _register_effects(one, false, true) {
        None => false,
        Some((reads, writes)) => !newly_written.iter().any(|lane| reads.contains(lane) || writes.contains(lane)),
    })
}

/// `one` writes neither register and does not read the temporary `t`.
pub fn clear_of(_: &Cx, t: Reg, s: Reg, one: &Arc<Insn>) -> bool {
    let (temporary, source) = (_lanes(t.register), _lanes(s.register));
    _register_effects(one, true, false)
        .is_some_and(|(reads, writes)| reads.is_disjoint(&temporary) && writes.is_disjoint(&temporary.or(&source)))
}

/// `load` may read its cell after `crossed`, which only materializes a
/// register: it neither consumes nor replaces the loaded register nor
/// changes anything the cell is addressed by. Instructions that write
/// memory stay outside: proving them disjoint belongs in MIR.
pub fn delays(_: &Cx, load: &Insn, crossed: &Arc<Insn>) -> bool {
    let Some(crossed_what) = &crossed.what else {
        return false;
    };
    if !crossed.clobbers.is_empty()
        || !crossed.requires.is_empty()
        || !crossed.delivers.is_empty()
        || !crossed.spread.is_empty()
        || crossed.group.is_some()
        || crossed.symbol == Some(true)
        || crossed.frame_adjust
    {
        return false;
    }
    if !(matches!(crossed_what.op, Operation::Move | Operation::Extend | Operation::Address)
        && matches!(crossed_what.dests.as_slice(), [Loc::Reg(_)]))
    {
        return false;
    }
    let (Some((load_reads, load_writes)), Some((crossed_reads, crossed_writes))) =
        (_register_effects(load, false, true), _register_effects(crossed, false, true))
    else {
        return false;
    };
    let address_lanes: Lanes = match load.what.as_ref().map(|what| what.sources.as_slice()) {
        Some([Loc::Mem(cell)]) => {
            let mut address_registers = BTreeSet::from([cell.through, cell.index_through]);
            if let Some(addr) = cell.addr {
                address_registers.insert(addr.segment);
                // Once MIR computed a base value, allocation's `through` is
                // the encoded register and BC's original `addr.base` is only
                // provenance. A cell with no value still encodes that base.
                if cell.base.is_none() {
                    address_registers.insert(addr.base);
                }
            }
            address_registers.into_iter().flat_map(_lanes).collect()
        }
        _ => return false,
    };
    let crossed_all: Lanes = crossed_reads.or(&crossed_writes);
    let load_all: Lanes = load_reads.or(&address_lanes);
    load_writes.is_disjoint(&crossed_all)
        && load_all.is_disjoint(&crossed_writes)
        && !load.defines.iter().any(|value| crossed.uses.contains(value))
}

/// An immediate without an address, or a register of another root than `r`.
pub fn operand(_: &Cx, one: &Loc, r: Reg) -> bool {
    matches!(one, Loc::Imm(value) if value.address.is_none())
        || matches!(one, Loc::Reg(value) if ir::root(value.register) != ir::root(r.register))
}

/// `r` is how the cell is reached.
pub fn addresses(_: &Cx, cell: &Mem, r: Reg) -> bool {
    [ir::root(cell.through), ir::root(cell.index_through)].contains(&ir::root(r.register))
}

/// The cell is reached through segment register `g`.
pub fn segment_of(_: &Cx, cell: &Mem, g: Reg) -> bool {
    cell.addr.is_some_and(|addr| addr.segment == g.register)
}

/// Two operands address the same bytes. The registers and displacement
/// carry the address; the values they name may differ when allocation
/// copied the same address into a register again as a new value. `Mem`'s
/// equality compares those values and leaves the registers out, so it
/// cannot say.
pub fn same_cell(_: &Cx, one: &Mem, other: &Mem) -> bool {
    let logical = |cell: &Mem| Mem { base: None, index: None, ..cell.clone() };
    let physical = |cell: &Mem| (cell.through, cell.index_through, cell.offset);
    let placed = |cell: &Mem| {
        (cell.base.is_none() || cell.through != Register::None) && (cell.index.is_none() || cell.index_through != Register::None)
    };
    logical(one) == logical(other) && physical(one) == physical(other) && placed(one) && placed(other)
}

/// `high` is the word right after `low`, reached the same way. The
/// displacement may be carried by the address, by the operand's offset,
/// or by both at once, so either may be the one two further on.
pub fn next_word(_: &Cx, low: &Mem, high: &Mem) -> bool {
    let same = |cell: &Mem| Mem { addr: cell.addr.map(|addr| ir::Addr { disp: 0, ..addr }), offset: 0, ..cell.clone() };
    if same(low) != same(high) {
        return false;
    }
    let (Some(low_addr), Some(high_addr)) = (low.addr, high.addr) else {
        return low.addr.is_none() && high.addr.is_none() && high.offset == low.offset + 2;
    };
    let moved = high_addr.disp - low_addr.disp;
    moved == 2 && [0, 2].contains(&(high.offset - low.offset)) || moved == 0 && high.offset == low.offset + 2
}

/// A plain move, which changes no flag.
pub fn moves(_: &Cx, one: &Arc<Insn>) -> bool {
    one.what.as_ref().is_some_and(|what| what.op == Operation::Move && what.name.as_deref() == Some("mov"))
        && one.clobbers.is_empty()
}
