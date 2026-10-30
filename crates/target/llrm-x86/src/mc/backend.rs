//! x86's `MCAsmBackend`.

use llrm_object::assembler::{AsmBackend, Cfi};
use llrm_object::{FixupKind, FixupKindInfo};

use super::fixup;

/// x86's backend for one mode and one longest no-operation (a 486 or P5 has
/// none past `nop`).
pub struct X86Backend {
    pub sixteen: bool,
    /// x86-64: the only width that has a compact unwind encoding here.
    pub wide: bool,
    pub longest: usize,
}

impl AsmBackend for X86Backend {
    fn kind_info(
        &self,
        kind: FixupKind,
    ) -> FixupKindInfo {
        fixup::kind_info(kind)
    }

    fn write_nop_data(
        &self,
        out: &mut Vec<u8>,
        count: usize,
    ) -> bool {
        out.extend(nops(count, self.longest, self.sixteen));
        true
    }

    fn generate_compact_unwind_encoding(
        &self,
        cfi: &[Cfi],
    ) -> u32 {
        if self.wide { super::compact_unwind::encoding(cfi) } else { 0 }
    }
}

/// `X86AsmBackend::writeNopData`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/MCTargetDesc/X86AsmBackend.cpp#L1167-L1223>):
/// `count` bytes of nops, none longer than `longest`; `sixteen` for 16-bit
/// mode's forms.
pub fn nops(
    count: usize,
    longest: usize,
    sixteen: bool,
) -> Vec<u8> {
    const NOPS_32: [&[u8]; 10] = [
        &[0x90],
        &[0x66, 0x90],
        &[0x0f, 0x1f, 0x00],
        &[0x0f, 0x1f, 0x40, 0x00],
        &[0x0f, 0x1f, 0x44, 0x00, 0x00],
        &[0x66, 0x0f, 0x1f, 0x44, 0x00, 0x00],
        &[0x0f, 0x1f, 0x80, 0x00, 0x00, 0x00, 0x00],
        &[0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
        &[0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
        &[0x66, 0x2e, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
    ];
    const NOPS_16: [&[u8]; 4] = [&[0x90], &[0x66, 0x90], &[0x8d, 0x74, 0x00], &[0x8d, 0xb4, 0x00, 0x00]];
    let table: &[&[u8]] = if sixteen { &NOPS_16 } else { &NOPS_32 };
    let longest = if sixteen { longest.min(4) } else { longest }.max(1);
    let (mut left, mut out) = (count, Vec::with_capacity(count));
    while left != 0 {
        let this = left.min(longest);
        // A nop longer than the table's longest has 0x66 prefixes.
        let prefixes = this.saturating_sub(10);
        out.extend(std::iter::repeat_n(0x66, prefixes));
        out.extend_from_slice(table[this - prefixes - 1]);
        left -= this;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `X86AsmBackend::writeNopData`: no `nopl` on a 486 or P5, so a pad is
    /// that many `nop`s; with it the longest single no-operation (10 bytes,
    /// then the rest); 16-bit mode has four forms.
    #[test]
    fn padding_is_the_nops_writenopdata_writes() {
        assert_eq!(nops(3, 1, false), [0x90, 0x90, 0x90]);
        assert_eq!(nops(3, 10, false), [0x0f, 0x1f, 0x00]);
        assert_eq!(nops(15, 10, false).len(), 15);
        assert_eq!(&nops(15, 10, false)[10..], [0x0f, 0x1f, 0x44, 0x00, 0x00], "the rest after a 10-byte nop");
        assert_eq!(nops(11, 11, false)[..2], [0x66, 0x66], "a nop over 10 bytes has 0x66 prefixes");
        assert_eq!(nops(4, 4, true), [0x8d, 0xb4, 0x00, 0x00]);
        assert_eq!(nops(5, 4, true), [0x8d, 0xb4, 0x00, 0x00, 0x90]);
        assert_eq!(nops(0, 1, false), Vec::<u8>::new());
    }

    /// The backend is `nops` behind the trait, never a different pad.
    #[test]
    fn the_backend_writes_what_nops_writes() {
        let mut out = Vec::new();
        assert!(X86Backend { sixteen: false, wide: false, longest: 10 }.write_nop_data(&mut out, 3));
        assert_eq!(out, [0x0f, 0x1f, 0x00]);
    }
}
