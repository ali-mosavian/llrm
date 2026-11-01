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
        name: Some("mov".into()),
        dests: vec![held(dst)],
        sources: vec![held(src)],
        ..Semantics::new(Operation::Move)
    };
    Arc::new(Insn::new(0, None, Some(what), vec![dst], vec![src]))
}

/// A definition of `v` from nothing: not a copy.
fn def(v: u32) -> Arc<Insn> {
    let what = Semantics {
        name: Some("mov".into()),
        dests: vec![held(v)],
        sources: vec![Loc::Imm(crate::model::ir::Imm { value: 7, width: 4, address: None })],
        ..Semantics::new(Operation::Move)
    };
    Arc::new(Insn::new(0, None, Some(what), vec![v], vec![]))
}

/// A definition of `v` from a read of memory: not a copy, and not cheap to
/// make again.
fn load(v: u32) -> Arc<Insn> {
    let what = Semantics {
        name: Some("mov".into()),
        dests: vec![held(v)],
        sources: vec![Loc::Mem(crate::model::ir::Mem::new(None, 0))],
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

/// `v1 := def; v2 := mov v1` and nothing reads `v2`: the copy is dead, and so
/// then is the definition it alone read. Only the copy went, leaving a `v1`
/// that nothing reads (C5: `eliminateDeadDefs` was not run).
#[test]
fn test_a_dead_copy_takes_the_definition_only_it_read() {
    let (a, c) = (def(1), copy(2, 1));
    let was = body(&[(&[Arc::clone(&a), Arc::clone(&c)], &[])]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id());
    assert_eq!(result, JoinResult::Joined);
    assert!(names(&lis).is_empty(), "both the copy and the def are gone: {:?}", names(&lis));
}

/// `v1 := call; v2 := mov v1` and nothing reads `v2`: the copy goes, the call
/// stays. Run on the sweep's regparm3 matrix callers (#4152) it erased the
/// calls whose delivered value nothing read: wrong output.
#[test]
fn test_a_dead_copy_does_not_take_a_call_that_delivered_its_source() {
    let what = Semantics { name: Some("call".into()), ..Semantics::new(Operation::Call) };
    let mut call = Insn::new(0, None, Some(what), vec![1], vec![]);
    call.delivers = vec![(Held { value: 1, width: 4 }, llrm_lir::registers::RegId::EAX)];
    let (call, c) = (Arc::new(call), copy(2, 1));
    let was = body(&[(&[Arc::clone(&call), Arc::clone(&c)], &[])]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id());
    assert_eq!(result, JoinResult::Joined);
    assert_eq!(names(&lis), vec![(vec![1], vec![])], "the call stays, the copy goes");
}

/// `v25 := mov v24; v25 += v3` (`v3` dies), `v3 := mov v25`, then `v25 += v25`
/// reads that value and writes a new one. Commuting the first add to write `v3`
/// rewrote the second add whole, its definition too, so the value it made was
/// named `v3` and its reader named `v25` (m16 `decode` of a canonical Huffman
/// table: the code of the next bit was read from the wrong register).
#[test]
fn test_commuting_a_def_leaves_a_reader_that_redefines_the_value_alone() {
    let op = |dest: u32, source: u32| {
        let what = Semantics {
            name: Some("add".into()),
            dests: vec![held(dest)],
            sources: vec![held(dest), held(source)],
            ..Semantics::new(Operation::Binary)
        };
        Arc::new(Insn::new(0, None, Some(what), vec![dest], vec![dest, source]))
    };
    let copy_back = copy(3, 25);
    let insns = [load(3), load(24), copy(25, 24), op(25, 3), Arc::clone(&copy_back), op(25, 25), read(25), read(3)];
    let was = body(&[(&insns, &[])]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(3);
    lis.interval(25);
    super::join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), copy_back.id());
    let after = lis.body_like(&was);
    let all: Vec<_> = after.blocks.iter().flat_map(|one| one.insns.iter()).collect();
    let (add, read) = (all[all.len() - 3], all[all.len() - 2]);
    assert_eq!(read.uses, add.defines, "the read of the second add's value names what the add defines");
}

/// `v1` redefined while `v2` (a copy of its first value) is live: the join
/// would make `v2` read the new value. The copy stays and nothing changes.
#[test]
fn test_a_copy_whose_source_is_redefined_while_the_copy_lives_is_not_joined() {
    let (a, c, b, r1, r2) = (load(1), copy(2, 1), load(1), read(1), read(2));
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
    let (a, c, b, r1, r2) = (load(1), copy(2, 1), load(1), read(1), read(2));
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
        name: Some("mov".into()),
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
            name: Some("idiv".into()),
            dests: vec![held(t)],
            sources: vec![held(a), held(b)],
            ..Semantics::new(Operation::Binary)
        };
        Arc::new(Insn::new(0, None, Some(what), vec![t], vec![a, b]))
    };
    let branch = {
        let what = Semantics { name: Some("jne".into()), target: Some(1), ..Semantics::new(Operation::Branch) };
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
    let what = Semantics { name: Some("jmp".into()), target: Some(target), ..Semantics::new(Operation::Jump) };
    Arc::new(Insn::new(0, None, Some(what), vec![], vec![]))
}

