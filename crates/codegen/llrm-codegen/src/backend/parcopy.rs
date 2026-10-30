//! Writes each parallel copy group (the copies a phi becomes on an edge, which
//! happen at once) as a sequence of moves that computes the same thing.
//!
//! # How it works
//! After allocation which copies conflict is a question about locations. A
//! group is the run of instructions that share a `group` number. Inside it:
//!
//! 1. An identity copy (same place both sides) emits nothing but stays as an
//!    anchor, because it still defines the virtual value its destination stands
//!    for.
//! 2. A copy is ready when no other copy still to come reads the place it
//!    writes (`ready`). Ready copies are written, in their order, and the step
//!    repeats.
//! 3. When none is ready, every remaining copy waits on another: a cycle
//!    (`in_cycle`, `_rotated`). It is broken by the cheapest of three routes,
//!    in this tie order.
//!
//! ```text
//!   cycle  a <- b <- c <- a
//!
//!   Exchanges   xchg a,b ; xchg b,c                 (n-1 exchanges; every
//!                                                   adjacent pair must be
//!                                                   exchangeable, one width)
//!   Register    mov t,<closing source> ; the other moves in order ;
//!               mov <closing dest>,t                (t: a register dead
//!                                                   after the group that no
//!                                                   copy names)
//!   Stack       push <closing source> ; the other moves ; pop <closing dest>
//! ```
//!
//! The target's prices (`CyclePrices`: `mov_rr`, `xchg_rr`, `push_r`, `pop_r`)
//! choose the route. Where the CPU is tuned for size there are no prices and
//! the route is exchanges where they apply, else the stack. A scratch register
//! is a candidate only if the convention does not preserve it or the body
//! already uses it, so no push and pop is added to save it. A cycle no move of
//! which the stack can carry is malformed, and so is a group that is not a
//! permutation of its places.
//!
//! A frame-to-frame copy of equal width (1, 2 or 4 bytes, a byte moved as a
//! word) becomes `push` then `pop`, which needs no scratch register and keeps
//! the flags.
//!
//! # Example
//! `swaps` in a loop exchanges `a` and `b` each trip. The back edge's three
//! copies are one group, and `AX`, `DX` form a cycle (`BX` only reads `CX`):
//!
//! Before:
//!
//! ```lir
//! block 0x000b
//!   0x000b  BX := mov CX  ; inserted; parallel-copy 2
//!   0x000b  DX := mov AX  ; inserted; parallel-copy 2
//!   0x000b  AX := mov DX  ; inserted; parallel-copy 2
//!   0x000b  jmp   ; inserted
//! ```
//!
//! After:
//!
//! ```lir
//! block 0x000b
//!   0x000b  BX := mov CX  ; inserted
//!   0x000b  DX, AX := xchg AX, DX  ; inserted
//!   0x000b     ; inserted
//!   0x000b  jmp   ; inserted
//! ```
//!
//! The ready copy is written first, the cycle becomes one `xchg`, and an anchor
//! keeps the virtual definition the dropped copies stood for.
//!
//! From `ISEL_DUMP=1` of a C function whose loop does `t = a; a = b; b = t;`,
//! compiled with `llrm-c -O2 -m16`: the `RegAlloc` and `ParallelCopy` stages.
//!
//! # Runs
//! After `regalloc`, before `prologue` and the peephole, once per body, at
//! every level. A body with no group is returned as it is. `parcopy` is the
//! phase name.
//!
//! # Needs
//! [`crate::backend::liveness`] (through `regthrash::_dead_after`) for the
//! registers dead after a group, and the groups `phielim` marks.

use std::fmt;
use std::sync::Arc;

use llrm_lir::registers::{RegId, Regs};

use crate::backend::cpu::Profile;
use crate::backend::lanes::Lanes;
use crate::backend::peephole::{_lanes, _register_effects, id};
use crate::backend::{liveness, masm, regthrash, target};
use crate::model::ir::{self, Loc, Mem, Operation, Reg, Semantics};
use crate::model::lir::{self, Insn, LirBody};
use crate::model::passes::{Exception, LIRTransform};
use crate::support::hash::{IndexMap, IndexSet};
use crate::support::pyrepr::Repr;

/// Something in a copy group that is not a move of one place to another.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Malformed(pub String);

impl fmt::Display for Malformed {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Malformed {}

/// What the target charges for the ways a cycle of copies is broken, in clocks.
#[derive(Clone, Copy, Debug)]
pub struct CyclePrices {
    pub copy: i64,
    pub exchange: i64,
    pub push: i64,
    pub pop: i64,
}

/// How a cycle of copies is broken.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    Exchanges,
    Register,
    Stack,
}

impl CyclePrices {
    /// None tuned for size: a register copy is two bytes, a push or a pop one.
    pub fn of(cpu: &Profile) -> Option<Self> {
        if cpu.size {
            return None;
        }
        Some(Self {
            copy: cpu.cost("mov_rr").ok()?,
            exchange: cpu.cost("xchg_rr").ok()?,
            push: cpu.cost("push_r").ok()?,
            pop: cpu.cost("pop_r").ok()?,
        })
    }

