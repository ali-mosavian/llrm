//! Clocks of the runtime routine a library multiply calls, which a function's
//! own instructions do not show.
//!
//! `__mulsi3` is the shift-add loop of `runtime/compiler-rt/lib/int/int32.c`:
//! `for (; b; b >>= 1, a <<= 1) if (b & 1) r += a;`, built branch-free, so its
//! clocks depend on the bit length of `b` and nothing else:
//! `base + per_bit * bits(b)` for `b != 0`, `zero` for `b == 0`.
//!
//! Measured 2026-11-09 with `cycles` (CYCLES_EXE, CYCLES_FUNCS=__mulsi3): 100
//! calls of `__mulsi3(i, K)` in a loop, `K` = 0, 1, 128, 255, 31, 21845, 2^31
//! (bits 0, 1, 8, 8, 5, 15, 32), clocks per call, `llrm-cc -O2` 68000 (md5
//! 1783a616b24fe393d550d2fd15ece58b, release/next 44a0657729 + #8532), the
//! model's instruction tables:
//!
//! | CPU   | zero | bits 1 | 5   | 8   | 15   | 32   | fit             |
//! |-------|------|--------|-----|-----|------|------|-----------------|
//! | 68000 | 70   | 162    | 426 | 624 | 1086 | 2208 | 96 + 66 * bits  |
//! | 68010 | 72   | 150    | -   | 556 | 962  | -    | 92 + 58 * bits  |
//! | 68020 | 36   | 69     | 165 | 237 | 405  | 813  | 45 + 24 * bits  |
//!
//! The price of a call site counts the routine at `b` = 1, one pass of the
//! loop, which is a lower bound: the chain it competes with costs 20 to 70
//! clocks, so a trial never keeps a library call for its clocks, and a larger
//! constant only makes the call dearer. Owner m68k-b. Exit 2026-12-20: the
//! constant read from the call's pushes.

/// FNV-1a of the routine's source text; the test below fails when it changes,
/// and the table above is measured again.
pub const MULSI3_SOURCE_STAMP: u64 = 0x2553_bcc3_7a5c_33dd;

/// Clocks of one `__mulsi3` call with `b` = 1 on `cpu`; 0 where no table was
/// measured (the 68030 and later, where a multiply is an instruction).
pub fn mulsi3_floor(cpu: crate::dagisel::hooks::Cpu) -> u32 {
    use crate::dagisel::hooks::Cpu;
    match cpu {
        Cpu::M68000 => 96 + 66,
        Cpu::M68010 => 92 + 58,
        Cpu::M68020 => 45 + 24,
        Cpu::M68030 | Cpu::M68040 | Cpu::M68060 => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The body of `__mulsi3` in `int32.c`: from its signature to the first
    /// `}` in column 0.
    fn body() -> String {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/compiler-rt/lib/int/int32.c");
        let text = std::fs::read_to_string(path).expect("the runtime source");
        let lines: Vec<&str> = text.lines().collect();
        let start = lines.iter().position(|line| line.starts_with("uint32_t SFB_NAME(mulsi3)")).unwrap();
        let end = start + lines[start..].iter().position(|line| *line == "}").unwrap();
        lines[start..=end].join("\n")
    }

    /// A trial priced `jsr __mulsi3` as the `jsr` alone (18 clocks) against a
    /// routine of 162 or more, so the library call looked cheaper than the
    /// chain it replaces. The table here is the routine's as measured; this
    /// fails when the routine changes and the numbers are stale.
    #[test]
    fn the_measured_routine_is_the_shipped_one() {
        let hash = body().bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, byte| (h ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3));
        assert_eq!(hash, MULSI3_SOURCE_STAMP, "__mulsi3 changed: measure the table in routine.rs again");
    }

    #[test]
    fn a_call_costs_more_than_any_chain() {
        use crate::dagisel::hooks::Cpu;
        assert!(mulsi3_floor(Cpu::M68000) > 100 && mulsi3_floor(Cpu::M68010) > 100);
    }
}
