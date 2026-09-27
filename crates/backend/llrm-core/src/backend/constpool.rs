//! Readonly constants the machine phases put in memory, as GCC's
//! `force_const_mem`: one datum per distinct bytes, which the driver names
//! and emits with the module's other readonly data.

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
    pub fn cell(&mut self, bytes: Vec<u8>) -> Mem {
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
