//! Port of `qbopt/frontend/blocks.py`.
//!
//! Ported so far: INLINE_TABLE, Ends, Block.

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

/// How a block ends. Port of `qbopt/frontend/blocks.py:Ends`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Ends {
    FallsThrough,
    Conditional,
    Jump,
    Indirect,
    Return,
    Leaves,
    Table,
}

/// Port of `qbopt/frontend/blocks.py:Block`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub at: usize,
    pub end: usize,
    pub insns: Vec<crate::frontend::declen::Insn>,
    pub ends: Ends,
    pub succ: Vec<usize>,
}

impl Block {
    /// Whether control goes somewhere this cannot see.
    #[must_use]
    pub fn leaves(&self) -> bool {
        matches!(self.ends, Ends::Return | Ends::Leaves | Ends::Indirect) || self.succ.is_empty()
    }
}
