//! Machine instructions to ca65 text: the pseudo-instructions of `mos6502.td`
//! expanded into 6502 code. Spike-local: a machine instruction is the generated
//! opcode id and its operands in the `.td` order (outs, then ins); swap for the
//! neutral machine function when that exists.
//!
//! # How it works
//! A byte operand is a register name: `a`, `x`, `y` or `rcN` (a zero-page
//! byte). A word is a pair `rsN`, the bytes `rc{2N}` and `rc{2N+1}`. The carry
//! is the hardware C flag: the instructions that define or use a `Carry`
//! operand leave it there, so nothing may be scheduled between a carry's
//! producer and its user.
//!
//! DEVIATE: the borrow of `USUBO` is the inverse of C (`SBC` clears it on a
//! borrow). A chain of `SUBO*` to `SUBOC*` is exact; an `i1` borrow read as a
//! value is not, and is not selected yet. Exit: when a `zext` of an overflow
//! flag is selected.

use std::fmt::Write;

use crate::dagisel::generated::ids::opcode as op;
use crate::platform::Platform;
use crate::target::imaginary;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operand {
    Reg(String),
    Imm(i64),
    /// A frame object, until the frame is laid out
    /// (`frame::Layout::eliminate`).
    FrameIndex(usize),
    /// A label or a symbol.
    Label(String),
}

impl Operand {
    pub fn reg(name: &str) -> Operand {
        Operand::Reg(name.to_owned())
    }
}

/// Pseudo-opcodes outside the generated range: `select.rs` keeps the carry and
/// the flags in memory between a producer and its user, because nothing in the
/// DAG keeps the hardware flags alive.
pub mod spill {
    /// `cell := C` and `C := cell`.
    pub const SAVE_CARRY: u16 = 0xF000;
    pub const LOAD_CARRY: u16 = 0xF001;
    /// `cell := P` and `P := cell`.
    pub const SAVE_FLAGS: u16 = 0xF002;
    pub const LOAD_FLAGS: u16 = 0xF003;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MInst {
    pub opcode: u16,
    pub operands: Vec<Operand>,
}

impl MInst {
    pub fn new(
        opcode: u16,
        operands: Vec<Operand>,
    ) -> MInst {
        MInst { opcode, operands }
    }
}

/// The two bytes of a pair register: `rs2` is `rc4` (low) and `rc5` (high). A
/// word cell `w_..` (`select.rs`) is two bytes in memory, `w_..` and `w_..+1`.
fn halves(name: &str) -> Option<(String, String)> {
    if name.starts_with("w_") {
        return Some((name.to_owned(), format!("{name}+1")));
    }
    let n: usize = name.strip_prefix("rs")?.parse().ok()?;
    Some((imaginary::name(2 * n), imaginary::name(2 * n + 1)))
}

fn reg(operand: &Operand) -> Result<&str, String> {
    match operand {
        Operand::Reg(name) => Ok(name),
        other => Err(format!("{other:?} is not a register")),
    }
}

fn imm(operand: &Operand) -> Result<u128, String> {
    match operand {
        Operand::Imm(v) => Ok(*v as u128),
        other => Err(format!("{other:?} is not an immediate")),
    }
}

/// The branch that is taken when condition code `code` holds (`cond` in
/// `lowering`).
fn branch_on(code: u128) -> Result<&'static str, String> {
    Ok(match code {
        0 => "beq",
        1 => "bne",
        2 => "bcc",
        3 => "bcs",
        4 => "bmi",
        5 => "bpl",
        other => return Err(format!("{other} is not a condition code")),
    })
}

/// `symbol+offset` for an absolute access.
pub fn address(
    symbol: &Operand,
    offset: &Operand,
) -> Result<String, String> {
    match (symbol, offset) {
        (Operand::Label(symbol), Operand::Imm(0)) => Ok(symbol.clone()),
        (Operand::Label(symbol), Operand::Imm(offset)) => Ok(format!("{symbol}+{offset}")),
        other => Err(format!("{other:?} is not an absolute address")),
    }
}

