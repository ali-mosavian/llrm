//! LLVM's PostRAMachineSinking: a copy whose result only one successor reads
//! moves into that successor. A spill store does the same: a slot only one
//! successor reads is stored there (hanoi's `n == 0` exit stored a slot it
//! never read, and so built the frame).
//!
//! `mov edi, edx` ahead of `test eax, eax / jne body` is dead on the path that
//! returns at once, where nothing reads EDI (a saved register's incoming value
//! is on the stack): the copy belongs to the block that reads it.
//! Shrink wrapping can then place the prologue after the test, as gcc and LLVM
//! do. Sound while the successor is entered only from here, nothing between the
//! copy and the branch reads its result or writes its source, and no other
//! successor has the result live.

use std::sync::Arc;

use crate::analysis::dataflow::{self, Direction};
use crate::backend::copysink::{copy_of, touches};
use crate::backend::liveness::{_before, _effects, _universe};
use crate::backend::peephole::{_lanes, Lanes};
use crate::model::ir::{Loc, Mem, Operation, Reg, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

/// The lanes live on entry to each block.
fn live_in(body: &LirBody) -> IndexMap<i64, Lanes> {
    let universe = _universe();
    // Decoded once: the fixed point reads each block several times.
    let decoded: IndexMap<i64, _> = body.blocks.iter().map(|block| (block.at, _effects(body.bits, block))).collect();
    let nodes: Vec<&LirBlock> = body.blocks.iter().collect();
    let succ: IndexMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &block.succ)).collect();
    dataflow::solve(
        &nodes,
        Direction::Backward,
        |_| Lanes::new(),
        |at, into| succ[&at].iter().filter_map(|to| into.get(to)).flat_map(|lanes| lanes.iter().copied()).collect(),
        |at, after| _before(&decoded[&at], after.clone(), &universe),
    )
    .output
}

