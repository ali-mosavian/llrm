//! Integer arithmetic, logic, moves, bit fields, multiplies, selects and
//! conditional compares.

use super::*;
use crate::mc::inst::{AddSub, Bitfield, CcOperand, Cond, Div, Extend, Logic, Mul3, Select, Unary, Wide};

pub(super) const EXTENDS: [Extend; 8] =
    [Extend::Uxtb, Extend::Uxth, Extend::Uxtw, Extend::Uxtx, Extend::Sxtb, Extend::Sxth, Extend::Sxtw, Extend::Sxtx];

pub(super) const CONDS: [Cond; 16] = [
    Cond::Eq,
    Cond::Ne,
    Cond::Hs,
    Cond::Lo,
    Cond::Mi,
    Cond::Pl,
    Cond::Vs,
    Cond::Vc,
    Cond::Hi,
    Cond::Ls,
    Cond::Ge,
    Cond::Lt,
    Cond::Gt,
    Cond::Le,
    Cond::Al,
    Cond::Nv,
];

/// The logical mnemonics: the operation, whether the second operand is
/// inverted.
const LOGIC: [(Logic, bool, &str); 8] = [
    (Logic::And, false, "AND"),
    (Logic::And, true, "BIC"),
    (Logic::Orr, false, "ORR"),
    (Logic::Orr, true, "ORN"),
    (Logic::Eor, false, "EOR"),
    (Logic::Eor, true, "EON"),
    (Logic::Ands, false, "ANDS"),
    (Logic::Ands, true, "BICS"),
];

const WIDE: [(Wide, &str); 3] = [(Wide::Not, "MOVN"), (Wide::Zero, "MOVZ"), (Wide::Keep, "MOVK")];
const BITFIELD: [(Bitfield, &str); 3] = [(Bitfield::Sbfm, "SBFM"), (Bitfield::Bfm, "BFM"), (Bitfield::Ubfm, "UBFM")];
const DIV: [(Div, &str); 6] = [
    (Div::Udiv, "UDIV"),
    (Div::Sdiv, "SDIV"),
    (Div::Lslv, "LSLV"),
    (Div::Lsrv, "LSRV"),
    (Div::Asrv, "ASRV"),
    (Div::Rorv, "RORV"),
];
const UNARY: [(Unary, &str); 5] =
    [(Unary::Rbit, "RBIT"), (Unary::Rev16, "REV16"), (Unary::Rev, "REV"), (Unary::Clz, "CLZ"), (Unary::Cls, "CLS")];
const MUL3: [(Mul3, &str); 8] = [
    (Mul3::Madd, "MADD"),
    (Mul3::Msub, "MSUB"),
    (Mul3::Smaddl, "SMADDL"),
    (Mul3::Smsubl, "SMSUBL"),
    (Mul3::Smulh, "SMULH"),
    (Mul3::Umaddl, "UMADDL"),
    (Mul3::Umsubl, "UMSUBL"),
    (Mul3::Umulh, "UMULH"),
];
const SELECT: [(Select, &str); 4] =
    [(Select::Csel, "CSEL"), (Select::Csinc, "CSINC"), (Select::Csinv, "CSINV"), (Select::Csneg, "CSNEG")];

fn named<T: Copy + PartialEq>(
    table: &[(T, &'static str)],
    one: T,
) -> &'static str {
    table.iter().find(|(each, _)| *each == one).expect("a listed mnemonic").1
}

fn named_logic(
    op: Logic,
    invert: bool,
) -> &'static str {
    LOGIC.iter().find(|(each, inverted, _)| *each == op && *inverted == invert).expect("a logical mnemonic").2
}

