//! Counting the allocations a piece of code makes, for a test of it.
//!
//! # How it works
//! A test binary declares `Counting` as its global allocator and wraps what it
//! measures in [`counted`]:
//!
//! ```ignore
//! #[global_allocator]
//! static COUNTING: llrm_support::alloc_count::Counting = llrm_support::alloc_count::Counting;
//!
//! let (graph, made) = llrm_support::alloc_count::counted(|| cfg::graph(function));
//! assert!(made <= blocks + 2, "{made} allocations for {blocks} blocks");
//! ```
//!
//! The count is this thread's: an allocation (or a `realloc`, which is one) by
//! another thread is not in it, and nothing here allocates. A conversion to a
//! container that holds its elements inline is a fail-first test of this form:
//! the allocations of a fixed body before and after.
//!
//! # Used by
//! Tests of the inline vectors' users.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static MADE: Cell<usize> = const { Cell::new(0) };
}

/// The global allocator of a test binary that counts.
pub struct Counting;

/// The allocations this thread has made.
pub fn allocations() -> usize {
    MADE.with(Cell::get)
}

/// `run`, and how many allocations this thread made while it ran.
pub fn counted<R>(run: impl FnOnce() -> R) -> (R, usize) {
    let before = allocations();
    let out = run();
    (out, allocations() - before)
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        MADE.with(|made| made.set(made.get() + 1));
        // SAFETY: the caller's layout, passed on.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
    ) {
        // SAFETY: a block of `System`'s, with its layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        MADE.with(|made| made.set(made.get() + 1));
        // SAFETY: the caller's layout, passed on.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> *mut u8 {
        MADE.with(|made| made.set(made.get() + 1));
        // SAFETY: a block of `System`'s, resized as the caller asks.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
