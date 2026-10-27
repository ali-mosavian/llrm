//! Routines the compiler places inline where an operation has no instruction:
//! their code and the registers their operands go in and come out in. A
//! target assembles them for its own machine.

use llrm_lir::registers::RegId;

/// What every divide routine's name starts with, and every huge-pointer
/// routine's.
pub const DIVIDE_PREFIX: &str = "div64_";
pub const HUGE_PREFIX: &str = "huge_";

/// What a caller reads of a divide's results; a routine may leave the other
/// one unfixed (SD232).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Used {
    Both,
    Quotient,
    Remainder,
}

/// A 64-bit divide: its code and where its operands are.
#[derive(Clone, Debug)]
pub struct Divide {
    pub name: &'static str,
    pub code: Vec<u8>,
    /// The registers it reads: the dividend's low and high dwords, the
    /// divisor's low and, where the divisor is wider than a dword, high.
    pub dividend: [RegId; 2],
    pub divisor: [RegId; 2],
    pub wide_divisor: bool,
    /// Where the quotient's and the remainder's low and high dwords are left.
    pub quotient: [RegId; 2],
    pub remainder: [RegId; 2],
    /// The registers it uses, and what it changes: those and the flags.
    pub clobbers: [RegId; 4],
}

/// A fill or copy through huge pointers: its code and the registers it takes.
/// A huge pointer's run ends where its 16-bit offset wraps, so the routine
/// walks the runs, `rep`ing each and stepping the selector.
#[derive(Clone, Debug)]
pub struct HugeString {
    pub name: &'static str,
    pub code: Vec<u8>,
    /// The registers it reads: the byte or the source's selector, the count,
    /// the destination's selector and offset, and a copy's source offset.
    pub reads: Vec<RegId>,
    /// What it changes besides the flags (the data segment is kept).
    pub clobbers: Vec<RegId>,
}
