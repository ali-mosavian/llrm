//! The guards `peephole.peep` names in `when`: each answers one question about
//! the matched instructions, their operands or the block around them.
//!
//! # How it works
//! A guard takes the walk's [`Cx`] first where it needs the block, then the
//! operands or instructions the rule binds, and returns a bool. They fall into
//! kinds.
//!
//! ```text
//!   metadata    free(@k, set)         none of the set's metadata present
//!   liveness    dead, unread,         lanes or flags nothing reads after
//!               flags_dead,
//!               flags_dead_out        the instruction or the block
//!   encoding    encodable, no_longer, the rewrite encodes, in no more bytes
//!               same_length,          than the original (via `select`)
//!               splits_smaller, ...
//!   operands    same_width, narrower, widths, roots, register classes,
//!               same_root, segment    segments
//!   addresses   same_cell, next_word, two cells are the same bytes, or
//!               same_registers,       one word above the other
//!               stack_based, ...
//!   movement    independent,          an instruction between two matched
//!               hoistable, delays,    ones neither reads nor redefines what
//!               unchanged, clear_of   the rewrite moves across it
//!   cost        doubles_by_add,       what the target's timings price lower
//!               clears_before_...
//! ```
//!
//! Instructions that write memory are never crossed by `delays`: proving them
//! disjoint from a load belongs in MIR. `free` uses the bits of
//! [`super::field`]; a rule names a `meta` set of them so a rewrite cannot
//! drop a call's clobbers, a spill role or a group number.
//!
//! # Example
//! None: a guard answers a question and rewrites nothing. A rule that uses
//! guards, with the LIR it produces, is the `# Example` of [`super`] and of
//! `crate::backend::peephole`.
//! # Used by
//! The code generated from `peephole.peep`, through [`super::walk`].

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::walk::Cx;
use super::{Set, field};
use crate::backend::lanes::Lanes;
use crate::backend::peephole::{_lanes, _register_effects};
use crate::backend::{select, upperzero};
use crate::model::ir::{self, Held, Imm, Loc, Mem, Operation, Reg, Space};
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
            Loc::Address(_) | Loc::Field(_) => 0,
        }
    }
}

/// `one` carries none of the metadata `fields` names (`peep::field`).
pub fn free(
    _cx: &Cx,
    one: &Insn,
    fields: u32,
) -> bool {
    free_fields(one, fields)
}

/// `free`, for a caller with no context (`fixupbw.rs`).
pub fn free_fields(
    one: &Insn,
    fields: u32,
) -> bool {
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
        || has(field::UNOWNED) && one.covers.is_some_and(|(start, end)| start != end)
        || has(field::VOLATILE) && one.volatile)
}

/// `one` finds the upper half of `register`'s root zero.
pub fn upper_zero(
    cx: &Cx,
    one: &Arc<Insn>,
    register: Reg,
) -> bool {
    upperzero::bit(register.register).is_some_and(|bit| cx.upper_zero(one) & bit != 0)
}

/// Every lane in `lanes` is dead after `one`.
pub fn dead(
    cx: &Cx,
    one: &Arc<Insn>,
    lanes: Lanes,
) -> bool {
    let regs = cx.regs();
    lanes.is_subset(&cx.dead_after(regs, one))
}

/// No instruction after `one` reads any lane in `lanes`, even where a call may
/// clobber an upper half: a narrowed write cannot rely on the callee.
pub fn unread(
    cx: &Cx,
    one: &Arc<Insn>,
    lanes: Lanes,
) -> bool {
    lanes.is_subset(&cx.unread_after(cx.regs(), one))
}

/// Nothing after this instruction reads an arithmetic flag.
pub fn flags_dead(
    cx: &Cx,
    one: &Arc<Insn>,
) -> bool {
    use iced_x86::RflagsBits;
    dead(
        cx,
        one,
        Lanes::flags(
            RflagsBits::OF | RflagsBits::SF | RflagsBits::ZF | RflagsBits::AF | RflagsBits::CF | RflagsBits::PF,
        ),
    )
}

