//! How often each block of a LIR body runs per call, and each edge is taken:
//! the one estimate every consumer asks, LLVM's `MachineBlockFrequencyInfo`
//! over `MachineBranchProbabilityInfo`.
//!
//! The edge probabilities are `LirBody::odds`, which isel made from
//! `branchprob`'s heuristics over the MIR (an edge made by splitting one
//! inherits its odds, `BlockOdds::redirected`); a loop's proven
//! trips, which induction left in `loop_trip_counts`, replace the heuristic's
//! 31 in 32; and `branchprob::propagated` turns them into frequencies.

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use crate::model::lir::BlockOdds;

use llrm_analysis::branchprob;

use crate::analysis::loops;
use crate::model::lir::{LirBlock, LirBody};
use crate::support::hash::IndexMap;

/// What the last `Frequency::of` was asked, and its answer.
struct Held {
    entry: i64,
    shape: Vec<(i64, Vec<i64>)>,
    odds: BlockOdds,
    trips: Vec<(i64, i64)>,
    answer: Rc<Frequency>,
}

impl Held {
    fn of(body: &LirBody, answer: &Rc<Frequency>) -> Self {
        Self {
            entry: body.entry,
            shape: body.blocks.iter().map(|block| (block.at, block.succ.clone())).collect(),
            odds: body.odds.clone(),
            trips: body.loop_trip_counts.clone(),
            answer: Rc::clone(answer),
        }
    }

    fn is_of(&self, body: &LirBody) -> bool {
        self.entry == body.entry
            && self.shape.len() == body.blocks.len()
            && self.shape.iter().zip(&body.blocks).all(|((at, succ), block)| *at == block.at && *succ == block.succ)
            && self.odds == body.odds
            && self.trips == body.loop_trip_counts
    }
}

