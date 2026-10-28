//! What the assembler asks of a target (LLVM's `MCAsmBackend`).

use crate::{FixupKind, FixupKindInfo};

pub trait AsmBackend {
    /// What `kind` says of the field it patches.
    fn kind_info(
        &self,
        kind: FixupKind,
    ) -> FixupKindInfo;

    /// Appends `count` bytes that do nothing (`writeNopData`); false where the
    /// target has none of that length.
    fn write_nop_data(
        &self,
        out: &mut Vec<u8>,
        count: usize,
    ) -> bool;
}

/// `MCAssembler::relaxPrefAlign`: the alignment, as a power of two, a
/// `.prefalign` of `preferred` settles on at `at` when the body ends at `end`.
/// The body is what lies between the padding and its end, and the alignment
/// only grows from `computed`.
pub fn relax_pref_align(
    at: i64,
    computed: u8,
    preferred: u8,
    end: i64,
) -> u8 {
    let pad = (at as u64).wrapping_neg() as i64 & ((1i64 << computed) - 1);
    let body = end - (at + pad);
    let fit = (body.max(1) as u64).next_power_of_two().trailing_zeros() as u8;
    fit.min(preferred).max(computed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3-byte body at offset 1 with a 16-byte preference pads to 4 (the
    /// body's power of two), never to 16.
    #[test]
    fn a_prefalign_grows_to_the_body_and_no_further() {
        assert_eq!(relax_pref_align(1, 0, 4, 1 + 3), 2);
        assert_eq!(relax_pref_align(0, 0, 4, 40), 4);
        assert_eq!(relax_pref_align(5, 3, 1, 9), 3);
    }
}
