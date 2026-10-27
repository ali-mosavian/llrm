//! Moves a register copy, or a spill store, into the one successor that needs
//! it.
//!
//! # How it works
//! In a block that ends in a branch, a copy `mov d, s` whose result `d` is live
//! into only one successor is dead on the path to the others. The copy moves to
//! the head of that successor:
//!
//! ```text
//!  before                        after
//!  B: mov edi, edx               B: test eax, eax
//!     test eax, eax                 jne body
//!     jne body                   ret_path: (nothing reads EDI)
//!  ret_path: ret                 body: mov edi, edx ...
//!  body: ... reads edi
//! ```
//!
//! Blocks are taken once, bottom-up within a block, in reverse post-order, as
//! LLVM's `PostRAMachineSinking` does, so a copy that lands in a block is seen
//! when that block's turn comes, and later copies land ahead of earlier ones.
//! Liveness of register lanes is computed once for the body (`live_in`).
//!
//! A copy moves only when every one of these holds: it writes lanes it does not
//! read; exactly one successor has its result live; that successor is entered
//! only from this block and by one edge; and no instruction between the copy
//! and the branch touches (reads or writes) the copy's result or source
//! (`touches`).
//!
//! A spill store `mov [slot], r` (a plain store to a fixed frame cell) moves by
//! the same rule, with cells instead of registers: the one successor whose
//! reachable blocks may read the slot (`Reads`, cell liveness worked out once
//! for the function) receives it, provided nothing after the store in its block
//! may read or overwrite the cell or touch `r`. A target entered from elsewhere
//! too, such as a loop header, takes no store.
//!
//! The point is shrink wrapping. A path that returns at once then has no copy
//! and no store of a saved register, and the prologue can be placed after the
//! test, as gcc and LLVM do. A store of a slot only one successor reads can be
//! the only thing that made a frame necessary on the other path.
//!
//! `latched` is the other way round: a block of copies alone on a back edge,
//! whose one predecessor branches to it and elsewhere, has them run before that
//! branch when the other way out does not read what they write, and the edge
//! block keeps only its jump (a back edge's `mov; jmp` becomes a `jl`).
//!
//! # Example
//! A function copies its argument into `DI`, then either returns at once or
//! loops reading `DI`.
//!
//! Schematic, from the fixture of test
//! `test_a_copy_only_one_successor_reads_moves_into_it` (not printer output):
//! Before:
//!
//! ```lir
//! block 0x0001
//!   0x0001  DI := mov DX
//!   0x0002  cmp AX, BX
//!   0x0003  jl
//! block 0x0005
//!   0x0005  ret
//! block 0x0009
//!   0x0009  AX := add AX, DI
//!   0x000a  jmp
//! ```
//!
//! After:
//!
//! ```lir
//! block 0x0001
//!   0x0002  cmp AX, BX
//!   0x0003  jl
//! block 0x0005
//!   0x0005  ret
//! block 0x0009
//!   0x0001  DI := mov DX
//!   0x0009  AX := add AX, DI
//!   0x000a  jmp
//! ```
//!
//! Only block 0x0009 reads `DI` and has no other predecessor, so the copy moves
//! there and the path through 0x0005 runs without it.
//!
//! From unit test `test_a_copy_only_one_successor_reads_moves_into_it` in
//! `postrasink.rs`, which asserts the copy count per block (0 in 0x0001, 1 in
//! 0x0009).
//!
//! # Runs
//! A sub-pass of [`crate::backend::peephole`], after `copysink`. Once per
//! peephole run. `latched` follows it.
//!
//! # Keeps
//! The body is returned unchanged where no block with several successors holds
//! a copy or spill store.
//!
//! # Deviations from the pin
//! The copy half is `PostRAMachineSinkingImpl::tryToSinkCopy`; a copy moves
//! when no later instruction of the block touches what it reads or writes and
//! one successor, entered from this block alone, has its result live in.
//! - DEVIATE P1: the spill-store half (a store to a frame cell sunk by cell
//!   liveness, for shrink wrapping) has no counterpart in the pin, where
//!   `ShrinkWrap` places the prologue. Exit: ShrinkWrap translated.
//! - DEVIATE P2: `latched` has no counterpart in the pin (x_cordic `-Os`).
//!   Exit: the coalescer or block placement keeps the copy off the back edge.
//! - Blocks are walked in reverse post-order and past a call, not the pin's
//!   layout order with a call stopping a block: the pin's way gives 7 different
//!   objects of 556 (net +4 B, 2 larger), so ours stays.
//!
//! # See also
//! - LLVM `PostRAMachineSinking`:
//!   <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L2243>