thread_local! {
    static LAST: std::cell::RefCell<Option<Held>> = const { std::cell::RefCell::new(None) };
    static CARRIED: std::cell::RefCell<Option<(Arc<crate::model::lir::BlockFrequencies>, Held)>> = const { std::cell::RefCell::new(None) };
    static CARRIED_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many frequencies this thread has worked out, for a test that asking again of one shape does not.
/// How many tables this thread has carried over from a body's own, for a test that asking again of one shape does not.
pub fn carried_built() -> usize {
    CARRIED_BUILT.with(std::cell::Cell::get)
}

pub fn built() -> usize {
    BUILT.with(std::cell::Cell::get)
}

pub struct Frequency {
    block: IndexMap<i64, f64>,
    taken: IndexMap<(i64, i64), f64>,
}

impl Frequency {
    /// `body`'s blocks' frequencies. They depend on the blocks' shape, the odds and the trip counts
    /// and on no instruction, so the last answer stands for every body of that shape: a rewrite that
    /// only inserts instructions (a spill, a reload) asks the same question again and again.
    pub fn of(body: &LirBody) -> Rc<Self> {
        if let Some(table) = &body.frequencies {
            // The carried table gives the same answer for the same table and shape: a spill or a reload changes neither, and
            // working it out again for each ask was 1.5% of compiling d_faces.
            let again = CARRIED.with(|last| last.borrow().as_ref().filter(|(was, held)| Arc::ptr_eq(was, table) && held.is_of(body)).map(|(_, held)| Rc::clone(&held.answer)));
            if again.is_some() {
                return again.expect("checked");
            }
            if let Some(kept) = Self::carried(body, &table.0) {
                CARRIED_BUILT.with(|count| count.set(count.get() + 1));
                let kept = Rc::new(kept);
                CARRIED.with(|last| *last.borrow_mut() = Some((Arc::clone(table), Held::of(body, &kept))));
                return kept;
            }
        }
        LAST.with(|last| {
            if let Some(held) = last.borrow().as_ref().filter(|held| held.is_of(body)) {
                return Rc::clone(&held.answer);
            }
            let answer = Rc::new(Self::over(body, &body.blocks));
            *last.borrow_mut() = Some(Held::of(body, &answer));
            answer
        })
    }

    /// The table kept on the body, with the blocks made since filled in from their predecessors' (a block put on an
    /// edge runs as often as the edge is taken). `None` where one cannot be: a block made since that only a cycle
    /// reaches, or the entry.
    fn carried(body: &LirBody, table: &IndexMap<i64, f64>) -> Option<Self> {
        let mut taken = IndexMap::default();
        let mut predecessors: IndexMap<i64, Vec<i64>> = IndexMap::default();
        for block in &body.blocks {
            for to in block.succ.iter().copied().collect::<BTreeSet<_>>() {
                taken.insert((block.at, to), body.odds.chance(block.at, &block.succ, to));
                predecessors.entry(to).or_default().push(block.at);
            }
        }
        let mut block: IndexMap<i64, f64> = body.blocks.iter().filter_map(|one| table.get(&one.at).map(|runs| (one.at, *runs))).collect();
        let mut missing: Vec<i64> = body.blocks.iter().map(|one| one.at).filter(|at| !block.contains_key(at)).collect();
        while !missing.is_empty() {
            let before = missing.len();
            missing.retain(|at| {
                let Some(from) = predecessors.get(at) else { return true };
                if from.iter().any(|one| !block.contains_key(one)) {
                    return true;
                }
                let runs = from.iter().map(|one| block[one] * taken[&(*one, *at)]).sum();
                block.insert(*at, runs);
                false
            });
            if missing.len() == before {
                // A loop made since: its shape is worked out alone and scaled to the flow the table sends into it.
                let whole = Self::over(body, &body.blocks);
                let region: std::collections::BTreeSet<i64> = missing.iter().copied().collect();
                let (mut sent, mut guessed) = (0.0, 0.0);
                for (from, to) in taken.keys().filter(|(from, to)| region.contains(to) && !region.contains(from)) {
                    sent += block.get(from).copied().unwrap_or(0.0) * taken[&(*from, *to)];
                    guessed += whole.block(*from) * taken[&(*from, *to)];
                }
                let scale = if guessed > 0.0 { sent / guessed } else { 1.0 };
                for at in region {
                    block.insert(at, whole.block(at) * scale);
                }
                break;
            }
        }
        Some(Self { block, taken })
    }

    /// What no honest table breaks, whatever shape the blocks have: an entry for every block and none for others, no block
    /// running more often than the flow into it (each predecessor sends at most its own runs), and a block behind a
    /// predecessor that goes nowhere else running as often as it. A table left stale by an edge that moved breaks one.
    pub fn violations(body: &LirBody) -> Vec<String> {
        let Some(kept) = body.frequencies.as_deref() else { return Vec::new() };
        let mut out = Vec::new();
        let present: std::collections::BTreeSet<i64> = body.blocks.iter().map(|one| one.at).collect();
        for at in kept.0.keys().filter(|at| !present.contains(at)) {
            out.push(format!("frequency kept for block {at:#x}, which is gone"));
        }
        let Some(now) = Self::carried(body, &kept.0) else { return out };
        let mut inflow: IndexMap<i64, (f64, usize, Option<i64>)> = IndexMap::default();
        for one in &body.blocks {
            let list: std::collections::BTreeSet<i64> = one.succ.iter().copied().collect();
            for to in &list {
                let entry = inflow.entry(*to).or_insert((0.0, 0, None));
                entry.0 += now.block(one.at);
                entry.1 += 1;
                entry.2 = Some(one.at).filter(|_| list.len() == 1);
            }
        }
        for one in body.blocks.iter().filter(|one| one.at != body.entry) {
            let Some((sum, count, only)) = inflow.get(&one.at).copied() else { continue };
            let runs = now.block(one.at);
            // The estimate is not conservative where proven trips are nested: they fix a header's runs and the odds fix the
            // blocks around it (isel's own table reads 1.5x at most in gcc's memcpy-2, 1.05x in QCport). A stale entry is off
            // by factors.
            let slack = sum + 1e-9;
            if runs > sum + slack {
                out.push(format!("block {:#x} runs {runs:.3} times, its predecessors {sum:.3}", one.at));
            } else if count == 1 && only.is_some() && (runs - sum).abs() > slack {
                out.push(format!("block {:#x} runs {runs:.3} times, its only predecessor, which goes nowhere else, {sum:.3}", one.at));
            }
        }
        out
    }

    /// Every block's frequency, to keep on the body.
    pub fn table(&self) -> std::sync::Arc<crate::model::lir::BlockFrequencies> {
        std::sync::Arc::new(crate::model::lir::BlockFrequencies(self.block.clone()))
    }

    /// `blocks`, a later arrangement of `body`'s, on `body`'s odds and trips.
    pub fn over(body: &LirBody, blocks: &[LirBlock]) -> Self {
        BUILT.with(|built| built.set(built.get() + 1));
        let mut taken = IndexMap::default();
        for block in blocks {
            for to in block.succ.iter().copied().collect::<BTreeSet<_>>() {
                taken.insert((block.at, to), body.odds.chance(block.at, &block.succ, to));
            }
        }
        // A block that only jumps on says nothing about the loops, and a loop is not another loop for one put on an
        // edge: the estimate is made without it (`_part_frame` read 268 without its bridges and 2786 with them, and 5
        // trips read 6 with one on a back edge). A block with work in it stays: a body is not a bridge.
        let entry = body.entry;
        let by_at: IndexMap<i64, &LirBlock> = blocks.iter().map(|block| (block.at, block)).collect();
        let bridge = |at: i64| -> Option<i64> {
            let one = by_at.get(&at)?;
            let idle = one.insns.iter().all(|one| !crate::backend::masm::prints(one) || one.what.as_ref().is_some_and(|what| what.op == crate::model::ir::Operation::Jump));
            (at != entry && idle && one.succ.len() == 1 && one.succ[0] != at).then(|| one.succ[0])
        };
        // Past the bridges, and the share of the edge that reaches there.
        let beyond = |mut at: i64| -> i64 {
            let mut hops = 0;
            while let Some(next) = bridge(at) {
                at = next;
                hops += 1;
                if hops > blocks.len() {
                    break;
                }
            }
            at
        };
        let mut successors: IndexMap<i64, Vec<i64>> = IndexMap::default();
        let mut shared: IndexMap<(i64, i64), f64> = IndexMap::default();
        for block in blocks.iter().filter(|block| bridge(block.at).is_none() || block.at == entry) {
            let list = successors.entry(block.at).or_default();
            for to in block.succ.iter().copied().collect::<BTreeSet<_>>() {
                let past = beyond(to);
                *shared.entry((block.at, past)).or_default() += taken[&(block.at, to)];
                if !list.contains(&past) {
                    list.push(past);
                }
            }
        }
        let kept: Vec<LirBlock> = blocks.iter().filter(|block| successors.contains_key(&block.at)).map(|block| LirBlock { succ: successors[&block.at].clone(), ..LirBlock::new(block.at, Vec::new()) }).collect();
        let mut predecessors: IndexMap<i64, Vec<i64>> = IndexMap::default();
        for (from, list) in &successors {
            for to in list {
                predecessors.entry(*to).or_default().push(*from);
            }
        }
        let natural = loops::loops(&kept, Some(body.entry));
        let trips: IndexMap<i64, i64> = body.loop_trip_counts.iter().copied().collect();
        let cycles: Vec<branchprob::Cycle> = natural
            .iter()
            .map(|one| branchprob::Cycle { header: one.header, latches: &one.latches, body: &one.body, trips: trips.get(&one.header).copied() })
            .collect();
        let order = branchprob::reverse_postorder_of(body.entry, &|at| successors.get(&at).cloned().unwrap_or_default());
        let (mut block, edges) = branchprob::propagated_edges(
            &order,
            &|at| predecessors.get(&at).cloned().unwrap_or_default(),
            &|at| successors.get(&at).cloned().unwrap_or_default(),
            &cycles,
            &|from, to| shared.get(&(from, to)).copied().unwrap_or(0.0),
        );
        // A bridge runs as often as the edges into it are taken; a chain of them is settled from its first.
        let mut pending: Vec<i64> = blocks.iter().map(|one| one.at).filter(|at| bridge(*at).is_some()).collect();
        while !pending.is_empty() {
            let before = pending.len();
            pending.retain(|at| {
                let mut runs = 0.0;
                for from in blocks.iter().filter(|from| from.succ.contains(at)) {
                    let Some(found) = block.get(&from.at).copied() else { return true };
                    runs += if bridge(from.at).is_some() {
                        found
                    } else {
                        let past = beyond(*at);
                        found * edges.get(&(from.at, past)).copied().unwrap_or(0.0) * taken[&(from.at, *at)] / shared[&(from.at, past)].max(f64::MIN_POSITIVE)
                    };
                }
                block.insert(*at, runs);
                false
            });
            if pending.len() == before {
                break;
            }
        }
        Self { block: block.into_iter().collect(), taken }
    }

    /// Per call; zero for a block the entry does not reach.
    pub fn block(&self, at: i64) -> f64 {
        self.block.get(&at).copied().unwrap_or(0.0)
    }

    /// How often `from` goes to `to`, per call.
    pub fn edge(&self, from: i64, to: i64) -> f64 {
        self.block(from) * self.taken.get(&(from, to)).copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Frequency;
    use crate::model::ir::{Operation, Semantics};
    use crate::model::lir::{BlockOdds, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn insn(at: i64, op: Operation, name: &str) -> Arc<Insn> {
        let what = Semantics { name: Some(name.to_owned()), ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, 1)), Some(what), Vec::new(), Vec::new()))
    }

    fn block(at: i64, name: &str, op: Operation, succ: Vec<i64>) -> LirBlock {
        LirBlock { succ, ..LirBlock::new(at, vec![insn(at, op, name)]) }
    }

    /// entry 1 -> loop 2 (back to 2, out to 3) -> 3.
    /// A spill or a reload inserts instructions and leaves the blocks as they were: asked again, the
    /// frequencies are not worked out again (3 builds per rebuild of the allocator's facts, 8% of
    /// compiling `d_faces`, #559). A block's successors or odds changing is a new question.
    #[test]
    fn test_the_same_blocks_are_not_worked_out_twice() {
        let body = counted(Some(5), 124.0 / 128.0);
        let before = super::built();
        let first = Frequency::of(&body);
        let more = body.with_blocks(body.blocks.iter().map(|one| one.with_insns(vec![insn(one.at, Operation::Move, "mov"), one.insns[0].clone()])).collect());
        let again = Frequency::of(&more);
        assert_eq!(super::built() - before, 1, "a second question of the same blocks was worked out");
        assert_eq!(first.block(2), again.block(2));
        let mut moved = body.clone();
        moved.odds.taken.insert((2, 2), 1);
        Frequency::of(&moved);
        assert_eq!(super::built() - before, 2, "other odds were answered from the old frequencies");
    }

    /// A body that carries its frequencies is asked for them by every fact of the allocator, with the same table and blocks after each
    /// spill: the carried answer is worked out once for them (1.5% of compiling d_faces, 1.2 G, was working it out again).
    #[test]
    fn test_a_carried_table_is_not_carried_again_for_the_same_blocks() {
        let plain = counted(Some(5), 124.0 / 128.0);
        let mut body = plain.clone();
        body.frequencies = Some(Frequency::of(&plain).table());
        let before = super::carried_built();
        let first = Frequency::of(&body);
        let more = body.with_blocks(body.blocks.iter().map(|one| one.with_insns(vec![insn(one.at, Operation::Move, "mov"), one.insns[0].clone()])).collect());
        let again = Frequency::of(&more);
        assert_eq!(super::carried_built() - before, 1, "the same table over the same blocks was carried again");
        assert_eq!(first.block(2), again.block(2));
        let mut moved = body.clone();
        moved.odds.taken.insert((2, 2), 1);
        Frequency::of(&moved);
        assert_eq!(super::carried_built() - before, 2, "other odds were answered from the old table");
    }

    fn counted(trips: Option<i64>, stay: f64) -> LirBody {
        let blocks = vec![
            block(1, "jmp", Operation::Jump, vec![2]),
            block(2, "jne", Operation::Branch, vec![2, 3]),
            block(3, "ret", Operation::Return, vec![]),
        ];
        let mut body = LirBody::new("f", 1, blocks, IndexMap::default(), IndexMap::default());
        for (to, probability) in [(2, stay), (3, 1.0 - stay)] {
            body.odds.taken.insert((2, to), (probability * BlockOdds::CERTAIN).round() as u32);
        }
        body.loop_trip_counts = trips.map(|count| (2, count)).into_iter().collect();
        body
    }

    /// Spill weights and the instrument read 10 per loop level, whatever the
    /// branch said; the loop's heuristic odds make it 32, and a proven count its own.
    #[test]
    fn test_a_loop_runs_as_its_odds_and_proven_trips_say_not_ten() {
        let guessed = Frequency::of(&counted(None, 124.0 / 128.0));
        assert!((guessed.block(2) - 32.0).abs() < 1e-6 && (guessed.block(3) - 1.0).abs() < 1e-6, "{} {}", guessed.block(2), guessed.block(3));
        let proven = Frequency::of(&counted(Some(5), 124.0 / 128.0));
        assert!((proven.block(2) - 5.0).abs() < 1e-6 && (proven.block(3) - 1.0).abs() < 1e-6, "{} {}", proven.block(2), proven.block(3));
    }

    /// A block almost never reached weighs almost nothing: a branch to it at 1 in a
    /// million is the cold path 10^depth gave the weight of the code before it.
    #[test]
    fn test_a_block_that_is_all_but_never_taken_weighs_nothing() {
        let blocks = vec![
            block(1, "jne", Operation::Branch, vec![2, 3]),
            block(2, "ret", Operation::Return, vec![]),
            block(3, "ret", Operation::Return, vec![]),
        ];
        let mut body = LirBody::new("f", 1, blocks, IndexMap::default(), IndexMap::default());
        for (to, probability) in [(2, 1e-6), (3, 1.0 - 1e-6)] {
            body.odds.taken.insert((1, to), (probability * BlockOdds::CERTAIN).round() as u32);
        }
        let busy = Frequency::of(&body);
        assert!(busy.block(2) < 1e-5 && busy.block(3) > 0.99, "{} {}", busy.block(2), busy.block(3));
    }

    /// A block put on an edge (phielim's and the spiller's edge code) must leave the frequencies as they were: `d_faces`-sized
    /// `_part_frame` was estimated 268 on the blocks isel made, 2786 with the bridges, and 268 again once `jumps` removed them,
    /// and the route choice compared two bodies at different points of that.
    #[test]
    fn test_a_block_on_an_edge_leaves_the_frequencies_as_they_were() {
        for trips in [None, Some(5)] {
            for (from, old) in [(2, 3), (2, 2)] {
                let body = counted(trips, 124.0 / 128.0);
                let before = Frequency::of(&body);
                let mut split = body.clone();
                let bridge = 9;
                let succ: Vec<i64> = split.blocks.iter().find(|one| one.at == from).expect("a block").succ.clone();
                split.odds.rerouted(from, &succ, old, &[(bridge, 1.0)]);
                for one in &mut split.blocks {
                    if one.at == from {
                        one.succ = one.succ.iter().map(|to| if *to == old { bridge } else { *to }).collect();
                    }
                }
                split.blocks.push(block(bridge, "jmp", Operation::Jump, vec![old]));
                let after = Frequency::of(&split);
                for at in [1, 2, 3] {
                    assert!((before.block(at) - after.block(at)).abs() < 1e-6 * before.block(at).max(1.0), "block {at} trips {trips:?} split {from}->{old}: {} against {}", before.block(at), after.block(at));
                }
            }
        }
    }

    /// Copies put on a loop's back edge make a block with work in it, which the shape alone reads as the loop's body: a
    /// loop tested after its trip then reads as tested before it, and 5 trips as 6 (`_part_frame`, 8x). The frequencies
    /// worked out before stay, and the copy block runs as often as its edge.
    #[test]
    fn test_copies_on_the_back_edge_do_not_change_what_the_loop_runs() {
        let mut body = counted(Some(5), 124.0 / 128.0);
        body.frequencies = Some(Frequency::of(&body).table());
        let before = Frequency::of(&body);
        let mut split = body.clone();
        split.odds.rerouted(2, &[2, 3], 2, &[(9, 1.0)]);
        split.blocks[1].succ = vec![9, 3];
        split.blocks.push(LirBlock { succ: vec![2], ..LirBlock::new(9, vec![insn(9, Operation::Move, "mov"), insn(9, Operation::Jump, "jmp")]) });
        let after = Frequency::of(&split);
        for at in [1, 2, 3] {
            assert_eq!(before.block(at), after.block(at), "block {at}");
        }
        assert!((after.block(9) - 5.0 * 124.0 / 128.0).abs() < 1e-9, "the copies run {}", after.block(9));
    }

    /// A table left as it was after an edge moved (block 3 reached only through a rare edge now) ran the block as often as
    /// before: nothing said so. The invariant every honest table keeps catches it under LLRM_VERIFY.
    #[test]
    fn test_a_table_left_stale_by_a_moved_edge_is_caught() {
        let mut body = counted(Some(5), 124.0 / 128.0);
        body.frequencies = Some(Frequency::of(&body).table());
        assert!(Frequency::violations(&body).is_empty(), "{:?}", Frequency::violations(&body));
        // 2 -> 3 is replaced by 2 -> 9 -> 3, 9 taking 1 in 32 from 2: block 3 now runs a thirty-second as often.
        let mut moved = body.clone();
        moved.blocks.push(LirBlock { succ: vec![3], ..LirBlock::new(9, vec![insn(9, Operation::Jump, "jmp")]) });
        moved.blocks[1].succ = vec![2, 9];
        moved.odds.taken.insert((2, 9), (BlockOdds::CERTAIN / 32.0).round() as u32);
        moved.odds.taken.insert((2, 2), (BlockOdds::CERTAIN * 31.0 / 32.0).round() as u32);
        moved.blocks[2].succ = vec![];
        let said = Frequency::violations(&moved);
        assert!(said.iter().any(|one| one.contains("block 0x3 runs")), "{said:?}");
        let mut gone = body.clone();
        gone.blocks.retain(|one| one.at != 3);
        gone.blocks[1].succ = vec![2];
        gone.frequencies = body.frequencies.clone();
        assert!(Frequency::violations(&gone).iter().any(|one| one.contains("gone")));
        assert!(Frequency::violations(&gone.with_blocks(gone.blocks.clone())).is_empty());
    }

    /// A loop made after the table, with no entries and only a cycle reaching it, was worked out with the whole body's
    /// shape again. Its region alone is: scaled to what the table sends in.
    #[test]
    fn test_a_loop_made_since_is_worked_out_alone_and_scaled_to_its_entering_flow() {
        let mut body = counted(Some(5), 124.0 / 128.0);
        body.frequencies = Some(Frequency::of(&body).table());
        let mut more = body.clone();
        // 2 -> 3 now goes through a new self loop 9.
        more.blocks[1].succ = vec![2, 8];
        more.blocks.push(LirBlock { succ: vec![9], ..LirBlock::new(8, vec![insn(8, Operation::Jump, "jmp")]) });
        more.blocks.push(LirBlock { succ: vec![9, 3], ..LirBlock::new(9, vec![insn(9, Operation::Branch, "jne")]) });
        more.blocks[2].succ = vec![];
        for (from, to, probability) in [(2, 8, 1.0 / 32.0), (2, 2, 31.0 / 32.0), (9, 9, 31.0 / 32.0), (9, 3, 1.0 / 32.0)] {
            more.odds.taken.insert((from, to), (probability * BlockOdds::CERTAIN).round() as u32);
        }
        let busy = Frequency::of(&more);
        let entering = busy.block(2) * (1.0 / 32.0);
        assert!((busy.block(8) - entering).abs() < 1e-6, "{} {}", busy.block(8), entering);
        assert!((busy.block(9) - entering * 32.0).abs() < 1e-3 * entering * 32.0, "the new loop runs {} for {entering} sent in", busy.block(9));
    }
}
