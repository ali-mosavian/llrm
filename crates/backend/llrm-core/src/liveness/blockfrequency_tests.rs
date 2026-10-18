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
