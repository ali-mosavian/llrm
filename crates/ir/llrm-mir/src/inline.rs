//! A short vector held in the struct that owns it, spilling to the heap only
//! past `N` elements.
//!
//! # How it works
//! `InlineVec<T, N>` is `Few(len, [T; N])` or `Many(Vec<T>)`. Both read as the
//! slice of elements in the order they were pushed. Pushing past `N` moves the
//! elements to a `Vec` once, as growing a `Vec` would, and every later push is
//! the `Vec`'s. Most of a compile's allocations are small and short-lived (61%
//! of them are 64 bytes or less, 44% live under 10,000 instructions): a list of
//! one or two successors, a block's few operands. LLVM's `SmallVector` is the
//! same type.
//!
//! The elements are `Copy` and name a `FILL` for the slots not in use, so the
//! type has no uninitialised memory; its one `unsafe` is the slice of the
//! held elements, `len <= N` by construction.
//!
//! # Choosing `N`
//! A `Vec` is 24 bytes (pointer, capacity, length). `Few(u8, [T; N])` keeps the
//! struct at 24 bytes while `1 + N * size_of::<T>() <= 16`: N = 1 for an
//! 8-byte element, N = 3 for a 4-byte id. A larger `N` makes the struct bigger
//! by the difference and every move copies it; measure it (`size_of`) and say
//! so where it is chosen.
//!
//! # Used by
//! `BlockList` (`Function::successors`, `predecessors`) and the aliases in
//! this module's users.

use crate::module::{BlockId, InstId, ValueId};

/// A value an unused slot of an `InlineVec` holds.
pub trait Fill: Copy {
    const FILL: Self;
}

impl Fill for i64 {
    const FILL: Self = 0;
}

impl Fill for u32 {
    const FILL: Self = 0;
}

impl Fill for usize {
    const FILL: Self = 0;
}

impl Fill for BlockId {
    const FILL: Self = BlockId(0);
}

impl Fill for ValueId {
    const FILL: Self = ValueId(0);
}

impl Fill for InstId {
    const FILL: Self = InstId(0);
}

impl Fill for crate::module::Operand {
    const FILL: Self = crate::module::Operand::Value(ValueId(0));
}

/// How many of an `InlineVec`'s `N` slots are in use. Only this module makes
/// one, so `Few(len, _)` always has `len <= N`, which reading the held
/// elements relies on.
#[derive(Clone, Copy)]
pub struct Len<const N: usize>(u8);

#[derive(Clone)]
pub enum InlineVec<T: Fill, const N: usize> {
    Few(Len<N>, [T; N]),
    Many(Vec<T>),
}

impl<T: Fill, const N: usize> Default for InlineVec<T, N> {
    fn default() -> Self {
        Self::Few(Len(0), [T::FILL; N])
    }
}

impl<T: Fill, const N: usize> InlineVec<T, N> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Room for `capacity` elements: held inline where they fit.
    pub fn with_capacity(capacity: usize) -> Self {
        if capacity <= N { Self::default() } else { Self::Many(Vec::with_capacity(capacity)) }
    }

    pub fn push(
        &mut self,
        value: T,
    ) {
        match self {
            Self::Few(len, held) if usize::from(len.0) < N => {
                held[usize::from(len.0)] = value;
                len.0 += 1;
            }
            Self::Few(len, held) => {
                let mut all = Vec::with_capacity(N * 2);
                all.extend_from_slice(&held[..usize::from(len.0)]);
                all.push(value);
                *self = Self::Many(all);
            }
            Self::Many(all) => all.push(value),
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        match self {
            Self::Few(Len(0), _) => None,
            Self::Few(len, held) => {
                len.0 -= 1;
                Some(held[usize::from(len.0)])
            }
            Self::Many(all) => all.pop(),
        }
    }

    pub fn as_slice(&self) -> &[T] {
        self
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn truncate(
        &mut self,
        len: usize,
    ) {
        match self {
            Self::Few(held, _) => held.0 = held.0.min(u8::try_from(len).unwrap_or(u8::MAX)),
            Self::Many(all) => all.truncate(len),
        }
    }

    pub fn extend_from_slice(
        &mut self,
        values: &[T],
    ) {
        for &value in values {
            self.push(value);
        }
    }

    /// The elements as a `Vec`, moved out when they were already one.
    pub fn into_vec(self) -> Vec<T> {
        match self {
            Self::Few(len, held) => held[..usize::from(len.0)].to_vec(),
            Self::Many(all) => all,
        }
    }

    fn spill(&mut self) -> &mut Vec<T> {
        if let Self::Few(len, held) = self {
            *self = Self::Many(held[..usize::from(len.0)].to_vec());
        }
        match self {
            Self::Many(all) => all,
            Self::Few(..) => unreachable!("spilled above"),
        }
    }

    pub fn insert(
        &mut self,
        index: usize,
        value: T,
    ) {
        self.spill().insert(index, value);
    }

    pub fn remove(
        &mut self,
        index: usize,
    ) -> T {
        self.spill().remove(index)
    }

    pub fn retain(
        &mut self,
        keep: impl FnMut(&T) -> bool,
    ) {
        self.spill().retain(keep);
    }
}

impl<T: Fill, const N: usize> std::ops::Deref for InlineVec<T, N> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        match self {
            // SAFETY: `Len<N>` is made only here, from 0 and below `N`; the
            // checked slice was a branch on every access of an operand list.
            Self::Few(len, held) => unsafe { held.get_unchecked(..usize::from(len.0)) },
            Self::Many(all) => all,
        }
    }
}

