//! Readonly constants the machine phases put in memory, one datum per
//! distinct bytes.
//!
//! # How it works
//! The selector and the machine phases that need a constant no instruction can
//! encode ask the pool for
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
//! # Example
//! `x * 2.5 + 1.25` on an f32: each constant is a pool cell (`seg:4`, `seg:5`)
//! in the selected LIR, and a datum in the listing. `; inserted` notes are
//! dropped.
//!
//! ```lir
//! v1 := fld [[bp+0x4]]
//! v2 := fld [[seg:4+0x0]]
//! v3 := fmul v1, v2
//! v4 := fld [[seg:5+0x0]]
//! v5 := fadd v3, v4
//! ```
//!
//! ```text
//! $K4 label byte
//! db 000h,000h,020h,040h
//! $K5 label byte
//! db 000h,000h,0a0h,03fh
//! ```
//!
//! `$K4` holds 2.5 and `$K5` 1.25, as f32 bytes.
//!
//! From `ISEL_DUMP=1 llrm-nib -O2 -fno-inline-functions
//! -fno-inline-functions-called-once -fno-ipa-cp-clone
//! -march=i386` of `fn scale(x: f32) -> f32`, stage `FarIndirectCalls` and
//! `-S`.
//!
//! # Used by
//! `isel.rs`, for each float constant it loads, and `floatassign.rs`: an
//! integer other than 0 or 1 converted to float becomes a load of its float
//! constant (0 and 1 use `fldz` and `fld1`). The pool is shared by every
//! function of a module, so `$K` names are module-wide.
//!
//! Tuned for size, `isel/matcher.rs` stores a double 0.0, -0.0, 1.0 or -1.0,
//! or a float 1.0, as `fldz`/`fld1` (and `fchs`) and an `fstp`
//! (`x87_store_pays`); a lone float 0.0 or -1.0 stays an integer store.
//!
//! # See also
//! GCC `force_const_mem`; [`crate::backend`]

use crate::model::ir::{Addr, Mem, Space};
use crate::support::hash::IndexMap;

#[derive(Clone, Debug, Default)]
pub struct Pool {
    first: i64,
    /// Ids handed out: the data' and the jump tables'.
    count: i64,
    entries: IndexMap<Vec<u8>, i64>,
}

impl Pool {
    /// Data object ids from `first` on are the pool's.
    #[must_use]
    pub fn new(first: i64) -> Self {
        Self { first, count: 0, entries: IndexMap::default() }
    }

    /// The readonly cell holding `bytes`.
    pub fn cell(
        &mut self,
        bytes: Vec<u8>,
    ) -> Mem {
        let width = bytes.len() as u32;
        let id = match self.entries.get(&bytes) {
            Some(&id) => id,
            None => {
                let id = self.first + self.count;
                self.count += 1;
                self.entries.insert(bytes, id);
                id
            }
        };
        Mem { disp_width: 2, ..Mem::new(Some(Addr { index: id, ..Addr::new(Space::Segment, 0) }), width) }
    }

    /// Where the pool is now, for [`Pool::rewind`]: what a selection that is
    /// not kept asked for goes with it.
    #[must_use]
    pub fn mark(&self) -> (usize, i64) {
        (self.entries.len(), self.count)
    }

    /// The pool as it was at `mark`, if only data were added since.
    pub fn rewind(
        &mut self,
        mark: (usize, i64),
    ) {
        self.entries.truncate(mark.0);
        self.count = mark.1;
    }

    /// A new jump table's id. Its entries are the final procedure's, written
    /// where the module is assembled: a body that was selected and not kept
    /// leaves an id nothing names.
    pub fn table(&mut self) -> i64 {
        let id = self.first + self.count;
        self.count += 1;
        id
    }

    /// Every datum and its data object id, in the order first asked for.
    pub fn entries(&self) -> impl Iterator<Item = (&[u8], i64)> {
        self.entries.iter().map(|(bytes, id)| (bytes.as_slice(), *id))
    }
}

/// `entries` with the 8-byte and wider ones first: each is on an 8-byte
/// boundary (a 486 takes 3 clocks more for a double off one: Intel486 data
/// sheet 240440-002, section 10.1 and Table 2.1), and after them the narrow
/// ones fill no gap.
pub fn wide_first<'a>(entries: impl Iterator<Item = (&'a [u8], i64)>) -> Vec<(&'a [u8], i64)> {
    let mut all: Vec<_> = entries.collect();
    all.sort_by_key(|(bytes, _)| bytes.len() < 8);
    all
}

/// Whether `name` is a label assembly made for the pool: `$K` a constant, `$J`
/// a jump table.
pub fn is_pool(name: &str) -> bool {
    name.starts_with("$K") || name.starts_with("$J")
}

/// The x87 instruction that loads `value` with no memory operand, and whether
/// an `fchs` follows: +-0.0 and +-1.0, the ones gcc's
/// `standard_80387_constant_p` knows (i386.cc L5113-5153) and LLVM keeps legal
/// immediates (X86ISelLowering.cpp L777-780).
#[must_use]
pub fn x87_constant(value: f64) -> Option<(&'static str, bool)> {
    if value == 0.0 {
        Some(("fldz", value.is_sign_negative()))
    } else if value == 1.0 {
        Some(("fld1", false))
    } else if value == -1.0 {
        Some(("fld1", true))
    } else {
        None
    }
}

/// Whether storing `value` (`size` bytes) as `x87_constant` and an `fstp` beats
/// the integer stores. A float of 0.0 or -1.0 is one dword: it shares a zeroed
/// register (4 bytes, 3 at m32) or ties `fld1`, `fchs`, `fstp` (7), so only 1.0
/// and the doubles, two dwords, pay.
#[must_use]
pub fn x87_store_pays(
    value: f64,
    size: u64,
) -> bool {
    x87_constant(value).is_some() && (size == 8 || value == 1.0)
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
