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

/// A parameter is a value defined at the function's entry. Joining it into the
/// value of its copy must leave the body naming the survivor as the parameter:
/// a body that still named the absorbed value gave the survivor an interval
/// with no definition, which the check against a fresh computation caught.
#[test]
fn test_a_parameter_joined_into_its_copy_stays_a_parameter() {
    let (c, r) = (copy(2, 1), read(2));
    let was = LirBody { inputs: [1].into_iter().collect(), ..body(&[(&[Arc::clone(&c), r], &[])]) };
    let mut lis = LiveIntervals::new(&was);
    assert_eq!(join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id()), JoinResult::Joined);
    let after = lis.body_like(&was);
    assert_eq!(after.inputs.len(), 1);
    let survivor = *after.inputs.iter().next().expect("a parameter");
    assert_eq!(names(&lis), vec![(vec![], vec![survivor])], "the read names the parameter");
    assert!(lis.assert_intervals_cover_afresh(&after) >= 1);
}

/// Two parameters, both defined on entry and both live there, are different
/// values: copying the first into the second later does not make them one. They
/// were joined (the copy removed, the two arguments one register) before the
/// entry's definitions stopped counting as phis.
#[test]
fn test_two_parameters_live_on_entry_are_not_joined() {
    let (c, r1, r2, r) = (copy(2, 1), read(1), read(2), read(2));
    let was = LirBody { inputs: [1, 2].into_iter().collect(), ..body(&[(&[r1, r2, Arc::clone(&c), r], &[])]) };
    let mut lis = LiveIntervals::new(&was);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id());
    assert_eq!(result, JoinResult::Deferred);
    assert_eq!(names(&lis).len(), 4, "no instruction was erased");
}

/// Two arms each copy their own value into `v2`, read after the join: the
/// copies join (or not) so that every value read is one something defines.
/// (Ported from the old coalescer's
/// `two_copies_into_one_value_leave_every_read_defined`.)
#[test]
fn test_two_copies_into_one_value_leave_every_read_defined() {
    let arm = |value: u32, at: i64| block_at(at, vec![def(value), copy(2, value), jump(0x20)], &[0x20]);
    let was = body_of(vec![arm(61, 0), arm(63, 0x10), block_at(0x20, vec![read(2)], &[])]);
    let mut lis = LiveIntervals::new(&was);
    let stats = super::driver::run(&mut lis, &AnyClass, &[0, 0, 0]);
    assert!(stats.joined >= 1, "premise: something joined: {stats:?}");
    let after = lis.body_like(&was);
    let made: std::collections::BTreeSet<u32> =
        after.blocks.iter().flat_map(|one| one.insns.iter()).flat_map(|one| one.defines.clone()).collect();
    let read: std::collections::BTreeSet<u32> =
        after.blocks.iter().flat_map(|one| one.insns.iter()).flat_map(|one| one.uses.clone()).collect();
    let missing: Vec<u32> = read.difference(&made).copied().collect();
    assert!(missing.is_empty(), "read with nothing defining it: {missing:?}");
    assert!(lis.assert_intervals_cover_afresh(&after) >= 1);
}

/// A joined copy's destination is the address of a later load: the load's
/// memory operand names the surviving value, which something still defines.
#[test]
fn test_a_coalesced_address_keeps_its_memory_operand_defined() {
    use crate::model::ir::Mem;
    let memory = Mem { base: Some(Held { value: 2, width: 2 }), ..Mem::new(None, 2) };
    let what = Semantics {
        name: Some("mov".to_owned()),
        dests: vec![held(3)],
        sources: vec![Loc::Mem(memory)],
        ..Semantics::new(Operation::Move)
    };
    let load = Arc::new(Insn::new(0, None, Some(what), vec![3], vec![2]));
    let was = body(&[(&[def(1), copy(2, 1), load, read(1), read(3)], &[])]);
    let mut lis = LiveIntervals::new(&was);
    let stats = super::driver::run(&mut lis, &AnyClass, &[0]);
    assert!(stats.joined >= 1, "{stats:?}");
    let after = lis.body_like(&was);
    let made: std::collections::BTreeSet<u32> =
        after.blocks.iter().flat_map(|one| one.insns.iter()).flat_map(|one| one.defines.clone()).collect();
    for one in after.blocks.iter().flat_map(|one| one.insns.iter()) {
        for value in one.uses.iter().chain(&one.defines) {
            assert!(made.contains(value), "v{value} is named and defined by nothing");
        }
    }
}

