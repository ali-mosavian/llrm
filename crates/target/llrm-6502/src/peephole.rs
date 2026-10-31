//! Redundant loads and compares removed from a function's emitted assembly.
//!
//! # How it works
//! Three passes walk the lines of a function once each. What a line does to
//! `a` and to the zero and sign flags comes from one table, [`effect`], so a
//! pass asks "does this line set the flags" and does not list mnemonics:
//!
//! - [`drop_reloads`]: `sta m; lda m` (likewise `x`, `y`) of a zero-page or
//!   spill cell, when the load's flags are unread.
//! - [`drop_known_loads`]: `ldx #n` of what the register already holds, tracked
//!   through immediates and transfers, forgotten at a label, a call or a line
//!   not listed.
//! - [`drop_zero_compares`]: `cmp #0` before a branch on zero or sign, after a
//!   line that set those flags from `a`.
//!
//! DEVIATE: LLVM's `PeepholeOptimizer` and `MachineCSE` work on allocated
//! machine instructions; these work on the printed lines of one function,
//! because the 6502 emitter prints as it allocates. Exit: D-6502 of the greedy
//! PR #6259 replaces these textual passes with the shared
//! `PeepholeOptimizer`/`MachineCSE` on allocated MIR; delete this file then.
//!
//! # Example
//! ```text
//!   sta rc2          sta rc2
//!   lda rc2     =>   ora rc3
//!   ora rc3
//! ```
//!
//! # Runs
//! At the end of `alloc::emit_with`, before branch relaxation.

/// What a line does to the zero and sign flags (and, for [`Flags::OfA`], to
/// `a`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Flags {
    /// Neither `a` nor the flags change: a store, a carry flag, `pha`, `nop`.
    Keeps,
    /// The flags describe `a` afterwards.
    OfA,
    /// The flags are set from some other result, or clobbered (a call).
    Sets,
    /// A branch, a jump, a label or a line not listed: the flags may be read.
    Reads,
}

/// The effect of the line `mnemonic operand` on the flags.
fn effect(
    mnemonic: &str,
    operand: &str,
) -> Flags {
    match mnemonic {
        "sta" | "stx" | "sty" | "stz" | "clc" | "sec" | "pha" | "nop" => Flags::Keeps,
        "lda" | "txa" | "tya" | "tax" | "tay" | "pla" | "and" | "ora" | "eor" | "adc" | "sbc" => Flags::OfA,
        "asl" | "lsr" | "rol" | "ror" | "inc" | "dec" if operand == "a" => Flags::OfA,
        "ldx" | "ldy" | "cmp" | "cpx" | "cpy" | "asl" | "lsr" | "rol" | "ror" | "inc" | "dec" | "inx" | "iny"
        | "dex" | "dey" | "bit" | "jsr" | "rts" => Flags::Sets,
        _ => Flags::Reads,
    }
}

/// The mnemonic and operand of an instruction line, `None` for a label or a
/// directive.
fn instruction(line: &str) -> Option<(&str, &str)> {
    let body = line.strip_prefix("  ")?.trim();
    if body.starts_with('.') {
        return None;
    }
    Some(body.split_once(' ').unwrap_or((body, "")))
}

/// Whether the zero and sign flags are set again before anything reads them,
/// from `lines[from]` on.
fn flags_unread_after(
    lines: &[&str],
    from: usize,
) -> bool {
    for line in &lines[from..] {
        let Some((mnemonic, operand)) = instruction(line) else { return false };
        match effect(mnemonic, operand) {
            Flags::Keeps => {}
            Flags::OfA | Flags::Sets => return true,
            Flags::Reads => return false,
        }
    }
    false
}

/// Whether the emitted line leaves the zero and sign flags describing what `a`
/// now holds.
pub fn sets_nz_of_a(line: &str) -> bool {
    instruction(line).is_some_and(|(mnemonic, operand)| effect(mnemonic, operand) == Flags::OfA)
}

