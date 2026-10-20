use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::blocklist::BlockList;
use llrm_mir::cycleinfo::FlowGraph;
use llrm_mir::domtree::DomGraph;
use llrm_mir::module::BlockId;

use super::{Frame, Frequencies, Placement, run};

struct Graph {
    blocks: Vec<BlockId>,
    succs: Vec<Vec<u32>>,
}

impl Graph {
    fn of(edges: &[&[u32]]) -> Self {
        Self {
            blocks: (0..edges.len() as u32).map(BlockId).collect(),
            succs: edges.iter().map(|one| one.to_vec()).collect(),
        }
    }
}

impl DomGraph for Graph {
    fn entry(&self) -> Option<BlockId> {
        Some(BlockId(0))
    }

    fn max_number(&self) -> usize {
        self.blocks.len()
    }

    fn nodes(&self) -> &[BlockId] {
        &self.blocks
    }

    fn successors(
        &self,
        block: BlockId,
    ) -> BlockList {
        self.succs[block.0 as usize].iter().map(|&one| BlockId(one)).collect()
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> BlockList {
        (0..self.succs.len() as u32).filter(|&from| self.succs[from as usize].contains(&block.0)).map(BlockId).collect()
    }
}

impl FlowGraph for Graph {}

/// The blocks that need the frame, and the frequency of those that run more
/// often than the entry.
#[derive(Default)]
struct Body {
    dirty: BTreeSet<u32>,
    hot: BTreeMap<u32, u64>,
    returns: BTreeSet<u32>,
}

impl Frame for Body {
    fn touches(
        &self,
        block: BlockId,
        _: bool,
    ) -> bool {
        self.dirty.contains(&block.0)
    }

    fn terminators_touch(
        &self,
        _: BlockId,
    ) -> bool {
        false
    }

    fn is_return(
        &self,
        block: BlockId,
    ) -> bool {
        self.returns.contains(&block.0)
    }
}

impl Frequencies for Body {
    fn entry(&self) -> u64 {
        100
    }

    fn block(
        &self,
        block: BlockId,
    ) -> u64 {
        self.hot.get(&block.0).copied().unwrap_or(100)
    }
}

fn body(
    dirty: &[u32],
    returns: &[u32],
) -> Body {
    Body { dirty: dirty.iter().copied().collect(), returns: returns.iter().copied().collect(), ..Body::default() }
}

fn placed(
    save: u32,
    restore: u32,
    split: Option<&[u32]>,
) -> Option<Placement> {
    Some(Placement {
        save: BlockId(save),
        restore: BlockId(restore),
        split: split.map(|one| one.iter().map(|&at| BlockId(at)).collect()),
    })
}

#[test]
fn test_a_call_on_one_path_is_wrapped_there_and_not_at_the_entry() {
    // hanoi: `test; je ret`, the call block, the join that returns. The frame
    // was set up before the test and popped after the join, for both paths.
    let graph = Graph::of(&[&[1, 2], &[3], &[3], &[]]);
    let frame = body(&[1], &[3]);
    assert_eq!(run(&graph, &frame, &frame), placed(1, 1, None));
}

#[test]
fn test_a_use_in_the_entry_leaves_nothing_to_move() {
    let graph = Graph::of(&[&[1, 2], &[3], &[3], &[]]);
    let frame = body(&[0, 1], &[3]);
    assert_eq!(run(&graph, &frame, &frame), None);
}

#[test]
fn test_uses_on_both_paths_stay_at_the_entry() {
    let graph = Graph::of(&[&[1, 2], &[3], &[3], &[]]);
    let frame = body(&[1, 2], &[3]);
    assert_eq!(run(&graph, &frame, &frame), None);
}

#[test]
fn test_a_use_in_a_loop_is_wrapped_outside_the_loop() {
    // 0 -> {1, 5}; 1 (preheader) -> 2; 2 -> {3, 4}; 3 -> 2; 4 and 5 -> 6.
    // The frame is not set up each trip: save before the loop, restore after.
    let graph = Graph::of(&[&[1, 5], &[2], &[3, 4], &[2], &[6], &[6], &[]]);
    let frame = body(&[3], &[6]);
    assert_eq!(run(&graph, &frame, &frame), placed(1, 4, None));
}

#[test]
fn test_a_loop_the_entry_falls_into_leaves_the_frame_at_the_entry() {
    // The header's only way in is the entry: no block outside the loop to save
    // in.
    let graph = Graph::of(&[&[1, 4], &[2], &[1, 3], &[5], &[5], &[]]);
    let frame = body(&[2], &[5]);
    assert_eq!(run(&graph, &frame, &frame), None);
}

#[test]
fn test_a_save_hotter_than_the_entry_moves_up() {
    let graph = Graph::of(&[&[1, 2], &[3], &[3], &[]]);
    let mut frame = body(&[1], &[3]);
    frame.hot.insert(1, 500);
    assert_eq!(run(&graph, &frame, &frame), None);
}

#[test]
fn test_the_restore_is_split_so_the_clean_path_skips_it() {
    // 0 -> {1, 9}; 1 -> {2, 3}; 2 and 3 need the frame; 9 does not; all join in
    // 8, the only return. Save goes down to 1, the restore is split on 2
    // and 3.
    let graph = Graph::of(&[&[1, 4], &[2, 3], &[5], &[5], &[5], &[]]);
    let frame = body(&[2, 3], &[5]);
    assert_eq!(run(&graph, &frame, &frame), placed(1, 5, Some(&[2, 3])));
}

#[test]
fn test_two_returns_with_no_common_exit_are_not_split() {
    let graph = Graph::of(&[&[1, 2], &[], &[]]);
    let frame = body(&[1, 2], &[1, 2]);
    assert_eq!(run(&graph, &frame, &frame), None);
}

#[test]
fn test_a_body_that_touches_nothing_has_nothing_to_wrap() {
    let graph = Graph::of(&[&[1], &[]]);
    let frame = body(&[], &[1]);
    assert_eq!(run(&graph, &frame, &frame), None);
}
