//! What x86 says of GCC inline assembly: clang's `X86TargetInfo` and the
//! constraint hooks of `X86TargetLowering`.

use llrm_target::inline_asm::{Immediates, InlineAsm, StackPop};

/// The inline-assembly facts of an x86 target of mode `bits` (16, 32 or 64)
/// whose register file is `registers`.
pub fn inline_asm(
    bits: u8,
    registers: String,
) -> InlineAsm {
    let long = bits == 64;
    InlineAsm {
        bits,
        flat: bits != 16,
        registers,
        mode: Some(match bits {
            16 => ".code16",
            32 => ".code32",
            _ => ".code64",
        }),
        aliases: &[("st", "st0"), ("st(0)", "st0"), ("st(1)", "st1")],
        immediates: Some(Immediates { letters: b"iInJKLMNOeZg", fits: if long { fits64 } else { fits32 } }),
        // `reg-stack` pops a dead stack input: `fscale` leaves st(1).
        dead_stack_input: Some(StackPop { register: "st1", att: "fstp %st(1)", intel: "fstp st(1)" }),
    }
}

fn fits32(
    letter: u8,
    zero_extended: i128,
    width: u32,
) -> bool {
    immediate_fits(letter, zero_extended, width, false)
}

fn fits64(
    letter: u8,
    zero_extended: i128,
    width: u32,
) -> bool {
    immediate_fits(letter, zero_extended, width, true)
}

/// `X86TargetLowering::LowerAsmOperandForConstraint`: whether the constant
/// `zero_extended` (of `width` bits) is one the immediate letter takes.
fn immediate_fits(
    letter: u8,
    zero_extended: i128,
    width: u32,
    long: bool,
) -> bool {
    let signed = if zero_extended >= 1 << (width - 1) { zero_extended - (1 << width) } else { zero_extended };
    match letter {
        b'I' => zero_extended <= 31,
        b'J' => zero_extended <= 63,
        b'K' => (-128..=127).contains(&signed),
        b'L' => matches!(zero_extended, 0xff | 0xffff) || long && zero_extended == 0xffff_ffff,
        b'M' => zero_extended <= 3,
        b'N' => zero_extended <= 255,
        b'O' => zero_extended <= 127,
        b'e' => (i128::from(i32::MIN)..=i128::from(i32::MAX)).contains(&signed),
        b'Z' => zero_extended <= 0xffff_ffff,
        _ => matches!(letter, b'i' | b'n' | b'g'),
    }
}
