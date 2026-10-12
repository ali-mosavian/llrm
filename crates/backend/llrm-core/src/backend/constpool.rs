//! Readonly constants the machine phases put in memory, one datum per
//! distinct bytes.
//!
//! # How it works
//! A phase that needs a constant no instruction can encode asks the pool for
//! a cell holding its bytes. [`Pool::cell`] returns a readonly segment
//! cell; asking again for the same bytes returns the same cell, so a
//! constant appears once however often it is used. Ids start at
//! the module's global count, past the ids of its own data objects.
//! [`narrowest`] gives a float constant in `f32` bytes when that
//! is exact (or the value is NaN), else in `f64` bytes, as GCC's
//! `compress_float_constant`.
//!
//! ```text
//!   fild-style load of integer 5      fld  dword [$K12]
//!   (Imm, not 0 or 1)           =>    ...
//!                                     data:  $K12: 00 00 A0 40
//! ```
//!
//! After the last function is assembled the driver
//! (`assemble.rs`) walks [`Pool::entries`] in first-asked order, names each
//! datum `$K<id>` and emits it with the module's other readonly data.
//!
//! # Used by
//! `floatassign.rs`: an integer other than 0 or 1 converted to float becomes
//! a load of its float constant (0 and 1 use `fldz` and `fld1`). The pool is
//! shared by every function of a module, so `$K` names are module-wide.
//!
//! # See also
//! GCC `force_const_mem`; [`crate::backend`]

use crate::model::ir::{Addr, Mem, Space};
use crate::support::hash::IndexMap;

#[derive(Clone, Debug, Default)]
pub struct Pool {
    first: i64,
    entries: IndexMap<Vec<u8>, i64>,
}

impl Pool {
    /// Data object ids from `first` on are the pool's.
    #[must_use]
    pub fn new(first: i64) -> Self {
        Self { first, entries: IndexMap::default() }
    }

    /// The readonly cell holding `bytes`.
    pub fn cell(
        &mut self,
        bytes: Vec<u8>,
    ) -> Mem {
        let width = bytes.len() as u32;
        let next = self.first + self.entries.len() as i64;
        let id = *self.entries.entry(bytes).or_insert(next);
        Mem { disp_width: 2, ..Mem::new(Some(Addr { index: id, ..Addr::new(Space::Segment, 0) }), width) }
    }

    /// Every datum and its data object id, in the order first asked for.
    pub fn entries(&self) -> impl Iterator<Item = (&[u8], i64)> {
        self.entries.iter().map(|(bytes, id)| (bytes.as_slice(), *id))
    }
}

/// `value` in the narrowest float format that holds it exactly, as GCC's
/// `compress_float_constant`: its bytes.
#[must_use]
pub fn narrowest(value: f64) -> Vec<u8> {
    let single = value as f32;
    if f64::from(single) == value || value.is_nan() {
        single.to_le_bytes().to_vec()
    } else {
        value.to_le_bytes().to_vec()
    }
}