/// The three passes, in the order they feed each other.
pub fn run(text: &str) -> String {
    drop_zero_compares(&drop_known_loads(&drop_reloads(text)))
}

/// `cmp #0` before a branch on zero or sign, after an instruction that set
/// those flags from `a` (stores in between leave them): the branch reads them
/// as they are.
pub fn drop_zero_compares(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::with_capacity(text.len());
    for (at, line) in lines.iter().enumerate() {
        let branch = lines.get(at + 1).and_then(|next| next.trim().split(' ').next());
        let tested = (line.trim() == "cmp #0" && matches!(branch, Some("beq" | "bne" | "bmi" | "bpl")))
            .then(|| {
                lines[..at].iter().rev().find(|before| {
                    !instruction(before).is_some_and(|(mnemonic, operand)| effect(mnemonic, operand) == Flags::Keeps)
                })
            })
            .flatten()
            .is_some_and(|before| sets_nz_of_a(before));
        if !tested {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// `ldx #n` (likewise `lda` and `ldy`) of what the register already holds: it
/// goes, unless a branch reads the flags it sets. The values known are the
/// immediates loaded and the transfers between registers, forgotten at a
/// label, a call or an instruction not listed.
pub fn drop_known_loads(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut known: [Option<&str>; 3] = [None; 3];
    let slot = |register: &str| match register {
        "lda" | "a" => Some(0),
        "ldx" | "x" => Some(1),
        "ldy" | "y" => Some(2),
        _ => None,
    };
    let mut out = String::with_capacity(text.len());
    for (at, line) in lines.iter().enumerate() {
        let Some((mnemonic, operand)) = instruction(line) else {
            known = [None; 3];
            out.push_str(line);
            out.push('\n');
            continue;
        };
        match (mnemonic, slot(mnemonic)) {
            (_, Some(register)) if operand.starts_with('#') => {
                if known[register] == Some(operand) && flags_unread_after(&lines, at + 1) {
                    continue;
                }
                known[register] = Some(operand);
            }
            ("lda" | "pla" | "adc" | "sbc" | "and" | "ora" | "eor", _) => known[0] = None,
            ("asl" | "lsr" | "rol" | "ror" | "inc" | "dec", _) if operand.is_empty() || operand == "a" => {
                known[0] = None
            }
            ("ldx" | "inx" | "dex", _) => known[1] = None,
            ("ldy" | "iny" | "dey", _) => known[2] = None,
            ("txa", _) => known[0] = known[1],
            ("tya", _) => known[0] = known[2],
            ("tax", _) => known[1] = known[0],
            ("tay", _) => known[2] = known[0],
            // a register's value survives what neither writes it nor leaves
            // the function: stores, compares, branches, memory
            // read-modify-write
            ("cmp" | "cpx" | "cpy" | "bit" | "inc" | "dec" | "asl" | "lsr" | "rol" | "ror", _) => {}
            _ if effect(mnemonic, operand) == Flags::Keeps => {}
            ("beq" | "bne" | "bcc" | "bcs" | "bmi" | "bpl" | "bvc" | "bvs", _) => {}
            _ => known = [None; 3],
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// `sta m` then `lda m` (likewise `x` and `y`) of a zero-page or spill cell:
/// the register still holds it, so the load goes. It also set the zero and sign
/// flags, so it stays when a branch reads them before anything sets them again.
pub fn drop_reloads(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let cell = |operand: &str| operand.starts_with("rc") || operand.starts_with("s_");
    let mut out = String::with_capacity(text.len());
    for (at, line) in lines.iter().enumerate() {
        let reload = at > 0
            && lines[at - 1].starts_with("  st")
            && line.starts_with("  ld")
            && lines[at - 1].trim()[2..] == line.trim()[2..]
            && line.trim().split(' ').nth(1).is_some_and(cell)
            && flags_unread_after(&lines, at + 1);
        if !reload {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}
