//! A float copied from one cell to another is copied through a free general
//! register where that is cheaper than the x87 pair.
//!
//! # How it works
//! `fld dword [A] ; fstp dword [B]` leaves the stack as it found it and moves
//! four bytes. A 486 takes 3 clocks for the load and 7 for the store; `mov
//! r,[A] ; mov [B],r` takes 1 and 1 per word. The pair becomes one such move
//! per register-sized word (a dword; a qword in long mode) of the cell where a
//! register is dead after the pair, the cells share no byte, and the clocks
//! saved, at the target's speed rate, outweigh the bytes added.
//!
//! ```text
//!  before                          after
//!  fld  dword [bp-4]               mov eax, [bp-4]
//!  fstp dword [bp-8]               mov [bp-8], eax
//!  10 clocks, 6 bytes              2 clocks, 8 bytes (m16: 66h prefixes)
//! ```
//!
//! The pair is a plain `fld` of a 4 or 8 byte cell
//! and the `fstp` of the same width that pops it next (anchors may sit
//! between). The price is the target's own (`x87_load`, `x87_store`, `mov_rm`,
//! `mov_mr`), the encoded sizes are the selector's. Nothing is done where bytes
//! are strict (-O1, -Os), where the target has no speed rate, or where a cost
//! is missing.
//!
//! # Example
//! The ten-float rotation of `x87rotate.c` at -O2 -m16 spills the carried set
//! through frame cells; each shift of a cell is such a pair.
//!
//! # Runs
//! A sub-pass of [`crate::backend::peephole`], once per body, after
//! `slotcopies::merged`.
//!
//! # Keeps
//! None is returned where no pair qualifies.
//!
//! # Deviations from the pin
//! - DEVIATE X87ROT-GPR-COPY (owner x87, review 2026-12-10): the pin leaves the
//!   pair (`llc -O2 -mcpu=i486` on the same IR keeps `flds`/`fstps` slot pairs
//!   and eleven `fxch` a trip); gcc and Watcom copy a spilled float through
//!   general registers. The bits move as they are: a signalling NaN is not
//!   quieted and no x87 exception flag is raised by the copy.

use std::sync::Arc;

use llrm_lir::registers::RegId;
use llrm_lir::registers::Regs;

use crate::backend::classes::RegisterClasses;
use crate::backend::cpu::Profile;
use crate::backend::lanes::Lanes;
use crate::backend::peephole::{_lanes, _register_effects, DeadAfter, id};
use crate::backend::storedhomes::apart;
use crate::backend::{liveness, masm, regthrash, select};
use crate::model::ir::{Loc, Mem, Operation, Reg, Semantics};
use crate::model::lir::{Insn, LirBody};

/// The clocks of the x87 pair and of one word moved through a register, and
/// the clocks a byte is worth (thousandths).
struct Prices {
    load: i64,
    store: i64,
    mov_load: i64,
    mov_store: i64,
    rate: i64,
}

impl Prices {
    fn of(cpu: &Profile) -> Option<Self> {
        if !cpu.has_speed_goal() || cpu.bytes_strict() {
            return None;
        }
        let rate = cpu.speed_milliclocks_per_byte();
        let get = |name: &str| cpu.cost(name).ok();
        (rate > 0).then_some(Self {
            load: get("x87_load")?,
            store: get("x87_store")?,
            mov_load: get("mov_rm")?,
            mov_store: get("mov_mr")?,
            rate,
        })
    }
}

/// The cell a plain `fld` of a 4 or 8 byte cell reads.
fn loaded(one: &Insn) -> Option<&Mem> {
    if one.volatile || one.group.is_some() || !one.clobbers.is_empty() || !one.requires.is_empty() {
        return None;
    }
    let what = one.what.as_ref()?;
    match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::FloatLoad, Some("fld"), [top], [Loc::Mem(cell)])
            if *top == Loc::st(0) && [4, 8].contains(&cell.width) =>
        {
            Some(cell)
        }
        _ => None,
    }
}

/// The cell a plain `fstp` of a 4 or 8 byte cell writes.
fn stored(one: &Insn) -> Option<&Mem> {
    if one.volatile || one.group.is_some() || !one.clobbers.is_empty() || !one.requires.is_empty() {
        return None;
    }
    let what = one.what.as_ref()?;
    match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::FloatStore, Some("fstp"), [Loc::Mem(cell)], [top])
            if *top == Loc::st(0) && [4, 8].contains(&cell.width) =>
        {
            Some(cell)
        }
        _ => None,
    }
}

/// The encoded size of `what`.
fn bytes(
    bits: u32,
    what: &Semantics,
) -> Option<usize> {
    select::priced_in(bits, what, 0, None, false, false, None).map(|code| code.code.len())
}

/// `cell`, `by` bytes on and `width` wide.
fn moved(
    cell: &Mem,
    by: i64,
    width: u32,
) -> Mem {
    Mem {
        addr: cell.addr.map(|addr| addr.plus(by)),
        offset: if cell.addr.is_some() { cell.offset } else { cell.offset + by },
        width,
        ..cell.clone()
    }
}