/// A join verified both ranges and the joined one (`assert(verify())` of
/// LiveInterval.cpp is compiled out of LLVM's release builds): each is linear
/// in the range, so a run of joins was quadratic in the body (`diamond` 800:
/// `LiveRange::verify` 14 M of the coalescer's 86 M). Only
/// `LLRM_CHECK_LIVEINTERVALS` asks.
#[test]
fn test_a_join_does_not_verify_the_ranges() {
    let (a, c, r) = (def(1), copy(2, 1), read(2));
    let was = body(&[(&[a, Arc::clone(&c), r], &[])]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    let before = crate::liveness::liverange::verifies();
    assert_eq!(join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id()), JoinResult::Joined);
    assert_eq!(crate::liveness::liverange::verifies() - before, 0, "a join verified a range");
}

/// `v1 := 7; v2 := mov v1; v1 := 7; read v1; read v2`: the join is refused (the
/// source is redefined while the copy lives) and the source is a constant, so
/// the copy becomes `v2 := 7` as LLVM's `reMaterializeDef` makes it. It was
/// left as a copy, one register move that kept both values live.
#[test]
fn test_a_copy_of_a_constant_that_cannot_be_joined_is_made_again() {
    let (a, c, b, r1, r2) = (def(1), copy(2, 1), def(1), read(1), read(2));
    let was = body(&[(&[a, Arc::clone(&c), b, r1, r2], &[])]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id());
    assert_eq!(result, JoinResult::Joined);
    let kept = names(&lis);
    // The first definition of v1 is read by nothing now: it goes too.
    assert_eq!(kept, [(vec![2], vec![]), (vec![1], vec![]), (vec![], vec![1]), (vec![], vec![2])]);
    assert_consistent(&lis, &was, 2);
}

/// A copy of a loaded value is not cheap to make again: it stays.
#[test]
fn test_a_copy_of_a_load_that_cannot_be_joined_is_not_made_again() {
    let (a, c, b, r1, r2) = (load(1), copy(2, 1), def(1), read(1), read(2));
    let was = body(&[(&[a, Arc::clone(&c), b, r1, r2], &[])]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id());
    assert_eq!(result, JoinResult::Deferred);
    assert_eq!(names(&lis).len(), 5);
}

struct ForSize;

impl CoalescerEnv for ForSize {
    fn can_share(
        &self,
        _a: u32,
        _b: u32,
    ) -> bool {
        true
    }

    fn tuned_for_size(&self) -> bool {
        true
    }
}

/// For size the copy of a constant that stays read elsewhere is left a copy:
/// it was a 5-byte move for a 2-byte copy (`regparm3_matrix_22` +18 bytes).
#[test]
fn test_for_size_a_copy_of_a_constant_that_stays_read_is_not_made_again() {
    let (a, c, b, r1, r2) = (def(1), copy(2, 1), def(1), read(1), read(2));
    let was = body(&[(&[a, Arc::clone(&c), b, r1, r2], &[])]);
    let mut lis = LiveIntervals::new(&was);
    let result = join_copy(&mut lis, &ForSize, &mut State::new(), &mut Default::default(), c.id());
    assert_eq!(result, JoinResult::Deferred);
    assert_eq!(names(&lis).len(), 5);
}

