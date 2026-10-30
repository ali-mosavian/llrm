//! Scheduling a chain took 12 allocations per node (a `Vec` of users, a work
//! stack per depth, height and Sethi-Ullman walk, a cloned operand list, a
//! cloned unit): 12,230 for 1,000 nodes. It takes 7.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_codegen::backend::selectiondag::schedule::{SchedInfo, schedule};
use llrm_codegen::backend::selectiondag::{DagConfig, EVT, ISD, SDValue, SelectionDAG};

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

const LOAD: u32 = 2;
const STORE: u32 = 3;

struct Info;

impl SchedInfo for Info {
    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        usize::from(opcode == LOAD)
    }

    fn implicit_defs(
        &self,
        _opcode: u32,
    ) -> &[u16] {
        &[]
    }

    fn is_call(
        &self,
        _opcode: u32,
    ) -> bool {
        false
    }
}

#[test]
fn test_scheduling_a_chain_takes_under_eight_allocations_per_node() {
    const N: usize = 1000;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let address = dag.get_register(EVT::I32, 1);
    let mut chain = dag.entry_node();
    for _ in 0..N {
        let load = dag.get_node_multi(ISD::Machine(LOAD), &[EVT::I32, EVT::OTHER], &[address, chain]);
        chain = SDValue { node: load.node, res: 1 };
    }
    let root = dag.get_node_multi(ISD::Machine(STORE), &[EVT::OTHER], &[address, address, chain]);
    let before = ALLOCATED.with(Cell::get);
    let units = schedule(&dag, root, &Info);
    let allocated = ALLOCATED.with(Cell::get) - before;
    assert_eq!(units.len(), N + 1);
    assert!(allocated < MOST_PER_NODE * N, "{allocated} allocations to schedule {N} nodes");
}

const MOST_PER_NODE: usize = 8;
