//! An append-only table whose entries never move: LLVM's uniquing tables hold
//! a constant or a type for as long as the context lives and hand out
//! pointers to it, and `push` takes `&self` so that interning does too.

use std::alloc::{self, Layout};
use std::cell::{Cell, UnsafeCell};
use std::ptr::NonNull;

/// Entries to a chunk: a power of two, so that `get` is a shift and a mask.
const CHUNK: usize = 1 << 10;

/// Entries live in chunks that are allocated once, filled and never grown or
/// freed before the arena is, so a `&T` stays valid while later entries are
/// pushed. Nothing here calls out: `push` and `get` touch the chunk list for
/// the length of one statement each.
pub struct Arena<T> {
    // Behind a pointer, so that a `&Arena` (and the `&Context` holding it) is
    // still read-only to the optimizer: an interior-mutable field inline would
    // make every reference to the context assume it can change.
    held: Box<Held<T>>,
}

struct Held<T> {
    chunks: UnsafeCell<Vec<NonNull<T>>>,
    len: Cell<usize>,
}

/// Chunk and offset of entry `index`.
fn locate(index: usize) -> (usize, usize) {
    (index / CHUNK, index % CHUNK)
}

fn layout<T>() -> Layout {
    Layout::array::<T>(CHUNK).expect("a chunk fits in memory")
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        assert!(size_of::<T>() > 0, "an arena of zero-sized entries has no addresses");
        Self { held: Box::new(Held { chunks: UnsafeCell::new(Vec::new()), len: Cell::new(0) }) }
    }
}

impl<T> Arena<T> {
    #[inline]
    pub fn len(&self) -> usize {
        self.held.len.get()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.held.len.get() == 0
    }

    /// Entry `index`, which must have been pushed.
    #[inline]
    pub fn get(
        &self,
        index: usize,
    ) -> &T {
        assert!(index < self.held.len.get(), "entry {index} of {}", self.held.len.get());
        let (chunk, offset) = locate(index);
        // SAFETY: entries below `len` are initialized, their chunk is listed
        // (the list holds one chunk per `CHUNK` entries pushed) and
        // never moved or freed while `self` lives. The list is read for this
        // one expression, and no reference into it escapes.
        unsafe {
            let list = &*self.held.chunks.get();
            let base = *list.get_unchecked(chunk);
            &*base.as_ptr().add(offset)
        }
    }

    /// Appends `value`; its index.
    #[inline]
    pub fn push(
        &self,
        value: T,
    ) -> usize {
        let index = self.held.len.get();
        let (chunk, offset) = locate(index);
        // SAFETY: slot `offset` of `chunk` is allocated (a fresh chunk when
        // `offset` is 0) and not yet initialized; `len` grows only
        // after the write, and no `&T` ever points at an unwritten slot.
        unsafe {
            let chunks = &mut *self.held.chunks.get();
            if offset == 0 {
                debug_assert_eq!(chunks.len(), chunk);
                let layout = layout::<T>();
                let fresh = alloc::alloc(layout).cast::<T>();
                chunks.push(NonNull::new(fresh).unwrap_or_else(|| alloc::handle_alloc_error(layout)));
            }
            chunks[chunk].as_ptr().add(offset).write(value);
        }
        self.held.len.set(index + 1);
        index
    }

    #[inline]
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        (0..self.len()).map(|at| self.get(at))
    }

    #[inline]
    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> + ExactSizeIterator {
        let chunks = self.held.chunks.get_mut().clone();
        (0..self.held.len.get()).map(move |at| {
            let (chunk, offset) = locate(at);
            // SAFETY: `&mut self` is held for the iterator's life; each index
            // is visited once.
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
        let len = self.held.len.get();
        for (chunk, base) in self.held.chunks.get_mut().iter().enumerate() {
            let held = len.saturating_sub(chunk * CHUNK).min(CHUNK);
            // SAFETY: the first `held` entries of the chunk are initialized,
            // and the chunk was allocated with this layout.
            unsafe {
                std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(base.as_ptr(), held));
                alloc::dealloc(base.as_ptr().cast(), layout::<T>());
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