/// Values pinned to different registers are not joined, whatever their
/// intervals: the environment says they cannot share.
#[test]
fn test_values_pinned_to_different_registers_are_not_joined() {
    use llrm_lir::registers::RegId;
    struct Pinned;
    impl CoalescerEnv for Pinned {
        fn can_share(
            &self,
            a: u32,
            b: u32,
        ) -> bool {
            let pin = |value: u32| match value {
                1 => Some(RegId::ES),
                2 => Some(RegId::FS),
                _ => None,
            };
            !matches!((pin(a), pin(b)), (Some(x), Some(y)) if x != y)
        }
    }
    let (c, r) = (copy(2, 1), read(2));
    let was = body(&[(&[def(1), Arc::clone(&c), r], &[])]);
    let mut lis = LiveIntervals::new(&was);
    assert_eq!(join_copy(&mut lis, &Pinned, &mut State::new(), &mut Default::default(), c.id()), JoinResult::Rejected);
    assert_eq!(names(&lis).len(), 3);
}

/// `a, b = b, t` at the end of a loop, as one parallel copy (`gcd`'s
/// `a = b; b = a % b`): after the group `a` is the old `b`, and `b` the new
/// value, both live round the loop. Joining `a` with `b` (the first copy then
/// "an identity") made `a` the new `b`: gcd(1071, 462) printed 0.
fn swap_loop() -> LirBody {
    let (a, b, t, saved) = (4, 3, 5, 12);
    let grouped = |one: Arc<Insn>| {
        let mut made = (*one).clone();
        made.group = Some(2);
        Arc::new(made)
    };
    let step = {
        let what = Semantics {
            name: Some("idiv".to_owned()),
            dests: vec![held(t)],
            sources: vec![held(a), held(b)],
            ..Semantics::new(Operation::Binary)
        };
        Arc::new(Insn::new(0, None, Some(what), vec![t], vec![a, b]))
    };
    let branch = {
        let what = Semantics { name: Some("jne".to_owned()), target: Some(1), ..Semantics::new(Operation::Branch) };
        Arc::new(Insn::new(0, None, Some(what), vec![], vec![]))
    };
    body_of(vec![
        block_at(0, vec![def(a), def(b), jump(1)], &[1]),
        block_at(1, vec![step, copy(saved, b), grouped(copy(a, b)), grouped(copy(b, t)), branch], &[1, 2]),
        block_at(2, vec![read(saved)], &[]),
    ])
}

#[test]
fn test_a_parallel_swap_round_a_loop_is_not_joined_away() {
    let was = swap_loop();
    let mut lis = LiveIntervals::new(&was);
    super::driver::run(&mut lis, &AnyClass, &[0, 1, 0]);
    let after = lis.body_like(&was);
    let loop_copies: Vec<(u32, u32)> = after.blocks[1]
        .insns
        .iter()
        .filter_map(|one| crate::coalescer::pair::full_copy(one).map(|(d, s)| (d.value, s.value)))
        .collect();
    // `a` takes the old `b` while `b` takes `t`: a copy of `b` into something
    // other than `b` itself must remain in the loop, and the exit's saved
    // value must not have become `b`.
    assert!(loop_copies.iter().any(|(dst, src)| src == &3 && dst != &3), "a's copy of b went away: {loop_copies:?}");
    let exit_read = after.blocks[2].insns[0].uses[0];
    assert!(
        loop_copies.iter().any(|(dst, src)| *dst == exit_read && *src == 3) || exit_read != 3,
        "the exit reads b after the loop rewrote it: v{exit_read}, {loop_copies:?}"
    );
}

fn block_at(
    at: i64,
    insns: Vec<Arc<Insn>>,
    succ: &[i64],
) -> LirBlock {
    LirBlock { succ: succ.to_vec().into(), ..LirBlock::new(at, insns) }
}

fn body_of(blocks: Vec<LirBlock>) -> LirBody {
    LirBody::new("joined", 0, blocks.into_iter().map(Block::from).collect(), IndexMap::default(), IndexMap::default())
}

fn jump(target: i64) -> Arc<Insn> {
    let what = Semantics { name: Some("jmp".to_owned()), target: Some(target), ..Semantics::new(Operation::Jump) };
    Arc::new(Insn::new(0, None, Some(what), vec![], vec![]))
}