use std::sync::Arc;

use crate::backend::copysink::{copy_of, touches};
use crate::backend::peephole::{_lanes, Lanes};
use crate::model::ir::{Loc, Mem, Operation, Reg};
use crate::model::lir::{Block, Insn, LirBody};
use crate::support::hash::IndexMap;

/// The lanes live on entry to each block: the answer the clean-up already
/// holds for the body, not another solve.
fn live_in(body: &LirBody) -> IndexMap<i64, Lanes> {
    crate::backend::liveness::live_into(body).0
}

/// Blocks reachable from the entry, each before the successors it does not come
/// back to (reverse post-order).
fn reverse_post_order(body: &LirBody) -> Vec<i64> {
    let succ: IndexMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &*block.succ)).collect();
    let mut seen = crate::support::hash::HashSet::default();
    let mut order = Vec::new();
    let mut stack = vec![(body.entry, 0usize)];
    seen.insert(body.entry);
    while let Some((at, next)) = stack.pop() {
        match succ.get(&at).and_then(|to| to.get(next)) {
            Some(&to) => {
                stack.push((at, next + 1));
                if seen.insert(to) {
                    stack.push((to, 0));
                }
            }
            None => order.push(at),
        }
    }
    order.reverse();
    order
}

#[cfg(test)]
thread_local! {
    /// Blocks whose instructions were read to find who reads a spill cell.
    static SCANNED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The register a plain spill store writes to a fixed frame cell, and the cell.
pub(crate) fn spill_store_of(one: &Insn) -> Option<(Reg, Mem)> {
    if !one.spill_store || !one.clobbers.is_empty() || !one.requires.is_empty() || !one.delivers.is_empty() {
        return None;
    }
    let what = one.what.as_ref()?;
    match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Move, Some("mov"), [Loc::Mem(cell)], [Loc::Reg(source)])
            if cell.addr.is_some() && cell.base.is_none() && cell.index.is_none() && cell.selector.is_none() =>
        {
            Some((*source, cell.clone()))
        }
        _ => None,
    }
}

/// Whether `one` may read a byte of `cell`: any mention but a plain store to
/// it. An address of the frame taken may be read through.
pub(crate) fn may_read(
    one: &Insn,
    cell: &Mem,
) -> bool {
    let Some(what) = one.what.as_ref() else { return false };
    let reads = |place: &Loc| match place {
        Loc::Mem(other) => !crate::backend::storedhomes::apart(cell, other),
        Loc::Address(_) => true,
        _ => false,
    };
    let plain_store =
        what.op == Operation::Move && what.dests.len() == 1 && what.sources.iter().all(|place| !reads(place));
    what.sources.iter().any(reads) || (!plain_store && what.dests.iter().any(reads))
}

/// Whether `one` may write a byte of `cell`: a store sunk past it would land
/// over the newer value.
pub(crate) fn overwrites(
    one: &Insn,
    cell: &Mem,
) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| what.dests
                .iter()
                .any(
                    |place| match place {
                        Loc::Mem(other) => !crate::backend::storedhomes::apart(cell, other),
                        Loc::Address(_) => true,
                        _ => false,
                    },
                ),
        )
}

/// Which spill cells each block, and the blocks reachable from it, may read:
/// liveness over cells, worked out once for the function. `read_beyond` asked
/// it per store and successor by a walk of everything reachable, which grew
/// with the square of the stores.
struct Reads {
    /// The cells of the spill stores, as the byte ranges they cover.
    keys: IndexMap<(i64, i64), usize>,
    /// Per block, a bit for each cell and one past them for "any cell".
    later: IndexMap<i64, Vec<u64>>,
}

