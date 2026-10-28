//! `Inst` in the syntax LLVM's assembler reads and its `AArch64InstPrinter`
//! writes: the canonical spelling of each class, not the preferred alias
//! (`subs xzr, x0, x1` and not `cmp x0, x1`).

use std::fmt;

use super::inst::*;

/// A general register `n` of `width` bits; number 31 is the stack pointer where
/// `sp`, else the zero register.
fn gpr(
    register: Gpr,
    width: Width,
    sp: bool,
) -> String {
    match (register.0, width, sp) {
        (31, Width::X, true) => "sp".into(),
        (31, Width::W, true) => "wsp".into(),
        (31, Width::X, false) => "xzr".into(),
        (31, Width::W, false) => "wzr".into(),
        (n, Width::X, _) => format!("x{n}"),
        (n, Width::W, _) => format!("w{n}"),
    }
}

fn fpr(
    register: Vreg,
    size: FpSize,
) -> String {
    format!("{}{}", size.letter(), register.0)
}

fn sized(
    register: Vreg,
    bytes: u8,
) -> String {
    let letter = match bytes {
        1 => 'b',
        2 => 'h',
        4 => 's',
        8 => 'd',
        _ => 'q',
    };
    format!("{letter}{}", register.0)
}

fn shift_name(shift: Shift) -> &'static str {
    ["lsl", "lsr", "asr", "ror"][shift as usize]
}

fn extend_name(extend: Extend) -> &'static str {
    ["uxtb", "uxth", "uxtw", "uxtx", "sxtb", "sxth", "sxtw", "sxtx"][extend as usize]
}

/// A branch or address target, with the relocation specifier `at` asks for
/// (`@PAGE`, `@PAGEOFF`, `@GOTPAGE`...).
fn target(
    target: &Target,
    specifier: &str,
) -> String {
    match target {
        Target::Offset(bytes) => format!("#{bytes}"),
        Target::Label(name) => name.clone(),
        Target::Symbol { name, addend: 0 } => format!("{name}{specifier}"),
        Target::Symbol { name, addend } => format!("{name}{specifier}{addend:+}"),
    }
}

/// Whether a load or store at `imm` takes the scaled unsigned-offset form.
pub(super) fn scaled(
    imm: i64,
    bytes: u8,
) -> bool {
    imm >= 0 && imm % i64::from(bytes) == 0 && imm / i64::from(bytes) < 4096
}

fn memory_bytes(mem: &Memory) -> u8 {
    match mem {
        Memory::Int { bytes, .. } => *bytes,
        Memory::Fp { bytes, .. } => *bytes,
    }
}

fn memory_register(mem: &Memory) -> String {
    match mem {
        Memory::Int { width, reg, .. } => gpr(*reg, *width, false),
        Memory::Fp { bytes, reg } => sized(*reg, *bytes),
    }
}

fn memory_mnemonic(
    mem: &Memory,
    load: bool,
    unscaled: bool,
) -> String {
    let stem = match (load, unscaled) {
        (true, false) => "ldr",
        (false, false) => "str",
        (true, true) => "ldur",
        (false, true) => "stur",
    };
    match mem {
        Memory::Int { bytes, sign, .. } => {
            let signed = if *sign && load { "s" } else { "" };
            let size = match bytes {
                1 => "b",
                2 => "h",
                4 if *sign => "w",
                _ => "",
            };
            format!("{stem}{signed}{size}")
        }
        Memory::Fp { .. } => stem.to_owned(),
    }
}

fn address(
    addr: &Addr,
    bytes: u8,
) -> String {
    let base = |register: &Gpr| gpr(*register, Width::X, true);
    match addr {
        Addr::Offset { base: b, imm: 0 } => format!("[{}]", base(b)),
        Addr::Offset { base: b, imm } => format!("[{}, #{imm}]", base(b)),
        Addr::Pre { base: b, imm } => format!("[{}, #{imm}]!", base(b)),
        Addr::Post { base: b, imm } => format!("[{}], #{imm}", base(b)),
        Addr::Register { base: b, index, extend, shifted } => {
            let width = if matches!(extend, Extend::Uxtx | Extend::Sxtx) { Width::X } else { Width::W };
            let amount = if *shifted { format!(" #{}", bytes.trailing_zeros()) } else { String::new() };
            // An unshifted `lsl` is not written.
            let extension = match (extend, shifted) {
                (Extend::Uxtx, false) => String::new(),
                (Extend::Uxtx, true) => format!(", lsl{amount}"),
                _ => format!(", {}{amount}", extend_name(*extend)),
            };
            format!("[{}, {}{extension}]", base(b), gpr(*index, width, false))
        }
        Addr::Slot { slot, offset } => format!("[slot{slot}, #{offset}]"),
        Addr::PageOffset { base: b, target: to, got } => {
            format!("[{}, {}]", base(b), target(to, if *got { "@GOTPAGEOFF" } else { "@PAGEOFF" }))
        }
    }
}

