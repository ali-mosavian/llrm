use std::sync::Arc;

use super::{CoalescerEnv, JoinResult, join_copy, register_coalescer::State};
use crate::liveness::liveintervals::LiveIntervals;
use crate::model::ir::{Held, Loc, Operation, Semantics};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

struct AnyClass;

impl CoalescerEnv for AnyClass {
    fn can_share(
        &self,
        _a: u32,
        _b: u32,
    ) -> bool {
        true
    }
}

fn held(value: u32) -> Loc {
    Loc::Held(Held { value, width: 4 })
}

fn copy(
    dst: u32,
    src: u32,
) -> Arc<Insn> {
    let what = Semantics {
        name: Some("mov".to_owned()),
        dests: vec![held(dst)],
        sources: vec![held(src)],
        ..Semantics::new(Operation::Move)
    };
    Arc::new(Insn::new(0, None, Some(what), vec![dst], vec![src]))
}

/// A definition of `v` from nothing: not a copy.
fn def(v: u32) -> Arc<Insn> {
    let what = Semantics {
        name: Some("mov".to_owned()),
        dests: vec![held(v)],
        sources: vec![Loc::Imm(crate::model::ir::Imm { value: 7, width: 4, address: None })],
        ..Semantics::new(Operation::Move)
    };
    Arc::new(Insn::new(0, None, Some(what), vec![v], vec![]))
}

fn read(v: u32) -> Arc<Insn> {
    Arc::new(Insn::new(0, None, None, vec![], vec![v]))
}

fn body(blocks: &[(&[Arc<Insn>], &[i64])]) -> LirBody {
    let made = blocks
        .iter()
        .enumerate()
        .map(|(at, (insns, succ))| {
            Block::from(LirBlock { succ: succ.to_vec().into(), ..LirBlock::new(at as i64, insns.to_vec()) })
        })
        .collect();
    LirBody::new("joined", 0, made, IndexMap::default(), IndexMap::default())
}

fn names(lis: &LiveIntervals) -> Vec<(Vec<u32>, Vec<u32>)> {
    lis.blocks().iter().flatten().map(|one| (one.defines.clone(), one.uses.clone())).collect()
}

/// The join is right when the intervals it left are the ones a fresh
/// computation makes over the function as edited, and at least one was
/// compared (a check that compares nothing passes anything).
fn assert_consistent(
    lis: &LiveIntervals,
    was: &LirBody,
    at_least: usize,
) {
    assert!(lis.assert_intervals_cover_afresh(&lis.body_like(was)) >= at_least);
}

/// `v1 := def; v2 := mov v1; read v2`: the copy goes and the value is one.
#[test]
fn test_a_copy_between_values_that_do_not_overlap_is_joined_and_removed() {
    let (a, c, r) = (def(1), copy(2, 1), read(2));
    let was = body(&[(&[Arc::clone(&a), Arc::clone(&c), Arc::clone(&r)], &[])]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id());
    assert_eq!(result, JoinResult::Joined);
    let kept = names(&lis);
    assert_eq!(kept.len(), 2, "the copy is gone");
    let survivor = kept[0].0[0];
    assert_eq!(kept[1], (vec![], vec![survivor]), "the read names the value the def does");
    assert!(!lis.has_interval(if survivor == 1 { 2 } else { 1 }), "the absorbed interval is gone");
    assert_consistent(&lis, &was, 1);
}

/// `v1` redefined while `v2` (a copy of its first value) is live: the join
/// would make `v2` read the new value. The copy stays and nothing changes.
#[test]
fn test_a_copy_whose_source_is_redefined_while_the_copy_lives_is_not_joined() {
    let (a, c, b, r1, r2) = (def(1), copy(2, 1), def(1), read(1), read(2));
    let insns = [a, Arc::clone(&c), b, r1, r2];
    let was = body(&[(&insns, &[])]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id());
    assert_eq!(result, JoinResult::Deferred);
    assert_eq!(names(&lis).len(), 5, "no instruction was erased");
    assert!(lis.has_interval(1) && lis.has_interval(2));
    assert_consistent(&lis, &was, 2);
}

/// `v1 := def; v2 := mov v1; v1 := mov v2; read v1`: after joining, the second
/// copy is an identity too, and goes with the first.
#[test]
fn test_copies_back_and_forth_are_both_removed() {
    let (a, c, back, r) = (def(1), copy(2, 1), copy(1, 2), read(1));
    let was = body(&[(&[a, Arc::clone(&c), back, r], &[])]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    assert_eq!(join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id()), JoinResult::Joined);
    let kept = names(&lis);
    assert_eq!(kept.len(), 2, "the def and the read are left: {kept:?}");
    assert_consistent(&lis, &was, 1);
}

