//! An append-only table whose entries never move: LLVM's uniquing tables hold
//! a constant or a type for as long as the context lives and hand out
//! pointers to it, and `push` takes `&self` so that interning does too.

use std::alloc::{self, Layout};
use std::cell::{Cell, UnsafeCell};
use std::ptr::NonNull;

/// Entries in the first chunk; chunk `k` holds `FIRST << k`.
const FIRST: usize = 64;

/// Entries live in chunks that are allocated once, filled and never grown or
/// freed before the arena is, so a `&T` stays valid while later entries are
/// pushed. Nothing here calls out: `push` and `get` touch the chunk list for
/// the length of one statement each.
pub struct Arena<T> {
    chunks: UnsafeCell<Vec<NonNull<T>>>,
    len: Cell<usize>,
}

/// Chunk and offset of entry `index`.
fn locate(index: usize) -> (usize, usize) {
    let chunk = (index / FIRST + 1).ilog2() as usize;
    (chunk, index - FIRST * ((1 << chunk) - 1))
}

fn layout<T>(chunk: usize) -> Layout {
    Layout::array::<T>(FIRST << chunk).expect("a chunk fits in memory")
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        assert!(size_of::<T>() > 0, "an arena of zero-sized entries has no addresses");
        Self { chunks: UnsafeCell::new(Vec::new()), len: Cell::new(0) }
    }
}

impl<T> Arena<T> {
    pub fn len(&self) -> usize {
        self.len.get()
    }

    pub fn is_empty(&self) -> bool {
        self.len.get() == 0
    }

    /// Entry `index`, which must have been pushed.
    pub fn get(
        &self,
        index: usize,
    ) -> &T {
        assert!(index < self.len.get(), "entry {index} of {}", self.len.get());
        let (chunk, offset) = locate(index);
        // SAFETY: entries below `len` are initialized, and their chunk is never moved or freed while `self` lives.
        // The list is read for this one expression, and no reference into it escapes.
        unsafe {
            let base = (&(*self.chunks.get()))[chunk];
            &*base.as_ptr().add(offset)
        }
    }

    /// Appends `value`; its index.
    pub fn push(
        &self,
        value: T,
    ) -> usize {
        let index = self.len.get();
        let (chunk, offset) = locate(index);
        // SAFETY: slot `offset` of `chunk` is allocated (a fresh chunk when `offset` is 0) and not yet initialized;
        // `len` grows only after the write, and no `&T` ever points at an unwritten slot.
        unsafe {
            let chunks = &mut *self.chunks.get();
            if offset == 0 {
                debug_assert_eq!(chunks.len(), chunk);
                let layout = layout::<T>(chunk);
                let fresh = alloc::alloc(layout).cast::<T>();
                chunks.push(NonNull::new(fresh).unwrap_or_else(|| alloc::handle_alloc_error(layout)));
            }
            chunks[chunk].as_ptr().add(offset).write(value);
        }
        self.len.set(index + 1);
        index
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        (0..self.len()).map(|at| self.get(at))
    }

    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> + ExactSizeIterator {
        let chunks = self.chunks.get_mut().clone();
        (0..self.len.get()).map(move |at| {
            let (chunk, offset) = locate(at);
            // SAFETY: `&mut self` is held for the iterator's life; each index is visited once.
            unsafe { &mut *chunks[chunk].as_ptr().add(offset) }
        })
    }
}

impl<T: Clone> Clone for Arena<T> {
    fn clone(&self) -> Self {
        let copy = Self::default();
        for one in self.iter() {
            copy.push(one.clone());
        }
        copy
    }
}

impl<T> Drop for Arena<T> {
    fn drop(&mut self) {
        let len = self.len.get();
        for (chunk, base) in self.chunks.get_mut().iter().enumerate() {
            let held = len.saturating_sub(FIRST * ((1 << chunk) - 1)).min(FIRST << chunk);
            // SAFETY: the first `held` entries of the chunk are initialized, and the chunk was allocated with this layout.
            unsafe {
                std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(base.as_ptr(), held));
                alloc::dealloc(base.as_ptr().cast(), layout::<T>(chunk));
            }
        }
    }
}

impl<T> std::fmt::Debug for Arena<T> {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(formatter, "Arena({})", self.len())
    }
}
