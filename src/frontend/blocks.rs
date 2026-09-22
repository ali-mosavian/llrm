//! Port of `qbopt/frontend/blocks.py`.
//!
//! Ported so far: INLINE_TABLE.

use std::collections::BTreeSet;
use std::sync::LazyLock;

// The one routine that reads a table laid inline after its own call site.
// Established from the shipped libraries rather than inferred -- gosub.asm
// in BCOM45.LIB, BCL71ENR.LIB and VBDCL10E.LIB, read with tools/libdump.py.
// The implementations differ; the QB form below shows the table protocol:
//
//   lds  si,[bp+2]     si = the return address, which IS the table
//   lodsb              al = the count, si now on the entries
//   mov  dl,al / shl dx,1 / add dx,si    dx = past the table
//   mov  cx,[bx+si]    cx = entry[index-1]
//   cmp  al,bl / jbe   out of range falls through to dx
//   push bx / push dx  and a far return goes to whichever was chosen
//
// So an entry is a two-byte offset into this same segment, the count is one
// byte in front of them. Selectors 0 or count+1..255 fall through; values
// above 255 enter B$FrameFC instead. These are normal CFG successors only.
pub static INLINE_TABLE: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| BTreeSet::from(["B$OGTA"]));
