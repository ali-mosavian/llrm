//! Routines the compiler places inline where an operation has no instruction,
//! written as assembly (`helpers/*.asm`) and assembled for the target's code
//! width. The operands go in and come out in fixed registers, which the
//! routine's description states.

use iced_x86::Register;

use crate::asm::{self, Mode, Refusal};

/// A 64-bit divide: its code and where its operands are.
#[derive(Clone, Debug)]
pub struct Divide {
    pub name: &'static str,
    pub code: Vec<u8>,
    /// The registers it reads: the dividend's low and high dwords, the
    /// divisor's low and, where the divisor is wider than a dword, high.
    pub dividend: [Register; 2],
    pub divisor: [Register; 2],
    pub wide_divisor: bool,
    /// Where the quotient's and the remainder's low and high dwords are left.
    pub quotient: [Register; 2],
    pub remainder: [Register; 2],
    /// The registers it uses, and what it changes: those and the flags.
    pub clobbers: [Register; 4],
}

const CHECK: &str = include_str!("helpers/check.asm");
const NARROW: &str = include_str!("helpers/narrow.asm");
const WIDE: &str = include_str!("helpers/wide.asm");
const SIGNS: &str = include_str!("helpers/signs.asm");
const UNSIGNS: &str = include_str!("helpers/unsigns.asm");
const SIGNS32: &str = include_str!("helpers/signs32.asm");
const UNSIGNS32: &str = include_str!("helpers/unsigns32.asm");

/// The divide of a `signed` or unsigned i64 by a divisor of `wide_divisor`
/// (two dwords) or one dword, assembled for code of `bits` bits: real mode
/// with the 32-bit registers, or flat.
pub fn divide(
    signed: bool,
    wide_divisor: bool,
    bits: u32,
) -> Result<Divide, Refusal> {
    let mode = if bits == 32 {
        Mode { bits: 32, segmented: false, address_bytes: 4, wide: true }
    } else {
        Mode { bits: 16, segmented: true, address_bytes: 2, wide: true }
    };
    let source = match (signed, wide_divisor) {
        (false, true) => [CHECK, NARROW, WIDE, "done:\n"].concat(),
        (true, true) => [SIGNS, CHECK, NARROW, WIDE, "done:\n", UNSIGNS].concat(),
        (false, false) => ["    xor ecx, ecx\n", NARROW, "done:\n"].concat(),
        (true, false) => [SIGNS32, NARROW, "done:\n", UNSIGNS32].concat(),
    };
    let lines: Vec<&str> = source.lines().collect();
    let name = match (signed, wide_divisor) {
        (false, true) => "div64_u",
        (true, true) => "div64_s",
        (false, false) => "div64_u32",
        (true, false) => "div64_s32",
    };
    Ok(Divide {
        name,
        code: asm::assembled(&lines, mode)?,
        dividend: [Register::EAX, Register::EDX],
        divisor: [Register::EBX, Register::ECX],
        wide_divisor,
        quotient: [Register::EAX, Register::EDX],
        remainder: [Register::EBX, Register::ECX],
        clobbers: [Register::EAX, Register::EBX, Register::ECX, Register::EDX],
    })
}

#[cfg(test)]
#[path = "helpers_tests.rs"]
mod tests;
