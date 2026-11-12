//! Refusing a copy copied intervals it then did not change:
//! - `adjust_copies_back_from` copied the destination's whole interval before
//!   it checked the copy: 74 KB for two 400-segment intervals, 55 KB after.
//! - `join_virt_regs` copied both intervals before the high-cost check that
//!   refuses a long interval joined too often: 55 KB for each of the refusals
//!   (fill_nest on 6502: 1540 of 3402 calls, 8% of the compile).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use llrm_codegen::coalescer::register_coalescer::State;
use llrm_codegen::coalescer::{CoalescerEnv, JoinResult, join_copy};
use llrm_codegen::liveness::liveintervals::LiveIntervals;
use llrm_codegen::model::ir::{Held, Loc, Mem, Operation, Semantics};
use llrm_codegen::model::lir::{Block, Insn, LirBlock, LirBody};

struct Counting;

thread_local! {
    static BYTES: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        BYTES.with(|count| count.set(count.get() + layout.size()));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(
        &self,
        pointer: *mut u8,
        layout: Layout,
    ) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

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
    Loc::Held(Held { value, width: 4, offset: 0 })
}

fn mov(
    dst: u32,
    src: Option<u32>,
) -> Arc<Insn> {
    let source = src.map_or(Loc::Mem(Mem::new(None, 0)), held);
    let what = Semantics {
        name: Some("mov".into()),
        dests: vec![held(dst)],
        sources: vec![source],
        ..Semantics::new(Operation::Move)
    };
    Arc::new(Insn::new(0, None, Some(what), vec![dst], src.into_iter().collect()))
}

fn read(values: &[u32]) -> Arc<Insn> {
    Arc::new(Insn::new(0, None, None, vec![], values.to_vec()))
}

/// `v1 := [m]; v2 := v1; read v2; v2 := [m]; read v1`, then `blocks` blocks
/// that define and read both again: `v1` is live where `v2` takes a second
/// value, so the copy cannot join, and each interval has 400 or more values.
fn interfering() -> (LiveIntervals, u64) {
    let blocks = 400;
    let copy = mov(2, Some(1));
    let mut made = vec![LirBlock::new(0, vec![mov(1, None), Arc::clone(&copy), read(&[2]), mov(2, None), read(&[1])])];
    for at in 1..=blocks {
        made.push(LirBlock::new(at, vec![mov(1, None), mov(2, None), read(&[1, 2])]));
    }
    for (at, block) in made.iter_mut().enumerate() {
        if at < blocks as usize {
            block.succ = vec![at as i64 + 1].into();
        }
    }
    let body =
        LirBody::new("long", 0, made.into_iter().map(Block::from).collect(), Default::default(), Default::default());
    let mut lis = LiveIntervals::new(&body);
    lis.interval(1);
    lis.interval(2);
    (lis, copy.id())
}

fn bytes_of_join(
    lis: &mut LiveIntervals,
    state: &mut State,
    copy: u64,
) -> usize {
    let before = BYTES.with(Cell::get);
    let result = join_copy(lis, &AnyClass, state, &mut Default::default(), copy);
    assert_ne!(result, JoinResult::Joined, "the premise: the values interfere");
    BYTES.with(Cell::get) - before
}

#[test]
fn refusing_a_copy_between_long_intervals_copies_no_interval_for_the_copy_back() {
    let (mut lis, copy) = interfering();
    let spent = bytes_of_join(&mut lis, &mut State::new(), copy);
    assert!(spent < 65_000, "refusing the join allocated {spent} bytes");
}

#[test]
fn a_copy_refused_as_high_cost_copies_no_interval() {
    let (mut lis, copy) = interfering();
    let mut state = State::new();
    let mut last = 0;
    for _ in 0..300 {
        last = bytes_of_join(&mut lis, &mut state, copy);
    }
    assert!(last < 1_000, "a join refused as high cost allocated {last} bytes");
}
