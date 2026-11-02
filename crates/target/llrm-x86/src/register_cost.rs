//! Encoding costs of x86 registers that the allocator prices a choice by.

use llrm_lir::registers::RegId;
use llrm_lir::{Loc, Semantics};

/// What register `phys` adds to the instruction `what` for `value`, from x86's
/// encoding rules: `r8`..`r15` and, at byte width, `sil`/`dil`/`spl`/`bpl` need
/// a REX prefix (one byte) unless the instruction has one anyway (a 64-bit
/// operand); the accumulator has the shorter form of an ALU operation with an
/// immediate that does not fit a byte (`cmp eax, imm32`, one byte less), and of
/// every byte one.
pub fn register_extra_bytes(
    what: &Semantics,
    value: u32,
    phys: RegId,
) -> i64 {
    let name = format!("{phys:?}").to_lowercase();
    let extended = name
        .strip_prefix('r')
        .and_then(|n| n.trim_end_matches(char::is_alphabetic).parse::<u32>().ok())
        .is_some_and(|n| n >= 8);
    let byte_rex = matches!(name.as_str(), "sil" | "dil" | "spl" | "bpl");
    let accumulator = matches!(name.as_str(), "al" | "ax" | "eax" | "rax");
    let operands = what.dests.iter().chain(&what.sources);
    // A 64-bit data operand puts REX.W on the instruction: a prefix is there.
    let wide = operands
        .clone()
        .any(
            |loc| match loc {
                Loc::Held(held) => held.width == 8 && held.value != value,
                Loc::Mem(cell) => cell.width == 8,
                _ => false,
            },
        );
    let (mut register_width, mut as_register, mut as_address) = (0, false, false);
    for loc in operands.clone() {
        match loc {
            Loc::Held(held) if held.value == value => {
                register_width = register_width.max(held.width);
                as_register = true;
            }
            Loc::Mem(cell) => {
                as_address |= [cell.base, cell.index].into_iter().flatten().any(|held| held.value == value);
            }
            _ => {}
        }
    }
    let mut bytes = 0;
    let needs_rex = if as_register { extended && register_width < 8 || byte_rex && register_width == 1 } else { false };
    if (needs_rex || (as_address && extended)) && !wide && register_width < 8 {
        bytes += 1;
    }
    if as_register && accumulator {
        let alu =
            matches!(
                what.name.as_deref(),
                Some("add" | "adc" | "sub" | "sbb" | "and" | "or" | "xor" | "cmp" | "test")
            );
        let immediate = what
            .sources
            .iter()
            .find_map(
                |loc| match loc {
                    Loc::Imm(imm) if imm.address.is_none() => Some(imm.value),
                    _ => None,
                },
            );
        if alu && immediate.is_some_and(|v| register_width == 1 || !(-128..=127).contains(&v)) {
            bytes -= 1;
        }
    }
    bytes
}