fn word(operand: &Operand) -> Result<(String, String), String> {
    halves(reg(operand)?).ok_or_else(|| format!("{operand:?} is not a pair register"))
}

/// The zero-page byte `(zp),y` can use for a pointer whose low byte is `lo`:
/// itself when it is an `rcN`, else the pair is copied to `rs3` (`rc6`, `rc7`),
/// which the convention lets any routine clobber.
fn zero_page_pointer(
    out: &mut String,
    lo: &str,
    hi: &str,
) -> String {
    if imaginary::is(lo) {
        return lo.to_owned();
    }
    let (low, high) = (imaginary::name(6), imaginary::name(7));
    writeln!(out, "  lda {lo}\n  sta {low}\n  lda {hi}\n  sta {high}").unwrap();
    low
}

/// A byte copy `d := s` through the accumulator where neither side is a
/// register that can do it.
fn copy_byte(
    out: &mut String,
    d: &str,
    s: &str,
) {
    match (d, s) {
        ("a", "a") | ("x", "x") | ("y", "y") => {}
        ("a" | "x" | "y", _) => writeln!(out, "  ld{d} {s}").unwrap(),
        (_, "a" | "x" | "y") => writeln!(out, "  st{s} {d}").unwrap(),
        _ => {
            writeln!(out, "  lda {s}").unwrap();
            writeln!(out, "  sta {d}").unwrap();
        }
    }
}