/// Nothing after the block may read these flags.
pub fn flags_dead_out(
    cx: &Cx,
    lanes: Lanes,
) -> bool {
    cx.flags_out().is_disjoint(&lanes)
}

pub fn empty(
    _cx: &Cx,
    lanes: Lanes,
) -> bool {
    lanes.is_empty()
}

pub fn disjoint(
    _cx: &Cx,
    one: Lanes,
    other: Lanes,
) -> bool {
    one.is_disjoint(&other)
}

fn emitted(
    bits: u32,
    one: &Insn,
) -> Option<select::Emitted> {
    select::priced_in(bits, one.what.as_ref()?, 0, None, false, false, None)
}

pub fn encodable(
    cx: &Cx,
    one: &Arc<Insn>,
) -> bool {
    emitted(cx.bits(), one).is_some()
}

/// Both encode, `after` in no more bytes than `before`.
pub fn no_longer(
    cx: &Cx,
    before: &Arc<Insn>,
    after: &Arc<Insn>,
) -> bool {
    matches!(
        (emitted(cx.bits(), before), emitted(cx.bits(), after)),
        (Some(before), Some(after)) if after.code.len() <= before.code.len()
    )
}

/// `after` encodes, and in real mode in no more bytes than `before`: the
/// dword form takes the operand-size prefix there. DEVIATE W1 of the byte/word
/// fixup (`peephole.peep`).
pub fn short_in_real_mode(
    cx: &Cx,
    before: &Arc<Insn>,
    after: &Arc<Insn>,
) -> bool {
    short_in_real_mode_at(cx.bits(), before, after)
}

/// `short_in_real_mode` at `bits`, for a caller with no context.
pub fn short_in_real_mode_at(
    bits: u32,
    before: &Insn,
    after: &Insn,
) -> bool {
    if bits == 16 {
        matches!(
            (emitted(bits, before), emitted(bits, after)),
            (Some(before), Some(after)) if after.code.len() <= before.code.len()
        )
    } else {
        emitted(bits, after).is_some()
    }
}

/// Both encode, in as many bytes.
pub fn same_length(
    cx: &Cx,
    before: &Arc<Insn>,
    after: &Arc<Insn>,
) -> bool {
    matches!(
        (emitted(cx.bits(), before), emitted(cx.bits(), after)),
        (Some(before), Some(after)) if after.code.len() == before.code.len()
    )
}

/// A push of `value` as a dword is more bytes than its two word pushes
/// (`select` sizes both).
pub fn split_push_smaller(
    bits: u32,
    value: i64,
) -> bool {
    let push = |width: u32, value: i64| {
        let what = ir::Semantics {
            name: Some("push".into()),
            sources: vec![Loc::Imm(Imm { value, width, address: None })],
            ..ir::Semantics::new(Operation::Push)
        };
        select::priced_in(bits, &what, 0, None, false, false, None).map(|code| code.code.len())
    };
    matches!(
        (push(4, value), push(2, (value >> 16) & 0xFFFF), push(2, value & 0xFFFF)),
        (Some(whole), Some(high), Some(low)) if high + low < whole
    )
}

/// Tuned for size, the dword push of `i` is more bytes than its two word
/// pushes. Two pushes are a clock slower than one, so only there.
pub fn splits_smaller(
    cx: &Cx,
    i: &Imm,
) -> bool {
    cx.cpu().size && i.width == 4 && i.address.is_none() && split_push_smaller(cx.bits(), i.value)
}

/// Two word pushes join into one dword push unless tuned for size and the dword
/// is longer.
pub fn joins_no_larger(
    cx: &Cx,
    high: &Imm,
    low: &Imm,
) -> bool {
    !cx.cpu().size || !split_push_smaller(cx.bits(), ((high.value & 0xFFFF) << 16) | (low.value & 0xFFFF))
}

/// The target prices clearing a register and loading its low byte below
/// `movzx` from the byte.
pub fn clears_before_byte_load(cx: &Cx) -> bool {
    cx.cpu().clears_before_byte_load().unwrap_or(false)
}