fn by_name<T: Copy>(
    table: &[(T, &'static str)],
    mnemonic: &str,
) -> Option<T> {
    table.iter().find(|(_, each)| *each == mnemonic).map(|(one, _)| *one)
}

pub(super) fn add_sub_name(
    op: AddSub,
    width: Width,
    form: &str,
) -> String {
    format!(
        "{}{}{}{form}",
        if op.subtract { "SUB" } else { "ADD" },
        if op.set_flags { "S" } else { "" },
        width_letter(width)
    )
}

/// The multiplies that read 32-bit registers and write a 64-bit one.
fn long_multiply(op: Mul3) -> bool {
    matches!(op, Mul3::Smaddl | Mul3::Smsubl | Mul3::Umaddl | Mul3::Umsubl)
}

pub(super) fn to(
    cx: &mut To,
    inst: &Inst,
) -> Result<Option<Semantics>, Unmapped> {
    let g = gpr_loc;
    Ok(Some(match inst {
        Inst::AddSubImm { op, width, rd, rn, imm12, shift12 } => semantics(
            Operation::Binary,
            add_sub_name(*op, *width, "ri"),
            vec![g(*rd, *width)],
            vec![g(*rn, *width), imm(i64::from(*imm12)), imm(i64::from(*shift12))],
            None,
        ),
        Inst::AddSubShifted { op, width, rd, rn, rm, shift, amount } => semantics(
            Operation::Binary,
            add_sub_name(*op, *width, "rs"),
            vec![g(*rd, *width)],
            vec![g(*rn, *width), g(*rm, *width), imm(position(&SHIFTS, *shift)), imm(i64::from(*amount))],
            None,
        ),
        Inst::AddSubExtended { op, width, rd, rn, rm, extend, amount } => semantics(
            Operation::Binary,
            add_sub_name(*op, *width, "rx"),
            vec![g(*rd, *width)],
            vec![g(*rn, *width), g(*rm, *width), imm(position(&EXTENDS, *extend)), imm(i64::from(*amount))],
            None,
        ),
        Inst::AddPageOffset { rd, rn, target } => semantics(
            Operation::Binary,
            "ADDXri.lo12",
            vec![g(*rd, Width::X)],
            vec![g(*rn, Width::X), Loc::Address(cx.address(target)?)],
            None,
        ),
        Inst::LogicImm { op, width, rd, rn, imm: value } => semantics(
            Operation::Binary,
            format!("{}{}ri", named_logic(*op, false), width_letter(*width)),
            vec![g(*rd, *width)],
            vec![g(*rn, *width), imm(*value as i64)],
            None,
        ),
        Inst::LogicShifted { op, invert, width, rd, rn, rm, shift, amount } => semantics(
            Operation::Binary,
            format!("{}{}rs", named_logic(*op, *invert), width_letter(*width)),
            vec![g(*rd, *width)],
            vec![g(*rn, *width), g(*rm, *width), imm(position(&SHIFTS, *shift)), imm(i64::from(*amount))],
            None,
        ),
        Inst::MoveWide { op, width, rd, imm16, lsl } => {
            let keep = *op == Wide::Keep;
            let mut sources = if keep { vec![g(*rd, *width)] } else { vec![] };
            sources.extend([imm(i64::from(*imm16)), imm(i64::from(*lsl))]);
            semantics(
                Operation::Move,
                format!("{}{}i", named(&WIDE, *op), width_letter(*width)),
                vec![g(*rd, *width)],
                sources,
                None,
            )
        }
        Inst::BitfieldMove { op, width, rd, rn, immr, imms } => {
            let mut sources = if *op == Bitfield::Bfm { vec![g(*rd, *width)] } else { vec![] };
            sources.extend([g(*rn, *width), imm(i64::from(*immr)), imm(i64::from(*imms))]);
            semantics(
                Operation::BitField,
                format!("{}{}ri", named(&BITFIELD, *op), width_letter(*width)),
                vec![g(*rd, *width)],
                sources,
                None,
            )
        }
        Inst::Extract { width, rd, rn, rm, lsb } => semantics(
            Operation::Funnel,
            format!("EXTR{}rri", width_letter(*width)),
            vec![g(*rd, *width)],
            vec![g(*rn, *width), g(*rm, *width), imm(i64::from(*lsb))],
            None,
        ),
        Inst::Divide { op, width, rd, rn, rm } => semantics(
            if matches!(op, Div::Udiv | Div::Sdiv) { Operation::Divide } else { Operation::Binary },
            format!("{}{}r", named(&DIV, *op), width_letter(*width)),
            vec![g(*rd, *width)],
            vec![g(*rn, *width), g(*rm, *width)],
            None,
        ),
        Inst::OneSource { op, width, rd, rn } => semantics(
            Operation::Unary,
            format!("{}{}r", named(&UNARY, *op), width_letter(*width)),
            vec![g(*rd, *width)],
            vec![g(*rn, *width)],
            None,
        ),
        Inst::ThreeSource { op, width, rd, rn, rm, ra } => {
            let narrow = if long_multiply(*op) { Width::W } else { *width };
            // The high multiplies ignore `ra`, which is `xzr` when selected.
            let sources = vec![g(*rn, narrow), g(*rm, narrow), g(*ra, *width)];
            semantics(
                if matches!(op, Mul3::Smulh | Mul3::Umulh) { Operation::Multiply } else { Operation::MultiplyAdd },
                format!("{}{}rrr", named(&MUL3, *op), width_letter(*width)),
                vec![g(*rd, *width)],
                sources,
                None,
            )
        }
        Inst::CondSelect { op, width, rd, rn, rm, cond } => semantics(
            Operation::Select,
            format!("{}{}r", named(&SELECT, *op), width_letter(*width)),
            vec![g(*rd, *width)],
            vec![g(*rn, *width), g(*rm, *width), imm(position(&CONDS, *cond))],
            None,
        ),
        Inst::AddCarry { op, width, rd, rn, rm } => semantics(
            Operation::Binary,
            format!(
                "{}{}{}r",
                if op.subtract { "SBC" } else { "ADC" },
                if op.set_flags { "S" } else { "" },
                width_letter(*width)
            ),
            vec![g(*rd, *width)],
            vec![g(*rn, *width), g(*rm, *width)],
            None,
        ),
        Inst::CondCompare { negate, width, rn, rm_or_imm, nzcv, cond } => {
            let (second, form) = match rm_or_imm {
                CcOperand::Reg(rm) => (g(*rm, *width), 'r'),
                CcOperand::Imm5(value) => (imm(i64::from(*value)), 'i'),
            };
            semantics(
                Operation::ConditionalCompare,
                format!("{}{}{form}", if *negate { "CCMN" } else { "CCMP" }, width_letter(*width)),
                vec![],
                vec![g(*rn, *width), second, imm(i64::from(*nzcv)), imm(position(&CONDS, *cond))],
                None,
            )
        }
        _ => return Ok(None),
    }))
}

pub(super) fn back(
    cx: &Back,
    what: &Semantics,
) -> Option<Inst> {
    let name = what.name.as_deref()?;
    let (dest, s) = (what.dests.first(), &what.sources);
    if name == "ADDXri.lo12" {
        return Some(Inst::AddPageOffset { rd: gpr_of(dest?), rn: gpr_of(&s[0]), target: cx.target_of(&s[1]) });
    }
    if name.contains('.') {
        return None;
    }
    let (mnemonic, letter, form) = split(name);
    let width = width_named(letter);
    let rd = || dest.map(gpr_of);
    let rn = || s.first().filter(|loc| matches!(loc, Loc::Reg(_) | Loc::Held(_))).map(gpr_of);
    let reg = |at: usize| gpr_of(&s[at]);
    let small = |at: usize| value_of(&s[at]) as u8;
    if matches!(mnemonic, "ADD" | "ADDS" | "SUB" | "SUBS") {
        let op = AddSub { subtract: mnemonic.starts_with("SUB"), set_flags: mnemonic.ends_with('S') };
        let (rd, rn) = (rd()?, rn()?);
        return Some(match form {
            "ri" => Inst::AddSubImm { op, width, rd, rn, imm12: value_of(&s[1]) as u16, shift12: value_of(&s[2]) != 0 },
            "rs" => {
                Inst::AddSubShifted { op, width, rd, rn, rm: reg(1), shift: member(&SHIFTS, &s[2]), amount: small(3) }
            }
            "rx" => Inst::AddSubExtended {
                op,
                width,
                rd,
                rn,
                rm: reg(1),
                extend: member(&EXTENDS, &s[2]),
                amount: small(3),
            },
            _ => return None,
        });
    }
    if let Some(&(op, invert, _)) = LOGIC.iter().find(|(_, _, each)| *each == mnemonic) {
        let (rd, rn) = (rd()?, rn()?);
        return Some(match form {
            "ri" => Inst::LogicImm { op, width, rd, rn, imm: value_of(&s[1]) as u64 },
            "rs" => Inst::LogicShifted {
                op,
                invert,
                width,
                rd,
                rn,
                rm: reg(1),
                shift: member(&SHIFTS, &s[2]),
                amount: small(3),
            },
            _ => return None,
        });
    }
    if let Some(op) = by_name(&WIDE, mnemonic) {
        let at = usize::from(op == Wide::Keep);
        return Some(Inst::MoveWide { op, width, rd: rd()?, imm16: value_of(&s[at]) as u16, lsl: small(at + 1) });
    }
    if let Some(op) = by_name(&BITFIELD, mnemonic) {
        let at = usize::from(op == Bitfield::Bfm);
        return Some(Inst::BitfieldMove {
            op,
            width,
            rd: rd()?,
            rn: reg(at),
            immr: small(at + 1),
            imms: small(at + 2),
        });
    }
    if mnemonic == "EXTR" {
        return Some(Inst::Extract { width, rd: rd()?, rn: reg(0), rm: reg(1), lsb: small(2) });
    }
    if let Some(op) = by_name(&DIV, mnemonic) {
        return Some(Inst::Divide { op, width, rd: rd()?, rn: reg(0), rm: reg(1) });
    }
    if let Some(op) = by_name(&UNARY, mnemonic) {
        return Some(Inst::OneSource { op, width, rd: rd()?, rn: reg(0) });
    }
    if let Some(op) = by_name(&MUL3, mnemonic) {
        return Some(Inst::ThreeSource { op, width, rd: rd()?, rn: reg(0), rm: reg(1), ra: reg(2) });
    }
    if matches!(mnemonic, "ADC" | "ADCS" | "SBC" | "SBCS") {
        let op = AddSub { subtract: mnemonic.starts_with("SBC"), set_flags: mnemonic.ends_with('S') };
        return Some(Inst::AddCarry { op, width, rd: rd()?, rn: reg(0), rm: reg(1) });
    }
    if let Some(op) = by_name(&SELECT, mnemonic) {
        return Some(Inst::CondSelect { op, width, rd: rd()?, rn: reg(0), rm: reg(1), cond: member(&CONDS, &s[2]) });
    }
    if matches!(mnemonic, "CCMP" | "CCMN") {
        let rm_or_imm = if form == "r" { CcOperand::Reg(reg(1)) } else { CcOperand::Imm5(small(1)) };
        return Some(Inst::CondCompare {
            negate: mnemonic == "CCMN",
            width,
            rn: reg(0),
            rm_or_imm,
            nzcv: small(2),
            cond: member(&CONDS, &s[3]),
        });
    }
    None
}