/// One instruction as ca65 lines (each with its two-space indent).
pub fn expand(
    inst: &MInst,
    labels: &mut u32,
) -> Result<String, String> {
    let o = &inst.operands;
    let mut out = String::new();
    let byte3 = |out: &mut String, carry: &str, mnemonic: &str, d: &str, a: &str, b: &str| {
        if !carry.is_empty() {
            writeln!(out, "  {carry}").unwrap();
        }
        writeln!(out, "  lda {a}").unwrap();
        writeln!(out, "  {mnemonic} {b}").unwrap();
        writeln!(out, "  sta {d}").unwrap();
    };
    // Byte ops on three registers: d, a, b.
    let simple = |o: &[Operand]| -> Result<(String, String, String), String> {
        Ok((reg(&o[0])?.to_owned(), reg(&o[1])?.to_owned(), reg(&o[2])?.to_owned()))
    };
    // The same on pairs, low byte then high byte; the carry runs from the first
    // to the second.
    let wide =
        |out: &mut String, first: &str, next: &str, d: &Operand, a: &Operand, b: &Operand| -> Result<(), String> {
            let ((dl, dh), (al, ah), (bl, bh)) = (word(d)?, word(a)?, word(b)?);
            byte3(out, if first == "adc" { "clc" } else { "sec" }, first, &dl, &al, &bl);
            byte3(out, "", next, &dh, &ah, &bh);
            Ok(())
        };
    match inst.opcode {
        op::ADD8 | op::ADDO8 => {
            let (d, a, b) = if inst.opcode == op::ADD8 {
                simple(o)?
            } else {
                (reg(&o[0])?.into(), reg(&o[2])?.into(), reg(&o[3])?.into())
            };
            byte3(&mut out, "clc", "adc", &d, &a, &b);
        }
        op::SUB8 | op::SUBO8 => {
            let (d, a, b) = if inst.opcode == op::SUB8 {
                simple(o)?
            } else {
                (reg(&o[0])?.into(), reg(&o[2])?.into(), reg(&o[3])?.into())
            };
            byte3(&mut out, "sec", "sbc", &d, &a, &b);
        }
        // The carry in is already in C.
        op::ADDOC8 => byte3(&mut out, "", "adc", reg(&o[0])?, reg(&o[2])?, reg(&o[3])?),
        op::SUBOC8 => byte3(&mut out, "", "sbc", reg(&o[0])?, reg(&o[2])?, reg(&o[3])?),
        op::AND8 => {
            let (d, a, b) = simple(o)?;
            byte3(&mut out, "", "and", &d, &a, &b);
        }
        op::OR8 => {
            let (d, a, b) = simple(o)?;
            byte3(&mut out, "", "ora", &d, &a, &b);
        }
        op::XOR8 => {
            let (d, a, b) = simple(o)?;
            byte3(&mut out, "", "eor", &d, &a, &b);
        }
        op::ADD16 => wide(&mut out, "adc", "adc", &o[0], &o[1], &o[2])?,
        op::SUB16 => wide(&mut out, "sbc", "sbc", &o[0], &o[1], &o[2])?,
        op::ADDO16 => wide(&mut out, "adc", "adc", &o[0], &o[2], &o[3])?,
        op::SUBO16 => wide(&mut out, "sbc", "sbc", &o[0], &o[2], &o[3])?,
        op::ADDOC16 | op::SUBOC16 => {
            let m = if inst.opcode == op::ADDOC16 { "adc" } else { "sbc" };
            let ((dl, dh), (al, ah), (bl, bh)) = (word(&o[0])?, word(&o[2])?, word(&o[3])?);
            byte3(&mut out, "", m, &dl, &al, &bl);
            byte3(&mut out, "", m, &dh, &ah, &bh);
        }
        op::AND16 | op::OR16 | op::XOR16 => {
            let m = match inst.opcode {
                op::AND16 => "and",
                op::OR16 => "ora",
                _ => "eor",
            };
            let ((dl, dh), (al, ah), (bl, bh)) = (word(&o[0])?, word(&o[1])?, word(&o[2])?);
            byte3(&mut out, "", m, &dl, &al, &bl);
            byte3(&mut out, "", m, &dh, &ah, &bh);
        }
        op::LDI8 => match (&o[0], &o[1]) {
            (Operand::Reg(d), Operand::Imm(v)) => {
                writeln!(out, "  lda #{}", v & 0xFF).unwrap();
                writeln!(out, "  sta {d}").unwrap();
            }
            _ => return Err("LDI8 takes a register and an immediate".into()),
        },
        op::LDI16 => match &o[1] {
            Operand::Imm(v) => {
                let (lo, hi) = word(&o[0])?;
                writeln!(out, "  lda #{}", v & 0xFF).unwrap();
                writeln!(out, "  sta {lo}").unwrap();
                writeln!(out, "  lda #{}", (v >> 8) & 0xFF).unwrap();
                writeln!(out, "  sta {hi}").unwrap();
            }
            Operand::Label(symbol) => {
                let (lo, hi) = word(&o[0])?;
                writeln!(out, "  lda #<({symbol})\n  sta {lo}\n  lda #>({symbol})\n  sta {hi}").unwrap();
            }
            _ => return Err("LDI16 takes a pair and an immediate".into()),
        },
        // `(zp),y` through the pair's low byte, y the byte offset; a word is
        // two bytes, INY between.
        op::LDP8 => {
            let ((lo, hi), off) = (word(&o[1])?, imm(&o[2])?);
            let lo = zero_page_pointer(&mut out, &lo, &hi);
            writeln!(out, "  ldy #{off}\n  lda ({lo}),y\n  sta {}", reg(&o[0])?).unwrap();
        }
        op::LDP16 => {
            let ((dl, dh), (lo, hi), off) = (word(&o[0])?, word(&o[1])?, imm(&o[2])?);
            let lo = zero_page_pointer(&mut out, &lo, &hi);
            writeln!(out, "  ldy #{off}\n  lda ({lo}),y\n  sta {dl}\n  iny\n  lda ({lo}),y\n  sta {dh}").unwrap();
        }
        op::STP8 => {
            let ((lo, hi), off) = (word(&o[1])?, imm(&o[2])?);
            let lo = zero_page_pointer(&mut out, &lo, &hi);
            writeln!(out, "  ldy #{off}\n  lda {}\n  sta ({lo}),y", reg(&o[0])?).unwrap();
        }
        op::STP16 => {
            let ((sl, sh), (lo, hi), off) = (word(&o[0])?, word(&o[1])?, imm(&o[2])?);
            let lo = zero_page_pointer(&mut out, &lo, &hi);
            writeln!(out, "  ldy #{off}\n  lda {sl}\n  sta ({lo}),y\n  iny\n  lda {sh}\n  sta ({lo}),y").unwrap();
        }
        op::LDAX | op::STAX => {
            let load = inst.opcode == op::LDAX;
            let (value, symbol, index) = if load { (&o[0], &o[1], &o[2]) } else { (&o[0], &o[1], &o[2]) };
            let Operand::Label(symbol) = symbol else {
                return Err(format!("an indexed access takes a symbol, not {symbol:?}"));
            };
            let (value, index) = (reg(value)?, reg(index)?);
            if load {
                writeln!(out, "  ldx {index}\n  lda {symbol},x\n  sta {value}").unwrap();
            } else {
                writeln!(out, "  ldx {index}\n  lda {value}\n  sta {symbol},x").unwrap();
            }
        }
        op::LDABS => writeln!(
            out,
            "  lda {}
  sta {}",
            address(&o[1], &o[2])?,
            reg(&o[0])?
        )
        .unwrap(),
        op::STABS => writeln!(
            out,
            "  lda {}
  sta {}",
            reg(&o[0])?,
            address(&o[1], &o[2])?
        )
        .unwrap(),
        op::STZABS => writeln!(out, "  stz {}", address(&o[0], &o[1])?).unwrap(),
        op::STZP => {
            let ((lo, hi), off) = (word(&o[0])?, imm(&o[1])?);
            let lo = zero_page_pointer(&mut out, &lo, &hi);
            if off == 0 {
                writeln!(out, "  lda #0\n  sta ({lo})").unwrap();
            } else {
                writeln!(out, "  ldy #{off}\n  lda #0\n  sta ({lo}),y").unwrap();
            }
        }
        // A shift by a constant: copy to the destination, then one step per bit
        // in place.
        op::SHL8 | op::SRL8 | op::SRA8 => {
            let (d, a, k) = (reg(&o[0])?, reg(&o[1])?, imm(&o[2])?);
            copy_byte(&mut out, d, a);
            for _ in 0..k {
                match inst.opcode {
                    op::SHL8 => writeln!(out, "  asl {d}").unwrap(),
                    op::SRL8 => writeln!(out, "  lsr {d}").unwrap(),
                    _ => writeln!(out, "  lda {d}\n  cmp #$80\n  ror {d}").unwrap(),
                }
            }
        }
        op::SHL16 | op::SRL16 | op::SRA16 => {
            let ((dl, dh), (al, ah), k) = (word(&o[0])?, word(&o[1])?, imm(&o[2])?);
            copy_byte(&mut out, &dl, &al);
            copy_byte(&mut out, &dh, &ah);
            for _ in 0..k {
                match inst.opcode {
                    op::SHL16 => writeln!(out, "  asl {dl}\n  rol {dh}").unwrap(),
                    op::SRL16 => writeln!(out, "  lsr {dh}\n  ror {dl}").unwrap(),
                    _ => writeln!(out, "  lda {dh}\n  cmp #$80\n  ror {dh}\n  ror {dl}").unwrap(),
                }
            }
        }
        // rs0 += n; the carry runs from the low byte to the high.
        op::ADJSP => {
            let n = imm(&o[0])? as u16;
            crate::alloc::adjust(&mut out, i64::from(n));
        }
        op::LEASP16 => {
            let ((dl, dh), n) = (word(&o[0])?, imm(&o[1])? as u16);
            let (sp_low, sp_high) = (imaginary::name(0), imaginary::name(1));
            writeln!(
                out,
                "  clc\n  lda {sp_low}\n  adc #{}\n  sta {dl}\n  lda {sp_high}\n  adc #{}\n  sta {dh}",
                n & 0xFF,
                n >> 8
            )
            .unwrap();
        }
        op::MUL8 | op::SDIV8 | op::SREM8 | op::SHLV8 | op::SRLV8 | op::SRAV8 => {
            let routine = match inst.opcode {
                op::MUL8 => "__mulqi3",
                op::SDIV8 => "__divqi3",
                op::SREM8 => "__modqi3",
                op::SHLV8 => "__ashlqi3",
                op::SRLV8 => "__lshrqi3",
                _ => "__ashrqi3",
            };
            copy_byte(&mut out, &imaginary::name(2), reg(&o[1])?);
            copy_byte(&mut out, &imaginary::name(3), reg(&o[2])?);
            writeln!(out, "  jsr {routine}").unwrap();
            copy_byte(&mut out, reg(&o[0])?, "a");
        }
        op::UDIVREM8 => {
            copy_byte(&mut out, &imaginary::name(2), reg(&o[2])?);
            copy_byte(&mut out, &imaginary::name(3), reg(&o[3])?);
            writeln!(out, "  jsr __udivmodqi4").unwrap();
            copy_byte(&mut out, reg(&o[0])?, "a");
            copy_byte(&mut out, reg(&o[1])?, &imaginary::name(6));
        }
        op::UDIVREM16 => {
            let ((ql, qh), (rl, rh)) = (word(&o[0])?, word(&o[1])?);
            let ((xl, xh), (yl, yh)) = (word(&o[2])?, word(&o[3])?);
            copy_byte(&mut out, &imaginary::name(2), &xl);
            copy_byte(&mut out, &imaginary::name(3), &xh);
            copy_byte(&mut out, &imaginary::name(4), &yl);
            copy_byte(&mut out, &imaginary::name(5), &yh);
            writeln!(out, "  jsr __udivhi3").unwrap();
            copy_byte(&mut out, &ql, "a");
            copy_byte(&mut out, &qh, "x");
            copy_byte(&mut out, &rl, &imaginary::name(6));
            copy_byte(&mut out, &rh, &imaginary::name(7));
        }
        op::UMUL8X8 => {
            let (dl, dh) = word(&o[0])?;
            copy_byte(&mut out, &imaginary::name(2), reg(&o[1])?);
            copy_byte(&mut out, &imaginary::name(3), reg(&o[2])?);
            writeln!(out, "  jsr __umulqihi3").unwrap();
            copy_byte(&mut out, &dl, "a");
            copy_byte(&mut out, &dh, "x");
        }
        op::MUL16 | op::SDIV16 | op::SREM16 | op::SHLV16 | op::SRLV16 | op::SRAV16 => {
            let routine = match inst.opcode {
                op::MUL16 => "__mulhi3",
                op::SDIV16 => "__divhi3",
                op::SREM16 => "__modhi3",
                op::SHLV16 => "__ashlhi3",
                op::SRLV16 => "__lshrhi3",
                _ => "__ashrhi3",
            };
            let ((dl, dh), (al, ah)) = (word(&o[0])?, word(&o[1])?);
            copy_byte(&mut out, &imaginary::name(2), &al);
            copy_byte(&mut out, &imaginary::name(3), &ah);
            // A shift count is a byte in `rc4`; the other operations take a
            // second word.
            if matches!(inst.opcode, op::SHLV16 | op::SRLV16 | op::SRAV16) {
                copy_byte(&mut out, &imaginary::name(4), reg(&o[2])?);
            } else {
                let (bl, bh) = word(&o[2])?;
                copy_byte(&mut out, &imaginary::name(4), &bl);
                copy_byte(&mut out, &imaginary::name(5), &bh);
            }
            writeln!(out, "  jsr {routine}").unwrap();
            copy_byte(&mut out, &dl, "a");
            copy_byte(&mut out, &dh, "x");
        }
        // A long operation: the operands are copied to `__sa` and `__sb`
        // (runtime/mos_rt.s), the result comes back in `__sa`.
        op::MUL32 | op::UDIV32 | op::SDIV32 | op::UREM32 | op::SREM32 | op::SHL32 | op::SRL32 | op::SRA32 => {
            let routine = match inst.opcode {
                op::MUL32 => "__mulsi3",
                op::UDIV32 => "__udivsi3",
                op::SDIV32 => "__divsi3",
                op::UREM32 => "__umodsi3",
                op::SREM32 => "__modsi3",
                op::SHL32 => "__ashlsi3",
                op::SRL32 => "__lshrsi3",
                _ => "__ashrsi3",
            };
            let (low, high) = (word(&o[0])?, word(&o[1])?);
            for (at, source) in [word(&o[2])?, word(&o[3])?].into_iter().enumerate() {
                copy_byte(&mut out, &format!("__sa+{}", 2 * at), &source.0);
                copy_byte(&mut out, &format!("__sa+{}", 2 * at + 1), &source.1);
            }
            if matches!(inst.opcode, op::SHL32 | op::SRL32 | op::SRA32) {
                copy_byte(&mut out, "__sb", reg(&o[4])?);
            } else {
                for (at, source) in [word(&o[4])?, word(&o[5])?].into_iter().enumerate() {
                    copy_byte(&mut out, &format!("__sb+{}", 2 * at), &source.0);
                    copy_byte(&mut out, &format!("__sb+{}", 2 * at + 1), &source.1);
                }
            }
            writeln!(out, "  jsr {routine}").unwrap();
            copy_byte(&mut out, &low.0, "__sa");
            copy_byte(&mut out, &low.1, "__sa+1");
            copy_byte(&mut out, &high.0, "__sa+2");
            copy_byte(&mut out, &high.1, "__sa+3");
        }
        op::COPY8 => copy_byte(&mut out, reg(&o[0])?, reg(&o[1])?),
        op::COPY16 => {
            let ((dl, dh), (sl, sh)) = (word(&o[0])?, word(&o[1])?);
            copy_byte(&mut out, &dl, &sl);
            copy_byte(&mut out, &dh, &sh);
        }
        // operands: the flags it defines, then a and b
        op::CMP8 => {
            writeln!(out, "  lda {}\n  cmp {}", reg(&o[1])?, reg(&o[2])?).unwrap();
        }
        // lda alo; cmp blo; lda ahi; sbc bhi: C is a >= b over the word, N the
        // sign of the difference.
        op::CMP16 => {
            let ((al, ah), (bl, bh)) = (word(&o[1])?, word(&o[2])?);
            writeln!(out, "  lda {al}\n  cmp {bl}\n  lda {ah}\n  sbc {bh}").unwrap();
        }
        // Z only if both bytes are equal: the first mismatch leaves Z clear.
        op::CMPZ16 => {
            *labels += 1;
            let ((al, ah), (bl, bh)) = (word(&o[1])?, word(&o[2])?);
            writeln!(out, "  lda {al}\n  cmp {bl}\n  bne _z{labels}\n  lda {ah}\n  cmp {bh}\n_z{labels}:").unwrap();
        }
        op::CMOV16 => {
            *labels += 1;
            let skip = format!("_cc{labels}");
            let ((dl, dh), (fl, fh), (tl, th)) = (word(&o[0])?, word(&o[1])?, word(&o[2])?);
            let branch = branch_on(imm(&o[3])? ^ 1)?;
            writeln!(
                out,
                "  php\n  lda {fl}\n  sta {dl}\n  lda {fh}\n  sta {dh}\n  plp\n  {branch} {skip}\n  lda {tl}\n  sta {dl}\n  lda {th}\n  sta {dh}\n{skip}:"
            )
            .unwrap();
        }
        // The flags are in P; PHP/PLP carry them over the loads, which set N
        // and Z. SETCC: d := 1, branch on cc over d := 0.
        // CMOV d, false, true: d := false, branch on !cc over d := true.
        op::SETCC8 | op::CMOV8 => {
            *labels += 1;
            let skip = format!("_cc{labels}");
            let d = reg(&o[0])?;
            let (branch, first, second) = if inst.opcode == op::SETCC8 {
                (branch_on(imm(&o[1])?)?, "#1".to_owned(), "#0".to_owned())
            } else {
                (branch_on(imm(&o[3])? ^ 1)?, reg(&o[1])?.to_owned(), reg(&o[2])?.to_owned())
            };
            writeln!(
                out,
                "  php\n  lda {first}\n  sta {d}\n  plp\n  {branch} {skip}\n  lda {second}\n  sta {d}\n{skip}:"
            )
            .unwrap();
        }
        // A conditional branch reaches 127 bytes: the opposite branch over
        // a `jmp`.
        //
        // DEVIATE: always the long form, no branch relaxation. Reason: the
        // spike does not know the distance. Exit: the shared branch relaxation.
        op::BEQ | op::BNE | op::BCC | op::BCS | op::BMI | op::BPL => {
            let opposite = match inst.opcode {
                op::BEQ => "bne",
                op::BNE => "beq",
                op::BCC => "bcs",
                op::BCS => "bcc",
                op::BMI => "bpl",
                _ => "bmi",
            };
            *labels += 1;
            match &o[0] {
                Operand::Label(l) => writeln!(out, "  {opposite} _br{labels}\n  jmp {l}\n_br{labels}:").unwrap(),
                other => return Err(format!("a branch takes a label, not {other:?}")),
            }
        }
        op::BRA => match &o[0] {
            Operand::Label(l) => writeln!(out, "  bra {l}").unwrap(),
            other => return Err(format!("bra takes a label, not {other:?}")),
        },
        op::JMP | op::JSR => {
            let mnemonic = if inst.opcode == op::JMP { "jmp" } else { "jsr" };
            match &o[0] {
                Operand::Label(l) => writeln!(out, "  {mnemonic} {l}").unwrap(),
                other => return Err(format!("{mnemonic} takes a label, not {other:?}")),
            }
        }
        op::RET => out.push_str("  rts\n"),
        spill::SAVE_CARRY => writeln!(out, "  lda #0\n  rol a\n  sta {}", reg(&o[0])?).unwrap(),
        spill::LOAD_CARRY => writeln!(out, "  lda {}\n  lsr a", reg(&o[0])?).unwrap(),
        spill::SAVE_FLAGS => writeln!(out, "  php\n  pla\n  sta {}", reg(&o[0])?).unwrap(),
        spill::LOAD_FLAGS => writeln!(out, "  lda {}\n  pha\n  plp", reg(&o[0])?).unwrap(),
        other => return Err(format!("opcode {other} has no expansion")),
    }
    Ok(out)
}