/// The target prices the `movzx` of a low byte or word at or below `and` with
/// the mask, or the code is for size.
pub fn masks_by_extension(cx: &Cx) -> bool {
    cx.cpu().masks_by_extension().unwrap_or(false)
}

/// `d` has a byte view and the address of `src` reads none of its register.
pub fn byte_loadable(
    cx: &Cx,
    d: Reg,
    src: &Mem,
) -> bool {
    let regs = cx.regs();
    let root = ir::root(d.register);
    d.width == 4
        && regs.named(d.register, 1) != d.register
        && [src.through, src.index_through].iter().all(|one| ir::root(*one) != root)
}

/// `d` is the low part of its root (not `ah`): the byte or word starts the
/// dword, so a dword access to the root keeps its place.
pub fn low_view(
    cx: &Cx,
    d: Reg,
) -> bool {
    let regs = cx.regs();
    _lanes(regs, d.register).iter().all(|lane| lane.1 < d.width as u32)
}

/// The code is for size (`shouldOptimizeForSize`).
pub fn for_size(cx: &Cx) -> bool {
    cx.cpu().size
}

/// The target prices `add r,r` below `shl r,1`.
pub fn doubles_by_add(cx: &Cx) -> bool {
    cx.cpu().doubling().is_ok_and(|form| form == "alu_rr")
}

pub fn same_width(
    _cx: &Cx,
    one: impl Wide,
    other: impl Wide,
) -> bool {
    one.bytes() == other.bytes()
}

pub fn narrower(
    _cx: &Cx,
    one: impl Wide,
    other: impl Wide,
) -> bool {
    one.bytes() < other.bytes()
}

/// The width, in bits, is one of these.
pub fn width_in(
    _cx: &Cx,
    one: impl Wide,
    bits: &[u32],
) -> bool {
    bits.contains(&(one.bytes() * 8))
}

pub fn same_root(
    _cx: &Cx,
    one: Reg,
    other: Reg,
) -> bool {
    ir::root(one.register) == ir::root(other.register)
}

/// A general register at its own width.
pub fn register_width(
    cx: &Cx,
    one: Reg,
) -> bool {
    let regs = cx.regs();
    regs.integer(one.register) && regs.width_of(one.register) == Some(i64::from(one.width))
}

/// A general register.
pub fn register_named(
    cx: &Cx,
    one: Reg,
) -> bool {
    let regs = cx.regs();
    regs.integer(one.register)
}

pub fn stack_or_frame(
    cx: &Cx,
    one: Reg,
) -> bool {
    let regs = cx.regs();
    regs.is_stack(one.register) || regs.is_frame(one.register)
}

pub fn segment(
    cx: &Cx,
    one: Reg,
) -> bool {
    let regs = cx.regs();
    regs.is_segment(one.register)
}

/// `one` is the fixed register of a member of `set`.
pub fn fixed_by(
    _cx: &Cx,
    set: &Set,
    one: Reg,
) -> bool {
    set.by_fixed(one.register).is_some()
}

/// Its `covers` is the point it stands at.
pub fn placed(
    _cx: &Cx,
    one: &Insn,
) -> bool {
    one.covers == Some((one.at, one.at))
}

/// `one`'s bytes end where `next` stands.
pub fn ends_at(
    _cx: &Cx,
    one: &Insn,
    next: &Insn,
) -> bool {
    one.covers.is_some_and(|(_, end)| end == next.at)
}

/// `second` reads `first`'s one value, which nothing else reads.
pub fn feeds_once(
    cx: &Cx,
    first: &Insn,
    second: &Insn,
) -> bool {
    first.defines.len() == 1
        && second.uses == first.defines
        && cx.facts.users().get(&first.defines[0]).copied().unwrap_or(0) == 1
}

/// `one` neither reads nor redefines what `first` defines.
pub fn independent(
    _cx: &Cx,
    first: &Insn,
    one: &Arc<Insn>,
) -> bool {
    first.defines.is_empty()
        || !first.defines.iter().any(|value| one.uses.contains(value) || one.defines.contains(value))
}

