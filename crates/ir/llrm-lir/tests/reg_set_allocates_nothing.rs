//! `masm::_roots` put every register an instruction names in a `BTreeSet`: a
//! node and a tree walk for each of the 85,000 inserts of a d_faces -O2
//! compile. A set of registers is four words held in place, and the order of a
//! `BTreeSet`'s.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_lir::registers::{RegId, RegSet};

struct Counting;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCATED.fetch_add(1, Ordering::Relaxed);
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

#[test]
fn test_a_set_of_registers_allocates_nothing_and_orders_as_a_tree_does() {
    let registers: Vec<RegId> = RegId::values().step_by(3).collect();
    let before = ALLOCATED.load(Ordering::Relaxed);
    let mut set = RegSet::new();
    for _ in 0..100 {
        for register in registers.iter().rev() {
            set.insert(*register);
        }
    }
    let found = set.iter().count();
    let allocated = ALLOCATED.load(Ordering::Relaxed) - before;
    assert_eq!(allocated, 0, "{allocated} allocations to fill a set of registers");
    let before = ALLOCATED.load(Ordering::Relaxed);
    let tree: BTreeSet<RegId> = registers.iter().copied().collect();
    assert!(ALLOCATED.load(Ordering::Relaxed) > before, "premise: the counter sees a tree's nodes");
    assert_eq!(found, tree.len());
    assert_eq!(set.iter().collect::<Vec<_>>(), tree.iter().copied().collect::<Vec<_>>());
    assert!(registers.iter().all(|register| set.contains(register)));
    assert_eq!(set.len(), tree.len());
    for register in RegId::values() {
        assert_eq!(RegId::from_index(register.index()), register, "every register comes back from its number");
    }
}
