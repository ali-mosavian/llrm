//! Bytes and cycles of the instructions the emitter writes, on the NMOS 6502 and the 65C02.
//!
//! Sources: the W65C02S datasheet's Table 4-1 (cycles and bytes per addressing mode, with its notes:
//! +1 for a page cross, +1 for a taken branch, +2 for read-modify-write, STA abs,X always pays the page
//! cycle) and Table 7-1 (what the 65C02 changes), <https://westerndesigncenter.com/wdc/documentation/w65c02s.pdf>;
//! and masswerk's 6502 instruction set tables, <https://www.masswerk.at/6502/6502_instruction_set.html>,
//! for the per-instruction rows. `tests/timing.rs` runs each row on sim65 and compares.
//!
//! DEVIATE: a page cross is not charged (the address is not known here). Reason: the placement is the
//! linker's. Exit: none for a static estimate. A conditional branch counts as taken when it goes
//! backwards (a loop edge) and not taken when it goes forwards.

use crate::dagisel::hooks::Cpu;

/// The cost of one instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cost {
    pub bytes: u32,
    pub cycles: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Implied,
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    IndirectX,
    IndirectY,
    ZeroPageIndirect,
    AbsoluteIndirect,
    Relative,
}

/// Whether `name` is a zero-page byte the emitter names (`rc5`, `rc5+1`).
fn zero_page(name: &str) -> bool {
    let name = name.split('+').next().unwrap_or(name);
    name.strip_prefix("rc").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn mode(
    mnemonic: &str,
    operand: &str,
) -> Option<Mode> {
    let operand = operand.trim();
    if matches!(mnemonic, "bcc" | "bcs" | "beq" | "bmi" | "bne" | "bpl" | "bvc" | "bvs" | "bra") {
        return Some(Mode::Relative);
    }
    if operand.is_empty() {
        return Some(Mode::Implied);
    }
    if operand.eq_ignore_ascii_case("a") {
        return Some(Mode::Accumulator);
    }
    if operand.starts_with('#') {
        return Some(Mode::Immediate);
    }
    if let Some(inner) = operand.strip_prefix('(') {
        if let Some(inner) = inner.strip_suffix("),y") {
            return zero_page(inner).then_some(Mode::IndirectY);
        }
        if let Some(inner) = inner.strip_suffix(",x)") {
            return zero_page(inner).then_some(Mode::IndirectX);
        }
        let inner = inner.strip_suffix(')')?;
        return Some(if mnemonic == "jmp" { Mode::AbsoluteIndirect } else if zero_page(inner) { Mode::ZeroPageIndirect } else { return None });
    }
    let (base, index) = match operand.rsplit_once(',') {
        Some((base, index)) => (base, Some(index)),
        None => (operand, None),
    };
    let zp = zero_page(base);
    Some(match (index, zp) {
        (None, true) => Mode::ZeroPage,
        (None, false) => Mode::Absolute,
        (Some("x"), true) => Mode::ZeroPageX,
        (Some("x"), false) => Mode::AbsoluteX,
        (Some("y"), true) => Mode::ZeroPageY,
        (Some("y"), false) => Mode::AbsoluteY,
        _ => return None,
    })
}

fn bytes(mode: Mode) -> u32 {
    match mode {
        Mode::Implied | Mode::Accumulator => 1,
        Mode::Absolute | Mode::AbsoluteX | Mode::AbsoluteY | Mode::AbsoluteIndirect => 3,
        _ => 2,
    }
}

/// The bytes and cycles of `mnemonic operand` (ca65 text, lower case) on `cpu`, or `None` for a form
/// the emitter does not write or the CPU does not have.
pub fn instruction(
    cpu: Cpu,
    mnemonic: &str,
    operand: &str,
) -> Option<Cost> {
    let cmos = cpu == Cpu::W65c02;
    let m = mode(mnemonic, operand)?;
    let read = matches!(mnemonic, "lda" | "ldx" | "ldy" | "adc" | "sbc" | "and" | "ora" | "eor" | "cmp" | "cpx" | "cpy" | "bit");
    let store = matches!(mnemonic, "sta" | "stx" | "sty" | "stz");
    let modify = matches!(mnemonic, "asl" | "lsr" | "rol" | "ror" | "inc" | "dec");
    let cycles = if mnemonic == "jmp" {
        match m {
            Mode::Absolute => 3,
            Mode::AbsoluteIndirect => if cmos { 6 } else { 5 },
            _ => return None,
        }
    } else if mnemonic == "jsr" {
        6
    } else if matches!(mnemonic, "rts" | "rti") {
        6
    } else if m == Mode::Relative {
        // not taken; a taken one is added by the caller
        if mnemonic == "bra" {
            if !cmos {
                return None;
            }
            3
        } else {
            2
        }
    } else if read {
        match m {
            Mode::Immediate => 2,
            Mode::ZeroPage => 3,
            Mode::ZeroPageX | Mode::ZeroPageY | Mode::Absolute | Mode::AbsoluteX | Mode::AbsoluteY => 4,
            Mode::ZeroPageIndirect => if cmos { 5 } else { return None },
            Mode::IndirectY => 5,
            Mode::IndirectX => 6,
            _ => return None,
        }
    } else if store {
        if mnemonic == "stz" && !cmos {
            return None;
        }
        match m {
            Mode::ZeroPage => 3,
            Mode::ZeroPageX | Mode::ZeroPageY | Mode::Absolute => 4,
            Mode::AbsoluteX | Mode::AbsoluteY => 5,
            Mode::IndirectY => 6,
            Mode::IndirectX => 6,
            Mode::ZeroPageIndirect => if cmos { 5 } else { return None },
            _ => return None,
        }
    } else if modify {
        match m {
            Mode::Accumulator => {
                if matches!(mnemonic, "inc" | "dec") && !cmos {
                    return None;
                }
                2
            }
            Mode::ZeroPage => 5,
            Mode::ZeroPageX => 6,
            Mode::Absolute => 6,
            Mode::AbsoluteX => if cmos && !matches!(mnemonic, "inc" | "dec") { 6 } else { 7 },
            _ => return None,
        }
    } else if m == Mode::Implied {
        match mnemonic {
            "tax" | "tay" | "txa" | "tya" | "tsx" | "txs" | "inx" | "iny" | "dex" | "dey" | "clc" | "sec" | "cli"
            | "sei" | "cld" | "sed" | "clv" | "nop" => 2,
            "pha" | "php" => 3,
            "pla" | "plp" => 4,
            "phx" | "phy" if cmos => 3,
            "plx" | "ply" if cmos => 4,
            _ => return None,
        }
    } else {
        return None;
    };
    Some(Cost { bytes: bytes(m), cycles })
}

/// A static estimate of ca65 text from [`crate::alloc::emit`]: its bytes, and its cycles with each
/// line counted once per trip, where a trip through a loop is ten for each level it is nested.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Estimate {
    pub bytes: u32,
    pub cycles: u64,
}

