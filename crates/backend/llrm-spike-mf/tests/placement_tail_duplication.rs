//! `TailDupPlacement`: placement copies a small block that ends in a
//! conditional branch into its predecessors. rectwo -O2 at arm64: the join
//! block `cbnz` was left apart from both arms, so each copy ended
//! `cbnz X; b Y` (+5 instructions, +20 B against the pin).

use llrm_mir::blockfrequency::BranchProbability;
use llrm_spike_mf::cfg::Cfg;
use llrm_spike_mf::placement::{Graph, Layout, Odds, place_duplicating};

/// 0 -> 1, 2; both arms jump to 3, which branches to 4 or 5; both return.
fn join() -> Graph {
    let cfg = Cfg::from_successors(vec![vec![1, 2], vec![3], vec![3], vec![4, 5], vec![], vec![]]);
    let ids: Vec<u32> = (0..6).collect();
    let mut odds = Odds::default();
    odds.insert((0, 1), (BranchProbability::new(3, 8), 0));
    odds.insert((0, 2), (BranchProbability::new(5, 8), 1));
    odds.insert((3, 4), (BranchProbability::new(3, 8), 0));
    odds.insert((3, 5), (BranchProbability::new(5, 8), 1));
    let graph = Graph::from_cfg(&cfg, &ids, vec![false; 6], &odds, false);
    graph.with_tail_duplication(2, vec![1, 1, 1, 1, 1, 1], vec![false, true, true, false, false, false])
}

#[test]
fn a_join_block_with_two_ways_on_is_copied_into_both_arms() {
    let mut graph = join();
    let (order, copies) = place_duplicating(&mut graph);
    assert!(!copies.is_empty(), "no copy made: the join stays a block both arms jump to");
    assert!(!order.contains(&3), "the join has no predecessor left, so it is not laid out: {order:?}");
    for arm in [1, 2] {
        assert_eq!(graph.successors(arm), &[4, 5], "arm {arm} takes the join's ways on");
    }
}

#[test]
fn without_a_copy_size_placement_leaves_the_join_alone() {
    let cfg = Cfg::from_successors(vec![vec![1, 2], vec![3], vec![3], vec![4, 5], vec![], vec![]]);
    let mut graph = Graph::from_cfg(&cfg, &(0..6).collect::<Vec<u32>>(), vec![false; 6], &Odds::default(), false);
    let (order, copies) = place_duplicating(&mut graph);
    assert!(copies.is_empty() && order.contains(&3));
}

/// jpeg_fixed -O2 at arm64: a loop header copied into its predecessors left
/// placement indexing the removed block (index out of bounds; no object for
/// 6 cells). Graph found by fuzzing, with a loop through 1, 2, 5.
#[test]
fn a_copied_away_loop_block_does_not_break_placement() {
    let succ = vec![vec![5], vec![5], vec![1, 4], vec![6, 4], vec![5], vec![2, 4], vec![]];
    let n = succ.len();
    let jumps: Vec<bool> = succ.iter().map(|s| s.len() == 1).collect();
    let mut odds = Odds::default();
    for (b, v) in succ.iter().enumerate().filter(|(_, v)| v.len() == 2) {
        odds.insert((b as u32, v[0] as u32), (BranchProbability::new(7, 8), 0));
        odds.insert((b as u32, v[1] as u32), (BranchProbability::new(1, 8), 1));
    }
    let cfg = Cfg::from_successors(succ);
    let ids: Vec<u32> = (0..n as u32).collect();
    let graph = Graph::from_cfg(&cfg, &ids, vec![false; n], &odds, false);
    let mut graph = graph.with_tail_duplication(2, vec![1; n], jumps);
    let (order, _) = place_duplicating(&mut graph);
    let mut seen = order.clone();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), order.len(), "a block is laid out once: {order:?}");
}
