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