/// [`Estimate`] of `text`, or the first line it cannot price.
pub fn estimate(
    cpu: Cpu,
    text: &str,
) -> Result<Estimate, String> {
    let lines: Vec<&str> = text.lines().map(|l| l.split(';').next().unwrap_or("")).collect();
    let label_at = |name: &str| lines.iter().position(|l| l.trim_end().strip_suffix(':').is_some_and(|label| label == name));
    // a backward branch makes the lines from its label to it a loop
    let mut depth = vec![0u32; lines.len()];
    let mut backward = vec![false; lines.len()];
    for (at, line) in lines.iter().enumerate() {
        let mut parts = line.split_whitespace();
        let (Some(mnemonic), Some(target)) = (parts.next(), parts.next()) else { continue };
        if !matches!(mnemonic, "bcc" | "bcs" | "beq" | "bmi" | "bne" | "bpl" | "bvc" | "bvs" | "bra" | "jmp") {
            continue;
        }
        if let Some(to) = label_at(target).filter(|to| *to <= at) {
            backward[at] = true;
            for d in &mut depth[to..=at] {
                *d += 1;
            }
        }
    }
    let mut out = Estimate::default();
    let mut in_data = false;
    for (at, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.ends_with(':') {
            continue;
        }
        if let Some(directive) = trimmed.strip_prefix('.') {
            if directive.starts_with("segment") {
                in_data = !directive.contains("\"CODE\"");
            }
            continue;
        }
        if in_data || trimmed.contains(" = ") {
            continue;
        }
        let (mnemonic, operand) = trimmed.split_once(char::is_whitespace).unwrap_or((trimmed, ""));
        let cost = instruction(cpu, mnemonic, operand).ok_or_else(|| format!("cannot price `{trimmed}`"))?;
        let taken = u32::from(backward[at] && mnemonic != "jmp" && mnemonic != "bra");
        out.bytes += cost.bytes;
        out.cycles += u64::from(cost.cycles + taken) * 10u64.pow(depth[at].min(3));
    }
    Ok(out)
}