impl Reads {
    fn of(body: &LirBody) -> Self {
        let mut keys: IndexMap<(i64, i64), usize> = IndexMap::default();
        for one in body.blocks.iter().flat_map(|block| &block.insns) {
            if let Some((_, cell)) = spill_store_of(one) {
                let low = cell.addr.map_or(0, |addr| addr.disp);
                let next = keys.len();
                keys.entry((low, low + i64::from(cell.width))).or_insert(next);
            }
        }
        let any = keys.len();
        let words = (any + 1).div_ceil(64);
        let sorted: Vec<((i64, i64), usize)> = {
            let mut sorted: Vec<_> = keys.iter().map(|(range, id)| (*range, *id)).collect();
            sorted.sort();
            sorted
        };
        let widest = sorted.iter().map(|((low, high), _)| high - low).max().unwrap_or(0);
        // Whether the frame and another fixed space may overlap is a question
        // of the two spaces, not of the offsets.
        let mut across: IndexMap<(crate::objectfile::module::Space, i64), bool> = IndexMap::default();
        let proxy =
            body.blocks.iter().flat_map(|block| &block.insns).find_map(|one| spill_store_of(one)).map(|(_, cell)| cell);
        let mut reading: IndexMap<i64, Vec<u64>> = IndexMap::default();
        for block in &body.blocks {
            #[cfg(test)]
            SCANNED.with(|scanned| scanned.set(scanned.get() + 1));
            let mut bits = vec![0u64; words];
            let set = |id: usize, bits: &mut Vec<u64>| bits[id / 64] |= 1 << (id % 64);
            for one in &block.insns {
                let Some(what) = one.what.as_ref() else { continue };
                // A plain store to a cell reads nothing of it.
                let stores = what.op == Operation::Move && what.dests.len() == 1;
                let places = what.sources.iter().chain(if stores { [].iter() } else { what.dests.iter() });
                for place in places {
                    match place {
                        Loc::Address(_) => set(any, &mut bits),
                        Loc::Mem(other) => {
                            let fixed = other.base.is_none()
                                && other.index.is_none()
                                && other.selector.is_none()
                                && other.addr.is_some();
                            match other.addr.filter(|_| fixed) {
                                Some(addr) if addr.space == crate::objectfile::module::Space::Frame => {
                                    let (low, high) = (addr.disp, addr.disp + i64::from(other.width));
                                    let from = sorted.partition_point(|((start, _), _)| *start < low - widest);
                                    for ((start, end), id) in &sorted[from..] {
                                        if *start >= high {
                                            break;
                                        }
                                        if *end > low {
                                            set(*id, &mut bits);
                                        }
                                    }
                                }
                                Some(addr) => {
                                    let overlaps = *across
                                        .entry((addr.space, addr.index))
                                        .or_insert_with(
                                            || proxy
                                                .as_ref()
                                                .is_none_or(|cell| !crate::backend::storedhomes::apart(cell, other)),
                                        );
                                    if overlaps {
                                        set(any, &mut bits);
                                    }
                                }
                                None => set(any, &mut bits),
                            }
                        }
                        _ => {}
                    }
                }
            }
            reading.insert(block.at, bits);
        }
        // Backwards to a fixed point: what a block reads, and what is read
        // after it.
        let succ: IndexMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &*block.succ)).collect();
        let mut later = reading;
        let order: Vec<i64> = reverse_post_order(body).into_iter().rev().collect();
        loop {
            let mut changed = false;
            for at in &order {
                let mut bits = later[at].clone();
                for to in succ[at] {
                    if let Some(after) = later.get(to) {
                        for (word, other) in bits.iter_mut().zip(after) {
                            *word |= other;
                        }
                    }
                }
                if bits != later[at] {
                    later.insert(*at, bits);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Self { keys, later }
    }

    /// Whether the blocks from `from` on may read `cell`.
    fn beyond(
        &self,
        from: i64,
        cell: &Mem,
    ) -> bool {
        let Some(bits) = self.later.get(&from) else { return false };
        let low = cell.addr.map_or(0, |addr| addr.disp);
        let id = self.keys[&(low, low + i64::from(cell.width))];
        let has = |bit: usize| bits[bit / 64] >> (bit % 64) & 1 == 1;
        has(id) || has(self.keys.len())
    }
}

/// `body` with each copy moved into the one successor that reads it. One pass
/// over the blocks, as LLVM makes: a copy that lands in a block is seen when
/// that block's turn comes, and reverse post-order puts it after the block it
/// came from. Liveness is worked out once; a copy moved into a block changes
/// what is live into that block, not what is live into its successors, which is
/// what the next step asks.
pub fn sunk(body: &LirBody) -> LirBody {
    let regs = body.regs();
    if !body.blocks.iter().any(|block| {
        block.succ.len() > 1 && block.insns.iter().any(|one| copy_of(one).is_some() || spill_store_of(one).is_some())
    }) {
        return body.clone();
    }
    let live = live_in(body);
    let reads = std::cell::OnceCell::new();
    let mut preds: IndexMap<i64, usize> = IndexMap::default();
    for block in &body.blocks {
        for to in &block.succ {
            *preds.entry(*to).or_insert(0) += 1;
        }
    }
    let succ: IndexMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &*block.succ)).collect();
    let mut insns: IndexMap<i64, Vec<Arc<Insn>>> =
        body.blocks.iter().map(|block| (block.at, block.insns.to_vec())).collect();
    let mut changed = false;
    for at in reverse_post_order(body) {
        let to = succ[&at];
        if to.len() < 2 {
            continue;
        }
        for index in (0..insns[&at].len()).rev() {
            let one = Arc::clone(&insns[&at][index]);
            if let Some((source, cell)) = spill_store_of(&one) {
                let read = _lanes(regs, source.register);
                let readers: Vec<i64> = to
                    .iter()
                    .copied()
                    .filter(|next| reads.get_or_init(|| Reads::of(body)).beyond(*next, &cell))
                    .collect();
                let [target] = readers[..] else { continue };
                if target == at
                    || !insns.contains_key(&target)
                    || to.iter().filter(|next| **next == target).count() != 1
                    || insns[&at][index + 1..].iter().any(|other| {
                        touches(regs, body.bits, other, &read, false)
                            || may_read(other, &cell)
                            || overwrites(other, &cell)
                    })
                {
                    continue;
                }
                // A target entered from elsewhere too (a loop's header) takes
                // no store: the bridge block it needed costs a jump (jpeg_*
                // +8 B, bintree.nib +7 B at -O3), and a store left in the
                // branching block was no worse.
                if preds.get(&target) != Some(&1) {
                    continue;
                }
                insns.get_mut(&at).expect("a block").remove(index);
                insns.get_mut(&target).expect("a block").insert(0, one);
                changed = true;
                continue;
            }
            let Some((dest, source)) = copy_of(&one) else { continue };
            let (written, read) = (_lanes(regs, dest.register), _lanes(regs, source.register));
            if written.is_empty() || read.is_empty() || !written.is_disjoint(&read) {
                continue;
            }
            let readers: Vec<i64> = to
                .iter()
                .copied()
                .filter(|next| live.get(next).is_some_and(|lanes| !written.is_disjoint(lanes)))
                .collect();
            let [target] = readers[..] else { continue };
            if target == at
                || !insns.contains_key(&target)
                || preds.get(&target) != Some(&1)
                || to.iter().filter(|next| **next == target).count() != 1
            {
                continue;
            }
            let both = written.or(&read);
            if insns[&at][index + 1..].iter().any(|other| {
                touches(regs, body.bits, other, &both, false) || touches(regs, body.bits, other, &written, true)
            }) {
                continue;
            }
            insns.get_mut(&at).expect("a block").remove(index);
            // Bottom-up, so each earlier copy lands ahead of the later ones
            // already there.
            insns.get_mut(&target).expect("a block").insert(0, one);
            changed = true;
        }
    }
    if !changed {
        return body.clone();
    }
    let blocks: Vec<Block> = body
        .blocks
        .iter()
        .map(|block| {
            let now = &insns[&block.at];
            if now.len() == block.insns.len()
                && now.iter().zip(&block.insns).all(|(left, right)| Arc::ptr_eq(left, right))
            {
                block.clone()
            } else {
                block.with_insns(now.clone())
            }
        })
        .collect();
    body.with_blocks(blocks)
}