/// `v2 := mov v1` for a base (BX, BP) and `v3 := mov v1` for an index (SI, DI):
/// each copy alone may join, but once one has, the merged value takes only the
/// base registers. Judged against `v1`'s first palette the second joined too,
/// leaving a value no register could hold: the generated regalloc bodies
/// failed to allocate.
#[test]
fn test_a_join_narrows_the_class_the_next_join_is_judged_against() {
    use std::collections::BTreeSet;

    use llrm_lir::registers::RegId;

    use super::env::TargetEnv;
    let palette = |registers: &[RegId]| registers.iter().copied().collect::<BTreeSet<_>>();
    let palettes = IndexMap::from_iter([
        (1, palette(&[RegId::AX, RegId::BX, RegId::SI])),
        (2, palette(&[RegId::BX, RegId::BP])),
        (3, palette(&[RegId::SI, RegId::DI])),
    ]);
    let env = TargetEnv {
        palettes: palettes.into(),
        pins: IndexMap::default(),
        size: false,
        keep_pinned: false,
        aliases: Default::default(),
        should_join: Box::new(|_, _, _| true),
    };
    let (base, index) = (copy(2, 1), copy(3, 1));
    // `v1` has two values to `v2`'s one, so the join keeps it as the surviving
    // value (the larger interval is the destination) and its palette is the
    // one that must narrow; the index copy reads its second value.
    let was = body(&[(&[def(1), Arc::clone(&base), read(2), def(1), Arc::clone(&index), read(3), read(1)], &[])]);
    let mut lis = LiveIntervals::new(&was);
    for value in 1..=3 {
        lis.interval(value);
    }
    let mut state = State::new();
    assert_eq!(join_copy(&mut lis, &env, &mut state, &mut Default::default(), base.id()), JoinResult::Joined);
    let index = state.current(index.id());
    assert_eq!(join_copy(&mut lis, &env, &mut state, &mut Default::default(), index), JoinResult::Rejected);
}

/// `v1 := load; v2 := mov v1; v1 := mov v2; v2 := load; read v1; read v2`: the
/// second copy cannot join (`v2` is redefined while its copy `v1` is live),
/// but `v2` holds what `v1` held and `v1` is dead between:
/// `adjustCopiesBackFrom` extends the first `v1` to the copy, and the copy
/// goes. The loop-carried segment of a far pointer in `_dl_muzzle@3` kept two
/// such copies a trip (`mov ax, es` at the latch).
#[test]
fn test_a_copy_back_of_a_copy_goes_by_extending_the_first_value() {
    let (first, to, back, redefined) = (load(1), copy(2, 1), copy(1, 2), load(2));
    let (r1, r2) = (read(1), read(2));
    let was = body(&[(
        &[
            Arc::clone(&first),
            Arc::clone(&to),
            Arc::clone(&back),
            Arc::clone(&redefined),
            Arc::clone(&r1),
            Arc::clone(&r2),
        ],
        &[],
    )]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), back.id());
    assert_eq!(result, JoinResult::Joined);
    assert_eq!(
        names(&lis),
        vec![(vec![1], vec![]), (vec![2], vec![1]), (vec![2], vec![]), (vec![], vec![1]), (vec![], vec![2])],
        "only the copy back is gone"
    );
    assert_eq!(lis.interval(1).valnos.iter().filter(|vn| !vn.is_unused()).count(), 1, "one value of v1 is left");
    assert_consistent(&lis, &was, 2);
}

