//! `coalesce_copy` cloned the whole function for every `lda cell; sta
//! cell` pair it looked at, as `forward_cell_copy` did: a function of n pairs
//! cost n x n lines. Looking at a pair allocates nothing the pair does not
//! rewrite.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_6502::asm::Asm;
use llrm_6502::peephole_cells::coalesce_copy;

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
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
fn looking_at_copies_allocates_in_proportion_to_the_function() {
    let pairs = 1500;
    // `rc5` is read at once, but `rc4` is read again by the next group: no copy
    // goes
    let text: String = (0..pairs).map(|_| "  lda rc4\n  sta rc5\n  ldx rc5\n  stx rc6\n").collect();
    let mut asm = Asm::parse(&text);
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    coalesce_copy(&mut asm);
    let made = ALLOCATIONS.load(Ordering::Relaxed) - before;
    assert!(made < 20 * pairs, "{made} allocations for {pairs} pairs");
}