    /// The cheapest of the ways open to a cycle of `moves`, the exchanges
    /// before the register before the stack where they cost the same.
    fn route(
        &self,
        moves: usize,
        exchanges: bool,
        register: bool,
    ) -> Route {
        let moves = moves as i64;
        let stack = self.push + (moves - 1) * self.copy + self.pop;
        let mut best = (Route::Stack, stack);
        if register && (moves + 1) * self.copy < best.1 {
            best = (Route::Register, (moves + 1) * self.copy);
        }
        if exchanges && (moves - 1) * self.exchange <= best.1 {
            best = (Route::Exchanges, (moves - 1) * self.exchange);
        }
        best.0
    }
}

/// A cycle of copies is broken by exchanges, else through a register that is
/// dead there where one is and the target's prices say it is cheaper than the
/// stack, else through the stack. `scratch`: the registers a body may use that
/// it does not have to save.
#[derive(Default)]
pub struct ParallelCopy {
    pub prices: Option<CyclePrices>,
    pub available: Vec<RegId>,
    pub saved: Vec<RegId>,
}

/// The registers a cycle may be rotated through where it stands, and what
/// they cost.
struct Spare<'a> {
    prices: CyclePrices,
    /// Registers dead after the group, the group neither reads nor writes.
    free: &'a [RegId],
}

/// `body` without its `implicit_def`s: what reads such a value reads it as
/// undef, and the definition goes (`ExpandPostRAPseudos`' `IMPLICIT_DEF` case;
/// here the first phase after allocation, using `ProcessImplicitDefs`' own
/// work).
pub fn expanded(body: &LirBody) -> LirBody {
    let is_implicit = |one: &Insn| {
        one.uses.is_empty()
            && one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some(crate::processimpdefs::IMPLICIT_DEF))
    };
    if !body.blocks.iter().flat_map(|block| block.insns.iter()).any(|one| is_implicit(one)) {
        return body.clone();
    }
    crate::processimpdefs::processed(body)
}

impl LIRTransform for ParallelCopy {
    fn class_name(&self) -> &'static str {
        "ParallelCopy"
    }

    fn name(&self) -> &str {
        "parcopy"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        scheduled_with(&expanded(&body), self.prices, &self.available, &self.saved)
            .map_err(|refused| refused.to_string())
    }

    fn transform_raising(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, Exception> {
        scheduled_with(&expanded(&body), self.prices, &self.available, &self.saved)
            .map_err(|malformed| Exception::defined_in("qbopt.backend.parcopy", "Malformed", malformed.to_string()))
    }
}