/// `pre: v1 := load` and a one-block loop `v2 := mov v1; read v2; v1 := mov v2`
/// that branches to itself or out: the copy `v2 := mov v1` is redundant on the
/// way round the loop (`v1` holds `v2` after the reverse copy), so
/// `removePartialRedundancy` moves it to the preheader and the loop keeps the
/// reverse copy only. `_dl_muzzle@3` kept this pair for the segment of a far
/// pointer, two copies a trip.
#[test]
fn test_a_copy_at_a_loop_entry_that_the_latch_reverses_moves_to_the_preheader() {
    let (first, to, user, back) = (load(1), copy(2, 1), read(2), copy(1, 2));
    // After the loop `v1` is redefined while `v2` is still read: the join of
    // the two intervals fails, which is when the copy is moved instead.
    let (again, kept, after) = (load(1), read(2), read(1));
    let was = body(&[
        (&[Arc::clone(&first)], &[1]),
        (&[Arc::clone(&to), Arc::clone(&user), Arc::clone(&back)], &[1, 2]),
        (&[Arc::clone(&again), Arc::clone(&kept), Arc::clone(&after)], &[]),
    ]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    let copies_before = lis.blocks()[1].iter().filter(|one| one.defines == [2] && one.uses == [1]).count();
    assert_eq!(copies_before, 1, "the premise: the loop holds the copy");
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), to.id());
    assert_eq!(result, JoinResult::Joined);
    let in_loop = lis.blocks()[1].iter().filter(|one| one.defines == [2] && one.uses == [1]).count();
    let in_preheader = lis.blocks()[0].iter().filter(|one| one.defines == [2] && one.uses == [1]).count();
    assert_eq!((in_loop, in_preheader), (0, 1), "the copy moved out of the loop");
    assert_consistent(&lis, &was, 2);
}

/// A store of a register to memory, the other half of a spill: not a copy.
fn store(v: u32) -> Arc<Insn> {
    let what = Semantics {
        name: Some("mov".into()),
        dests: vec![Loc::Mem(crate::model::ir::Mem::new(None, 0))],
        sources: vec![held(v)],
        ..Semantics::new(Operation::Move)
    };
    Arc::new(Insn::new(0, None, Some(what), vec![], vec![v]))
}

/// The probe behind `LLRM_COALESCE_WHY` counted a store of a register to memory
/// as a left copy: nightmare_selftest read 68 left copies where the pin's 19
/// were the thing to match, and 13 were copies.
#[test]
fn test_a_store_of_a_register_is_no_left_copy() {
    let (a, c, b, r1, r2) = (load(1), copy(2, 1), load(1), read(1), read(2));
    let was = body(&[(&[a, c, b, r1, r2, store(1), store(2)], &[])]);
    let mut lis = LiveIntervals::new(&was);
    run(&mut lis, &AnyClass, &[0]);
    assert_eq!(super::env::why_left(&mut lis, &AnyClass), vec!["conflict (deferred)"]);
}

/// `v1 := op v1, v2` (tied) is `v1 := load; v1 := mov v3; v1 := add v1, v2;
/// v2 := mov v1; read v2; read v1`: `v2` dies at the `add` and the copy back
/// cannot join (the first `v1` overlaps `v2` with another value). The `add` is
/// commuted to `v2 := add v2, v1`, the readers read `v2` and the copy goes
/// (`removeCopyByCommutingDef`). Without it the copy stayed: a `mov` a trip in
/// the loops of `_dl_muzzle@3`.
#[test]
fn test_a_copy_after_a_commutable_two_address_def_goes_by_commuting_it() {
    let (b0, a2, tie, copy_back) = (load(2), load(3), copy(1, 3), copy(2, 1));
    let what = Semantics {
        name: Some("add".into()),
        dests: vec![held(1)],
        sources: vec![held(1), held(2)],
        ..Semantics::new(Operation::Binary)
    };
    let add = Arc::new(Insn::new(0, None, Some(what), vec![1], vec![1, 2]));
    let (r2, r1) = (read(2), read(1));
    let was = body(&[(
        &[
            Arc::clone(&b0),
            Arc::clone(&a2),
            Arc::clone(&tie),
            Arc::clone(&add),
            Arc::clone(&copy_back),
            Arc::clone(&r2),
            Arc::clone(&r1),
        ],
        &[],
    )]);
    let mut lis = LiveIntervals::new(&was);
    lis.interval(1);
    lis.interval(2);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), copy_back.id());
    assert_eq!(result, JoinResult::Joined);
    assert_eq!(
        names(&lis),
        vec![
            (vec![2], vec![]),
            (vec![3], vec![]),
            (vec![1], vec![3]),
            (vec![2], vec![2, 1]),
            (vec![], vec![2]),
            (vec![], vec![2]),
        ],
        "the add writes v2 and reads v1; the copy is gone; both readers read v2"
    );
    assert_consistent(&lis, &was, 2);
}