impl<T: Fill, const N: usize> std::ops::DerefMut for InlineVec<T, N> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        match self {
            // SAFETY: as in `deref`.
            Self::Few(len, held) => unsafe { held.get_unchecked_mut(..usize::from(len.0)) },
            Self::Many(all) => all,
        }
    }
}

impl<T: Fill + std::fmt::Debug, const N: usize> std::fmt::Debug for InlineVec<T, N> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: Fill, const N: usize> FromIterator<T> for InlineVec<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
        let mut out = Self::new();
        for value in values {
            out.push(value);
        }
        out
    }
}

impl<T: Fill, const N: usize> Extend<T> for InlineVec<T, N> {
    fn extend<I: IntoIterator<Item = T>>(
        &mut self,
        values: I,
    ) {
        for value in values {
            self.push(value);
        }
    }
}

impl<T: Fill, const N: usize> From<Vec<T>> for InlineVec<T, N> {
    fn from(all: Vec<T>) -> Self {
        if all.len() <= N { all.into_iter().collect() } else { Self::Many(all) }
    }
}

impl<T: Fill, const N: usize> From<&[T]> for InlineVec<T, N> {
    fn from(all: &[T]) -> Self {
        all.iter().copied().collect()
    }
}

impl<T: Fill, const N: usize, const M: usize> From<[T; M]> for InlineVec<T, N> {
    fn from(all: [T; M]) -> Self {
        all.into_iter().collect()
    }
}

impl<T: Fill, const N: usize> From<InlineVec<T, N>> for Vec<T> {
    fn from(list: InlineVec<T, N>) -> Self {
        list.into_vec()
    }
}

/// The elements of an `InlineVec`, by value.
pub struct IntoIter<T: Fill, const N: usize> {
    list: InlineVec<T, N>,
    at: usize,
}

impl<T: Fill, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let found = self.list.get(self.at).copied();
        self.at += usize::from(found.is_some());
        found
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.list.len() - self.at;
        (left, Some(left))
    }
}

impl<T: Fill, const N: usize> ExactSizeIterator for IntoIter<T, N> {}

impl<T: Fill, const N: usize> IntoIterator for InlineVec<T, N> {
    type Item = T;
    type IntoIter = IntoIter<T, N>;

    fn into_iter(self) -> IntoIter<T, N> {
        IntoIter { list: self, at: 0 }
    }
}

impl<'a, T: Fill, const N: usize> IntoIterator for &'a InlineVec<T, N> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: Fill, const N: usize> IntoIterator for &'a mut InlineVec<T, N> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: Fill + PartialEq, const N: usize> PartialEq for InlineVec<T, N> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        **self == **other
    }
}

impl<T: Fill + Eq, const N: usize> Eq for InlineVec<T, N> {}

impl<T: Fill + PartialEq, const N: usize> PartialEq<Vec<T>> for InlineVec<T, N> {
    fn eq(
        &self,
        other: &Vec<T>,
    ) -> bool {
        **self == other[..]
    }
}

impl<T: Fill + PartialEq, const N: usize, const M: usize> PartialEq<[T; M]> for InlineVec<T, N> {
    fn eq(
        &self,
        other: &[T; M],
    ) -> bool {
        **self == other[..]
    }
}

impl<T: Fill + PartialEq, const N: usize> PartialEq<&[T]> for InlineVec<T, N> {
    fn eq(
        &self,
        other: &&[T],
    ) -> bool {
        **self == **other
    }
}

#[cfg(test)]
#[path = "inline_tests.rs"]
mod tests;