impl fmt::Display for Inst {
    fn fmt(
        &self,
        out: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Inst::AddSubImm { op, width, rd, rn, imm12, shift12 } => {
                let shift = if *shift12 { ", lsl #12" } else { "" };
                write!(
                    out,
                    "{} {}, {}, #{imm12}{shift}",
                    add_sub_name(*op),
                    gpr(*rd, *width, !op.set_flags),
                    gpr(*rn, *width, true)
                )
            }
            Inst::AddSubShifted { op, width, rd, rn, rm, shift, amount } => write!(
                out,
                "{} {}, {}, {}, {} #{amount}",
                add_sub_name(*op),
                gpr(*rd, *width, false),
                gpr(*rn, *width, false),
                gpr(*rm, *width, false),
                shift_name(*shift)
            ),
            Inst::AddSubExtended { op, width, rd, rn, rm, extend, amount } => {
                let wide = if matches!(extend, Extend::Uxtx | Extend::Sxtx) { Width::X } else { Width::W };
                write!(
                    out,
                    "{} {}, {}, {}, {} #{amount}",
                    add_sub_name(*op),
                    gpr(*rd, *width, !op.set_flags),
                    gpr(*rn, *width, true),
                    gpr(*rm, wide, false),
                    extend_name(*extend)
                )
            }
            Inst::AddPageOffset { rd, rn, target: to } => write!(
                out,
                "add {}, {}, {}",
                gpr(*rd, Width::X, true),
                gpr(*rn, Width::X, true),
                target(to, "@PAGEOFF")
            ),
            Inst::LogicImm { op, width, rd, rn, imm } => {
                let value = if *width == Width::W { u64::from(*imm as u32) } else { *imm };
                write!(
                    out,
                    "{} {}, {}, #{value:#x}",
                    logic_name(*op, false),
                    gpr(*rd, *width, *op != Logic::Ands),
                    gpr(*rn, *width, false)
                )
            }
            Inst::LogicShifted { op, invert, width, rd, rn, rm, shift, amount } => write!(
                out,
                "{} {}, {}, {}, {} #{amount}",
                logic_name(*op, *invert),
                gpr(*rd, *width, false),
                gpr(*rn, *width, false),
                gpr(*rm, *width, false),
                shift_name(*shift)
            ),
            Inst::MoveWide { op, width, rd, imm16, lsl } => {
                let name = match op {
                    Wide::Not => "movn",
                    Wide::Zero => "movz",
                    Wide::Keep => "movk",
                };
                write!(out, "{name} {}, #{imm16}, lsl #{lsl}", gpr(*rd, *width, false))
            }
            Inst::BitfieldMove { op, width, rd, rn, immr, imms } => {
                let name = match op {
                    Bitfield::Sbfm => "sbfm",
                    Bitfield::Bfm => "bfm",
                    Bitfield::Ubfm => "ubfm",
                };
                write!(out, "{name} {}, {}, #{immr}, #{imms}", gpr(*rd, *width, false), gpr(*rn, *width, false))
            }
            Inst::Extract { width, rd, rn, rm, lsb } => write!(
                out,
                "extr {}, {}, {}, #{lsb}",
                gpr(*rd, *width, false),
                gpr(*rn, *width, false),
                gpr(*rm, *width, false)
            ),
            Inst::Divide { op, width, rd, rn, rm } => {
                let name = ["udiv", "sdiv", "lsl", "lsr", "asr", "ror"][*op as usize];
                write!(
                    out,
                    "{name} {}, {}, {}",
                    gpr(*rd, *width, false),
                    gpr(*rn, *width, false),
                    gpr(*rm, *width, false)
                )
            }
            Inst::OneSource { op, width, rd, rn } => {
                let name = ["rbit", "rev16", "rev", "clz", "cls"][*op as usize];
                write!(out, "{name} {}, {}", gpr(*rd, *width, false), gpr(*rn, *width, false))
            }
            Inst::ThreeSource { op, width, rd, rn, rm, ra } => {
                let name = ["madd", "msub", "smaddl", "smsubl", "smulh", "umaddl", "umsubl", "umulh"][*op as usize];
                let (destination, source) = match op {
                    Mul3::Madd | Mul3::Msub => (*width, *width),
                    Mul3::Smaddl | Mul3::Smsubl | Mul3::Umaddl | Mul3::Umsubl => (Width::X, Width::W),
                    Mul3::Smulh | Mul3::Umulh => (Width::X, Width::X),
                };
                write!(
                    out,
                    "{name} {}, {}, {}",
                    gpr(*rd, destination, false),
                    gpr(*rn, source, false),
                    gpr(*rm, source, false)
                )?;
                if !matches!(op, Mul3::Smulh | Mul3::Umulh) {
                    write!(out, ", {}", gpr(*ra, destination, false))?;
                }
                Ok(())
            }
            Inst::CondSelect { op, width, rd, rn, rm, cond } => {
                let name = ["csel", "csinc", "csinv", "csneg"][*op as usize];
                write!(
                    out,
                    "{name} {}, {}, {}, {cond}",
                    gpr(*rd, *width, false),
                    gpr(*rn, *width, false),
                    gpr(*rm, *width, false)
                )
            }
            Inst::CondCompare { negate, width, rn, rm_or_imm, nzcv, cond } => {
                let name = if *negate { "ccmn" } else { "ccmp" };
                let second = match rm_or_imm {
                    CcOperand::Reg(rm) => gpr(*rm, *width, false),
                    CcOperand::Imm5(imm) => format!("#{imm}"),
                };
                write!(out, "{name} {}, {second}, #{nzcv}, {cond}", gpr(*rn, *width, false))
            }
            Inst::Branch { link, target: to } => write!(out, "{} {}", if *link { "bl" } else { "b" }, target(to, "")),
            Inst::BranchCond { cond, target: to } => write!(out, "b.{cond} {}", target(to, "")),
            Inst::CompareBranch { nonzero, width, rt, target: to } => {
                write!(out, "{} {}, {}", if *nonzero { "cbnz" } else { "cbz" }, gpr(*rt, *width, false), target(to, ""))
            }
            Inst::TestBranch { nonzero, rt, bit, target: to } => {
                let width = if *bit >= 32 { Width::X } else { Width::W };
                write!(
                    out,
                    "{} {}, #{bit}, {}",
                    if *nonzero { "tbnz" } else { "tbz" },
                    gpr(*rt, width, false),
                    target(to, "")
                )
            }
            Inst::BranchReg { link, rn } => {
                write!(out, "{} {}", if *link { "blr" } else { "br" }, gpr(*rn, Width::X, false))
            }
            Inst::Ret { rn } => write!(out, "ret {}", gpr(*rn, Width::X, false)),
            Inst::Load { mem, addr, .. } => memory_text(out, mem, addr, true),
            Inst::Store { mem, addr, .. } => memory_text(out, mem, addr, false),
            Inst::LoadPair { first, second, addr } => pair_text(out, first, second, addr, true),
            Inst::StorePair { first, second, addr } => pair_text(out, first, second, addr, false),
            Inst::Adr { page, rd, target: to, got } => {
                write!(
                    out,
                    "{} {}, {}",
                    if *page { "adrp" } else { "adr" },
                    gpr(*rd, Width::X, false),
                    target(
                        to,
                        match (page, got) {
                            (true, true) => "@GOTPAGE",
                            (true, false) => "@PAGE",
                            _ => "",
                        }
                    )
                )
            }
            Inst::FpUnary { op, size, rd, rn } => {
                let name = match op {
                    FpUnary::Mov => "fmov",
                    FpUnary::Abs => "fabs",
                    FpUnary::Neg => "fneg",
                    FpUnary::Sqrt => "fsqrt",
                    FpUnary::Rint(Rounding::Nearest) => "frintn",
                    FpUnary::Rint(Rounding::Up) => "frintp",
                    FpUnary::Rint(Rounding::Down) => "frintm",
                    FpUnary::Rint(Rounding::Zero) => "frintz",
                    FpUnary::Rint(Rounding::Away) => "frinta",
                    FpUnary::Rint(Rounding::Exact) => "frintx",
                    FpUnary::Rint(Rounding::Current) => "frinti",
                };
                write!(out, "{name} {}, {}", fpr(*rd, *size), fpr(*rn, *size))
            }
            Inst::VectorMov { rd, rn } => write!(out, "mov v{}.16b, v{}.16b", rd.0, rn.0),
            Inst::FpBinary { op, size, rd, rn, rm } => {
                let name = ["fmul", "fdiv", "fadd", "fsub", "fmax", "fmin", "fmaxnm", "fminnm", "fnmul"][*op as usize];
                write!(out, "{name} {}, {}, {}", fpr(*rd, *size), fpr(*rn, *size), fpr(*rm, *size))
            }
            Inst::FpFused { op, size, rd, rn, rm, ra } => {
                let name = ["fmadd", "fmsub", "fnmadd", "fnmsub"][*op as usize];
                write!(out, "{name} {}, {}, {}, {}", fpr(*rd, *size), fpr(*rn, *size), fpr(*rm, *size), fpr(*ra, *size))
            }
            Inst::FpCompare { signalling, size, rn, rm } => {
                let name = if *signalling { "fcmpe" } else { "fcmp" };
                match rm {
                    Some(rm) => write!(out, "{name} {}, {}", fpr(*rn, *size), fpr(*rm, *size)),
                    None => write!(out, "{name} {}, #0.0", fpr(*rn, *size)),
                }
            }
            Inst::FpSelect { size, rd, rn, rm, cond } => {
                write!(out, "fcsel {}, {}, {}, {cond}", fpr(*rd, *size), fpr(*rn, *size), fpr(*rm, *size))
            }
            Inst::VectorFill { rd, fill } => match fill {
                Fill::Zero => write!(out, "movi.2d v{}, #0", rd.0),
                Fill::Byte(byte) => write!(out, "movi.16b v{}, #{byte}", rd.0),
                Fill::Gpr(source) => write!(out, "dup.16b v{}, {}", rd.0, gpr(*source, Width::W, false)),
            },
            Inst::FpMoveImm { size, rd, imm8 } => {
                write!(out, "fmov {}, #{}", fpr(*rd, *size), fp_immediate_value(*imm8))
            }
            Inst::FpToFp { to, from, rd, rn } => write!(out, "fcvt {}, {}", fpr(*rd, *to), fpr(*rn, *from)),
            Inst::FpInt { op, width, size, gpr: g, fpr: f } => {
                let name = match op {
                    FpConvert::ToFp { signed: true } => "scvtf",
                    FpConvert::ToFp { signed: false } => "ucvtf",
                    FpConvert::ToIntZero { signed: true } => "fcvtzs",
                    FpConvert::ToIntZero { signed: false } => "fcvtzu",
                    FpConvert::Bits { .. } => "fmov",
                };
                let to_fp = matches!(op, FpConvert::ToFp { .. } | FpConvert::Bits { to_fp: true });
                if to_fp {
                    write!(out, "{name} {}, {}", fpr(*f, *size), gpr(*g, *width, false))
                } else {
                    write!(out, "{name} {}, {}", gpr(*g, *width, false), fpr(*f, *size))
                }
            }
            Inst::FpFixed { op, width, size, gpr: g, fpr: f, fbits } => {
                let name = match op {
                    FpConvert::ToFp { signed: true } => "scvtf",
                    FpConvert::ToFp { signed: false } => "ucvtf",
                    FpConvert::ToIntZero { signed: true } => "fcvtzs",
                    _ => "fcvtzu",
                };
                if matches!(op, FpConvert::ToFp { .. }) {
                    write!(out, "{name} {}, {}, #{fbits}", fpr(*f, *size), gpr(*g, *width, false))
                } else {
                    write!(out, "{name} {}, {}, #{fbits}", gpr(*g, *width, false), fpr(*f, *size))
                }
            }
            Inst::Svc { imm16 } => write!(out, "svc #{imm16}"),
            Inst::Atomic { op, size, acquire, release, rs, rt, rn } => {
                let width = if *size == 3 { Width::X } else { Width::W };
                let suffix = ["b", "h", "", ""][usize::from(*size).min(3)];
                let order = format!("{}{}", if *acquire { "a" } else { "" }, if *release { "l" } else { "" });
                let base = gpr(*rn, Width::X, true);
                match op {
                    Atomic::Ldar => write!(out, "ldar{suffix} {}, [{base}]", gpr(*rt, width, false)),
                    Atomic::Ldapr => write!(out, "ldapr{suffix} {}, [{base}]", gpr(*rt, width, false)),
                    Atomic::Stlr => write!(out, "stlr{suffix} {}, [{base}]", gpr(*rt, width, false)),
                    Atomic::Cas => {
                        write!(
                            out,
                            "cas{order}{suffix} {}, {}, [{base}]",
                            gpr(*rs, width, false),
                            gpr(*rt, width, false)
                        )
                    }
                    other => {
                        let name = match other {
                            Atomic::Swp => "swp",
                            Atomic::Ldadd => "ldadd",
                            Atomic::Ldclr => "ldclr",
                            Atomic::Ldeor => "ldeor",
                            Atomic::Ldset => "ldset",
                            Atomic::Ldsmax => "ldsmax",
                            Atomic::Ldsmin => "ldsmin",
                            _ if *other == Atomic::Ldumax => "ldumax",
                            _ => "ldumin",
                        };
                        write!(
                            out,
                            "{name}{order}{suffix} {}, {}, [{base}]",
                            gpr(*rs, width, false),
                            gpr(*rt, width, false)
                        )
                    }
                }
            }
            Inst::Dmb { barrier } => write!(
                out,
                "dmb {}",
                match barrier {
                    Barrier::Ish => "ish",
                    Barrier::IshLd => "ishld",
                    Barrier::IshSt => "ishst",
                }
            ),
            Inst::Brk { imm16 } => write!(out, "brk #{imm16}"),
            Inst::Nop => out.write_str("nop"),
            Inst::Prefetch { op, base } => write!(out, "prfm #{op}, [{}]", gpr(*base, Width::X, true)),
        }
    }
}