/// The terminal rule (`applyTerminalRule`, on by default at the pin). A loop
/// that rotates `y2 := y1; y1 := y`: the header copy `v4 := v25` of `y2` has a
/// destination no other copy names, and `v25`'s other copy in the block
/// (`v25 := v1`) is of a value that overlaps it, so the header copy is tried
/// last and the bottom copy joins. Joined first, the header copy stretched
/// `v25` over the body, the bottom copy conflicted and stayed, and x_iir had a
/// `mov` at each end of its loop (one more per trip than the pin).
#[test]
fn test_a_header_copy_of_a_terminal_value_is_tried_after_the_copy_it_overlaps() {
    let (pre25, pre22) = (def(25), def(22));
    let (y2, y1) = (copy(4, 25), copy(1, 22));
    let (read_y2, read_y1, new_y) = (read(4), read(1), def(6));
    let (to22, to25) = (copy(22, 6), copy(25, 1));
    let was = body(&[
        (&[pre25, pre22], &[1]),
        (&[y2, y1, read_y2, read_y1, new_y, to22, Arc::clone(&to25)], &[1, 2]),
        (&[read(25), read(22)], &[]),
    ]);
    let mut lis = LiveIntervals::new(&was);
    run(&mut lis, &AnyClass, &[0, 1, 0]);
    let copies: Vec<(Vec<u32>, Vec<u32>)> = lis.blocks()[1]
        .iter()
        .filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Move) && !one.uses.is_empty())
        .map(|one| (one.defines.clone(), one.uses.clone()))
        .collect();
    assert_eq!(copies, vec![(vec![4], vec![25]), (vec![25], vec![22])], "both copies at the top: {:?}", names(&lis));
}

/// `copy` with its source an undef read: `v2 := mov undef v1`.
fn undef_copy(
    dst: u32,
    src: u32,
) -> Arc<Insn> {
    let mut one = (*copy(dst, src)).clone();
    one.uses.clear();
    one.undefs = vec![src];
    Arc::new(one)
}

/// `v2 := mov undef v1; read v2` (`eliminateUndefCopy`): the copy of nothing
/// becomes `v2 := implicit_def` and is not coalesced. Without the port the
/// copy stayed and spill-weight calculation panicked "Copy from non-existing
/// value" on bench/crc at -O2.
#[test]
fn test_a_copy_of_an_undefined_value_becomes_an_implicit_def() {
    let (c, r) = (undef_copy(2, 1), read(2));
    let was = body(&[(&[Arc::clone(&c), Arc::clone(&r)], &[])]);
    let mut lis = LiveIntervals::new(&was);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id());
    assert_eq!(result, JoinResult::Rejected);
    let kept: Vec<_> = lis.blocks().iter().flatten().cloned().collect();
    assert_eq!(kept.len(), 2);
    let name = kept[0].what.as_ref().and_then(|what| what.name.clone());
    assert_eq!(name.as_deref(), Some(crate::processimpdefs::IMPLICIT_DEF), "{:?}", kept[0]);
    assert!(kept[0].uses.is_empty() && kept[0].undefs.is_empty() && kept[0].defines == vec![2]);
    assert_eq!(kept[1].uses, vec![2]);
}

/// `v1 := mov undef v1; read v1`: the same, with one value (the crc shape).
#[test]
fn test_a_copy_of_an_undefined_value_to_itself_becomes_an_implicit_def() {
    let (c, r) = (undef_copy(1, 1), read(1));
    let was = body(&[(&[Arc::clone(&c), Arc::clone(&r)], &[])]);
    let mut lis = LiveIntervals::new(&was);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), c.id());
    assert_eq!(result, JoinResult::Rejected);
    let first = lis.blocks().iter().flatten().next().cloned().unwrap();
    assert_eq!(
        first.what.as_ref().and_then(|what| what.name.clone()).as_deref(),
        Some(crate::processimpdefs::IMPLICIT_DEF)
    );
}
