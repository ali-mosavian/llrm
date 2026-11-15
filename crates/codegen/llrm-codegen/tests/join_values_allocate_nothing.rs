//! Joining two intervals analyzed each of their values (`JoinVals::
//! compute_assignment`) through a fresh `Rc` of the defining instruction:
//! one allocation per value, 3% of the instructions of `idioms` on 6502.

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
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCATED.with(|count| count.set(count.get() + 1));
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

#[test]
fn joining_two_intervals_allocates_nothing_for_each_value_it_analyzes() {
    let blocks = 400;
    let copy = mov(2, Some(1));
    let mut made = vec![LirBlock::new(0, vec![mov(1, None), Arc::clone(&copy), read(&[2])])];
    for at in 1..=blocks {
        made.push(LirBlock::new(at, vec![mov(1, None), read(&[1]), mov(2, None), read(&[2])]));
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
    let before = ALLOCATED.with(Cell::get);
    let result = join_copy(&mut lis, &AnyClass, &mut State::new(), &mut Default::default(), copy.id());
    let made = ALLOCATED.with(Cell::get) - before;
    assert_eq!(result, JoinResult::Joined, "the premise: the two never overlap");
    // 802 values: 4246 allocations before, 3436 after
    assert!(made < 3800, "joining 802 values allocated {made} times");
}
