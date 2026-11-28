//! Encoding costs of x86 registers that the allocator prices a choice by.

use llrm_lir::registers::RegId;
use llrm_lir::{Loc, Semantics};

/// What the encoding rules ask of a register, read from its name once.
#[derive(Clone, Copy)]
struct RegisterTraits {
    extended: bool,
    byte_rex: bool,
    accumulator: bool,
}

fn traits_of(phys: RegId) -> RegisterTraits {
    static TABLE: std::sync::OnceLock<Vec<RegisterTraits>> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        RegId::values()
            .map(|reg| {
                let name = format!("{reg:?}").to_lowercase();
                RegisterTraits {
                    extended: name
                        .strip_prefix('r')
                        .and_then(|n| n.trim_end_matches(char::is_alphabetic).parse::<u32>().ok())
                        .is_some_and(|n| n >= 8),
                    byte_rex: matches!(name.as_str(), "sil" | "dil" | "spl" | "bpl"),
                    accumulator: matches!(name.as_str(), "al" | "ax" | "eax" | "rax"),
                }
            })
            .collect()
    });
    table[phys.index()]
}

/// The encoding traits of `phys` as a number: registers with the same one take
/// the same bytes in any instruction (`register_extra_bytes` reads no more).
pub fn encoding_class(phys: RegId) -> u8 {
    let RegisterTraits { extended, byte_rex, accumulator } = traits_of(phys);
    u8::from(extended) | u8::from(byte_rex) << 1 | u8::from(accumulator) << 2
}

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
    let RegisterTraits { extended, byte_rex, accumulator } = traits_of(phys);
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

#[cfg(test)]
mod tests {
    use llrm_lir::{Held, Imm, Operation};

    use super::*;

    /// `encoding_class` lets greedy price one register per class; two registers
    /// of a class that took different bytes in some instruction would make it
    /// pick differently from pricing each (C5 compile-time work).
    #[test]
    fn registers_of_one_class_take_the_same_bytes() {
        let held = |width| Loc::Held(Held { value: 1, width, offset: 0 });
        let imm = |value| Loc::Imm(Imm { value, width: 4, address: None });
        let mut shapes = Vec::new();
        for (name, op) in [("cmp", Operation::Binary), ("add", Operation::Binary), ("mov", Operation::Move)] {
            for width in [1, 2, 4, 8] {
                for source in [held(width), imm(5), imm(1000)] {
                    let mut what = Semantics::new(op);
                    what.name = Some(name.into());
                    what.dests = vec![held(width)];
                    what.sources = vec![held(width), source];
                    shapes.push(what);
                }
            }
        }
        for what in &shapes {
            let mut seen: std::collections::HashMap<u8, i64> = std::collections::HashMap::new();
            for phys in RegId::values() {
                let bytes = register_extra_bytes(what, 1, phys);
                let known = *seen.entry(encoding_class(phys)).or_insert(bytes);
                assert_eq!(known, bytes, "{phys:?} in {:?}", what.name);
            }
        }
    }
}