/// A block of copies alone on a back edge, entered from the one block that
/// branches to it and left by a jump, has its copies run in that block before
/// the branch where the other way out does not read what they write (`x_cordic
/// -Os`: the back edge's `mov eax,ebx; jmp` on 2,200 trips, a taken jump more
/// than the loop that had the copy in its arms). The edge block is left holding
/// its jump, which the layout drops.
pub fn latched(body: &LirBody) -> LirBody {
    let regs = body.regs();
    let mut preds: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        for to in &block.succ {
            preds.entry(*to).or_default().push(block.at);
        }
    }
    let edge_copies = |block: &Block| -> Option<Vec<Arc<Insn>>> {
        let [to] = block.succ[..] else { return None };
        let mut copies = Vec::new();
        let mut jumped = false;
        for one in block.insns.iter() {
            if crate::backend::peephole::_skippable_nothing(one) {
                continue;
            }
            if jumped {
                return None;
            }
            if copy_of(one).is_some() {
                copies.push(Arc::clone(one));
            } else if one.what.as_ref().is_some_and(|what| what.op == Operation::Jump) && to != block.at {
                jumped = true;
            } else {
                return None;
            }
        }
        (jumped && !copies.is_empty()).then_some(copies)
    };
    let mut live = None;
    let mut busy = None;
    let mut natural = None;
    let mut moves: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut emptied: Vec<i64> = Vec::new();
    for edge in &body.blocks {
        let Some(copies) = edge_copies(edge) else { continue };
        let [from] = preds.get(&edge.at).map(Vec::as_slice).unwrap_or_default() else { continue };
        let Some(source) = body.blocks.iter().find(|block| block.at == *from) else { continue };
        let [first, second] = source.succ[..] else { continue };
        let other = if first == edge.at { second } else { first };
        if (first != edge.at && second != edge.at) || other == edge.at || other == source.at || other == edge.succ[0] {
            continue;
        }
        // The branch reads the flags (and a `loop` or `jcxz` a counter): the
        // copies go before it and touch no flag.
        let Some(position) = source.insns.iter().position(|one| {
            one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Branch | Operation::Jump))
        }) else {
            continue;
        };
        let ends_plainly = source.insns[position..].iter().all(|one| {
            crate::backend::peephole::_skippable_nothing(one)
                || one.what.as_ref().is_some_and(|what| {
                    matches!(what.op, Operation::Branch | Operation::Jump)
                        && what.name.as_deref().is_some_and(|name| !name.contains("cxz") && !name.starts_with("loop"))
                })
        });
        if !ends_plainly || moves.contains_key(from) {
            continue;
        }
        let live = live.get_or_insert_with(|| live_in(body));
        let wanted = &live[&other];
        let dead = copies
            .iter()
            .all(
                |one| {
                    let (dest, _) = copy_of(one).expect("a copy");
                    _lanes(regs, dest.register).iter().all(|lane| !wanted.contains(lane))
                },
            );
        if !dead {
            continue;
        }
        // Only a back edge needs its jump: any other edge block the layout puts
        // before its target.
        let natural = natural.get_or_insert_with(|| crate::analysis::loops::loops(&body.blocks, Some(body.entry)));
        if !natural.iter().any(|one| one.header == edge.succ[0] && one.body.contains(&edge.at)) {
            continue;
        }
        // The other way out then runs the copies for nothing: only where it
        // runs less than the edge saves a jump.
        let busy = busy.get_or_insert_with(|| crate::analysis::frequency::Frequency::of(body));
        if busy.block(other) * copies.len() as f64 >= busy.block(edge.at) * 2.0 {
            continue;
        }
        moves.insert(*from, copies);
        emptied.push(edge.at);
    }
    if moves.is_empty() {
        return body.clone();
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            if let Some(copies) = moves.get(&block.at) {
                let mut insns = block.insns.to_vec();
                let at = insns
                    .iter()
                    .position(|one| {
                        one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Branch | Operation::Jump))
                    })
                    .expect("a terminator");
                insns.splice(at..at, copies.iter().cloned());
                return block.with_insns(insns);
            }
            if emptied.contains(&block.at) {
                let kept = block.insns.iter().filter(|one| copy_of(one).is_none()).cloned().collect();
                return block.with_insns(kept);
            }
            block.clone()
        })
        .collect();
    body.with_blocks(blocks)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    const AX: RegId = RegId::AX;
    const BX: RegId = RegId::BX;
    const DI: RegId = RegId::DI;
    const DX: RegId = RegId::DX;
    use llrm_lir::registers::RegId;

    use super::{latched, sunk};
    use crate::model::ir::{Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn r(register: RegId) -> Loc {
        Loc::Reg(Reg { register, width: 2 })
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        target: Option<i64>,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(Semantics { name: Some(name.into()), dests, sources, target, ..Semantics::new(op) }),
            vec![],
            vec![],
        ))
    }

    fn copy(
        at: i64,
        dest: RegId,
        source: RegId,
    ) -> Arc<Insn> {
        insn(at, Operation::Move, "mov", vec![r(dest)], vec![r(source)], None)
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
    ) -> LirBlock {
        LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }
    }

    /// `mov di, dx; cmp ax, bx; jl 9`, the loop at 9 reading DI, the exit at 5
    /// not: the shape of a function that leaves at once or works with a
    /// copy of its argument.
    fn shape(
        exit_reads: bool,
        body_has_two_predecessors: bool,
    ) -> LirBody {
        let cmp = insn(2, Operation::Compare, "cmp", vec![], vec![r(RegId::from(AX)), r(RegId::from(BX))], None);
        let branch = insn(3, Operation::Branch, "jl", vec![], vec![], Some(9));
        // A generated return reads only what it requires and the epilogue: DI
        // is not among them.
        let ret = Arc::new(Insn {
            reads_complete: true,
            ..Arc::unwrap_or_clone(insn(5, Operation::Return, "ret", vec![], vec![], None))
        });
        let use_di = |at| {
            insn(
                at,
                Operation::Binary,
                "add",
                vec![r(RegId::from(AX))],
                vec![r(RegId::from(AX)), r(RegId::from(DI))],
                None,
            )
        };
        let mut blocks = vec![
            block(1, vec![copy(1, DI, DX), cmp, branch], vec![9, 5]),
            block(5, if exit_reads { vec![use_di(5), ret] } else { vec![ret] }, vec![]),
        ];
        let tail = if body_has_two_predecessors {
            vec![
                use_di(9),
                insn(10, Operation::Compare, "cmp", vec![], vec![r(RegId::from(AX)), r(RegId::from(BX))], None),
                insn(11, Operation::Branch, "jl", vec![], vec![], Some(9)),
            ]
        } else {
            vec![use_di(9), insn(10, Operation::Jump, "jmp", vec![], vec![], Some(5))]
        };
        blocks.push(block(9, tail, if body_has_two_predecessors { vec![9, 5] } else { vec![5] }));
        LirBody::new("f", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default())
    }

    fn copies_in(
        body: &LirBody,
        at: i64,
    ) -> usize {
        body.blocks
            .iter()
            .find(|block| block.at == at)
            .unwrap()
            .insns
            .iter()
            .filter(|one| one.what.as_ref().unwrap().name.as_deref() == Some("mov"))
            .count()
    }

    /// The copy of a saved register's argument ran on the path that returns at
    /// once, where nothing reads it, and kept the prologue from moving past
    /// the test (hanoi, fib: -7% instructions).
    #[test]
    fn test_a_copy_only_one_successor_reads_moves_into_it() {
        let body = shape(false, false);
        let after = sunk(&body);
        assert_eq!((copies_in(&after, 1), copies_in(&after, 9)), (0, 1));
    }

    #[test]
    fn test_a_copy_both_successors_read_stays() {
        let after = sunk(&shape(true, false));
        assert_eq!((copies_in(&after, 1), copies_in(&after, 9)), (1, 0));
    }

    #[test]
    fn test_a_copy_does_not_move_into_a_block_with_another_predecessor() {
        let after = sunk(&shape(false, true));
        assert_eq!((copies_in(&after, 1), copies_in(&after, 9)), (1, 0));
    }

    /// One pass sinks a chain: the block a copy lands in is visited after the
    /// one it left (rectwo c -15% needed it; a repeat bought the same at
    /// 0.3% of compile time).
    #[test]
    fn test_a_copy_goes_on_through_a_second_branch_in_one_pass() {
        let cmp = |at| insn(at, Operation::Compare, "cmp", vec![], vec![r(RegId::from(AX)), r(RegId::from(BX))], None);
        let branch = |at, target| insn(at, Operation::Branch, "jl", vec![], vec![], Some(target));
        let ret = Arc::new(Insn {
            reads_complete: true,
            ..Arc::unwrap_or_clone(insn(5, Operation::Return, "ret", vec![], vec![], None))
        });
        let body = LirBody::new(
            "f",
            1,
            vec![
                block(
                    9,
                    vec![
                        insn(
                            9,
                            Operation::Binary,
                            "add",
                            vec![r(RegId::from(AX))],
                            vec![r(RegId::from(AX)), r(RegId::from(DI))],
                            None,
                        ),
                        insn(10, Operation::Jump, "jmp", vec![], vec![], Some(5)),
                    ],
                    vec![5],
                )
                .into(),
                block(7, vec![cmp(7), branch(8, 9)], vec![9, 5]).into(),
                block(1, vec![copy(1, DI, DX), cmp(2), branch(3, 7)], vec![7, 5]).into(),
                block(5, vec![ret], vec![]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let after = sunk(&body);
        assert_eq!((copies_in(&after, 1), copies_in(&after, 7), copies_in(&after, 9)), (0, 0, 1));
    }

    /// A chain of `n` diamonds, each head storing a spill cell ahead of the
    /// branch to an arm that reads it.
    fn diamonds(n: i64) -> LirBody {
        use crate::objectfile::module::{Addr, Space};
        let cell = |k: i64| {
            let addr = Addr {
                space: Space::Frame,
                disp: -4 * (k + 1),
                index: -(k + 1),
                base: (RegId::None).iced(),
                segment: (RegId::None).iced(),
            };
            crate::model::ir::Mem { through: RegId::BP, ..crate::model::ir::Mem::new(Some(addr), 2) }
        };
        let mut blocks = Vec::new();
        for k in 0..n {
            let (head, arm, next) = (10 * k + 1, 10 * k + 5, 10 * (k + 1) + 1);
            let store = Arc::new(Insn {
                spill_store: true,
                ..Arc::unwrap_or_clone(insn(
                    head,
                    Operation::Move,
                    "mov",
                    vec![Loc::Mem(cell(k))],
                    vec![r(RegId::from(AX))],
                    None,
                ))
            });
            let branch = insn(head + 2, Operation::Branch, "jl", vec![], vec![], Some(arm));
            blocks.push(block(
                head,
                vec![
                    store,
                    insn(
                        head + 1,
                        Operation::Compare,
                        "cmp",
                        vec![],
                        vec![r(RegId::from(AX)), r(RegId::from(BX))],
                        None,
                    ),
                    branch,
                ],
                vec![arm, next],
            ));
            let read = insn(arm, Operation::Move, "mov", vec![r(RegId::from(DX))], vec![Loc::Mem(cell(k))], None);
            blocks.push(block(
                arm,
                vec![read, insn(arm + 1, Operation::Jump, "jmp", vec![], vec![], Some(next))],
                vec![next],
            ));
        }
        let ret = Arc::new(Insn {
            reads_complete: true,
            ..Arc::unwrap_or_clone(insn(10 * n + 1, Operation::Return, "ret", vec![], vec![], None))
        });
        blocks.push(block(10 * n + 1, vec![ret], vec![]));
        LirBody::new("f", 1, crate::model::lir::shared(blocks), IndexMap::default(), IndexMap::default())
    }

    fn scanned(n: i64) -> usize {
        super::SCANNED.with(|scanned| scanned.set(0));
        let after = sunk(&diamonds(n));
        // Each store went to its arm.
        assert!(
            after
                .blocks
                .iter()
                .filter(|block| block.at % 10 == 1)
                .all(|head| head.insns.iter().all(|one| !one.spill_store))
        );
        super::SCANNED.with(std::cell::Cell::get)
    }

    /// Each spill store read the blocks beyond each successor again: the work
    /// grew with the square of the stores (`read_beyond`, found in review
    /// of the spill-store sinking).
    #[test]
    fn test_the_blocks_read_to_place_spill_stores_grow_with_the_stores() {
        let (small, large) = (scanned(40), scanned(80));
        assert!(large <= 3 * small, "40 stores read {small} blocks, 80 read {large}");
    }

    /// `mov [slot], ebx` (the planes copy) was sunk into the arm that reads the
    /// slot, past `mov [slot], esi` (the stride, stored over it): the arm read
    /// the planes where it wanted the stride, and pack_ref of qb-m32's fill_ops
    /// packed wrong rows.
    #[test]
    fn test_a_spill_store_does_not_move_past_a_later_store_to_its_cell() {
        use crate::objectfile::module::{Addr, Space};
        let addr = Addr {
            space: Space::Frame,
            disp: -8,
            index: -1,
            base: iced_x86::Register::None,
            segment: iced_x86::Register::None,
        };
        let cell = crate::model::ir::Mem { through: RegId::BP, ..crate::model::ir::Mem::new(Some(addr), 2) };
        let store = |at, source| {
            Arc::new(Insn {
                spill_store: true,
                ..Arc::unwrap_or_clone(insn(
                    at,
                    Operation::Move,
                    "mov",
                    vec![Loc::Mem(cell.clone())],
                    vec![r(source)],
                    None,
                ))
            })
        };
        let ret = Arc::new(Insn {
            reads_complete: true,
            ..Arc::unwrap_or_clone(insn(9, Operation::Return, "ret", vec![], vec![], None))
        });
        let body = LirBody::new(
            "f",
            1,
            vec![
                block(
                    1,
                    vec![
                        store(1, BX),
                        store(2, DI),
                        copy(3, DI, AX),
                        insn(3, Operation::Compare, "cmp", vec![], vec![r(AX), r(BX)], None),
                        insn(4, Operation::Branch, "jl", vec![], vec![], Some(5)),
                    ],
                    vec![5, 9],
                )
                .into(),
                block(
                    5,
                    vec![
                        insn(5, Operation::Move, "mov", vec![r(DX)], vec![Loc::Mem(cell.clone())], None),
                        insn(6, Operation::Jump, "jmp", vec![], vec![], Some(9)),
                    ],
                    vec![9],
                )
                .into(),
                block(9, vec![ret], vec![]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let after = sunk(&body);
        let stores = |at| {
            after.blocks.iter().find(|block| block.at == at).unwrap().insns.iter().filter(|one| one.spill_store).count()
        };
        assert_eq!((stores(1), stores(5)), (2, 0), "the older store ran after the newer one");
    }

    /// x_cordic -Os (m32): the back edge's `mov eax, ebx; jmp` ran 2,200 times
    /// beside a `jge` not taken and a taken `jmp`; the copy before the
    /// compare, where the exit does not read EAX, leaves one `jl`.
    #[test]
    fn test_an_edge_copy_runs_in_the_branching_block_where_the_other_way_does_not_read_it() {
        let build = |exit_reads: bool, back_edge: bool| {
            let cmp = insn(2, Operation::Compare, "cmp", vec![], vec![r(AX), r(BX)], None);
            let branch = insn(3, Operation::Branch, "jge", vec![], vec![], Some(5));
            let ret = |at| {
                Arc::new(Insn {
                    reads_complete: true,
                    ..Arc::unwrap_or_clone(insn(at, Operation::Return, "ret", vec![], vec![], None))
                })
            };
            let use_di = insn(5, Operation::Binary, "add", vec![r(AX)], vec![r(AX), r(DI)], None);
            let to = if back_edge { 1 } else { 9 };
            let mut blocks = vec![
                block(1, vec![cmp, branch], vec![5, 7]).into(),
                block(5, if exit_reads { vec![use_di, ret(6)] } else { vec![ret(6)] }, vec![]).into(),
                block(7, vec![copy(7, DI, DX), insn(8, Operation::Jump, "jmp", vec![], vec![], Some(to))], vec![to])
                    .into(),
            ];
            if !back_edge {
                blocks.push(block(9, vec![ret(9)], vec![]).into());
            }
            LirBody::new("edge", 1, blocks, IndexMap::default(), IndexMap::default())
        };
        let names = |body: &LirBody, at: i64| {
            body.blocks
                .iter()
                .find(|one| one.at == at)
                .expect("a block")
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect::<Vec<_>>()
        };
        let after = latched(&build(false, true));
        assert_eq!(names(&after, 1), ["cmp", "mov", "jge"]);
        assert_eq!(names(&after, 7), ["jmp"]);
        let kept = latched(&build(true, true));
        assert_eq!(names(&kept, 1), ["cmp", "jge"], "the exit reads DI: the copy stays on its edge");
        assert_eq!(names(&kept, 7), ["mov", "jmp"]);
        // The layout puts a forward edge's block before its target, with no
        // jump to save.
        let forward = latched(&build(false, false));
        assert_eq!(names(&forward, 1), ["cmp", "jge"]);
    }

    /// `postrasink` solved the liveness of the body for itself (decoding every
    /// instruction's effects and a worklist over the blocks) beside the answer
    /// the clean-up holds for it: 5.9 M of d_faces' 85 M for the peephole.
    #[test]
    fn test_sinking_makes_no_dataflow_solve_of_its_own() {
        let body = shape(false, false);
        crate::analysis::dataflow::WORKED.with(|worked| worked.set(0));
        let after = sunk(&body);
        let worked = crate::analysis::dataflow::WORKED.with(std::cell::Cell::get);
        assert_eq!((copies_in(&after, 1), copies_in(&after, 9)), (0, 1), "the premise: a copy moved");
        assert_eq!(worked, 0, "{worked} block transfers worked out");
    }
}