/// The moves that copy `from` into `to` through `register`, a word at a time.
fn word_moves(
    from: &Mem,
    to: &Mem,
    register: Reg,
) -> Vec<(Semantics, Semantics)> {
    let word = i64::from(register.width);
    (0..i64::from(from.width) / word)
        .map(|index| {
            let load = Semantics {
                name: Some("mov".into()),
                dests: vec![Loc::Reg(register)],
                sources: vec![Loc::Mem(moved(from, index * word, register.width))],
                ..Semantics::new(Operation::Move)
            };
            let store = Semantics {
                name: Some("mov".into()),
                dests: vec![Loc::Mem(moved(to, index * word, register.width))],
                sources: vec![Loc::Reg(register)],
                ..Semantics::new(Operation::Move)
            };
            (load, store)
        })
        .collect()
}

/// An anchor: no machine work, and nothing it requires or delivers.
fn inert(one: &Insn) -> bool {
    one.what.as_ref().is_none_or(|what| what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty))
        && one.clobbers.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
}

/// The index of the `fstp` that pops the `fld` at `at`: the next instruction
/// that is not an anchor.
fn popped_by(
    insns: &[Arc<Insn>],
    at: usize,
) -> Option<usize> {
    loaded(&insns[at])?;
    let next = at + 1 + insns[at + 1..].iter().position(|one| !inert(one))?;
    stored(&insns[next]).map(|_| next)
}

/// The pair as register moves, none where they are not cheaper or no register
/// is free.
fn copy_through_register(
    regs: Regs,
    bits: u32,
    load: &Arc<Insn>,
    store: &Arc<Insn>,
    prices: &Prices,
    scratch: &[RegId],
    dead: &DeadAfter,
) -> Option<Vec<Arc<Insn>>> {
    let (from, to) = (loaded(load)?, stored(store)?);
    if from.width != to.width || !apart(from, to) {
        return None;
    }
    // A word is the widest register: a dword in 16-bit code too (386 and up).
    let width = from.width.min(if bits >= 64 { 8 } else { 4 });
    let reads = [load, store]
        .into_iter()
        .filter_map(|one| _register_effects(regs, bits, one, false, true))
        .fold(Lanes::new(), |all, (reads, _)| all.or(&reads));
    let register = scratch
        .iter()
        .find_map(
            |full| {
                let register = regs.named(*full, i64::from(width));
                let register = (regs.width_of(register) == Some(i64::from(width))).then_some(Reg { register, width })?;
                (_lanes(regs, register.register).is_subset(&dead[&id(store)])
                    && _lanes(regs, register.register).is_disjoint(&reads))
                .then_some(register)
            },
        )?;
    let moves = word_moves(from, to, register);
    let words = moves.len() as i64;
    let before = bytes(bits, load.what.as_ref()?)? + bytes(bits, store.what.as_ref()?)?;
    let after =
        moves.iter().map(|(load, store)| Some(bytes(bits, load)? + bytes(bits, store)?)).sum::<Option<usize>>()?;
    let clocks = prices.load + prices.store - words * (prices.mov_load + prices.mov_store);
    // Clocks saved against the bytes added, at the speed rate.
    if clocks * 1000 <= (after as i64 - before as i64) * prices.rate {
        return None;
    }
    let mut out = Vec::with_capacity(moves.len() * 2);
    for (mov_load, mov_store) in moves {
        out.push(Arc::new(Insn { what: Some(mov_load), ..(**load).clone() }));
        out.push(Arc::new(Insn { what: Some(mov_store), ..(**store).clone() }));
    }
    Some(out)
}

/// `body` with each float cell-to-cell copy that a free register makes
/// cheaper made through that register; none where no pair qualifies.
pub fn copied(
    body: &LirBody,
    cpu: &Profile,
    saved: &[RegId],
    classes: &RegisterClasses,
) -> Option<LirBody> {
    let prices = Prices::of(cpu)?;
    let has_pair =
        body.blocks.iter().any(|block| (0..block.insns.len()).any(|at| popped_by(&block.insns, at).is_some()));
    if !has_pair {
        return None;
    }
    let regs = body.regs();
    let used = masm::_roots(body);
    let scratch: Vec<RegId> =
        classes.available.iter().copied().filter(|one| !saved.contains(one) || used.contains(one)).collect();
    let exits = liveness::dead_at_exit(body);
    let mut changed = false;
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let dead = regthrash::_dead_after(regs, body.bits, block, exits[&block.at].clone());
            let mut insns: Vec<Arc<Insn>> = Vec::with_capacity(block.insns.len());
            let mut at = 0;
            while at < block.insns.len() {
                if let Some(end) = popped_by(&block.insns, at) {
                    let (load, store) = (&block.insns[at], &block.insns[end]);
                    if let Some(made) = copy_through_register(regs, body.bits, load, store, &prices, &scratch, &dead) {
                        insns.extend(made);
                        // the anchors between the pair keep their place after
                        // the moves
                        insns.extend(block.insns[at + 1..end].iter().cloned());
                        changed = true;
                        at = end + 1;
                        continue;
                    }
                }
                insns.push(Arc::clone(&block.insns[at]));
                at += 1;
            }
            block.with_insns(insns)
        })
        .collect();
    changed.then(|| body.with_blocks(blocks))
}