fn semantics(
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Semantics {
    Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
}

fn _width(one: &Loc) -> u32 {
    match one {
        Loc::Reg(one) => one.width,
        Loc::Mem(one) => one.width,
        Loc::Imm(one) => one.width,
        Loc::Held(one) => one.width,
        Loc::Address(_) | Loc::Field(_) => panic!("{} has no width", one.repr()),
    }
}

/// Python `list.remove`: drop the first element equal to `one`.
fn remove(
    left: &mut Vec<Arc<Insn>>,
    one: &Arc<Insn>,
) {
    let index = left.iter().position(|each| **each == **one).expect("list.remove(x): x not in list");
    left.remove(index);
}

/// `body` with every copy group written in an order that computes it.
pub fn scheduled(body: &LirBody) -> Result<LirBody, Malformed> {
    scheduled_with(body, None, &[], &[])
}

/// `scheduled`, a cycle rotated through a dead register where `prices` say that
/// costs less than the stack: `available` are the registers the body may name
/// and `saved` those its convention preserves (one it does not use would be
/// pushed and popped for the cycle).
pub fn scheduled_with(
    body: &LirBody,
    prices: Option<CyclePrices>,
    available: &[RegId],
    saved: &[RegId],
) -> Result<LirBody, Malformed> {
    let regs = body.regs();
    if !body.blocks.iter().any(|block| block.insns.iter().any(|one| one.group.is_some())) {
        return Ok(body.clone());
    }
    let candidates: Vec<RegId> = match prices {
        Some(_) => {
            let used = masm::_roots(body);
            available.iter().copied().filter(|one| !saved.contains(one) || used.contains(one)).collect()
        }
        None => Vec::new(),
    };
    let exits = if candidates.is_empty() { None } else { Some(liveness::dead_at_exit(body)) };
    let mut out_body = body.clone();
    for block in &mut out_body.blocks {
        let dead = exits.as_ref().map(|exits| regthrash::_dead_after(regs, body.bits, block, exits[&block.at].clone()));
        let mut out: Vec<Arc<Insn>> = Vec::new();
        let mut run: Vec<Arc<Insn>> = Vec::new();
        for one in block.insns.iter().map(Some).chain([None]) {
            let group = one.and_then(|one| one.group);
            if !run.is_empty() && group != run[0].group {
                let free: Vec<RegId> = match (&dead, prices) {
                    (Some(dead), Some(_)) => _free(regs, body.bits, &run, &candidates, &dead[&id(&run[run.len() - 1])]),
                    _ => Vec::new(),
                };
                let spare = prices.map(|prices| Spare { prices, free: &free });
                for part in _ordered(regs, &run, spare.as_ref())? {
                    out.extend(_expanded(&part)?);
                }
                run = Vec::new();
            }
            let Some(one) = one else { break };
            if group.is_none() {
                out.push(Arc::clone(one));
                continue;
            }
            run.push(Arc::clone(one));
        }
        block.insns = out.into();
    }
    Ok(out_body)
}

/// Of `candidates`, the registers dead after the group `run` that no copy of it
/// reads or writes.
fn _free(
    regs: Regs,
    bits: u32,
    run: &[Arc<Insn>],
    candidates: &[RegId],
    dead_after: &Lanes,
) -> Vec<RegId> {
    let named = run
        .iter()
        .filter_map(|one| _register_effects(regs, bits, one, false, true))
        .fold(Lanes::new(), |all, (reads, writes)| all.or(&reads).or(&writes));
    candidates
        .iter()
        .copied()
        .filter(|full| {
            let lanes = _lanes(regs, *full);
            lanes.is_subset(dead_after) && lanes.is_disjoint(&named)
        })
        .collect()
}

/// An ordered frame copy needs no scratch register and preserves flags.
fn _expanded(one: &Arc<Insn>) -> Result<Vec<Arc<Insn>>, Malformed> {
    if one.what.as_ref().is_some_and(|what| what.op != Operation::Move) {
        return Ok(vec![Arc::clone(one)]);
    }
    let what = one.what.as_ref().expect("a move in a copy group");
    let (into, source) = (&what.dests[0], &what.sources[0]);
    let (Loc::Mem(into), Loc::Mem(source)) = (into, source) else {
        return Ok(vec![Arc::clone(one)]);
    };
    if into.width != source.width
        || !matches!(into.width, 1 | 2 | 4)
        || [into, source].iter().any(|cell| cell.through != ir::FRAME)
    {
        return Err(Malformed("memory parallel copy needs equal-width frame slots".to_owned()));
    }
    // A copy moves held values, so its cells are spill slots, and a slot
    // owns at least a word: a byte's is moved whole.
    let [into, source] = [into, source].map(|cell| Mem { width: cell.width.max(2), ..cell.clone() });
    let mut push = (**one).clone();
    push.what = Some(semantics(Operation::Push, "push", vec![], vec![Loc::Mem(source)]));
    push.defines = vec![];
    push.uses = vec![];
    let mut pop = (**one).clone();
    pop.what = Some(semantics(Operation::Pop, "pop", vec![Loc::Mem(into)], vec![]));
    pop.covers = Some((one.at, one.at));
    pop.call = None;
    pop.defines = vec![];
    pop.uses = vec![];
    pop.spread = vec![];
    Ok(vec![Arc::new(push), Arc::new(pop)])
}

fn ungrouped(one: &Insn) -> Arc<Insn> {
    let mut out = one.clone();
    out.group = None;
    Arc::new(out)
}

/// One group, in an order where no move reads what an earlier one wrote.
fn _ordered(
    regs: Regs,
    moves: &[Arc<Insn>],
    spare: Option<&Spare>,
) -> Result<Vec<Arc<Insn>>, Malformed> {
    let mut identities = Vec::new();
    for one in moves {
        if _into(one)? == _outof(one)? {
            identities.push(Arc::clone(one));
        }
    }
    let mut left = Vec::new();
    for one in moves {
        if _into(one)? != _outof(one)? {
            left.push(Arc::clone(one));
        }
    }
    // An identity emits no machine instruction but still defines the
    // virtual value its destination represents: keep a zero-cost marker.
    let mut out: Vec<Arc<Insn>> = identities.into_iter().map(lir::anchor).collect();
    while !left.is_empty() {
        let reads = left.iter().map(|one| Ok(vec![_outof(one)?])).collect::<Result<Vec<_>, Malformed>>()?;
        let writes = left.iter().map(|one| Ok(vec![_into(one)?])).collect::<Result<Vec<_>, Malformed>>()?;
        let ready: Vec<Arc<Insn>> = ready(&reads, &writes, &(0..left.len()).collect::<Vec<_>>())
            .into_iter()
            .map(|at| Arc::clone(&left[at]))
            .collect();
        if ready.is_empty() {
            let (made, used) = _rotated(regs, &left, spare)?;
            out.extend(made);
            for one in &used {
                remove(&mut left, one);
            }
            continue;
        }
        for one in &ready {
            out.push(ungrouped(one));
            remove(&mut left, one);
        }
    }
    Ok(out)
}

/// Of the copies `left` (indices into `reads` and `writes`, the places each
/// reads and writes), those that may go: nothing else still to come reads a
/// place they write.
pub(crate) fn ready<K: Eq + std::hash::Hash>(
    reads: &[Vec<K>],
    writes: &[Vec<K>],
    left: &[usize],
) -> Vec<usize> {
    left.iter()
        .copied()
        .filter(|one| {
            let others: IndexSet<&K> =
                left.iter().filter(|other| *other != one).flat_map(|other| &reads[*other]).collect();
            !writes[*one].iter().any(|place| others.contains(place))
        })
        .collect()
}

type Rotation = (Vec<Arc<Insn>>, Vec<Arc<Insn>>);

/// One cycle out of `left`, broken without a free register: by exchanges
/// where the target can exchange every adjacent pair, else through the
/// machine stack.
///
/// A cycle `p1 <- p2 <- ... <- pn <- p1` is `xchg p1,p2` then `xchg p2,p3`
/// and so on. Otherwise save the place the closing move reads on the stack,
/// perform the remaining moves in order, then pop into the closing move's
/// destination.
fn _rotated(
    regs: Regs,
    left: &[Arc<Insn>],
    spare: Option<&Spare>,
) -> Result<Rotation, Malformed> {
    let mut writes: IndexMap<String, &Arc<Insn>> = IndexMap::default();
    for one in left {
        writes.insert(_into(one)?, one);
    }
    let start = &left[0];
    let mut cycle: Vec<Arc<Insn>> = vec![Arc::clone(start)];
    let mut place = _outof(start)?;
    while place != _into(start)? {
        let one = writes.get(&place).copied().filter(|one| !cycle.iter().any(|each| **each == ***one));
        let Some(one) = one else {
            return Err(Malformed(format!(
                "{:#06x} is in a copy group that is not a permutation of its places",
                start.at
            )));
        };
        cycle.push(Arc::clone(one));
        place = _outof(one)?;
    }

    let dest = |one: &Arc<Insn>| one.what.as_ref().expect("a move").dests[0].clone();
    let source = |one: &Arc<Insn>| one.what.as_ref().expect("a move").sources[0].clone();
    let operands: Vec<Loc> = cycle.iter().map(dest).collect();
    // One width across the whole chain: `_named` keys a register by its
    // root, so `mov ax,bx` and `mov ebx,eax` walk as one clean 2-cycle
    // whose exchange would be `xchg ax,ebx`.
    let widths: IndexSet<u32> = operands.iter().map(_width).collect();
    let pairs: Vec<(&Loc, &Loc)> = operands.iter().zip(operands.iter().skip(1)).collect();
    let exchanges = widths.len() == 1 && pairs.iter().all(|(one, other)| target::exchangeable(regs, one, other));
    let width = _width(&dest(&cycle[cycle.len() - 1]));
    // A dead register for the closing copy's source, where one is.
    let held = spare.and_then(|spare| {
        spare
            .free
            .iter()
            .find_map(
                |full| {
                    let register = regs.named(*full, i64::from(width));
                    (regs.width_of(register) == Some(i64::from(width))).then_some(Reg { register, width })
                },
            )
    });
    // Where the target prices the ways, the cheapest; else the exchanges, then
    // the stack.
    let route = match spare {
        Some(spare) => spare.prices.route(cycle.len(), exchanges, held.is_some()),
        None if exchanges => Route::Exchanges,
        None => Route::Stack,
    };
    if route == Route::Exchanges {
        let mut made: Vec<Arc<Insn>> = cycle
            .iter()
            .zip(&pairs)
            .map(|(one, (a, b))| {
                let mut exchange = (**one).clone();
                exchange.what = Some(semantics(
                    Operation::Exchange,
                    "xchg",
                    vec![(*a).clone(), (*b).clone()],
                    vec![(*b).clone(), (*a).clone()],
                ));
                exchange.group = None;
                Arc::new(exchange)
            })
            .collect();
        // The closing logical move contributes no machine instruction, but
        // it still defines the virtual value consumed after this edge.
        made.push(lir::anchor(Arc::clone(&cycle[cycle.len() - 1])));
        return Ok((made, cycle));
    }

    // Through the dead register: the closing move's source is saved in it
    // first, and every other move, in order, writes only what was read.
    if let (Route::Register, Some(held)) = (route, held) {
        let count = cycle.len();
        let (first, last) = (&cycle[0], &cycle[count - 1]);
        let mut saving = (**first).clone();
        saving.what = Some(semantics(Operation::Move, "mov", vec![Loc::Reg(held)], vec![source(last)]));
        saving.group = None;
        saving.defines = vec![];
        saving.uses = vec![];
        let mut restoring = (**last).clone();
        restoring.what = Some(semantics(Operation::Move, "mov", vec![dest(last)], vec![Loc::Reg(held)]));
        restoring.group = None;
        let mut made = vec![Arc::new(saving)];
        made.extend(cycle[..count - 1].iter().map(|one| ungrouped(one)));
        made.push(Arc::new(restoring));
        return Ok((made, cycle));
    }

    // Close the cycle on a move the stack can carry: its source is saved
    // first, and every other move, in order, writes only what was read.
    let closing = cycle
        .iter()
        .rposition(|one| {
            target::pushed_width(&source(one)).is_some() && target::popped_width(regs, &dest(one)).is_some()
        })
        .ok_or_else(|| {
            Malformed(format!("{:#06x} is in a copy cycle no move of which the stack can carry", start.at))
        })?;
    let count = cycle.len();
    cycle.rotate_left((closing + 1) % count);
    let (first, last) = (&cycle[0], &cycle[count - 1]);
    let mut made = vec![push_of(first, source(last))];
    made.extend(cycle[..count - 1].iter().map(|one| ungrouped(one)));
    made.push(pop_into(last, dest(last)));
    Ok((made, cycle))
}

/// `place` at the width the stack carries it: a frame cell's byte moves whole.
fn _stacked(place: Loc) -> Loc {
    match (&place, target::pushed_width(&place)) {
        (Loc::Mem(cell), Some(width)) => Loc::Mem(Mem { width, ..cell.clone() }),
        _ => place,
    }
}

fn push_of(
    start: &Insn,
    saved: Loc,
) -> Arc<Insn> {
    let mut push = start.clone();
    push.what = Some(semantics(Operation::Push, "push", vec![], vec![_stacked(saved)]));
    push.group = None;
    push.defines = vec![];
    push.uses = vec![];
    Arc::new(push)
}

fn pop_into(
    last: &Insn,
    into: Loc,
) -> Arc<Insn> {
    let mut pop = last.clone();
    pop.what = Some(semantics(Operation::Pop, "pop", vec![_stacked(into)], vec![]));
    pop.group = None;
    Arc::new(pop)
}

pub fn _into(one: &Insn) -> Result<String, Malformed> {
    _place(one, one.what.as_ref().map_or(&[][..], |what| what.dests.as_slice()))
}

pub fn _outof(one: &Insn) -> Result<String, Malformed> {
    _place(one, one.what.as_ref().map_or(&[][..], |what| what.sources.as_slice()))
}

/// The location an operand names, as one comparable thing.
fn _place(
    one: &Insn,
    where_: &[Loc],
) -> Result<String, Malformed> {
    let what = one.what.as_ref();
    if what.is_none_or(|what| what.op != Operation::Move || what.dests.len() != 1 || what.sources.len() != 1) {
        return Err(Malformed(format!("{:#06x} is in a copy group and is not a move", one.at)));
    }
    if where_.len() != 1 {
        return Err(Malformed(format!("{:#06x} names {} places on one side", one.at, where_.len())));
    }
    _named(&where_[0])
}

pub fn _named(one: &Loc) -> Result<String, Malformed> {
    match one {
        Loc::Reg(reg) => Ok(format!("r{}", (ir::root(reg.register)).index() as u32)),
        Loc::Mem(memory) => Ok(format!("m{}:{}:{}", memory.addr.repr(), memory.through.index() as u32, memory.offset)),
        Loc::Imm(imm) => Ok(format!("i{}", imm.value)),
        _ => Err(Malformed(format!("a copy group names {}, which is not a place", one.repr()))),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::{_into, _named, _outof, CyclePrices, Malformed, scheduled, scheduled_with};
    use crate::backend::verify;
    use crate::model::ir::{Addr, Loc, Mem, Operation, Reg, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn semantics(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Option<Semantics> {
        Some(Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) })
    }

    fn _move(
        into: Loc,
        out_of: Loc,
        group: Option<i64>,
        at: i64,
    ) -> Insn {
        let mut one =
            Insn::new(at, Some((at, at)), semantics(Operation::Move, "mov", vec![into], vec![out_of]), vec![], vec![]);
        one.group = group;
        one
    }

    fn grouped(
        into: Loc,
        out_of: Loc,
        group: i64,
    ) -> Insn {
        _move(into, out_of, Some(group), 0x100)
    }

    fn _reg(one: RegId) -> Loc {
        Loc::Reg(Reg { register: one, width: 2 })
    }

    fn reg(
        one: RegId,
        width: u32,
    ) -> Loc {
        Loc::Reg(Reg { register: one, width })
    }

    /// Python passes the string `f"[bp-{offset:#x}]"` as the address; a
    /// frame `Addr` names the same distinct place.
    fn _slot(offset: i64) -> Loc {
        Loc::Mem(Mem {
            through: RegId::BP,
            offset: 0,
            disp_width: 2,
            ..Mem::new(Some(Addr::new(Space::Frame, -offset)), 2)
        })
    }

    fn _other(at: i64) -> Insn {
        Insn::new(
            at,
            Some((at, at + 1)),
            semantics(Operation::Push, "push", vec![], vec![_reg(RegId::AX)]),
            vec![],
            vec![],
        )
    }

    fn _body(insns: Vec<Insn>) -> LirBody {
        let block = LirBlock::new(0, insns.into_iter().map(Arc::new).collect());
        LirBody::new("one", 0, vec![block.into()], IndexMap::default(), IndexMap::default())
    }

    fn name(one: &Insn) -> String {
        one.what.as_ref().unwrap().name.as_deref().map(str::to_owned).unwrap()
    }

    fn names(insns: &[Arc<Insn>]) -> Vec<String> {
        insns.iter().map(|one| name(one)).collect()
    }

    fn _order(body: &LirBody) -> Vec<String> {
        scheduled(body)
            .unwrap()
            .insns()
            .iter()
            .filter(|one| {
                let what = one.what.as_ref().unwrap();
                what.op != Operation::Nothing || what.name.as_deref().is_some_and(|name| !name.is_empty())
            })
            .map(|one| {
                if one.group.is_none() && one.what.as_ref().unwrap().op == Operation::Move {
                    format!("{}<-{}", _into(one).unwrap(), _outof(one).unwrap())
                } else {
                    name(one)
                }
            })
            .collect()
    }

    fn named(one: &Loc) -> String {
        _named(one).unwrap()
    }

    /// A cycle of copies of two widths (`mov si, cx` with `mov ecx, esi`) has
    /// no exchange: it went through the stack, a pop of four clocks on the
    /// 486, with bx dead and free (the loop-carried registers of mandel's
    /// latch). Priced by the target, a dead register carries it in three
    /// moves; where a pop is as cheap as a copy the stack does.
    #[test]
    fn test_a_cycle_of_two_widths_goes_through_a_dead_register_where_that_costs_less() {
        // The block goes on to one that writes bx before reading it.
        let mut first = LirBlock::new(
            0,
            vec![
                Arc::new(grouped(reg(RegId::SI, 2), reg(RegId::CX, 2), 1)),
                Arc::new(grouped(reg(RegId::CX, 4), reg(RegId::SI, 4), 1)),
            ],
        );
        first.succ = vec![1].into();
        let reader = Insn::new(
            1,
            Some((1, 2)),
            semantics(Operation::Move, "mov", vec![reg(RegId::BX, 2)], vec![reg(RegId::SI, 2)]),
            vec![],
            vec![],
        );
        let body = LirBody::new(
            "one",
            0,
            vec![first.into(), LirBlock::new(1, vec![Arc::new(reader)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let order = |pop: i64| {
            let done =
                scheduled_with(&body, Some(CyclePrices { copy: 1, exchange: 3, push: 1, pop }), &[RegId::BX], &[])
                    .unwrap();
            names(&done.blocks[0].insns)
        };
        assert_eq!(order(4), ["mov", "mov", "mov"]);
        assert_eq!(order(1), ["push", "mov", "pop"]);
        assert_eq!(names(&scheduled(&body).unwrap().blocks[0].insns), ["push", "mov", "pop"]);
    }

    /// mandel with the chain copy: a cycle of three registers of one width was
    /// two `xchg` every trip, priced as an add (1 clock) where the 486's is
    /// 3: a dead register carries it in four copies, 4 clocks for 6. Where
    /// the exchange costs one, the exchanges stay.
    #[test]
    fn test_a_cycle_of_one_width_is_exchanged_only_where_that_costs_less_than_a_dead_register() {
        let mut first = LirBlock::new(
            0,
            vec![
                Arc::new(grouped(reg(RegId::AX, 4), reg(RegId::CX, 4), 1)),
                Arc::new(grouped(reg(RegId::CX, 4), reg(RegId::DX, 4), 1)),
                Arc::new(grouped(reg(RegId::DX, 4), reg(RegId::AX, 4), 1)),
            ],
        );
        first.succ = vec![1].into();
        let reader = Insn::new(
            1,
            Some((1, 2)),
            semantics(Operation::Move, "mov", vec![reg(RegId::BX, 4)], vec![reg(RegId::SI, 4)]),
            vec![],
            vec![],
        );
        let body = LirBody::new(
            "one",
            0,
            vec![first.into(), LirBlock::new(1, vec![Arc::new(reader)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let order = |exchange: i64| {
            let prices = CyclePrices { copy: 1, exchange, push: 1, pop: 4 };
            names(&scheduled_with(&body, Some(prices), &[RegId::BX], &[]).unwrap().blocks[0].insns)
        };
        assert_eq!(order(3), ["mov", "mov", "mov", "mov"]);
        assert_eq!(order(1), ["xchg", "xchg", ""]);
    }

    #[test]
    fn test_memory_copy_expands_after_dependency_ordering() {
        // NESTED needs a spilled phi copied before another move overwrites its
        // source.
        let (source, destination) = (_slot(4), _slot(8));
        let result = scheduled(&_body(vec![
            grouped(source.clone(), _reg(RegId::AX), 1),
            grouped(destination.clone(), source.clone(), 1),
        ]))
        .unwrap();
        let instructions = &result.blocks[0].insns;
        assert_eq!(names(instructions), ["push", "pop", "mov"]);
        assert_eq!(instructions[0].what.as_ref().unwrap().sources, vec![source]);
        assert_eq!(instructions[1].what.as_ref().unwrap().dests, vec![destination]);
        assert!(instructions.iter().all(|one| one.group.is_none()));
    }

    /// sc.c's `sc_alloc` spilled a byte phi: its slot copy was refused as
    /// "memory parallel copy needs equal-width frame slots".
    #[test]
    fn test_a_byte_slot_copy_moves_its_word() {
        let byte = |offset: i64| match _slot(offset) {
            Loc::Mem(cell) => Loc::Mem(Mem { width: 1, ..cell }),
            _ => unreachable!(),
        };
        let result = scheduled(&_body(vec![grouped(byte(0x1e), byte(0x1a), 1)])).unwrap();
        let instructions = &result.blocks[0].insns;
        assert_eq!(names(instructions), ["push", "pop"]);
        assert_eq!(instructions[0].what.as_ref().unwrap().sources, vec![_slot(0x1a)]);
        assert_eq!(instructions[1].what.as_ref().unwrap().dests, vec![_slot(0x1e)]);
    }

    #[test]
    fn test_a_move_goes_after_everything_that_reads_what_it_writes() {
        // pressx's own shape: the reload into r24 must come last.
        let got =
            _order(&_body(vec![grouped(_reg(RegId::DI), _slot(8), 1), grouped(_reg(RegId::BP), _reg(RegId::DI), 1)]));
        let (di, bp) = (named(&_reg(RegId::DI)), named(&_reg(RegId::BP)));
        let slot = named(&_slot(8));
        assert_eq!(got, [format!("{bp}<-{di}"), format!("{di}<-{slot}")]);
    }

    #[test]
    fn test_a_move_of_a_place_into_itself_is_dropped() {
        let got = _order(&_body(vec![
            grouped(_reg(RegId::AX), _reg(RegId::AX), 1),
            grouped(_reg(RegId::BX), _reg(RegId::CX), 1),
        ]));
        assert_eq!(got, [format!("{}<-{}", named(&_reg(RegId::BX)), named(&_reg(RegId::CX)))]);
    }

    #[test]
    fn test_what_is_not_in_a_group_keeps_its_place() {
        let got = _order(&_body(vec![_other(0x100), grouped(_reg(RegId::AX), _reg(RegId::CX), 1), _other(0x300)]));
        let moved = format!("{}<-{}", named(&_reg(RegId::AX)), named(&_reg(RegId::CX)));
        assert_eq!(got, ["push".to_owned(), moved, "push".to_owned()]);
    }

    #[test]
    fn test_two_groups_are_scheduled_apart() {
        // One group's move may write what another's reads; they are not
        // simultaneous.
        let got =
            _order(&_body(vec![grouped(_reg(RegId::DI), _slot(8), 1), grouped(_reg(RegId::BP), _reg(RegId::DI), 2)]));
        let (di, bp) = (named(&_reg(RegId::DI)), named(&_reg(RegId::BP)));
        assert_eq!(got, [format!("{di}<-{}", named(&_slot(8))), format!("{bp}<-{di}")]);
    }

    #[test]
    fn test_register_and_spilled_cycles_are_preserved() {
        // A register cycle uses xchg; a slot cycle uses the balanced machine
        // stack.
        let swapped = scheduled(&_body(vec![
            grouped(_reg(RegId::AX), _reg(RegId::CX), 1),
            grouped(_reg(RegId::CX), _reg(RegId::AX), 1),
        ]))
        .unwrap()
        .blocks[0]
            .insns
            .clone();
        let machine: Vec<String> = swapped
            .iter()
            .filter(|one| one.what.as_ref().unwrap().op != Operation::Nothing)
            .map(|one| name(one))
            .collect();
        assert_eq!(machine, ["xchg"]);
        assert_eq!(swapped[swapped.len() - 1].what.as_ref().unwrap().op, Operation::Nothing);
        let spilled =
            scheduled(&_body(vec![grouped(_slot(4), _slot(8), 1), grouped(_slot(8), _slot(4), 1)])).unwrap().blocks[0]
                .insns
                .clone();
        assert_eq!(names(&spilled), ["push", "push", "pop", "pop"]);
        assert_eq!(spilled[0].what.as_ref().unwrap().sources, vec![_slot(4)]);
        assert_eq!(spilled[spilled.len() - 1].what.as_ref().unwrap().dests, vec![_slot(8)]);
    }

    #[test]
    fn test_register_cycle_retains_every_virtual_definition() {
        // R_WALK stopped at parcopy: values 11 and 12 were read but undefined.
        let mut first = grouped(_reg(RegId::AX), _reg(RegId::CX), 1);
        first.defines = vec![11];
        first.uses = vec![1];
        let mut closing = grouped(_reg(RegId::CX), _reg(RegId::AX), 1);
        closing.defines = vec![12];
        closing.uses = vec![2];
        let consumer = Insn::new(
            0x102,
            Some((0x102, 0x102)),
            semantics(Operation::Push, "push", vec![], vec![_reg(RegId::CX)]),
            vec![],
            vec![12],
        );
        let mut body = _body(vec![first, closing, consumer]);
        body.inputs = BTreeSet::from([1, 2]);

        let got = scheduled(&body).unwrap();

        assert_eq!(verify::verify(&got, false), Vec::<String>::new());
        let defined: BTreeSet<u32> = got.iter_insns().flat_map(|one| one.defines.clone()).collect();
        assert_eq!(defined, BTreeSet::from([11, 12]));
    }

    #[test]
    fn test_register_cycle_retains_covered_bytes_as_an_anchor() {
        // sieve's shared array base made a three-register phi cycle whose final
        // move owned original bytes.
        let mut closing = grouped(_reg(RegId::CX), _reg(RegId::DX), 1);
        closing.covers = Some((0x100, 0x102));
        let body = _body(vec![
            grouped(_reg(RegId::DX), _reg(RegId::SI), 1),
            grouped(_reg(RegId::SI), _reg(RegId::CX), 1),
            closing,
        ]);

        let got = scheduled(&body).unwrap().blocks[0].insns.clone();

        assert_eq!(names(&got), ["xchg", "xchg", ""]);
        assert_eq!(got[got.len() - 1].what.as_ref().unwrap().op, Operation::Nothing);
        assert_eq!(got[got.len() - 1].covers, Some((0x100, 0x102)));
    }

    /// `conc7` at `-march=core2` was refused ("need a temporary", #106): a slot
    /// written from edx and read back into dx as a word is a two-place cycle
    /// no exchange takes, since the places are read at two widths.
    #[test]
    fn test_a_slot_written_as_a_dword_and_read_as_a_word_cycles_through_the_stack() {
        let dword_slot = Loc::Mem(Mem {
            width: 4,
            ..match _slot(32) {
                Loc::Mem(cell) => cell,
                _ => unreachable!(),
            }
        });
        let body =
            _body(vec![grouped(dword_slot.clone(), reg(RegId::EDX, 4), 1), grouped(reg(RegId::DX, 2), _slot(32), 1)]);
        let got = scheduled(&body).unwrap().blocks[0].insns.clone();
        assert_eq!(names(&got), ["push", "mov", "pop"]);
        assert_eq!(got[0].what.as_ref().unwrap().sources, vec![_slot(32)], "the word the closing move reads is saved");
        assert_eq!(got[2].what.as_ref().unwrap().dests, vec![reg(RegId::DX, 2)]);
    }

    /// No `xchg` names a segment register.
    #[test]
    fn test_a_cycle_through_a_segment_register_goes_through_the_stack() {
        let body = _body(vec![grouped(reg(RegId::ES, 2), _slot(4), 1), grouped(_slot(4), reg(RegId::ES, 2), 1)]);
        let got = scheduled(&body).unwrap().blocks[0].insns.clone();
        assert_eq!(names(&got), ["push", "mov", "pop"]);
        assert_eq!(got[0].what.as_ref().unwrap().sources, vec![reg(RegId::ES, 2)]);
    }

    #[test]
    fn test_mixed_width_register_cycle_uses_a_balanced_temporary() {
        // sieve rotates EDX->CX->SI->EDX without exchanging incompatible
        // register widths.
        let body = _body(vec![
            grouped(reg(RegId::EDX, 4), reg(RegId::ESI, 4), 1),
            grouped(reg(RegId::CX, 2), reg(RegId::DX, 2), 1),
            grouped(reg(RegId::SI, 2), reg(RegId::CX, 2), 1),
        ]);

        let got = scheduled(&body).unwrap().blocks[0].insns.clone();

        assert_eq!(names(&got), ["push", "mov", "mov", "pop"]);
        assert_eq!(got[0].what.as_ref().unwrap().sources, vec![reg(RegId::DX, 2)]);
        assert_eq!(got[got.len() - 1].what.as_ref().unwrap().dests, vec![reg(RegId::CX, 2)]);
    }

    #[test]
    fn test_a_mixed_width_cycle_closing_on_a_byte_saves_a_word() {
        // rcflip's AX <- CX, CL <- AL was refused: its temporary was `push al`.
        let body = _body(vec![
            grouped(reg(RegId::AX, 2), reg(RegId::CX, 2), 1),
            grouped(reg(RegId::CL, 1), reg(RegId::AL, 1), 1),
        ]);

        let got = scheduled(&body).unwrap().blocks[0].insns.clone();

        assert_eq!(names(&got), ["push", "mov", "pop"]);
        assert_eq!(got[0].what.as_ref().unwrap().sources, vec![reg(RegId::CX, 2)]);
        assert_eq!(got[1].what.as_ref().unwrap().dests, vec![reg(RegId::CL, 1)]);
        assert_eq!(got[2].what.as_ref().unwrap().dests, vec![reg(RegId::AX, 2)]);
    }

    #[test]
    fn named_places_match_python() {
        // Expected strings printed by Python's parcopy._named.
        use crate::model::ir::Imm;
        assert_eq!(named(&_reg(RegId::DI)), "r44");
        assert_eq!(named(&_reg(RegId::BP)), "r42");
        assert_eq!(named(&_slot(8)), "m[bp-0x8]:26:0");
        assert_eq!(named(&Loc::Imm(Imm { value: -3, width: 2, address: None })), "i-3");
    }

    #[test]
    fn test_something_that_is_not_a_move_in_a_group_is_refused() {
        let mut other = _other(0x100);
        other.group = Some(1);
        assert!(matches!(scheduled(&_body(vec![other])), Err(Malformed(_))));
    }

    /// PHIElimination makes `implicit_def`s the allocator gives registers; one
    /// left in the body after allocation printed as an instruction no
    /// assembler knows, and one simply dropped left its readers reading a value
    /// nothing defines (jpeg_fixed and desk.nib: "value is read but never
    /// defined"). The definition goes and the readers read it as undef.
    #[test]
    fn test_an_implicit_def_is_gone_after_allocation_and_its_readers_read_undef() {
        let held = |value: u32| Loc::Held(crate::model::ir::Held { value, width: 4 });
        let undef = Insn::new(
            0,
            Some((0, 0)),
            semantics(Operation::Nothing, crate::processimpdefs::IMPLICIT_DEF, vec![held(1)], vec![]),
            vec![1],
            vec![],
        );
        let reader = Insn::new(
            1,
            Some((1, 1)),
            semantics(
                Operation::Binary,
                "add",
                vec![held(2)],
                vec![held(1), Loc::Imm(crate::model::ir::Imm { value: 3, width: 4, address: None })],
            ),
            vec![2],
            vec![1],
        );
        let body = LirBody::new(
            "expanded",
            0,
            vec![LirBlock::new(0, vec![Arc::new(undef), Arc::new(reader)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let got = super::expanded(&body);
        let names: Vec<String> =
            got.iter_insns().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
        assert_eq!(names, ["add"]);
        let add = &got.insns()[0];
        assert!(add.uses.is_empty() && add.undefs == [1], "uses {:?} undefs {:?}", add.uses, add.undefs);
    }
}