/// Writing `made`'s wider destination where `first` stood crosses no
/// instruction in `crossed` that observes or replaces a lane it newly
/// writes. Source frontends batch their entry loads, so independent
/// parameter loads commonly stand between a load and its extension.
pub fn hoistable(
    cx: &Cx,
    made: &Arc<Insn>,
    first: &Insn,
    crossed: &[Arc<Insn>],
) -> bool {
    let regs = cx.regs();
    if crossed.is_empty() {
        // Nothing to cross. Asking anyway refused every unrolled clone,
        // whose effects `_register_effects` will not read.
        return true;
    }
    let (Some(original), Some(combined)) =
        (_register_effects(regs, cx.bits(), first, false, true), _register_effects(regs, cx.bits(), made, false, true))
    else {
        return false;
    };
    let newly_written: Lanes = combined.1.minus(&original.1);
    crossed.iter().all(|one| match _register_effects(regs, cx.bits(), one, false, true) {
        None => false,
        Some((reads, writes)) => !newly_written.iter().any(|lane| reads.contains(lane) || writes.contains(lane)),
    })
}

/// `one` writes neither register and does not read the temporary `t`.
pub fn clear_of(
    cx: &Cx,
    t: Reg,
    s: Reg,
    one: &Arc<Insn>,
) -> bool {
    let regs = cx.regs();
    let (temporary, source) = (_lanes(regs, t.register), _lanes(regs, s.register));
    _register_effects(regs, cx.bits(), one, true, false)
        .is_some_and(|(reads, writes)| reads.is_disjoint(&temporary) && writes.is_disjoint(&temporary.or(&source)))
}

/// What `crossed` leaves alone of a store `mov [cell], r`: the register `r`
/// and what addresses the cell are not written, and the cell still holds what
/// the store put there (`bodyfacts::_keeps`: a call, a volatile access, a
/// barrier or any write that may reach the cell fails).
pub fn holds_after(
    cx: &Cx,
    store: &Insn,
    crossed: &Arc<Insn>,
) -> bool {
    let (Some(what), Some(stored)) = (&crossed.what, &store.what) else {
        return false;
    };
    let ([Loc::Mem(cell)], [Loc::Reg(held)]) = (stored.dests.as_slice(), stored.sources.as_slice()) else {
        return false;
    };
    if store.volatile
        || crossed.volatile
        || what.op == Operation::Barrier
        || !crossed.clobbers.is_empty()
        || !crossed.requires.is_empty()
        || !crossed.delivers.is_empty()
        || !crossed.spread.is_empty()
        || crossed.group.is_some()
        || crossed.symbol == Some(true)
        || crossed.frame_adjust
    {
        return false;
    }
    let regs = cx.regs();
    let Some((_, writes)) = _register_effects(regs, cx.bits(), crossed, false, true) else {
        return false;
    };
    let mut guarded = BTreeSet::from([cell.through, cell.index_through, held.register]);
    if let Some(addr) = cell.addr {
        guarded.insert(RegId::from(addr.segment));
    }
    let guarded: Lanes = guarded.into_iter().flat_map(|one| _lanes(regs, one)).collect();
    if !writes.is_disjoint(&guarded) {
        return false;
    }
    // A slot only the allocator's spills use is reached by no program pointer
    // (nothing takes its address): a write through a register, a string
    // instruction or a call cannot change it, only an access naming the slot.
    if cell.addr.and_then(|addr| addr.slot_home()).is_some_and(|home| cx.facts.spill_homes().contains(&home)) {
        return !crate::backend::bodyfacts::_written(crossed, cell).any(|dest| {
            crate::backend::bodyfacts::_in_frame(dest)
                && crate::backend::overlap::may_overlap(cell.addr, cell.width, dest.addr, dest.width)
        });
    }
    crate::backend::bodyfacts::_keeps(crossed, store, cell, true, cx.facts.body())
}