fn add_sub_name(op: AddSub) -> &'static str {
    match (op.subtract, op.set_flags) {
        (false, false) => "add",
        (false, true) => "adds",
        (true, false) => "sub",
        (true, true) => "subs",
    }
}

fn logic_name(
    op: Logic,
    invert: bool,
) -> &'static str {
    match (op, invert) {
        (Logic::And, false) => "and",
        (Logic::And, true) => "bic",
        (Logic::Orr, false) => "orr",
        (Logic::Orr, true) => "orn",
        (Logic::Eor, false) => "eor",
        (Logic::Eor, true) => "eon",
        (Logic::Ands, false) => "ands",
        (Logic::Ands, true) => "bics",
    }
}

/// The value an 8-bit floating-point immediate code stands for, spelled as the
/// assembler reads it.
fn fp_immediate_value(imm8: u8) -> String {
    let sign = if imm8 & 0x80 != 0 { -1.0 } else { 1.0 };
    let exponent = i32::from(((imm8 >> 4) & 0x7) ^ 0x4) - 3;
    let mantissa = 1.0 + f64::from(imm8 & 0xF) / 16.0;
    format!("{:?}", sign * mantissa * f64::powi(2.0, exponent))
}

fn memory_text(
    out: &mut fmt::Formatter<'_>,
    mem: &Memory,
    addr: &Addr,
    load: bool,
) -> fmt::Result {
    let bytes = memory_bytes(mem);
    let unscaled = matches!(addr, Addr::Offset { imm, .. } if !scaled(*imm, bytes));
    write!(out, "{} {}, {}", memory_mnemonic(mem, load, unscaled), memory_register(mem), address(addr, bytes))
}

fn pair_text(
    out: &mut fmt::Formatter<'_>,
    first: &Memory,
    second: &Memory,
    addr: &Addr,
    load: bool,
) -> fmt::Result {
    write!(
        out,
        "{} {}, {}, {}",
        if load { "ldp" } else { "stp" },
        memory_register(first),
        memory_register(second),
        address(addr, memory_bytes(first))
    )
}
