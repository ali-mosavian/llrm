//! `memcpy`, `memmove` and `memset` of a constant length that fits the pin's
//! budget are expanded in place (`AArch64TargetLowering`'s `MaxStoresPerMem*`,
//! `TargetLowering::findOptimalMemOpLowering`): 16-byte `q` accesses, then the
//! next smaller power of two for the remainder. A remainder that a smaller
//! access would not cover is covered by the last size again, overlapping the
//! bytes before it.
//!
//! Copies load a batch before storing it, so `pairs` makes `ldp`/`stp q` of
//! them. A `memmove` loads all before it stores.

/// Accesses a copy of up to this many may use, and a fill; half of each under
/// `minsize`.
pub(crate) const COPY_OPS: usize = 16;
pub(crate) const FILL_OPS: usize = 32;
/// Loads a copy has in flight before it stores them.
pub(crate) const BATCH: usize = 4;

/// The `(bytes, offset)` accesses that cover `total` bytes.
pub(crate) fn pieces(total: u64) -> Vec<(u8, u64)> {
    let mut ops: Vec<(u8, u64)> = Vec::new();
    let (mut done, mut size) = (0, 16u64);
    while done < total {
        let left = total - done;
        if size > left {
            let mut fits = size;
            while fits > left {
                fits /= 2;
            }
            if !ops.is_empty() && fits < left {
                ops.push((size as u8, total - size));
                break;
            }
            size = fits;
        }
        ops.push((size as u8, done));
        done += size;
    }
    ops
}

/// The `q` accesses that cover `total >= 16` bytes, the last overlapping the
/// one before it.
pub(crate) fn wide_pieces(total: u64) -> Vec<(u8, u64)> {
    (0..total.div_ceil(16)).map(|at| (16, (at * 16).min(total - 16))).collect()
}