/// A function: its label, then each instruction; a `Label` pseudo is `None` and
/// prints a label line.
pub fn function(
    name: &str,
    body: &[Result<MInst, &str>],
) -> Result<String, String> {
    let mut out = format!(".export {name}\n{name}:\n");
    let mut labels = 0;
    for item in body {
        match item {
            Ok(inst) => out.push_str(&expand(inst, &mut labels)?),
            Err(label) => writeln!(out, "{label}:").unwrap(),
        }
    }
    Ok(out)
}

/// `bra` reaches 127 bytes. An instruction is at most 3 bytes, so a `bra` whose
/// target is within 40 instruction lines stays and any other becomes `jmp`.
///
/// DEVIATE: line counting, not sizes. Reason: the spike has no branch
/// relaxation. Exit: the shared one.
pub fn relax_bra(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let is_instruction = |l: &str| l.starts_with("  ") && !l.trim_start().starts_with('.');
    let mut out = String::new();
    for (at, line) in lines.iter().enumerate() {
        let Some(target) = line.strip_prefix("  bra ") else {
            writeln!(out, "{line}").unwrap();
            continue;
        };
        let label = format!("{target}:");
        let near = lines
            .iter()
            .position(|l| *l == label)
            .is_some_and(
                |to| {
                    let (from, until) = if to > at { (at + 1, to) } else { (to, at) };
                    lines[from..until].iter().filter(|l| is_instruction(l)).count() <= 40
                },
            );
        writeln!(out, "  {} {target}", if near { "bra" } else { "jmp" }).unwrap();
    }
    out
}

/// The equates for the imaginary registers on `platform`: `rc0 = $80` and so
/// on.
pub fn equates(platform: &Platform) -> String {
    (0..usize::from(platform.zero_page))
        .map(|n| format!("{} = ${:02X}\n", imaginary::name(n), platform.imag_base as usize + n))
        .collect()
}