/// A copy across a block edge: the value is live through the diamond's arms.
#[test]
fn test_a_copy_in_one_block_for_a_read_in_a_later_one_is_joined() {
    let (a, c, r) = (def(1), copy(2, 1), read(2));
    let was = body(&[(&[Arc::clone(&a), Arc::clone(&c)], &[1, 2]), (&[], &[2]), (&[Arc::clone(&r)], &[])]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    assert_eq!(join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id()), JoinResult::Joined);
    assert_eq!(names(&lis).len(), 2);
    assert_consistent(&lis, &was, 1);
}

/// The interval with more segments stays (`size()`): `v1` has a hole, `v2` is
/// one segment, so `v2` goes into `v1` although the copy reads `v1`.
#[test]
fn test_the_interval_with_more_segments_is_the_one_that_stays() {
    let (a, ra, b, c, r2, rb) = (def(1), read(1), def(1), copy(2, 1), read(2), read(1));
    let was = body(&[(&[a, ra, b, Arc::clone(&c), r2, rb], &[])]);
    let mut lis = LiveIntervals::new(&was);
    assert!(lis.interval(1).size() > lis.interval(2).size(), "the premise: v1 has the more segments");
    assert_eq!(join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id()), JoinResult::Joined);
    assert!(lis.has_interval(1) && !lis.has_interval(2), "v2 was absorbed");
    assert_consistent(&lis, &was, 1);
}

use super::driver::{MbbPriority, compare_mbb_priority, run};

fn priority(
    block: usize,
    depth: u32,
    is_split: bool,
    connectivity: usize,
) -> MbbPriority {
    MbbPriority { block, depth, is_split, connectivity }
}

/// `compareMBBPriority`: the deeper loop, then the split edge, then the better
/// connected, then the lower number.
#[test]
fn test_blocks_are_ordered_by_depth_then_split_edge_then_connectivity_then_number() {
    let mut blocks = vec![
        priority(0, 0, false, 2),
        priority(1, 1, false, 2),
        priority(2, 1, true, 2),
        priority(3, 1, false, 4),
        priority(4, 1, false, 4),
    ];
    blocks.sort_by(compare_mbb_priority);
    assert_eq!(blocks.iter().map(|one| one.block).collect::<Vec<_>>(), vec![2, 3, 4, 1, 0]);
}

/// A chain of copies in one block is all joined by one round: one value stays.
#[test]
fn test_a_chain_of_copies_is_joined_to_one_value() {
    let (a, c1, c2, r) = (def(1), copy(2, 1), copy(3, 2), read(3));
    let was = body(&[(&[a, c1, c2, r], &[])]);
    let mut lis = LiveIntervals::new(&was);
    let stats = run(&mut lis, &AnyClass, &[0]);
    assert_eq!(stats.joined, 2);
    assert_eq!(stats.left, 0);
    let kept = names(&lis);
    assert_eq!(kept.len(), 2, "{kept:?}");
    assert_eq!(kept[0].0, kept[1].1, "the read names the value the def defines");
    assert_consistent(&lis, &was, 1);
}

/// A copy that cannot join stays, and the run ends: no round loops forever on
/// it.
#[test]
fn test_a_copy_that_cannot_join_is_left_and_the_run_ends() {
    let (a, c, b, r1, r2) = (def(1), copy(2, 1), def(1), read(1), read(2));
    let was = body(&[(&[a, c, b, r1, r2], &[])]);
    let mut lis = LiveIntervals::new(&was);
    let stats = run(&mut lis, &AnyClass, &[0]);
    assert_eq!((stats.joined, stats.left), (0, 1));
    assert_eq!(names(&lis).len(), 5);
    assert_consistent(&lis, &was, 2);
}

/// Copies across blocks: the copy in a loop and the one after it, both joined.
#[test]
fn test_copies_in_and_after_a_loop_are_joined() {
    let (a, inner, after, r) = (def(1), copy(2, 1), copy(3, 1), read(3));
    let was = body(&[(&[a], &[1]), (&[inner.clone(), read(2)], &[1, 2]), (&[after, r], &[])]);
    let mut lis = LiveIntervals::new(&was);
    let stats = run(&mut lis, &AnyClass, &[0, 1, 0]);
    assert_eq!(stats.left, 0);
    assert!(stats.joined >= 1);
    assert_consistent(&lis, &was, 1);
}

/// Two copies of one value: the first join renames the second copy's source,
/// which gives the instruction a new key. The work list must follow it. A run
/// that looked the stale key up panicked on `d_faces` (`no entry found for
/// key` in `LiveIntervals::instr`).
#[test]
fn test_a_copy_renamed_by_an_earlier_join_is_still_tried() {
    let (a, c1, c2, r2, r3) = (def(1), copy(2, 1), copy(3, 1), read(2), read(3));
    let was = body(&[(&[a, c1, c2, r2, r3], &[])]);
    let mut lis = LiveIntervals::new(&was);
    let stats = run(&mut lis, &AnyClass, &[0]);
    assert_eq!(stats.joined, 2);
    assert_eq!(names(&lis).len(), 3, "{:?}", names(&lis));
    assert_consistent(&lis, &was, 1);
}
