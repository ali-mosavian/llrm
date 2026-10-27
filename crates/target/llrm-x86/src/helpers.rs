//! Routines the compiler places inline where an operation has no instruction,
//! written as assembly (`helpers/*.asm`) and assembled for the target's code
//! width. The operands go in and come out in fixed registers, which the
//! routine's description states.

use std::sync::{LazyLock, Mutex};

use llrm_lir::registers::RegId;
use llrm_support::hash::HashMap;
pub use llrm_target::routines::{Divide, HugeString, Used};

use crate::asm::{self, Mode, Refusal};

const CHECK: &str = include_str!("helpers/check.asm");
const NARROW: &str = include_str!("helpers/narrow.asm");
const WIDE: &str = include_str!("helpers/wide.asm");
const SIGNS: &str = include_str!("helpers/signs.asm");
const UNSIGNS: &str = include_str!("helpers/unsigns.asm");
const SIGNS32: &str = include_str!("helpers/signs32.asm");
const UNSIGNS32: &str = include_str!("helpers/unsigns32.asm");
const UNSIGNS32_QUOTIENT: &str = include_str!("helpers/unsigns32q.asm");
const UNSIGNS32_REMAINDER: &str = include_str!("helpers/unsigns32r.asm");

/// The divide of a `signed` or unsigned i64 by a divisor of `wide_divisor`
/// (two dwords) or one dword, assembled for code of `bits` bits: real mode
/// with the 32-bit registers, or flat.
pub fn divide(
    signed: bool,
    wide_divisor: bool,
    bits: u32,
) -> Result<Divide, Refusal> {
    divide_used(signed, wide_divisor, bits, Used::Both)
}

/// `divide` for a caller that reads only `used`: the signed routine by a dword
/// divisor leaves the sign of the other result unfixed (SD232).
pub fn divide_used(
    signed: bool,
    wide_divisor: bool,
    bits: u32,
    used: Used,
) -> Result<Divide, Refusal> {
    // only the signed dword-divisor routine has shorter forms
    let used = if signed && !wide_divisor { used } else { Used::Both };
    static CACHE: LazyLock<Mutex<HashMap<(bool, bool, u32, u8), Result<Divide, Refusal>>>> =
        LazyLock::new(Default::default);
    memoized(&CACHE, (signed, wide_divisor, bits, used as u8), || divided(signed, wide_divisor, bits, used))
}

/// A cached routine: the source and mode fix it, and the compiler asks for it
/// at every call site of every function.
fn memoized<K: std::hash::Hash + Eq + Copy, V: Clone>(
    cache: &Mutex<HashMap<K, V>>,
    key: K,
    make: impl FnOnce() -> V,
) -> V {
    if let Some(found) = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&key) {
        return found.clone();
    }
    let made = make();
    cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(key, made.clone());
    made
}

fn divided(
    signed: bool,
    wide_divisor: bool,
    bits: u32,
    used: Used,
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
        (true, false) => [
            SIGNS32,
            NARROW,
            "done:\n",
            match used {
                Used::Both => UNSIGNS32,
                Used::Quotient => UNSIGNS32_QUOTIENT,
                Used::Remainder => UNSIGNS32_REMAINDER,
            },
        ]
        .concat(),
    };
    let lines: Vec<&str> = source.lines().collect();
    let name = match (signed, wide_divisor) {
        (false, true) => "div64_u",
        (true, true) => "div64_s",
        (false, false) => "div64_u32",
        (true, false) => match used {
            Used::Both => "div64_s32",
            Used::Quotient => "div64_s32q",
            Used::Remainder => "div64_s32r",
        },
    };
    Ok(Divide {
        name,
        code: asm::assembled(&lines, mode)?,
        dividend: [RegId::EAX, RegId::EDX],
        divisor: [RegId::EBX, RegId::ECX],
        wide_divisor,
        quotient: [RegId::EAX, RegId::EDX],
        remainder: [RegId::EBX, RegId::ECX],
        clobbers: [RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX],
    })
}

const REAL: Mode = Mode { bits: 16, segmented: true, address_bytes: 2, wide: true };

/// `ecx` cells of `cell` bytes (1, 2 or 4) of the pattern in `eax` through the
/// huge pointer `bx:di`: the cell, its low bytes, becomes the dword the fill
/// stores, and the count of cells becomes bytes.
pub fn huge_fill(cell: u32) -> Result<HugeString, Refusal> {
    static CACHE: LazyLock<Mutex<HashMap<u32, Result<HugeString, Refusal>>>> = LazyLock::new(Default::default);
    memoized(&CACHE, cell, || filled(cell))
}

fn filled(cell: u32) -> Result<HugeString, Refusal> {
    let before = match cell {
        1 => "    and eax, 0xFF\n    imul eax, eax, 0x01010101\n",
        2 => "    and eax, 0xFFFF\n    imul eax, eax, 0x10001\n    shl ecx, 1\n",
        4 => "    shl ecx, 2\n",
        _ => return Err(Refusal { line: 0, message: format!("a huge fill of {cell}-byte cells") }),
    };
    let source = [before, include_str!("helpers/huge_fill.asm")].concat();
    let lines: Vec<&str> = source.lines().collect();
    Ok(HugeString {
        name: match cell {
            1 => "huge_fill1",
            2 => "huge_fill2",
            _ => "huge_fill4",
        },
        code: asm::assembled(&lines, REAL)?,
        reads: vec![RegId::EAX, RegId::ECX, RegId::BX, RegId::DI],
        clobbers: vec![RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX, RegId::DI, RegId::ES],
    })
}

/// `ecx` bytes from the huge pointer `ax:si` to `bx:di`, upward or, `down`,
/// from the last byte.
pub fn huge_copy(down: bool) -> Result<HugeString, Refusal> {
    static CACHE: LazyLock<Mutex<HashMap<bool, Result<HugeString, Refusal>>>> = LazyLock::new(Default::default);
    memoized(&CACHE, down, || copied(down))
}

fn copied(down: bool) -> Result<HugeString, Refusal> {
    let source = if down { include_str!("helpers/huge_copy_down.asm") } else { include_str!("helpers/huge_copy.asm") };
    let lines: Vec<&str> = source.lines().collect();
    Ok(HugeString {
        name: if down { "huge_copy_down" } else { "huge_copy" },
        code: asm::assembled(&lines, REAL)?,
        reads: vec![RegId::ECX, RegId::AX, RegId::SI, RegId::BX, RegId::DI],
        clobbers: vec![RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX, RegId::SI, RegId::DI, RegId::ES],
    })
}

#[cfg(test)]
#[path = "helpers_tests.rs"]
mod tests;
