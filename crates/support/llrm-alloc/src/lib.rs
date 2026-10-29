//! The compilers' global allocator: a free list for each small size class,
//! mimalloc for the rest.
//!
//! # How it works
//! A request of up to `LIMIT` (1,024) bytes and an alignment of up to 16 is
//! rounded up to a multiple of 16: its size class. Each thread keeps one
//! singly linked list of free blocks for each class. `alloc` pops the list of
//! its class, or cuts a block from the thread's current chunk (a 1 MiB block
//! of mimalloc's); `dealloc` pushes the block on the freeing thread's list.
//! Nothing is coalesced or returned: a block stays in its class for the life
//! of the process. Anything larger, or aligned more than 16, is mimalloc's
//! unchanged.
//!
//! A compile makes 3.2 M allocations, 61% of them of 64 bytes or less and
//! 44% living under 10 000 instructions. mimalloc takes about 71
//! instructions for an allocation and its free; a pop and a push take about
//! 10, which is 4.6% of the `d_faces` -O2 compile (6% to 7% of a small
//! program's) for 7% more resident memory. mimalloc does not move that: it
//! hands out the same blocks, but through its page and segment bookkeeping.
//!
//! # Invariants (the only unsafe code is here)
//! 1. A block's class is a function of its `Layout` alone (size and alignment),
//!    so `alloc`, `dealloc` and `realloc` agree on whether it is the slab's or
//!    mimalloc's without a header: the caller passes the layout it allocated
//!    with.
//! 2. A free block holds one pointer, the next free block of its class (a class
//!    is at least 16 bytes, aligned to 16).
//! 3. The lists and the chunk are thread-local and only their own thread
//!    touches them. A block freed by another thread joins that thread's list of
//!    the same class: no list is shared, so nothing is corrupted; it is simply
//!    handed out by that thread afterwards. A thread that ends keeps its free
//!    blocks (they are not returned to any list); the compilers are
//!    single-threaded, so this is the tests' only case.
//! 4. The thread-local has a `const` initializer and no destructor, so it can
//!    be used from the allocator at any point of a thread's life, including its
//!    start and its end.
//! 5. Chunks are never freed, so a block is valid for as long as the process.
//!
//! # Used by
//! The compiler binaries (`llrm-cc`, `llrm-qb`, `llrm-nib`, `llrm-mir`)
//! declare it as `#[global_allocator]`; no library crate does.

use std::alloc::{GlobalAlloc, Layout};
use std::cell::UnsafeCell;
use std::ptr::null_mut;

use mimalloc::MiMalloc;

/// The largest request the slab serves.
const LIMIT: usize = 1024;
/// A class is a multiple of this, and every block is aligned to it.
const UNIT: usize = 16;
const CLASSES: usize = LIMIT / UNIT;
/// The memory a thread takes from mimalloc at a time.
const CHUNK: usize = 1 << 20;

struct Local {
    free: [*mut u8; CLASSES],
    bump: *mut u8,
    end: *mut u8,
    chunks: usize,
}

thread_local! {
    static LOCAL: UnsafeCell<Local> =
        const { UnsafeCell::new(Local { free: [null_mut(); CLASSES], bump: null_mut(), end: null_mut(), chunks: 0 }) };
}

/// The global allocator of the compilers.
pub struct Slab;

/// Whether the slab serves `layout`.
#[inline]
fn small(layout: &Layout) -> bool {
    layout.size() <= LIMIT && layout.align() <= UNIT
}

/// The index of `size`'s class (a zero size is the first class).
#[inline]
fn class(size: usize) -> usize {
    (size.max(1) - 1) / UNIT
}

impl Slab {
    /// How many chunks this thread has taken from mimalloc.
    pub fn chunks() -> usize {
        LOCAL.with(|local| unsafe { (*local.get()).chunks })
    }
}

unsafe impl GlobalAlloc for Slab {
    #[inline]
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        if !small(&layout) {
            // SAFETY: the caller's layout, passed on.
            return unsafe { MiMalloc.alloc(layout) };
        }
        let index = class(layout.size());
        LOCAL.with(|local| {
            // SAFETY: this thread's list, with no other reference live (the
            // closure does not call back into the allocator).
            let local = unsafe { &mut *local.get() };
            // SAFETY: `small` bounds the size, so `index < CLASSES`.
            let list = unsafe { local.free.get_unchecked_mut(index) };
            let head = *list;
            if !head.is_null() {
                // SAFETY: a free block holds the next free block (invariant 2).
                *list = unsafe { *head.cast::<*mut u8>() };
                return head;
            }
            let bytes = (index + 1) * UNIT;
            if (local.end as usize).wrapping_sub(local.bump as usize) < bytes || local.bump.is_null() {
                // SAFETY: a valid layout (a power-of-two alignment, a size
                // that fits `isize`).
                let chunk = unsafe { MiMalloc.alloc(Layout::from_size_align_unchecked(CHUNK, 4096)) };
                if chunk.is_null() {
                    return null_mut();
                }
                local.bump = chunk;
                // SAFETY: `chunk` is `CHUNK` bytes.
                local.end = unsafe { chunk.add(CHUNK) };
                local.chunks += 1;
            }
            let found = local.bump;
            // SAFETY: `bytes` fit before `end`, checked above.
            local.bump = unsafe { found.add(bytes) };
            found
        })
    }

    #[inline]
    unsafe fn dealloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
    ) {
        if !small(&layout) {
            // SAFETY: mimalloc's block, with the layout it was made with.
            return unsafe { MiMalloc.dealloc(ptr, layout) };
        }
        let index = class(layout.size());
        LOCAL.with(|local| {
            // SAFETY: as in `alloc`; `ptr` is a block of this class, 16 bytes
            // at least and aligned to 16, which holds the link.
            unsafe {
                let local = &mut *local.get();
                let list = local.free.get_unchecked_mut(index);
                *ptr.cast::<*mut u8>() = *list;
                *list = ptr;
            }
        });
    }

    #[inline]
    unsafe fn alloc_zeroed(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        if !small(&layout) {
            // SAFETY: the caller's layout, passed on.
            return unsafe { MiMalloc.alloc_zeroed(layout) };
        }
        // SAFETY: a small layout, allocated and then cleared for its size.
        unsafe {
            let ptr = self.alloc(layout);
            if !ptr.is_null() {
                ptr.write_bytes(0, layout.size());
            }
            ptr
        }
    }

    #[inline]
    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> *mut u8 {
        // SAFETY: the caller guarantees `new_size`, rounded to the alignment,
        // fits `isize`.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        match (small(&layout), small(&new_layout)) {
            // SAFETY: mimalloc's block, grown or shrunk by mimalloc.
            (false, false) => unsafe { MiMalloc.realloc(ptr, layout, new_size) },
            // The same class: the block already has the room.
            (true, true) if class(layout.size()) == class(new_size) => ptr,
            _ => {
                // SAFETY: a new block, the common bytes copied, the old one
                // freed.
                unsafe {
                    let new = self.alloc(new_layout);
                    if !new.is_null() {
                        std::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
                        self.dealloc(ptr, layout);
                    }
                    new
                }
            }
        }
    }
}
