//! The dead-store and dead-register walks looked a branch's label up by
//! building a `Line::Label(String)` to compare with: one allocation per branch
//! walked, so a function of n branches cost n allocations a query (`idioms`
//! -O2: 12% of the compile's instructions in `dead_from_pushed`, 2026-11-15).
//! Walking a branch allocates nothing.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_6502::asm::Asm;

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

const BRANCHES: usize = 1000;

fn allocations(
    text: &str,
    pass: fn(&mut Asm),
) -> usize {
    let mut asm = Asm::parse(text);
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    pass(&mut asm);
    ALLOCATIONS.load(Ordering::Relaxed) - before
}

fn branches() -> String {
    (0..BRANCHES).map(|i| format!("  beq l{i}\nl{i}:\n  nop\n")).collect()
}

/// One test: the counter is the process's, so two running side by side would
/// count each other's allocations.
#[test]
fn a_walk_allocates_nothing_per_branch() {
    // the store is dead: the walk follows every branch to the `rts`
    let text = format!("  lda #1\n  sta rc4\n{}  rts\n", branches());
    let made = allocations(&text, llrm_6502::peephole_cells::drop_dead_stores);
    assert!(made < BRANCHES / 4, "dead store: {made} allocations for {BRANCHES} branches");
    // `lda #1` is dead: the next `lda` writes `a` after every branch
    let text = format!("  lda #1\n{}  lda #2\n  sta rc4\n  rts\n", branches());
    let made = allocations(&text, llrm_6502::peephole::drop_dead_writes);
    assert!(made < BRANCHES / 4, "dead register: {made} allocations for {BRANCHES} branches");
}
