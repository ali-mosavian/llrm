use super::*;
use crate::model::lir::{Block, LirBlock};
use crate::support::hash::IndexMap;

/// Blocks `0..succs.len()` at `at` = index * 16, each with its successors, and
/// the odds by edge in 2^31sts.
fn body(
    succs: &[&[i64]],
    odds: &[((i64, i64), u32)],
) -> LirBody {
    let blocks = succs
        .iter()
        .enumerate()
        .map(|(at, succ)| {
            let mut block = LirBlock::new(at as i64 * 16, Vec::new());
            block.succ = succ.iter().map(|to| to * 16).collect::<Vec<_>>().into();
            Block::from(block)
        })
        .collect();
    let mut body = LirBody::new("f", 0, blocks, IndexMap::default(), IndexMap::default());
    for &((from, to), prob) in odds {
        body.odds.taken.insert((from * 16, to * 16), prob);
    }
    body
}

const HALF: u32 = 1 << 30;

/// A diamond split evenly: each arm runs half as often as the entry, the join
/// as often.
#[test]
fn test_a_diamond_runs_each_arm_by_its_odds() {
    let mbfi = MachineBlockFrequencyInfo::of(&body(&[&[1, 2], &[3], &[3], &[]], &[((0, 1), HALF), ((0, 2), HALF)]));
    let rel: Vec<f64> = (0..4).map(|b| mbfi.get_block_freq_relative_to_entry_block(b)).collect();
    assert_eq!(rel, [1.0, 0.5, 0.5, 1.0]);
}

/// A loop taken back 31 times in 32 runs 32 times per entry (1 / (1 - 31/32)),
/// and an edge without odds shares what its block's stated edges leave.
#[test]
fn test_a_loop_runs_its_trips_and_unstated_edges_share_the_rest() {
    let mbfi = MachineBlockFrequencyInfo::of(&body(&[&[1], &[1, 2], &[]], &[((1, 1), ((31u64 << 31) / 32) as u32)]));
    let header = mbfi.get_block_freq_relative_to_entry_block(1);
    assert!((header - 32.0).abs() < 1e-6, "{header}");
    assert_eq!(mbfi.get_block_freq_relative_to_entry_block(2), 1.0);
}

/// 40 nested loops of 32 trips: 32^40 is past what a u64 holds, and the
/// frequencies stay ordered (the innermost the most frequent) rather than
/// wrapping or all saturating to one number.
#[test]
fn test_a_deep_nest_keeps_its_blocks_ordered() {
    let depth = 40;
    // Headers 1..=depth each enter the next; latch of level k (at
    // 2 * depth + 1 - k) goes back to header k 31 times in 32, else to the
    // latch of level k - 1, the last to the exit.
    let mut succs: Vec<Vec<i64>> = (0..=depth as i64).map(|k| vec![k + 1]).collect();
    let mut odds = Vec::new();
    for k in (1..=depth as i64).rev() {
        let at = 2 * depth as i64 + 1 - k;
        succs.push(vec![k, at + 1]);
        odds.push(((at, k), ((31u64 << 31) / 32) as u32));
    }
    succs.push(vec![]);
    let succs: Vec<&[i64]> = succs.iter().map(Vec::as_slice).collect();
    let mbfi = MachineBlockFrequencyInfo::of(&body(&succs, &odds));
    let freqs: Vec<u64> = (0..succs.len()).map(|b| mbfi.get_block_freq(b).get_frequency()).collect();
    assert!(freqs[..=depth].windows(2).all(|pair| pair[0] <= pair[1]), "{freqs:?}");
    assert!(freqs[depth] > freqs[1], "{freqs:?}");
}

/// Row 44 (MachineSink) reads an edge by block and successor index, as
/// `MachineBranchProbabilityInfo::getEdgeProbability(Src, SuccIdx)`: a 31 in 32
/// edge is hot, the 1 in 32 left to the other is not, and a split of a block
/// with no odds is 1/N each.
#[test]
fn test_an_edge_is_read_by_block_and_successor_index() {
    let b = body(&[&[0, 1], &[2, 3], &[], &[]], &[((0, 0), ((31u64 << 31) / 32) as u32)]);
    let mbpi = MachineBranchProbabilityInfo::of(&b);
    assert_eq!(mbpi.get_edge_probability(0, 0), BranchProbability::get_raw(((31u64 << 31) / 32) as u32));
    assert_eq!(mbpi.get_edge_probability(0, 1), BranchProbability::get_raw(1 << 26));
    assert!(mbpi.is_edge_hot(0, 0) && !mbpi.is_edge_hot(0, 1));
    assert_eq!(mbpi.get_edge_probability(1, 0), BranchProbability::new(1, 2));
}

/// One graph and cycle info feed the frequencies and the edge probabilities of
/// a body: the same numbers as `of` builds for each on its own (MBFI::of and
/// MBPI::of built the graph, and MBFI the cycle info, again for every ask:
/// 1.7 M Ir for 9 bodies of d_faces -O2).
#[test]
fn test_one_graph_feeds_the_frequencies_and_the_probabilities() {
    let looped = body(&[&[1], &[1, 2], &[]], &[((1, 1), ((31u64 << 31) / 32) as u32)]);
    let graph = std::rc::Rc::new(Graph::of(&looped));
    let ci = graph.cycle_info();
    let shared = MachineBlockFrequencyInfo::of_with(&graph, &ci);
    let own = MachineBlockFrequencyInfo::of(&looped);
    for block in 0..3 {
        assert_eq!(shared.get_block_freq(block), own.get_block_freq(block), "block {block}");
    }
    let probabilities = MachineBranchProbabilityInfo::of_with(std::rc::Rc::clone(&graph));
    let alone = MachineBranchProbabilityInfo::of(&looped);
    for (block, edge) in [(1, 0), (1, 1)] {
        assert_eq!(probabilities.get_edge_probability(block, edge), alone.get_edge_probability(block, edge));
    }
}

/// Eighteen passes of a compile asked the frequencies of one body, each
/// building its graph and working them out (d_faces dist: 4.3 M Ir in
/// `MachineBlockFrequencyInfo::of`). The same blocks, edges and odds are asked
/// once; other odds are another answer, not the kept one.
#[test]
fn test_the_frequencies_of_one_cfg_are_worked_out_once_and_odds_make_another() {
    let succs: &[&[i64]] = &[&[1, 2], &[3], &[3], &[]];
    let even = body(succs, &[((0, 1), HALF), ((0, 2), HALF)]);
    let before = GRAPH_BUILDS.with(std::cell::Cell::get);
    let first = MachineBlockFrequencyInfo::of(&even);
    let again = MachineBlockFrequencyInfo::of(&body(succs, &[((0, 1), HALF), ((0, 2), HALF)]));
    assert_eq!(GRAPH_BUILDS.with(std::cell::Cell::get) - before, 1, "graphs built for two asks of one CFG");
    assert_eq!(first.get_block_freq_relative_to_entry_block(1), again.get_block_freq_relative_to_entry_block(1));
    let skewed = MachineBlockFrequencyInfo::of(&body(succs, &[((0, 1), HALF / 2), ((0, 2), HALF + HALF / 2)]));
    assert_eq!(skewed.get_block_freq_relative_to_entry_block(1), 0.25, "other odds are worked out afresh");
}