/// `load` may read its cell after `crossed`, which only materializes a
/// register: it neither consumes nor replaces the loaded register nor
/// changes anything the cell is addressed by. Instructions that write
/// memory stay outside: proving them disjoint belongs in MIR.
pub fn delays(
    cx: &Cx,
    load: &Insn,
    crossed: &Arc<Insn>,
) -> bool {
    delayed(cx, load, crossed, false)
}

/// `delays`, and past a push too: its slot is below the stack pointer, which no
/// cell the load reads through a register is, and a cell read through the stack
/// pointer is in the lanes the push writes. For a load a call reads, which the
/// arguments pushed before it stand between.
pub fn delays_pushes(
    cx: &Cx,
    load: &Insn,
    crossed: &Arc<Insn>,
) -> bool {
    delayed(cx, load, crossed, true)
}

fn delayed(
    cx: &Cx,
    load: &Insn,
    crossed: &Arc<Insn>,
    pushes: bool,
) -> bool {
    let regs = cx.regs();
    let Some(crossed_what) = &crossed.what else {
        return false;
    };
    // Two volatile accesses keep their order: the device sees each.
    if load.volatile && crossed.volatile {
        return false;
    }
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
    // A write to memory crosses where the cell still holds after it
    // (`crate::backend::bodyfacts::_keeps`: the optimizer's proofs included);
    // anything else must write registers alone, which the lane check below
    // keeps from the load's.
    let writes_memory = crossed_what.dests.iter().any(|dest| matches!(dest, Loc::Mem(_)));
    let load_cell = match load.what.as_ref().map(|what| what.sources.as_slice()) {
        Some([Loc::Mem(cell)]) => Some(cell),
        _ => None,
    };
    if writes_memory {
        let Some(cell) = load_cell else { return false };
        if !crate::backend::bodyfacts::_keeps(crossed, load, cell, true, cx.facts.body()) {
            return false;
        }
    } else if !(matches!(
        crossed_what.op,
        Operation::Move
            | Operation::Extend
            | Operation::Address
            | Operation::Binary
            | Operation::Unary
            | Operation::Multiply
            | Operation::Funnel
            | Operation::Compare
    ) && crossed_what.dests.iter().all(|dest| matches!(dest, Loc::Reg(_)))
        || pushes && crossed_what.op == Operation::Push)
    {
        return false;
    }
    let (Some((load_reads, load_writes)), Some((crossed_reads, crossed_writes))) = (
        _register_effects(regs, cx.bits(), load, false, true),
        _register_effects(regs, cx.bits(), crossed, false, true),
    ) else {
        return false;
    };
    let address_lanes: Lanes = match load.what.as_ref().map(|what| what.sources.as_slice()) {
        Some([Loc::Mem(cell)]) => {
            let mut address_registers = BTreeSet::from([cell.through, cell.index_through]);
            if let Some(addr) = cell.addr {
                address_registers.insert(RegId::from(addr.segment));
                // Once MIR computed a base value, allocation's `through` is
                // the encoded register and BC's original `addr.base` is only
                // provenance. A cell with no value still encodes that base.
                if cell.base.is_none() {
                    address_registers.insert(RegId::from(addr.base));
                }
            }
            address_registers.into_iter().flat_map(|one| _lanes(regs, one)).collect()
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
pub fn operand(
    _cx: &Cx,
    one: &Loc,
    r: Reg,
) -> bool {
    matches!(one, Loc::Imm(value) if value.address.is_none())
        || matches!(
            one,
            Loc::Reg(value) if ir::root(value.register) != ir::root(r.register)
        )
}

/// The registers, scale and displacement of the address an LEA names, whether
/// LIR spells it as an address or, as the two-address pass makes it, as a
/// memory operand: (symbol, base, index, scale, offset).
pub fn lea_cell(one: &Loc) -> Option<(bool, RegId, RegId, i64, i64)> {
    match one {
        Loc::Address(a) => Some((a.addr.is_some(), a.through, a.index_through, a.scale, a.offset)),
        Loc::Mem(m) => Some((m.addr.is_some(), m.through, m.index_through, m.scale, m.offset)),
        _ => None,
    }
}

/// `r` is how the cell is reached.
pub fn addresses(
    _cx: &Cx,
    cell: &Mem,
    r: Reg,
) -> bool {
    [ir::root(cell.through), ir::root(cell.index_through)].contains(&ir::root(r.register))
}

/// The cell is reached through segment register `g`.
pub fn segment_of(
    _cx: &Cx,
    cell: &Mem,
    g: Reg,
) -> bool {
    cell.addr.is_some_and(|addr| addr.segment == g.register.iced())
}

/// Two operands address the same bytes. The registers and displacement
/// carry the address; the values they name may differ when allocation
/// copied the same address into a register again as a new value. `same_place`
/// compares the address and the values the operand names, not the registers,
/// so the registers are compared beside it.
pub fn same_cell(
    _cx: &Cx,
    one: &Mem,
    other: &Mem,
) -> bool {
    let logical = |cell: &Mem| Mem { base: None, index: None, ..cell.clone() };
    let physical = |cell: &Mem| (cell.through, cell.index_through, cell.offset);
    let placed = |cell: &Mem| {
        (cell.base.is_none() || cell.through != RegId::None)
            && (cell.index.is_none() || cell.index_through != RegId::None)
    };
    logical(one).same_place(&logical(other)) && physical(one) == physical(other) && placed(one) && placed(other)
}

/// `high` is the word right after `low`, reached the same way. The
/// displacement may be carried by the address, by the operand's offset,
/// or by both at once, so either may be the one two further on.
pub fn next_word(
    _cx: &Cx,
    low: &Mem,
    high: &Mem,
) -> bool {
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
pub fn moves(
    _cx: &Cx,
    one: &Arc<Insn>,
) -> bool {
    one.what.as_ref().is_some_and(|what| what.op == Operation::Move && what.name.as_deref() == Some("mov"))
        && one.clobbers.is_empty()
}

/// The caller counted `value` read once.
pub fn used_once(
    cx: &Cx,
    value: Held,
) -> bool {
    cx.facts.count(value.value) == 1
}

pub fn defines_only(
    _cx: &Cx,
    one: &Insn,
    value: Held,
) -> bool {
    one.defines == [value.value]
}

pub fn uses_only(
    _cx: &Cx,
    one: &Insn,
    value: Held,
) -> bool {
    one.uses == [value.value]
}

/// Nothing between `definition` and `at` writes memory or a register,
/// clobbers one, or is a barrier, call or return: the loaded value still
/// stands where it was read.
pub fn unchanged(
    cx: &Cx,
    definition: &Arc<Insn>,
    at: &Arc<Insn>,
) -> bool {
    cx.insns[cx.place(definition) + 1..cx.place(at)]
        .iter()
        .all(
            |one| one.clobbers.is_empty()
                && one.what.as_ref().is_some_and(|what| {
                    !what.dests.iter().any(|dest| matches!(dest, Loc::Mem(_) | Loc::Reg(_)))
                        && !matches!(
                            what.op,
                            Operation::Barrier | Operation::Call | Operation::Return
                        )
                }),
        )
}

/// `high` is the word above `low`.
pub fn above(
    _cx: &Cx,
    high: &Mem,
    low: &Mem,
) -> bool {
    high.word_above(low)
}

/// Both cells are reached through the same registers.
pub fn same_registers(
    _cx: &Cx,
    one: &Mem,
    other: &Mem,
) -> bool {
    (one.through, one.index_through) == (other.through, other.index_through)
}

/// The cell is reached through the stack pointer.
pub fn stack_based(
    cx: &Cx,
    cell: &Mem,
) -> bool {
    let regs = cx.regs();
    regs.is_stack(cell.through)
}

/// A frame cell at or above the arguments, reached without the stack pointer.
pub fn frame_argument(
    cx: &Cx,
    cell: &Mem,
) -> bool {
    let regs = cx.regs();
    cell.addr.is_some_and(|addr| addr.space == Space::Frame && addr.disp >= 4)
        && !regs.is_stack(cell.through)
        && !regs.is_stack(cell.index_through)
        && !cell.stack_argument
}