/// Blocks reachable from the entry, each before the successors it does not come
/// back to (reverse post-order).
fn reverse_post_order(body: &LirBody) -> Vec<i64> {
    let succ: IndexMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &block.succ)).collect();
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
fn spill_store_of(one: &Insn) -> Option<(Reg, Mem)> {
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
fn may_read(
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
        let succ: IndexMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &block.succ)).collect();
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
    let succ: IndexMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &block.succ)).collect();
    let by_at: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut insns: IndexMap<i64, Vec<Arc<Insn>>> =
        body.blocks.iter().map(|block| (block.at, block.insns.to_vec())).collect();
    let mut changed = false;
    let mut bridged: IndexMap<(i64, i64), Vec<Arc<Insn>>> = IndexMap::default();
    for at in reverse_post_order(body) {
        let to = succ[&at];
        if to.len() < 2 {
            continue;
        }
        for index in (0..insns[&at].len()).rev() {
            let one = Arc::clone(&insns[&at][index]);
            if let Some((source, cell)) = spill_store_of(&one) {
                let read = _lanes(source.register);
                let readers: Vec<i64> = to
                    .iter()
                    .copied()
                    .filter(|next| reads.get_or_init(|| Reads::of(body)).beyond(*next, &cell))
                    .collect();
                let [target] = readers[..] else { continue };
                if target == at
                    || !insns.contains_key(&target)
                    || to.iter().filter(|next| **next == target).count() != 1
                    || insns[&at][index + 1..]
                        .iter()
                        .any(|other| touches(body.bits, other, &read, false) || may_read(other, &cell))
                {
                    continue;
                }
                insns.get_mut(&at).expect("a block").remove(index);
                if preds.get(&target) == Some(&1) {
                    insns.get_mut(&target).expect("a block").insert(0, one);
                } else {
                    // The target is entered from elsewhere too (a loop's
                    // header): the store goes on the edge,
                    // in a block of its own that the layout puts in line.
                    bridged.entry((at, target)).or_default().insert(0, one);
                }
                changed = true;
                continue;
            }
            let Some((dest, source)) = copy_of(&one) else { continue };
            let (written, read) = (_lanes(dest.register), _lanes(source.register));
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
            if insns[&at][index + 1..]
                .iter()
                .any(|other| touches(body.bits, other, &both, false) || touches(body.bits, other, &written, true))
            {
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
    let mut odds = body.odds.clone();
    let mut next_at = body.blocks.iter().map(|block| block.at).max().unwrap_or(0) + 1;
    let mut retarget: IndexMap<i64, (i64, i64)> = IndexMap::default();
    let mut bridges: Vec<LirBlock> = Vec::new();
    for ((from, target), stores) in &bridged {
        let last = stores.last().expect("a store");
        let mut stored = stores.clone();
        let mut jump = Insn::new(
            last.at,
            Some((last.at, last.at)),
            Some(Semantics { name: Some("jmp".to_owned()), target: Some(*target), ..Semantics::new(Operation::Jump) }),
            Vec::new(),
            Vec::new(),
        );
        jump.call = last.call.clone();
        stored.push(Arc::new(jump));
        let source = by_at[from];
        odds.rerouted(*from, &source.succ, *target, &[(next_at, 1.0)]);
        bridges.push(LirBlock { succ: vec![*target], ..LirBlock::new(next_at, stored) });
        retarget.insert(*from, (*target, next_at));
        next_at += 1;
    }
    let mut blocks: Vec<LirBlock> = body
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
    for block in &mut blocks {
        let Some(&(target, bridge)) = retarget.get(&block.at) else { continue };
        let mut insns = block.insns.to_vec();
        let mut jumped = false;
        for one in &mut insns {
            if let Some(what) = one.what.as_ref().filter(|what| {
                matches!(what.op, Operation::Branch | Operation::Jump) && !what.indirect && what.target == Some(target)
            }) {
                let mut made = (**one).clone();
                made.what = Some(Semantics { target: Some(bridge), ..what.clone() });
                *one = Arc::new(made);
                jumped = true;
            }
        }
        if !jumped {
            // The edge was a fall-through: the block now jumps to the bridge.
            let last = insns.last().cloned().expect("a block with a branch");
            let mut jump = Insn::new(
                last.at,
                Some((last.at, last.at)),
                Some(Semantics {
                    name: Some("jmp".to_owned()),
                    target: Some(bridge),
                    ..Semantics::new(Operation::Jump)
                }),
                Vec::new(),
                Vec::new(),
            );
            jump.call = last.call.clone();
            insns.push(Arc::new(jump));
        }
        block.succ = block.succ.iter().map(|to| if *to == target { bridge } else { *to }).collect();
        *block = block.with_insns(insns);
    }
    blocks.extend(bridges);
    let mut out = body.with_blocks(blocks);
    out.odds = odds;
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register::{AX, BX, DI, DX};
    use llrm_lir::registers::RegId;

    use super::sunk;
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
            Some(Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) }),
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
        LirBlock { succ, ..LirBlock::new(at, insns) }
    }

    /// `mov di, dx; cmp ax, bx; jl 9`, the loop at 9 reading DI, the exit at 5
    /// not: the shape of a function that leaves at once or works with a
    /// copy of its argument.
    fn shape(
        exit_reads: bool,
        body_has_two_predecessors: bool,
    ) -> LirBody {
        let cmp = insn(2, Operation::Compare, "cmp", vec![], vec![r(AX), r(BX)], None);
        let branch = insn(3, Operation::Branch, "jl", vec![], vec![], Some(9));
        // A generated return reads only what it requires and the epilogue: DI
        // is not among them.
        let ret = Arc::new(Insn {
            reads_complete: true,
            ..Arc::unwrap_or_clone(insn(5, Operation::Return, "ret", vec![], vec![], None))
        });
        let use_di = |at| insn(at, Operation::Binary, "add", vec![r(AX)], vec![r(AX), r(DI)], None);
        let mut blocks = vec![
            block(1, vec![copy(1, DI, DX), cmp, branch], vec![9, 5]),
            block(5, if exit_reads { vec![use_di(5), ret] } else { vec![ret] }, vec![]),
        ];
        let tail = if body_has_two_predecessors {
            vec![
                use_di(9),
                insn(10, Operation::Compare, "cmp", vec![], vec![r(AX), r(BX)], None),
                insn(11, Operation::Branch, "jl", vec![], vec![], Some(9)),
            ]
        } else {
            vec![use_di(9), insn(10, Operation::Jump, "jmp", vec![], vec![], Some(5))]
        };
        blocks.push(block(9, tail, if body_has_two_predecessors { vec![9, 5] } else { vec![5] }));
        LirBody::new("f", 1, blocks, IndexMap::default(), IndexMap::default())
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
        let cmp = |at| insn(at, Operation::Compare, "cmp", vec![], vec![r(AX), r(BX)], None);
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
                        insn(9, Operation::Binary, "add", vec![r(AX)], vec![r(AX), r(DI)], None),
                        insn(10, Operation::Jump, "jmp", vec![], vec![], Some(5)),
                    ],
                    vec![5],
                ),
                block(7, vec![cmp(7), branch(8, 9)], vec![9, 5]),
                block(1, vec![copy(1, DI, DX), cmp(2), branch(3, 7)], vec![7, 5]),
                block(5, vec![ret], vec![]),
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
                base: RegId::None,
                segment: RegId::None,
            };
            crate::model::ir::Mem { through: RegId::BP, ..crate::model::ir::Mem::new(Some(addr), 2) }
        };
        let mut blocks = Vec::new();
        for k in 0..n {
            let (head, arm, next) = (10 * k + 1, 10 * k + 5, 10 * (k + 1) + 1);
            let store = Arc::new(Insn {
                spill_store: true,
                ..Arc::unwrap_or_clone(insn(head, Operation::Move, "mov", vec![Loc::Mem(cell(k))], vec![r(AX)], None))
            });
            let branch = insn(head + 2, Operation::Branch, "jl", vec![], vec![], Some(arm));
            blocks.push(block(
                head,
                vec![store, insn(head + 1, Operation::Compare, "cmp", vec![], vec![r(AX), r(BX)], None), branch],
                vec![arm, next],
            ));
            let read = insn(arm, Operation::Move, "mov", vec![r(DX)], vec![Loc::Mem(cell(k))], None);
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
        LirBody::new("f", 1, blocks, IndexMap::default(), IndexMap::default())
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
}
