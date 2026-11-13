//! Scheduled machine nodes to an [`MFunc`]: the pin's `InstrEmitter`.
//!
//! # How it works
//! Each unit is walked in order. A `CopyFromReg` of a live-in virtual register
//! is that register's value; of a physical one it is a `COPY`. A `CopyToReg`
//! to a physical register is a `COPY` into it. A machine node becomes one
//! instruction through [`build`], which names its opcode (`ADDWrr`) and reads
//! its operands. `RET_ReallyLR` becomes `Op::Return` over the physical
//! registers it names.
//!
//! DEVIATE SD528: the float arithmetic, compare, select, conversion, move and
//! `LDR`/`STR` `ui` rows, and the integer `ADD`/`SUB`/logic, variable shift,
//! `SBFM`/`UBFM`, `MADD`, divide and one-source forms, `MOVi32imm`/`MOVi64imm`,
//! a frame address, the integer `LDR`/`STR` (`ui`, `UR`, `roX`, `roW`)
//! forms and the sub-register pseudos only (no `BFM`, no insert into a defined
//! register), with a memory operand of the volatile flag and the global or
//! frame object only (no offset); a machine
//! opcode with no row is a named error (`machine opcode X has no arm64
//! instruction yet`), a compile error. Rows added
//! to [`build`] each carry an encoder byte check. Exit: C2..Cn.

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{NodeData, SDValue, SelectionDAG};
use llrm_codegen::model::lir::SrcLoc;
use llrm_dagisel::generated::aarch64::aarch64_apple_m1::MACHINE_OPCODES;
use llrm_lir::registers::RegId;
use llrm_support::hash::HashMap;

use super::function::Selected;
use super::globals::Globals;
use crate::REGISTER_INFO;
use crate::mc::immediate::decode_logical;
use crate::mc::inst::{
    AddSub, Addr, Bitfield, CcOperand, Cond, Div, Extend, FpBinary, FpConvert, FpFused, FpSize, FpUnary, Gpr, Inst,
    Logic, MemObject, MemOperand, Memory, Mul3, Rounding, Select, Shift, Target, Unary, Vreg, Width,
};
use crate::mc::inst::{
    Arrangement, Atomic, Barrier, Fill, ImmShape, NarrowLane, VectorFma, VectorFpOp, VectorFpUnary, VectorLongOp,
    VectorOp, VectorReduce, VectorShift,
};
use crate::mc::visit::Reg;
use crate::mfunc::{Callee, INCOMING_ARGUMENTS, MBlock, MFunc, Op, OpList, PoolEntry, block_label};

/// The machine opcode's name.
fn opcode_name(code: u32) -> Option<&'static str> {
    // `TargetOpcode::COPY` is no output of a pattern, so the table does not
    // list it; the selector's own code makes it.
    const COPY: u32 = 20;
    MACHINE_OPCODES
        .iter()
        .find(|(v, _)| u32::from(*v) == code)
        .map(|(_, name)| *name)
        .or((code == COPY).then_some("COPY"))
        .or(match code {
            super::call::MEMCPY_Q => Some("LLRM_MEMCPY_Q"),
            super::call::MEMMOVE_Q => Some("LLRM_MEMMOVE_Q"),
            super::call::MEMSET_Q => Some("LLRM_MEMSET_Q"),
            super::call::SAVE_Q => Some("LLRM_SAVE_Q"),
            _ => None,
        })
}

/// The machine register number of an arm64 register id: `x5` and `w5` are 5.
fn gpr_of(id: RegId) -> Result<Gpr, String> {
    let name = REGISTER_INFO.name(id).ok_or("a register the arm64 file does not name")?;
    if matches!(name, "wzr" | "xzr" | "sp" | "wsp") {
        return Ok(Gpr::R31);
    }
    let number = name
        .strip_prefix(['x', 'w'])
        .and_then(|n| n.parse::<u32>().ok())
        .filter(|&n| n <= 30)
        .ok_or_else(|| format!("register {name} is no x/w register"))?;
    Ok(Gpr(number))
}

/// The float register an arm64 `s`/`d`/`q`/`v` register id is.
fn fpr_of(id: RegId) -> Result<Vreg, String> {
    let name = REGISTER_INFO.name(id).ok_or("a register the arm64 file does not name")?;
    name.strip_prefix(['s', 'd', 'h', 'q', 'v'])
        .and_then(|n| n.parse::<u32>().ok())
        .filter(|&n| n <= 31)
        .map(Vreg)
        .ok_or_else(|| format!("register {name} is no float register"))
}

fn is_fpr(id: RegId) -> bool {
    fpr_of(id).is_ok()
}

/// Either class of an arm64 register id.
fn reg_of(id: RegId) -> Result<Reg, String> {
    if is_fpr(id) { fpr_of(id).map(Reg::Fpr) } else { gpr_of(id).map(Reg::Gpr) }
}

/// An operand of a machine node as the instruction builders read it.
#[derive(Clone, Copy)]
enum Operand {
    Reg(Gpr),
    Imm(u64),
    /// A frame object, by the builder's frame index.
    Slot(u32),
    /// A float register.
    Fpr(Vreg),
    /// A global by id, and the offset into it.
    Symbol(u32, i64),
    /// A constant pool entry, by its place in the function's pool.
    Pool(usize),
    /// A routine the legalizer named (a libcall), by index into the DAG's
    /// symbol names.
    External(u32),
    /// A block by id, as a branch names it.
    Block(u32),
    /// An incoming argument at this offset into the caller's outgoing area.
    Fixed(i64),
    /// The flags a compare set: the instruction before this one wrote them.
    Flags,
    /// A jump table, by the builder's number.
    Table(u32),
}

/// `ADDWrr`, `SUBSXri` and the rest of the add/sub family: (subtract, set
/// flags, width, immediate form).
fn add_sub(name: &str) -> Option<(AddSub, Width, bool)> {
    let (op, rest) = if let Some(rest) = name.strip_prefix("ADDS") {
        (AddSub { subtract: false, set_flags: true }, rest)
    } else if let Some(rest) = name.strip_prefix("ADD") {
        (AddSub { subtract: false, set_flags: false }, rest)
    } else if let Some(rest) = name.strip_prefix("SUBS") {
        (AddSub { subtract: true, set_flags: true }, rest)
    } else {
        (AddSub { subtract: true, set_flags: false }, name.strip_prefix("SUB")?)
    };
    let (width, form) = match rest {
        "Wrr" => (Width::W, false),
        "Xrr" => (Width::X, false),
        "Wri" => (Width::W, true),
        "Xri" => (Width::X, true),
        "Wrs" => (Width::W, false),
        "Xrs" => (Width::X, false),
        _ => return None,
    };
    Some((op, width, form))
}

/// `ANDWri`, `ORNXrs`, `BICSWrs` and the rest of the logic family: (operation,
/// inverted operand, width, form).
fn logic(name: &str) -> Option<(Logic, bool, Width, &str)> {
    let (op, invert, rest) = [
        ("ANDS", Logic::Ands, false),
        ("AND", Logic::And, false),
        ("ORR", Logic::Orr, false),
        ("ORN", Logic::Orr, true),
        ("EOR", Logic::Eor, false),
        ("EON", Logic::Eor, true),
        ("BICS", Logic::Ands, true),
        ("BIC", Logic::And, true),
    ]
    .into_iter()
    .find_map(|(prefix, op, invert)| name.strip_prefix(prefix).map(|rest| (op, invert, rest)))?;
    let (width, form) = match rest {
        "Wrr" | "Wrs" | "Wri" => (Width::W, &rest[1..]),
        "Xrr" | "Xrs" | "Xri" => (Width::X, &rest[1..]),
        _ => return None,
    };
    Some((op, invert, width, form))
}

/// The shift a `rs` operand names: the type in bits 6.., the amount below.
fn shifter(shifter: u64) -> (Shift, u8) {
    let shift = match shifter >> 6 & 3 {
        0 => Shift::Lsl,
        1 => Shift::Lsr,
        2 => Shift::Asr,
        _ => Shift::Ror,
    };
    (shift, (shifter & 0x3f) as u8)
}

/// What a load or store opcode moves, how it addresses, and whether it stores.
struct MemoryForm {
    width: Width,
    bytes: u8,
    sign: bool,
    store: bool,
    form: AddressForm,
    /// The value is in a float register.
    fp: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum AddressForm {
    /// `ui`: an offset in units of the access size.
    Scaled,
    /// `UR`: an unscaled signed offset.
    Unscaled,
    /// `roX`: a base and an index register.
    Register,
    /// `roW`: a base and a 32-bit index register, extended.
    RegisterWord,
    /// `post`: `[base], #imm`; the node's first result is the new base.
    Post,
    /// `pre`: `[base, #imm]!`; the node's first result is the new base.
    Pre,
}

/// `LDRWui`, `STURXi`, `LDRSBWroX` and the rest of the integer load/store
/// family.
/// The pin's load-acquire, store-release, LSE read-modify-write and `cas`
/// machine opcodes (`LDARW`, `STLRX`, `LDADDALW`, `CASALX`, ...): what they
/// do, log2 of the access size, and the ordering bits.
fn atomic_form(name: &str) -> Option<(Atomic, u8, bool, bool)> {
    let size = match name.chars().last()? {
        'B' => 0,
        'H' => 1,
        'W' => 2,
        'X' => 3,
        _ => return None,
    };
    let stem = &name[..name.len() - 1];
    for (prefix, op) in [("LDAPR", Atomic::Ldapr), ("LDAR", Atomic::Ldar)] {
        if stem == prefix {
            return Some((op, size, true, false));
        }
    }
    if stem == "STLR" {
        return Some((Atomic::Stlr, size, false, true));
    }
    let table = [
        ("SWP", Atomic::Swp),
        ("LDADD", Atomic::Ldadd),
        ("LDCLR", Atomic::Ldclr),
        ("LDEOR", Atomic::Ldeor),
        ("LDSET", Atomic::Ldset),
        ("LDSMAX", Atomic::Ldsmax),
        ("LDSMIN", Atomic::Ldsmin),
        ("LDUMAX", Atomic::Ldumax),
        ("LDUMIN", Atomic::Ldumin),
        ("CAS", Atomic::Cas),
    ];
    for (prefix, op) in table {
        let Some(order) = stem.strip_prefix(prefix) else { continue };
        return match order {
            "" => Some((op, size, false, false)),
            "A" => Some((op, size, true, false)),
            "L" => Some((op, size, false, true)),
            "AL" => Some((op, size, true, true)),
            _ => None,
        };
    }
    None
}

fn memory_form(name: &str) -> Option<MemoryForm> {
    let (store, rest) = if let Some(rest) = name.strip_prefix("LDUR") {
        (false, ("UR", rest))
    } else if let Some(rest) = name.strip_prefix("STUR") {
        (true, ("UR", rest))
    } else if let Some(rest) = name.strip_prefix("LDR") {
        (false, ("", rest))
    } else {
        (true, ("", name.strip_prefix("STR")?))
    };
    let (unscaled, rest) = rest;
    let (element, form) = if unscaled == "UR" {
        (rest.strip_suffix('i')?, AddressForm::Unscaled)
    } else if let Some(element) = rest.strip_suffix("ui") {
        (element, AddressForm::Scaled)
    } else if let Some(element) = rest.strip_suffix("post") {
        (element, AddressForm::Post)
    } else if let Some(element) = rest.strip_suffix("pre") {
        (element, AddressForm::Pre)
    } else {
        match rest.strip_suffix("roX") {
            Some(element) => (element, AddressForm::Register),
            None => (rest.strip_suffix("roW")?, AddressForm::RegisterWord),
        }
    };
    let (width, bytes, sign) = match element {
        // `LDRSui` and `LDRDui` move a float; `LDRBui` and `LDRHui` a byte or
        // halfword of a vector register
        "B" | "H" | "S" | "D" | "Q" => (
            Width::W,
            match element {
                "B" => 1,
                "H" => 2,
                "S" => 4,
                "D" => 8,
                _ => 16,
            },
            false,
        ),
        "BB" => (Width::W, 1, false),
        "HH" => (Width::W, 2, false),
        "W" => (Width::W, 4, false),
        "X" => (Width::X, 8, false),
        "SBW" => (Width::W, 1, true),
        "SBX" => (Width::X, 1, true),
        "SHW" => (Width::W, 2, true),
        "SHX" => (Width::X, 2, true),
        "SW" => (Width::X, 4, true),
        _ => return None,
    };
    Some(MemoryForm { width, bytes, sign, store, form, fp: matches!(element, "B" | "H" | "S" | "D" | "Q") })
}

/// `CSELWr`, `CSINCXr` and the rest: the select and its width.
fn cond_select(name: &str) -> Option<(Select, Width)> {
    let (op, rest) = if let Some(rest) = name.strip_prefix("CSEL") {
        (Select::Csel, rest)
    } else if let Some(rest) = name.strip_prefix("CSINC") {
        (Select::Csinc, rest)
    } else if let Some(rest) = name.strip_prefix("CSINV") {
        (Select::Csinv, rest)
    } else {
        (Select::Csneg, name.strip_prefix("CSNEG")?)
    };
    match rest {
        "Wr" => Some((op, Width::W)),
        "Xr" => Some((op, Width::X)),
        _ => None,
    }
}

/// `ADCXr`, `SBCSWr` and the rest: the add or subtract with carry and its
/// width.
fn add_carry(name: &str) -> Option<(AddSub, Width)> {
    let (op, rest) = if let Some(rest) = name.strip_prefix("ADCS") {
        (AddSub { subtract: false, set_flags: true }, rest)
    } else if let Some(rest) = name.strip_prefix("ADC") {
        (AddSub { subtract: false, set_flags: false }, rest)
    } else if let Some(rest) = name.strip_prefix("SBCS") {
        (AddSub { subtract: true, set_flags: true }, rest)
    } else {
        (AddSub { subtract: true, set_flags: false }, name.strip_prefix("SBC")?)
    };
    match rest {
        "Wr" => Some((op, Width::W)),
        "Xr" => Some((op, Width::X)),
        _ => None,
    }
}

/// The `AArch64CC` number as the condition field.
fn cond_of(code: u64) -> Result<Cond, String> {
    use Cond::*;
    const ALL: [Cond; 16] = [Eq, Ne, Hs, Lo, Mi, Pl, Vs, Vc, Hi, Ls, Ge, Lt, Gt, Le, Al, Nv];
    ALL.get(code as usize).copied().ok_or_else(|| format!("condition code {code}"))
}

/// `ADDXrx`, `SUBSWrx` and the rest of the extended-register add/sub family.
fn add_sub_extended(name: &str) -> Option<(AddSub, Width)> {
    let (op, rest) = if let Some(rest) = name.strip_prefix("ADDS") {
        (AddSub { subtract: false, set_flags: true }, rest)
    } else if let Some(rest) = name.strip_prefix("ADD") {
        (AddSub { subtract: false, set_flags: false }, rest)
    } else if let Some(rest) = name.strip_prefix("SUBS") {
        (AddSub { subtract: true, set_flags: true }, rest)
    } else {
        (AddSub { subtract: true, set_flags: false }, name.strip_prefix("SUB")?)
    };
    match rest {
        "Wrx" => Some((op, Width::W)),
        "Xrx" => Some((op, Width::X)),
        _ => None,
    }
}

/// The extend type and shift of an `arith_extend` operand: `getArithExtendImm`
/// puts the type in bits 3.. (UXTB, UXTH, UXTW, UXTX, SXTB, SXTH, SXTW, SXTX)
/// and the shift amount below.
fn extender(operand: u64) -> Result<(Extend, u8), String> {
    const TYPES: [Extend; 8] = [
        Extend::Uxtb,
        Extend::Uxth,
        Extend::Uxtw,
        Extend::Uxtx,
        Extend::Sxtb,
        Extend::Sxth,
        Extend::Sxtw,
        Extend::Sxtx,
    ];
    let extend = TYPES.get((operand >> 3) as usize).copied().ok_or_else(|| format!("extend operand {operand}"))?;
    Ok((extend, (operand & 7) as u8))
}

/// The instruction of machine opcode `name` writing `rd`.
fn build(
    name: &str,
    rd: Gpr,
    operands: &[Operand],
) -> Result<Inst, String> {
    if let Some((op, width, immediate)) = add_sub(name) {
        return match (immediate, operands) {
            (false, [Operand::Reg(rn), Operand::Reg(rm)]) => {
                Ok(Inst::AddSubShifted { op, width, rd, rn: *rn, rm: *rm, shift: Shift::Lsl, amount: 0 })
            }
            // `ADDWrs rn, rm, shifter`: the shift type in bits 6.., the amount
            // below
            (false, [Operand::Reg(rn), Operand::Reg(rm), Operand::Imm(shifter)]) => {
                let (shift, amount) = self::shifter(*shifter);
                if shift == Shift::Ror {
                    return Err(format!("{name}: a rotate"));
                }
                Ok(Inst::AddSubShifted { op, width, rd, rn: *rn, rm: *rm, shift, amount })
            }
            // `ADDWri rn, imm12, shift` where the shift operand is 0 or 12
            (true, [Operand::Reg(rn), Operand::Imm(imm), Operand::Imm(shift)]) => Ok(Inst::AddSubImm {
                op,
                width,
                rd,
                rn: *rn,
                imm12: u16::try_from(*imm).map_err(|_| format!("{name}: immediate {imm} is no imm12"))?,
                shift12: *shift == 12,
            }),
            _ => Err(format!("{name}: operands are not those of its form")),
        };
    }
    if let Some((op, width)) = add_sub_extended(name) {
        return match operands {
            [Operand::Reg(rn), Operand::Reg(rm), Operand::Imm(extend)] => {
                let (extend, amount) = extender(*extend)?;
                Ok(Inst::AddSubExtended { op, width, rd, rn: *rn, rm: *rm, extend, amount })
            }
            _ => Err(format!("{name}: operands are not those of its form")),
        };
    }
    if matches!(name, "SMULHrr" | "UMULHrr") {
        return match operands {
            [Operand::Reg(rn), Operand::Reg(rm)] => {
                let op = if name == "SMULHrr" { Mul3::Smulh } else { Mul3::Umulh };
                Ok(Inst::ThreeSource { op, width: Width::X, rd, rn: *rn, rm: *rm, ra: Gpr::R31 })
            }
            _ => Err(format!("{name}: operands are not those of its form")),
        };
    }
    // `CCMPXi`, `CCMNWr`: the compare runs when the flags the previous
    // instruction set meet `cond`, else the flags become `nzcv`.
    if let Some(rest) = name
        .strip_prefix("CCMP")
        .map(|rest| (false, rest))
        .or_else(|| name.strip_prefix("CCMN").map(|rest| (true, rest)))
    {
        let (negate, rest) = rest;
        let width = match rest {
            "Wr" | "Wi" => Width::W,
            "Xr" | "Xi" => Width::X,
            _ => return Err(format!("{name}: no such conditional compare")),
        };
        return match operands {
            [Operand::Reg(rn), second, Operand::Imm(nzcv), Operand::Imm(cond), ..] => {
                let rm_or_imm = match (second, rest.ends_with('i')) {
                    (Operand::Reg(rm), false) => CcOperand::Reg(*rm),
                    (Operand::Imm(imm), true) => CcOperand::Imm5(*imm as u8),
                    _ => return Err(format!("{name}: operands are not those of its form")),
                };
                Ok(Inst::CondCompare { negate, width, rn: *rn, rm_or_imm, nzcv: *nzcv as u8, cond: cond_of(*cond)? })
            }
            _ => Err(format!("{name}: operands are not those of its form")),
        };
    }
    if let Some((op, invert, width, form)) = logic(name) {
        let bits = if width == Width::W { 32 } else { 64 };
        return match (form, operands) {
            ("rr", [Operand::Reg(rn), Operand::Reg(rm)]) => {
                Ok(Inst::LogicShifted { op, invert, width, rd, rn: *rn, rm: *rm, shift: Shift::Lsl, amount: 0 })
            }
            ("rs", [Operand::Reg(rn), Operand::Reg(rm), Operand::Imm(by)]) => {
                let (shift, amount) = shifter(*by);
                Ok(Inst::LogicShifted { op, invert, width, rd, rn: *rn, rm: *rm, shift, amount })
            }
            ("ri", [Operand::Reg(rn), Operand::Imm(code)]) if !invert => {
                Ok(Inst::LogicImm { op, width, rd, rn: *rn, imm: decode_logical(*code as u32, bits) })
            }
            _ => Err(format!("{name}: operands are not those of its form")),
        };
    }
    let width = |suffix: &str| if suffix.starts_with('W') { Width::W } else { Width::X };
    // `SMADDLrrr`, `UMULHrr`: a 64-bit result, no width letter.
    // `EXTRWrri`: the stem has an `X` in it.
    if let Some(rest) = name.strip_prefix("EXTR") {
        return match operands {
            [Operand::Reg(rn), Operand::Reg(rm), Operand::Imm(lsb)] if matches!(rest, "Wrri" | "Xrri") => {
                Ok(Inst::Extract { width: width(rest), rd, rn: *rn, rm: *rm, lsb: *lsb as u8 })
            }
            _ => Err(format!("{name}: operands are not those of its form")),
        };
    }
    let long = ["SMADDL", "SMSUBL", "UMADDL", "UMSUBL"]
        .iter()
        .find_map(|stem| name.strip_prefix(stem).map(|rest| (*stem, rest)));
    let (stem, suffix) = match long {
        Some((stem, "rrr")) => (stem, "Xrrr"),
        _ => name.split_at(name.find(['W', 'X']).unwrap_or(name.len())),
    };
    let two = |rn: &Operand, rm: &Operand| match (rn, rm) {
        (Operand::Reg(rn), Operand::Reg(rm)) => Ok((*rn, *rm)),
        _ => Err(format!("{name}: operands are not registers")),
    };
    match (stem, suffix, operands) {
        ("SDIV" | "UDIV" | "LSLV" | "LSRV" | "ASRV" | "RORV", "Wr" | "Xr", [rn, rm]) => {
            let op = match stem {
                "SDIV" => Div::Sdiv,
                "UDIV" => Div::Udiv,
                "LSLV" => Div::Lslv,
                "LSRV" => Div::Lsrv,
                "ASRV" => Div::Asrv,
                _ => Div::Rorv,
            };
            let (rn, rm) = two(rn, rm)?;
            Ok(Inst::Divide { op, width: width(suffix), rd, rn, rm })
        }
        ("MADD" | "MSUB" | "SMADDL" | "SMSUBL" | "UMADDL" | "UMSUBL", "Wrrr" | "Xrrr", [rn, rm, Operand::Reg(ra)]) => {
            let op = match stem {
                "MADD" => Mul3::Madd,
                "MSUB" => Mul3::Msub,
                "SMADDL" => Mul3::Smaddl,
                "SMSUBL" => Mul3::Smsubl,
                "UMADDL" => Mul3::Umaddl,
                _ => Mul3::Umsubl,
            };
            let (rn, rm) = two(rn, rm)?;
            Ok(Inst::ThreeSource { op, width: width(suffix), rd, rn, rm, ra: *ra })
        }
        ("SBFM" | "UBFM", "Wri" | "Xri", [Operand::Reg(rn), Operand::Imm(immr), Operand::Imm(imms)]) => {
            let op = if stem == "SBFM" { Bitfield::Sbfm } else { Bitfield::Ubfm };
            Ok(Inst::BitfieldMove { op, width: width(suffix), rd, rn: *rn, immr: *immr as u8, imms: *imms as u8 })
        }
        ("CLZ" | "CLS" | "RBIT" | "REV" | "REV16", "Wr" | "Xr", [Operand::Reg(rn)]) => {
            let op = match stem {
                "CLZ" => Unary::Clz,
                "CLS" => Unary::Cls,
                "RBIT" => Unary::Rbit,
                "REV" => Unary::Rev,
                _ => Unary::Rev16,
            };
            Ok(Inst::OneSource { op, width: width(suffix), rd, rn: *rn })
        }
        _ => Err(format!("machine opcode {name} has no arm64 instruction yet")),
    }
}

/// The size letter of a float opcode: `S` or `D`.
fn fp_size(letter: &str) -> Option<FpSize> {
    match letter {
        "S" => Some(FpSize::S),
        "D" => Some(FpSize::D),
        _ => None,
    }
}

fn gpr_width(letter: &str) -> Option<Width> {
    match letter {
        "W" => Some(Width::W),
        "X" => Some(Width::X),
        _ => None,
    }
}

/// The values the float register file holds across blocks: floats and the
/// vectors.
fn in_float_file(ty: EVT) -> bool {
    ty.is_floating_point() || ty.is_vector() || is_quad(ty)
}

/// A value of 128 bits in a float register (`q`): `F128` and the vectors.
fn is_quad(ty: EVT) -> bool {
    matches!(
        ty,
        EVT::Simple(MVT::F128 | MVT::V16I8 | MVT::V8I16 | MVT::V4I32 | MVT::V2I64 | MVT::V4F32 | MVT::V2F64)
    )
}

/// `DUPv8i16gpr`, `MOVIv2d_ns`, `MOVIv8i16`, `MVNIv4s_msl`: a vector made from
/// a general register or an immediate.
fn vector_fill(
    name: &str,
    operands: &[Operand],
) -> Option<Result<Fill, String>> {
    let wrong = || Err(format!("{name}: operands are not those of its form"));
    let arrangement = [
        ("DUPv16i8gpr", Arrangement::B16),
        ("DUPv8i16gpr", Arrangement::H8),
        ("DUPv4i32gpr", Arrangement::S4),
        ("DUPv2i64gpr", Arrangement::D2),
    ]
    .into_iter()
    .find(|(one, _)| *one == name)
    .map(|(_, arrangement)| arrangement);
    if let Some(arrangement) = arrangement {
        return Some(match operands {
            [Operand::Reg(source)] => Ok(Fill::Dup(arrangement, *source)),
            _ => wrong(),
        });
    }
    match (name, operands) {
        ("MOVIv16b_ns", [Operand::Imm(byte)]) => return Some(Ok(Fill::Byte(*byte as u8))),
        ("MOVIv2d_ns", [Operand::Imm(mask)]) => return Some(Ok(Fill::Mask(*mask as u8))),
        ("MOVID", [Operand::Imm(mask)]) => return Some(Ok(Fill::MaskD(*mask as u8))),
        _ => {}
    }
    for (stem, invert) in [("MOVI", false), ("MVNI", true)] {
        let Some(lanes) = name.strip_prefix(stem) else { continue };
        let shaped = |make: fn(u8) -> ImmShape, bias: u64| match operands {
            [Operand::Imm(imm8), Operand::Imm(shift)] => {
                Ok(Fill::Imm { invert, shape: make((*shift - bias) as u8), imm8: *imm8 as u8 })
            }
            _ => wrong(),
        };
        // An `msl` shift is the shifter operand `(MSL << 6) | amount`.
        return match lanes {
            "v8i16" => Some(shaped(ImmShape::H8, 0)),
            "v4i32" => Some(shaped(ImmShape::S4, 0)),
            "v2i32" => Some(shaped(ImmShape::S2, 0)),
            "v4s_msl" => Some(shaped(ImmShape::S4Msl, 256)),
            _ => continue,
        };
    }
    None
}

/// `ADDVv4i32v`, `SMAXVv16i8v`: every lane into a scalar register;
/// `ADDPv2i64p`: the two `d` lanes. The pin types the result `i32`, but it
/// is in a float register.
fn reduction(name: &str) -> Option<(VectorReduce, Arrangement)> {
    let lanes_of = |text: &str| match text {
        "16i8" => Some(Arrangement::B16),
        "8i16" => Some(Arrangement::H8),
        "4i32" => Some(Arrangement::S4),
        _ => None,
    };
    [
        ("ADDV", VectorReduce::Addv),
        ("SMAXV", VectorReduce::Smaxv),
        ("SMINV", VectorReduce::Sminv),
        ("UMAXV", VectorReduce::Umaxv),
        ("UMINV", VectorReduce::Uminv),
    ]
    .into_iter()
    .find_map(|(stem, op)| {
        let lanes = name.strip_prefix(stem)?.strip_prefix('v')?.strip_suffix('v').and_then(lanes_of)?;
        Some((op, lanes))
    })
    .or_else(|| (name == "ADDPv2i64p").then_some((VectorReduce::Addp, Arrangement::D2)))
}

/// `ADDv8i16`, `ANDv16i8`, `USHLv4i32`, `SHLv8i16_shift`, `NOTv16i8`: an
/// integer vector operation on whole 128-bit registers.
fn vector_inst(
    name: &str,
    rd: Vreg,
    operands: &[Operand],
) -> Option<Result<Inst, String>> {
    let wrong = || Err(format!("{name}: operands are not those of its form"));
    let lanes_of = |text: &str| match text {
        "16i8" => Some(Arrangement::B16),
        "8i16" => Some(Arrangement::H8),
        "4i32" => Some(Arrangement::S4),
        "2i64" => Some(Arrangement::D2),
        _ => None,
    };
    // `INSvi64gpr`, `INSvi32lane`: a general register or a lane of another
    // vector into one lane of `rd`, which starts as the first operand.
    if let Some((lanes, form)) = name
        .strip_prefix("INSv")
        .and_then(|rest| {
            rest.strip_suffix("gpr")
                .map(|lane| (lane, true))
                .or_else(|| rest.strip_suffix("lane").map(|lane| (lane, false)))
        })
        .and_then(|(lane, gpr)| {
            let lanes = match lane {
                "i8" => Arrangement::B16,
                "i16" => Arrangement::H8,
                "i32" => Arrangement::S4,
                "i64" => Arrangement::D2,
                _ => return None,
            };
            Some((lanes, gpr))
        })
    {
        return Some(match (operands, form) {
            ([Operand::Fpr(_), Operand::Imm(index), Operand::Reg(rn)], true) => {
                Ok(Inst::VectorInsGpr { lanes, rd, rn: *rn, index: *index as u8 })
            }
            ([Operand::Fpr(_), Operand::Imm(index), Operand::Fpr(rn), Operand::Imm(src)], false) => {
                Ok(Inst::VectorInsLane { lanes, rd, rn: *rn, index: *index as u8, src: *src as u8 })
            }
            _ => wrong(),
        });
    }
    // `DUPv4i32lane`, `DUPv2i64lane`: a lane of a vector register in every
    // lane (the float splat's `dup v.4s, v.s[0]`).
    if let Some(lanes) = name.strip_prefix("DUPv").and_then(|rest| rest.strip_suffix("lane")).and_then(lanes_of) {
        return Some(match operands {
            [Operand::Fpr(rn), Operand::Imm(index)] => Ok(Inst::VectorDupLane { lanes, rd, rn: *rn, index: *index as u8 }),
            _ => wrong(),
        });
    }
    if let Some((op, lanes)) = reduction(name) {
        return Some(match operands {
            [Operand::Fpr(rn)] => Ok(Inst::VectorReduce { op, lanes, rd, rn: *rn }),
            _ => wrong(),
        });
    }
    // `USHLLv8i8_shift`, `UADDWv16i8_v8i16`, `SMULLv4i16_v4i32`: a narrow
    // operand (a half of a vector, with `2`) into wide lanes.
    for op in [
        VectorLongOp::Ushll,
        VectorLongOp::Sshll,
        VectorLongOp::Uaddw,
        VectorLongOp::Saddw,
        VectorLongOp::Umull,
        VectorLongOp::Smull,
    ] {
        let Some(rest) = name.strip_prefix(&op.mnemonic().to_uppercase()).and_then(|rest| rest.strip_prefix('v'))
        else {
            continue;
        };
        let narrow = rest.split('_').next().unwrap_or(rest);
        let Some((lane, high)) = [
            ("8i8", NarrowLane::B, false),
            ("16i8", NarrowLane::B, true),
            ("4i16", NarrowLane::H, false),
            ("8i16", NarrowLane::H, true),
            ("2i32", NarrowLane::S, false),
            ("4i32", NarrowLane::S, true),
        ]
        .into_iter()
        .find(|(text, ..)| *text == narrow)
        .map(|(_, lane, high)| (lane, high)) else {
            continue;
        };
        return Some(match (op.is_shift(), operands) {
            (true, [Operand::Fpr(rn), Operand::Imm(amount)]) => {
                Ok(Inst::VectorLongShift { op, lane, high, rd, rn: *rn, amount: *amount as u8 })
            }
            (false, [Operand::Fpr(rn), Operand::Fpr(rm)]) => {
                Ok(Inst::VectorLong { op, lane, high, rd, rn: *rn, rm: *rm })
            }
            _ => wrong(),
        });
    }
    if name == "NOTv16i8" {
        return Some(match operands {
            [Operand::Fpr(rn)] => Ok(Inst::VectorNot { rd, rn: *rn }),
            _ => wrong(),
        });
    }
    // `FADDv4f32`, `FSUBv2f64`, `FNEGv4f32`: the float vector operations.
    let fp_lanes = |text: &str| match text {
        "4f32" => Some(Arrangement::S4),
        "2f64" => Some(Arrangement::D2),
        _ => None,
    };
    for (stem, op) in
        [("FADD", VectorFpOp::Add), ("FSUB", VectorFpOp::Sub), ("FMUL", VectorFpOp::Mul), ("FDIV", VectorFpOp::Div)]
    {
        let Some(lanes) = name.strip_prefix(stem).and_then(|rest| rest.strip_prefix('v')).and_then(fp_lanes) else {
            continue;
        };
        return Some(match operands {
            [Operand::Fpr(rn), Operand::Fpr(rm)] => Ok(Inst::VectorFpBinary { op, lanes, rd, rn: *rn, rm: *rm }),
            _ => wrong(),
        });
    }
    // `FMLAv4i32_indexed`: the multiplier is a lane of `rm`.
    if let Some(lanes) =
        name.strip_prefix("FMULv").and_then(|rest| rest.strip_suffix("_indexed")).and_then(|rest| match rest {
            "4i32" => Some(Arrangement::S4),
            "2i64" => Some(Arrangement::D2),
            _ => None,
        })
    {
        return Some(match operands {
            [Operand::Fpr(rn), Operand::Fpr(rm), Operand::Imm(index)] => {
                Ok(Inst::VectorFmulLane { lanes, rd, rn: *rn, rm: *rm, index: *index as u8 })
            }
            _ => wrong(),
        });
    }
    for (stem, op) in [("FMLA", VectorFma::Add), ("FMLS", VectorFma::Sub)] {
        let Some(lanes) =
            name.strip_prefix(stem).and_then(|rest| rest.strip_suffix("_indexed")).and_then(|rest| match rest {
                "v4i32" => Some(Arrangement::S4),
                "v2i64" => Some(Arrangement::D2),
                _ => None,
            })
        else {
            continue;
        };
        return Some(match operands {
            [Operand::Fpr(_), Operand::Fpr(rn), Operand::Fpr(rm), Operand::Imm(index)] => {
                Ok(Inst::VectorFmaLane { op, lanes, rd, rn: *rn, rm: *rm, index: *index as u8 })
            }
            _ => wrong(),
        });
    }
    for (stem, op) in [("FMLA", VectorFma::Add), ("FMLS", VectorFma::Sub)] {
        let Some(lanes) = name.strip_prefix(stem).and_then(|rest| rest.strip_prefix('v')).and_then(fp_lanes) else {
            continue;
        };
        // The first operand is the accumulator `rd` starts as: the caller
        // copies it in.
        return Some(match operands {
            [Operand::Fpr(_), Operand::Fpr(rn), Operand::Fpr(rm)] => {
                Ok(Inst::VectorFma { op, lanes, rd, rn: *rn, rm: *rm })
            }
            _ => wrong(),
        });
    }
    for (stem, op) in [
        ("FNEG", VectorFpUnary::Neg),
        ("FABS", VectorFpUnary::Abs),
        ("SCVTF", VectorFpUnary::Scvtf),
        ("UCVTF", VectorFpUnary::Ucvtf),
        ("FCVTZS", VectorFpUnary::Fcvtzs),
        ("FCVTZU", VectorFpUnary::Fcvtzu),
    ] {
        let Some(lanes) = name.strip_prefix(stem).and_then(|rest| rest.strip_prefix('v')).and_then(fp_lanes) else {
            continue;
        };
        return Some(match operands {
            [Operand::Fpr(rn)] => Ok(Inst::VectorFpUnary { op, lanes, rd, rn: *rn }),
            _ => wrong(),
        });
    }
    for (stem, op) in [
        ("ADD", VectorOp::Add),
        ("SUB", VectorOp::Sub),
        ("MUL", VectorOp::Mul),
        ("AND", VectorOp::And),
        ("BIC", VectorOp::Bic),
        ("ORR", VectorOp::Orr),
        ("ORN", VectorOp::Orn),
        ("EOR", VectorOp::Eor),
        ("USHL", VectorOp::Ushl),
        ("SSHL", VectorOp::Sshl),
    ] {
        let Some(lanes) = name.strip_prefix(stem).and_then(|rest| rest.strip_prefix('v')).and_then(lanes_of) else {
            continue;
        };
        if (op.is_logical() && lanes != Arrangement::B16) || (op == VectorOp::Mul && lanes == Arrangement::D2) {
            continue;
        }
        return Some(match operands {
            [Operand::Fpr(rn), Operand::Fpr(rm)] => Ok(Inst::VectorBinary { op, lanes, rd, rn: *rn, rm: *rm }),
            _ => wrong(),
        });
    }
    for (stem, op) in [("SHL", VectorShift::Shl), ("USHR", VectorShift::Ushr), ("SSHR", VectorShift::Sshr)] {
        let Some(lanes) = name
            .strip_prefix(stem)
            .and_then(|rest| rest.strip_prefix('v'))
            .and_then(|rest| rest.strip_suffix("_shift"))
            .and_then(lanes_of)
        else {
            continue;
        };
        return Some(match operands {
            [Operand::Fpr(rn), Operand::Imm(amount)] => {
                Ok(Inst::VectorShift { op, lanes, rd, rn: *rn, amount: *amount as u8 })
            }
            _ => wrong(),
        });
    }
    None
}

/// A float machine node as an instruction: the float arithmetic, compare,
/// select, conversion and move rows. `None` for any other opcode. The
/// destination is a float register, or a general one for a conversion to an
/// integer.
fn float_inst(
    name: &str,
    rd: Vreg,
    gd: Gpr,
    operands: &[Operand],
) -> Option<Result<Inst, String>> {
    let wrong = || Err(format!("{name}: operands are not those of its form"));
    if let Some(fill) = vector_fill(name, operands) {
        return Some(fill.map(|fill| Inst::VectorFill { rd, fill }));
    }
    // `UMOVvi32`: one lane of a vector into a general register; the pin's
    // `_idx0` forms read lane 0 and take no index.
    if let Some((lanes, idx0)) = name
        .strip_prefix("UMOVv")
        .map(|rest| rest.strip_suffix("_idx0").map_or((rest, false), |lane| (lane, true)))
        .and_then(|(lane, idx0)| {
            let lanes = match lane {
                "i8" => Arrangement::B16,
                "i16" => Arrangement::H8,
                "i32" => Arrangement::S4,
                "i64" => Arrangement::D2,
                _ => return None,
            };
            Some((lanes, idx0))
        })
    {
        return Some(match (operands, idx0) {
            ([Operand::Fpr(rn), Operand::Imm(index)], false) => {
                Ok(Inst::VectorUmov { lanes, rd: gd, rn: *rn, index: *index as u8 })
            }
            ([Operand::Fpr(rn)], true) => Ok(Inst::VectorUmov { lanes, rd: gd, rn: *rn, index: 0 }),
            _ => wrong(),
        });
    }
    if let Some(inst) = vector_inst(name, rd, operands) {
        return Some(inst);
    }
    // The size letter sits before `rr`, `r`, `rrr` or `i`.
    let binary = [
        ("FADD", FpBinary::Add),
        ("FSUB", FpBinary::Sub),
        ("FMUL", FpBinary::Mul),
        ("FDIV", FpBinary::Div),
        ("FNMUL", FpBinary::Nmul),
        ("FMAXNM", FpBinary::MaxNm),
        ("FMINNM", FpBinary::MinNm),
        ("FMAX", FpBinary::Max),
        ("FMIN", FpBinary::Min),
    ];
    for (stem, op) in binary {
        if let Some(size) = name.strip_prefix(stem).and_then(|rest| rest.strip_suffix("rr")).and_then(fp_size) {
            return Some(match operands {
                [Operand::Fpr(rn), Operand::Fpr(rm)] => Ok(Inst::FpBinary { op, size, rd, rn: *rn, rm: *rm }),
                _ => wrong(),
            });
        }
    }
    let unary = [
        ("FNEG", FpUnary::Neg),
        ("FABS", FpUnary::Abs),
        ("FSQRT", FpUnary::Sqrt),
        ("FMOV", FpUnary::Mov),
        ("FRINTN", FpUnary::Rint(Rounding::Nearest)),
        ("FRINTP", FpUnary::Rint(Rounding::Up)),
        ("FRINTM", FpUnary::Rint(Rounding::Down)),
        ("FRINTZ", FpUnary::Rint(Rounding::Zero)),
        ("FRINTA", FpUnary::Rint(Rounding::Away)),
        ("FRINTX", FpUnary::Rint(Rounding::Exact)),
        ("FRINTI", FpUnary::Rint(Rounding::Current)),
    ];
    for (stem, op) in unary {
        if let Some(size) = name.strip_prefix(stem).and_then(|rest| rest.strip_suffix('r')).and_then(fp_size) {
            return Some(match operands {
                [Operand::Fpr(rn)] => Ok(Inst::FpUnary { op, size, rd, rn: *rn }),
                _ => wrong(),
            });
        }
    }
    let fused =
        [("FMADD", FpFused::Madd), ("FMSUB", FpFused::Msub), ("FNMADD", FpFused::Nmadd), ("FNMSUB", FpFused::Nmsub)];
    for (stem, op) in fused {
        if let Some(size) = name.strip_prefix(stem).and_then(|rest| rest.strip_suffix("rrr")).and_then(fp_size) {
            return Some(match operands {
                [Operand::Fpr(rn), Operand::Fpr(rm), Operand::Fpr(ra)] => {
                    Ok(Inst::FpFused { op, size, rd, rn: *rn, rm: *rm, ra: *ra })
                }
                _ => wrong(),
            });
        }
    }
    if let Some(size) = name.strip_prefix("FCSEL").and_then(|rest| rest.strip_suffix("rrr")).and_then(fp_size) {
        return Some(match operands {
            [Operand::Fpr(rn), Operand::Fpr(rm), Operand::Imm(cond), ..] => match cond_of(*cond) {
                Ok(cond) => Ok(Inst::FpSelect { size, rd, rn: *rn, rm: *rm, cond }),
                Err(why) => Err(why),
            },
            _ => wrong(),
        });
    }
    // `FCVTZSSXSri`, `SCVTFSWDri`: the fixed-point forms, the second operand
    // the number of fractional bits.
    for (stem, signed, to_int) in
        [("FCVTZSS", true, true), ("FCVTZUS", false, true), ("SCVTFS", true, false), ("UCVTFS", false, false)]
    {
        if let Some(rest) = name.strip_prefix(stem).and_then(|rest| rest.strip_suffix("ri")) {
            let (width, size) = (gpr_width(&rest[..1])?, fp_size(rest.get(1..)?)?);
            return Some(match (to_int, operands) {
                (true, [Operand::Fpr(fpr), Operand::Imm(fbits)]) => Ok(Inst::FpFixed {
                    op: FpConvert::ToIntZero { signed },
                    width,
                    size,
                    gpr: gd,
                    fpr: *fpr,
                    fbits: *fbits as u8,
                }),
                (false, [Operand::Reg(gpr), Operand::Imm(fbits)]) => Ok(Inst::FpFixed {
                    op: FpConvert::ToFp { signed },
                    width,
                    size,
                    gpr: *gpr,
                    fpr: rd,
                    fbits: *fbits as u8,
                }),
                _ => wrong(),
            });
        }
    }
    // `UCVTFv1i32`: the integer in the low lane of a vector register.
    if let Some(rest) = name.strip_prefix("SCVTFv1i").or_else(|| name.strip_prefix("UCVTFv1i")) {
        let size = match rest {
            "32" => FpSize::S,
            "64" => FpSize::D,
            _ => return None,
        };
        return Some(match operands {
            [Operand::Fpr(rn)] => Ok(Inst::FpLaneToFp { signed: name.starts_with('S'), size, rd, rn: *rn }),
            _ => wrong(),
        });
    }
    // `SCVTFUWDri`: unscaled, from a `W`, to a `D`.
    for (stem, signed) in [("SCVTFU", true), ("UCVTFU", false)] {
        if let Some(rest) = name.strip_prefix(stem).and_then(|rest| rest.strip_suffix("ri")) {
            let (width, size) = (gpr_width(&rest[..1])?, fp_size(rest.get(1..)?)?);
            return Some(match operands {
                [Operand::Reg(gpr)] => {
                    Ok(Inst::FpInt { op: FpConvert::ToFp { signed }, width, size, gpr: *gpr, fpr: rd })
                }
                _ => wrong(),
            });
        }
    }
    for (stem, signed) in [("FCVTZSU", true), ("FCVTZUU", false)] {
        if let Some(rest) = name.strip_prefix(stem).and_then(|rest| rest.strip_suffix('r')) {
            let (width, size) = (gpr_width(&rest[..1])?, fp_size(rest.get(1..)?)?);
            return Some(match operands {
                [Operand::Fpr(fpr)] => {
                    Ok(Inst::FpInt { op: FpConvert::ToIntZero { signed }, width, size, gpr: gd, fpr: *fpr })
                }
                _ => wrong(),
            });
        }
    }
    // `FCVTSDr`: to `S` from `D`.
    if let Some(rest) = name.strip_prefix("FCVT").and_then(|rest| rest.strip_suffix('r')) {
        if let (Some(to), Some(from)) = (rest.get(..1).and_then(fp_size), rest.get(1..).and_then(fp_size)) {
            return Some(match operands {
                [Operand::Fpr(rn)] => Ok(Inst::FpToFp { to, from, rd, rn: *rn }),
                _ => wrong(),
            });
        }
    }
    if let Some(size) = name.strip_prefix("FMOV").and_then(|rest| rest.strip_suffix('i')).and_then(fp_size) {
        return Some(match operands {
            [Operand::Imm(imm8)] => Ok(Inst::FpMoveImm { size, rd, imm8: *imm8 as u8 }),
            _ => wrong(),
        });
    }
    // `FMOVD0`: zero is the zero register moved across.
    if let Some(size) = name.strip_prefix("FMOV").and_then(|rest| rest.strip_suffix('0')).and_then(fp_size) {
        let width = if size == FpSize::D { Width::X } else { Width::W };
        return Some(Ok(Inst::FpInt { op: FpConvert::Bits { to_fp: true }, width, size, gpr: Gpr::R31, fpr: rd }));
    }
    // `FMOVSWr` is a `W` into an `S`, `FMOVWSr` the other way.
    if let Some(rest) = name.strip_prefix("FMOV").and_then(|rest| rest.strip_suffix('r')) {
        if let (Some(size), Some(width)) = (rest.get(..1).and_then(fp_size), rest.get(1..).and_then(gpr_width)) {
            return Some(match operands {
                [Operand::Reg(gpr)] => {
                    Ok(Inst::FpInt { op: FpConvert::Bits { to_fp: true }, width, size, gpr: *gpr, fpr: rd })
                }
                _ => wrong(),
            });
        }
        if let (Some(width), Some(size)) = (rest.get(..1).and_then(gpr_width), rest.get(1..).and_then(fp_size)) {
            return Some(match operands {
                [Operand::Fpr(fpr)] => {
                    Ok(Inst::FpInt { op: FpConvert::Bits { to_fp: false }, width, size, gpr: gd, fpr: *fpr })
                }
                _ => wrong(),
            });
        }
    }
    None
}

/// The register of the builder's virtual register `number`, in the float class
/// where `float`.
fn phi_register(
    emitter: &mut Emitter,
    number: u32,
    float: bool,
    quad: bool,
) -> Reg {
    if !float {
        return Reg::Gpr(emitter.vreg(number));
    }
    let register = Reg::Fpr(emitter.fvreg(number));
    if quad {
        emitter.mf.wide.push(register);
    }
    register
}

struct Emitter<'a> {
    dag: &'a SelectionDAG,
    globals: &'a Globals,
    function: &'a str,
    layout: &'a [u32],
    /// The blocks each jump table goes to, by table number.
    tables: &'a [Vec<u32>],
    mf: MFunc,
    /// The virtual register each value is in.
    values: HashMap<(u32, u8), Gpr>,
    /// The machine register each of the builder's virtual registers became.
    vregs: HashMap<u32, Gpr>,
    /// The same for the float registers, and the values in them.
    fvregs: HashMap<u32, Vreg>,
    fvalues: HashMap<(u32, u8), Vreg>,
    ops: Vec<Op>,
    /// The constant pool entries the function has used so far.
    pool: std::cell::RefCell<Vec<PoolEntry>>,
    /// The label of the block segment `ops` ends: the selector's block, or the
    /// join of the last split.
    segment: String,
    /// The splits (`F128CSEL`) in the block being emitted.
    cuts: Vec<Cut>,
    /// The splits so far in the function, for fresh labels.
    splits: usize,
    /// The registers that hold a copy of the stack pointer (`CopyFromReg SP`):
    /// an address with one as its base uses the stack pointer itself, as
    /// `InstrEmitter` uses a reserved register where it is read.
    sp_copies: rustc_hash::FxHashSet<Gpr>,
    /// Some block writes the stack pointer (a dynamic `alloca`,
    /// `llvm.stackrestore`): the outgoing area is no longer reserved in the
    /// frame (`hasReservedCallFrame`), each call makes room for it.
    moves_sp: bool,
    /// The register each phi's edges copy into, by the phi's register
    /// (`PHIElimination`'s `IncomingReg`): the block's first copies read it.
    incoming: HashMap<u32, Reg>,
}

/// A block split in two where a pseudo-instruction is expanded to control flow
/// (`EmitF128CSEL`): the `ops` before `at` end the head, the rest begin the
/// join `end`, after the empty block `taken` that falls into it.
struct Cut {
    at: usize,
    taken: String,
    end: String,
    phi: Op,
}

impl Emitter<'_> {
    fn vreg(
        &mut self,
        number: u32,
    ) -> Gpr {
        if let Some(&one) = self.vregs.get(&number) {
            return one;
        }
        let one = self.mf.gpr();
        self.vregs.insert(number, one);
        one
    }

    fn fvreg(
        &mut self,
        number: u32,
    ) -> Vreg {
        if let Some(&one) = self.fvregs.get(&number) {
            return one;
        }
        let one = self.mf.fpr();
        self.fvregs.insert(number, one);
        one
    }

    /// The register operand `value` names: a physical or a virtual register.
    fn register_operand(
        &mut self,
        value: SDValue,
    ) -> Result<(Gpr, bool), String> {
        match self.dag.node(value).data {
            NodeData::PhysReg(id) => Ok((gpr_of(id)?, true)),
            NodeData::Register(number) => Ok((self.vreg(number), false)),
            other => Err(format!("a register operand that is {other:?}")),
        }
    }

    /// `InstrEmitter::AddRegisterOperand`'s `constrainRegClass`: a value in the
    /// other register file than the operand's class is copied across (`fmov`),
    /// as when a lane of a vector, held in a `V` register, is read as an
    /// integer.
    fn constrain_register_classes(
        &mut self,
        code: u32,
        node: &llrm_codegen::backend::selectiondag::SDNode,
        operands: &mut [Operand],
    ) {
        use llrm_dagisel::generated::aarch64::ids::reg;
        let Some(desc) = u16::try_from(code).ok().and_then(llrm_dagisel::generated::aarch64::instrinfo::desc) else {
            return;
        };
        if desc.is_pseudo() {
            return;
        }
        let values: Vec<SDValue> = node
            .ops
            .iter()
            .copied()
            .filter(|op| {
                let vt = self.dag.value_type(*op);
                (vt != EVT::OTHER || self.dag.opcode(*op) == ISD::BasicBlock) && vt != EVT::GLUE
            })
            .collect();
        const GPR: [u16; 7] = [
            reg::GPR32RegClassID,
            reg::GPR32spRegClassID,
            reg::GPR32commonRegClassID,
            reg::GPR64RegClassID,
            reg::GPR64spRegClassID,
            reg::GPR64commonRegClassID,
            reg::GPR64noipRegClassID,
        ];
        const FPR: [u16; 7] = [
            reg::FPR8RegClassID,
            reg::FPR16RegClassID,
            reg::FPR32RegClassID,
            reg::FPR64RegClassID,
            reg::FPR128RegClassID,
            reg::FPR128_loRegClassID,
            reg::FPR128_0to7RegClassID,
        ];
        for (at, operand) in operands.iter_mut().enumerate() {
            let Some(want) = desc.operands.get(usize::from(desc.num_defs) + at) else { break };
            let Ok(class) = u16::try_from(want.reg_class) else { continue };
            let bits = self.dag.value_type(values[at]).size_in_bits();
            let (width, size) = if bits > 32 { (Width::X, FpSize::D) } else { (Width::W, FpSize::S) };
            match *operand {
                Operand::Fpr(fpr) if GPR.contains(&class) => {
                    let gpr = self.mf.gpr();
                    self.ops.push(Op::I(Inst::FpInt { op: FpConvert::Bits { to_fp: false }, width, size, gpr, fpr }));
                    *operand = Operand::Reg(gpr);
                }
                Operand::Reg(gpr) if FPR.contains(&class) && !self.dag.value_type(values[at]).is_vector() => {
                    let fpr = self.mf.fpr();
                    self.ops.push(Op::I(Inst::FpInt { op: FpConvert::Bits { to_fp: true }, width, size, gpr, fpr }));
                    *operand = Operand::Fpr(fpr);
                }
                _ => {}
            }
        }
    }

    fn operand(
        &self,
        value: SDValue,
    ) -> Result<Operand, String> {
        let node = self.dag.node(value);
        // The second result of a machine node is the flags it set, except
        // for a writeback load, whose results are the new base and the value.
        if value.res == 1
            && matches!(
                node.opcode,
                ISD::Machine(code) if !opcode_name(code).is_some_and(|name| memory_form(name).is_some_and(|form| {
                    matches!(form.form, AddressForm::Post | AddressForm::Pre)
                }))
            )
        {
            return Ok(Operand::Flags);
        }
        // `FCMP` has no result but the flags.
        if matches!(
            node.opcode,
            ISD::Machine(code) if opcode_name(code).is_some_and(|name| name.starts_with("FCMP"))
        ) {
            return Ok(Operand::Flags);
        }
        match node.data {
            NodeData::Const { bits, .. } if matches!(node.opcode, ISD::TargetConstant | ISD::Constant) => {
                Ok(Operand::Imm(bits as u64))
            }
            NodeData::Block(id) => Ok(Operand::Block(id)),
            NodeData::JumpTable(table) if node.opcode == ISD::TargetJumpTable => Ok(Operand::Table(table)),
            NodeData::Symbol(id) if node.opcode == ISD::TargetExternalSymbolNode => Ok(Operand::External(id)),
            NodeData::PoolIndex(index, 0) if node.opcode == ISD::TargetConstantPool => {
                Ok(Operand::Pool(self.pool_entry(index)))
            }
            // The zero register as an operand of `CSINC` and the like.
            NodeData::PhysReg(id) if gpr_of(id).is_ok_and(|g| g == Gpr::R31) => Ok(Operand::Reg(Gpr::R31)),
            NodeData::Global { symbol, offset } if node.opcode == ISD::TargetGlobalAddress => {
                Ok(Operand::Symbol(symbol, offset))
            }
            NodeData::FrameIndex(index) if node.opcode == ISD::TargetFrameIndex && index < 0 => self
                .dag
                .fixed_offsets
                .get(&index)
                .map(|&offset| Operand::Fixed(offset))
                .ok_or_else(|| format!("fixed object {index} has no offset")),
            NodeData::FrameIndex(index) if node.opcode == ISD::TargetFrameIndex => Ok(Operand::Slot(index as u32)),
            _ => self
                .values
                .get(&(value.node, value.res))
                .map(|&g| Operand::Reg(g))
                .or_else(|| self.fvalues.get(&(value.node, value.res)).map(|&f| Operand::Fpr(f)))
                .ok_or_else(|| format!("operand {:?} has no register", node.opcode)),
        }
    }

    /// `BL symbol` or `BLR reg`: the registers it reads are the `Register`
    /// operands (the arguments), those it writes the physical registers a
    /// glued `CopyFromReg` after its `CALLSEQ_END` reads (the results).
    fn call(
        &mut self,
        id: u32,
        name: &str,
    ) -> Result<(), String> {
        let node = self.dag.node(SDValue { node: id, res: 0 });
        let callee = match (name, self.operand(node.ops[0])?) {
            ("BL", Operand::Symbol(global, _)) => match self.target(global, 0)?.0 {
                Target::Symbol { name, .. } => Callee::Symbol(name),
                _ => return Err("a callee that is no symbol".into()),
            },
            ("BL", Operand::External(symbol)) => {
                Callee::Symbol(self.globals.external(&self.dag.symbol_names[symbol as usize]))
            }
            ("BLR", Operand::Reg(register)) => Callee::Register(register),
            _ => return Err(format!("{name}: a callee that is neither a symbol nor a register")),
        };
        let uses = node.ops[1..]
            .iter()
            .filter_map(|op| match self.dag.node(*op).data {
                NodeData::PhysReg(register) => Some(reg_of(register)),
                _ => None,
            })
            .collect::<Result<Vec<_>, _>>()?;
        // The glue out of the call goes to `CALLSEQ_END`, and from it to the
        // result copies.
        let mut defs = Vec::new();
        let mut glue = SDValue { node: id, res: self.dag.node(SDValue { node: id, res: 0 }).values.len() as u8 - 1 };
        while let Some(user) = self.dag.users(glue).next() {
            let user_node = self.dag.node(SDValue { node: user, res: 0 });
            match user_node.opcode {
                ISD::CopyFromReg => {
                    let NodeData::PhysReg(register) = self.dag.node(user_node.ops[1]).data else { break };
                    defs.push(reg_of(register)?);
                    glue = SDValue { node: user, res: 2 };
                }
                _ => glue = SDValue { node: user, res: user_node.values.len() as u8 - 1 },
            }
        }
        self.mf.calls = true;
        self.ops.push(Op::Call { callee, uses, defs, tail: false });
        Ok(())
    }

    /// `TCRETURNdi` and `TCRETURNri`: the epilogue and a jump to the callee.
    /// The arguments are the `Register` operands. An indirect callee is moved
    /// to `x9` first: the epilogue restores the callee-saved registers and uses
    /// `x16`/`x17` itself. DEVIATE SD531: the pin lets the allocator pick any
    /// `tcGPR64`; here the register is fixed.
    fn tail_call(
        &mut self,
        id: u32,
        name: &str,
    ) -> Result<(), String> {
        let node = self.dag.node(SDValue { node: id, res: 0 });
        let callee = match (name, self.operand(node.ops[0])?) {
            ("TCRETURNdi", Operand::Symbol(global, _)) => match self.target(global, 0)?.0 {
                Target::Symbol { name, .. } => Callee::Symbol(name),
                _ => return Err("a callee that is no symbol".into()),
            },
            ("TCRETURNdi", Operand::External(symbol)) => {
                Callee::Symbol(self.globals.external(&self.dag.symbol_names[symbol as usize]))
            }
            (_, Operand::Reg(register)) => {
                let target = Gpr(9);
                self.ops.push(Op::Copy { dst: Reg::Gpr(target), src: Reg::Gpr(register) });
                Callee::Register(target)
            }
            _ => return Err(format!("{name}: a callee that is neither a symbol nor a register")),
        };
        let uses = node
            .ops
            .iter()
            .filter_map(|op| match self.dag.node(*op).data {
                NodeData::PhysReg(register) => Some(reg_of(register)),
                _ => None,
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.ops.push(Op::Return { uses, tail: Some(callee) });
        Ok(())
    }

    /// `COPY_TO_REGCLASS`: a copy, across the general and float classes by
    /// `fmov`.
    fn copy_to_class(
        &mut self,
        id: u32,
        operands: &[Operand],
    ) -> Result<(), String> {
        let to = self.dag.value_type(SDValue { node: id, res: 0 });
        let bits = to.size_in_bits();
        let (width, size) = if bits == 64 { (Width::X, FpSize::D) } else { (Width::W, FpSize::S) };
        // The class says it where the type cannot: a vector has no float type
        // (`FPR8` 6, `FPR16` 7, `FPR32` 29, `FPR64` 43, `FPR128` 77). Scalars
        // keep the type, so no scalar compile can change.
        let to_float = match operands.get(1) {
            Some(Operand::Imm(class)) if to.is_vector() => matches!(class, 6 | 7 | 29 | 43 | 77),
            _ => to.is_floating_point(),
        };
        match (operands.first(), to_float) {
            (Some(Operand::Reg(gpr)), true) => {
                let fpr = self.mf.fpr();
                self.ops.push(Op::I(Inst::FpInt { op: FpConvert::Bits { to_fp: true }, width, size, gpr: *gpr, fpr }));
                self.fvalues.insert((id, 0), fpr);
            }
            (Some(Operand::Fpr(fpr)), false) => {
                let gpr = self.mf.gpr();
                self.ops.push(Op::I(Inst::FpInt { op: FpConvert::Bits { to_fp: false }, width, size, gpr, fpr: *fpr }));
                self.values.insert((id, 0), gpr);
            }
            (Some(Operand::Reg(src)), false) => {
                let dst = self.mf.gpr();
                self.ops.push(Op::Copy { dst: Reg::Gpr(dst), src: Reg::Gpr(*src) });
                self.values.insert((id, 0), dst);
            }
            (Some(Operand::Fpr(src)), true) => {
                let dst = self.mf.fpr();
                self.ops.push(Op::Copy { dst: Reg::Fpr(dst), src: Reg::Fpr(*src) });
                self.fvalues.insert((id, 0), dst);
            }
            _ => return Err("COPY_TO_REGCLASS of something that is no register".into()),
        }
        Ok(())
    }

    fn is_flags(
        &self,
        register: SDValue,
    ) -> bool {
        matches!(
            self.dag.node(register).data,
            NodeData::PhysReg(id) if id == llrm_codegen::backend::selectiondag::dagisel_adapter::EFLAGS_ID
        )
    }

    /// The label of block `id`.
    fn label(
        &self,
        id: u32,
    ) -> Result<Target, String> {
        let at = self.layout.iter().position(|&one| one == id).ok_or("a branch to a block outside the function")?;
        Ok(Target::Label(block_label(self.function, at)))
    }

    /// `B`, `Bcc`, `CBZW`..`CBNZX` and `TBZW`..`TBNZX`; `None` for any other
    /// opcode.
    fn branch(
        &self,
        name: &str,
        operands: &[Operand],
    ) -> Result<Option<Inst>, String> {
        let width = |suffix: &str| if suffix == "W" { Width::W } else { Width::X };
        Ok(Some(match (name, operands) {
            ("B", [Operand::Block(to)]) => Inst::Branch { link: false, target: self.label(*to)? },
            ("Bcc", [Operand::Imm(cond), Operand::Block(to), ..]) => {
                Inst::BranchCond { cond: cond_of(*cond)?, target: self.label(*to)? }
            }
            (_, [Operand::Reg(rt), Operand::Block(to)]) if matches!(name, "CBZW" | "CBZX" | "CBNZW" | "CBNZX") => {
                let (nonzero, suffix) = (name.starts_with("CBNZ"), &name[name.len() - 1..]);
                Inst::CompareBranch { nonzero, width: width(suffix), rt: *rt, target: self.label(*to)? }
            }
            (_, [Operand::Reg(rt), Operand::Imm(bit), Operand::Block(to)])
                if matches!(name, "TBZW" | "TBZX" | "TBNZW" | "TBNZX") =>
            {
                let nonzero = name.starts_with("TBNZ");
                Inst::TestBranch { nonzero, rt: *rt, bit: *bit as u8, target: self.label(*to)? }
            }
            _ => return Ok(None),
        }))
    }

    /// The place in the function's pool of entry `index` of this block's DAG,
    /// made on first use: equal constants of the function share one entry.
    fn pool_entry(
        &self,
        index: u32,
    ) -> usize {
        let (bits, vt) = self.dag.constant_pool[index as usize];
        let bytes = vt.size_in_bits().div_ceil(8) as usize;
        let image = bits.to_le_bytes()[..bytes].to_vec();
        let mut pool = self.pool.borrow_mut();
        let at = match pool.iter().position(|entry| entry.bytes == image) {
            Some(at) => at,
            None => {
                let label = format!("{}CPI_{}_{}", self.globals.local_prefix(), self.function, pool.len());
                pool.push(PoolEntry { label, bytes: image, align: bytes });
                pool.len() - 1
            }
        };
        at
    }

    /// The relocation target of a symbol or pool operand.
    fn symbol_target(
        &self,
        operand: &Operand,
    ) -> Result<(Target, bool), String> {
        match operand {
            Operand::Symbol(id, addend) => self.target(*id, *addend),
            Operand::Pool(at) => Ok((Target::Symbol { name: self.pool.borrow()[*at].label.clone(), addend: 0 }, false)),
            _ => Err("an operand that is no symbol".into()),
        }
    }

    /// The relocation target of global `id` at `addend`, and whether it is a
    /// GOT slot.
    fn target(
        &self,
        id: u32,
        addend: i64,
    ) -> Result<(Target, bool), String> {
        let global = self.globals.get(id).ok_or("a global the module does not have")?;
        Ok((Target::Symbol { name: global.name.clone(), addend }, global.got))
    }

    /// A load or store: operands are `[value,] base, offset...` without chains.
    /// The address `offset` bytes past the pointer `value`.
    fn pointer_at(
        &self,
        value: SDValue,
        offset: u64,
    ) -> Result<Addr, String> {
        let imm = offset as i64;
        match self.operand(value)? {
            Operand::Reg(base) => {
                Ok(Addr::Offset { base: if self.sp_copies.contains(&base) { Gpr::R31 } else { base }, imm })
            }
            Operand::Slot(slot) => Ok(Addr::Slot { slot, offset: imm }),
            Operand::Fixed(at) => Ok(Addr::Slot { slot: INCOMING_ARGUMENTS, offset: at + imm }),
            _ => Err("a block of memory at something that is no pointer register or frame object".into()),
        }
    }

    /// `pointer_at` for an access of `bytes`: an offset the instruction cannot
    /// encode (scaled unsigned 12 bits, or unscaled signed 9) is added to the
    /// base first, as selecting the access at `ptr + offset` does.
    fn access_at(
        &mut self,
        value: SDValue,
        offset: u64,
        bytes: u8,
    ) -> Result<Addr, String> {
        let legal = |imm: i64| {
            let size = i64::from(bytes);
            (imm >= 0 && imm % size == 0 && imm / size <= 4095) || (-256..=255).contains(&imm)
        };
        let Addr::Offset { base, imm } = self.pointer_at(value, offset)? else { return self.pointer_at(value, offset) };
        if legal(imm) {
            return Ok(Addr::Offset { base, imm });
        }
        let (mut base, mut rest) = (base, imm);
        // The high part first (`add rd, rn, #hi, lsl #12`), then the low.
        for (part, shift12) in [(rest >> 12, true), (rest & 0xfff, false)] {
            if part == 0 || (!shift12 && legal(part)) {
                continue;
            }
            let rd = self.mf.gpr();
            self.ops.push(Op::I(Inst::AddSubImm {
                op: AddSub { subtract: false, set_flags: false },
                width: Width::X,
                rd,
                rn: base,
                imm12: part as u16,
                shift12,
            }));
            base = rd;
            rest -= if shift12 { part << 12 } else { part };
        }
        Ok(Addr::Offset { base, imm: rest })
    }

    /// `str q<n>, [slot, #offset]`: an unnamed `q` argument register saved
    /// whole.
    fn save_q(
        &mut self,
        id: u32,
    ) -> Result<(), String> {
        let node = self.dag.node(SDValue { node: id, res: 0 });
        let number = self.dag.constant(node.ops[2]).ok_or("a saved q register of no number")? as u32;
        let offset = self.dag.constant(node.ops[3]).ok_or("a saved q register of no offset")? as u64;
        let addr = self.pointer_at(node.ops[1], offset)?;
        self.ops.push(Op::I(Inst::Store {
            mem: Memory::Fp { bytes: 16, reg: Vreg(number) },
            addr,
            mmo: MemOperand::default(),
        }));
        Ok(())
    }

    /// A fresh register to carry `bytes` of a block.
    fn piece(
        &mut self,
        bytes: u8,
    ) -> Memory {
        if bytes == 16 {
            Memory::Fp { bytes, reg: self.mf.fpr() }
        } else {
            Memory::Int { width: if bytes == 8 { Width::X } else { Width::W }, bytes, sign: false, reg: self.mf.gpr() }
        }
    }

    /// A constant-size `memcpy`, `memmove` or `memset` as `q` and smaller
    /// accesses (`findOptimalMemOpLowering`); a copy loads a batch before it
    /// stores, a move all of it.
    fn block_memory(
        &mut self,
        id: u32,
        name: &str,
    ) -> Result<(), String> {
        use crate::dag::memops::{BATCH, pieces, wide_pieces};
        let node = self.dag.node(SDValue { node: id, res: 0 });
        let operand = |at: usize| node.ops[at];
        let mmo = MemOperand { volatile: self.dag.volatile_accesses.contains(&id), ..MemOperand::default() };
        if name == "LLRM_MEMSET_Q" {
            let total = self.dag.constant(operand(3)).ok_or("a fill of no size")? as u64;
            let byte = self.dag.constant(operand(2)).map(|value| value as u8);
            let ops = if byte.is_none() { wide_pieces(total) } else { pieces(total) };
            let vector = if ops.iter().any(|&(bytes, _)| bytes == 16) {
                let rd = self.mf.fpr();
                let fill = match byte {
                    Some(0) => Fill::Zero,
                    Some(byte) => Fill::Byte(byte),
                    None => {
                        let Operand::Reg(reg) = self.operand(operand(2))? else { return Err("a fill byte".into()) };
                        Fill::Dup(Arrangement::B16, reg)
                    }
                };
                self.ops.push(Op::I(Inst::VectorFill { rd, fill }));
                Some(rd)
            } else {
                None
            };
            let scalar = match byte {
                Some(0) => Gpr::R31,
                Some(byte) if ops.iter().any(|&(bytes, _)| bytes != 16) => {
                    let reg = self.mf.gpr();
                    self.ops.push(Op::Imm { dst: reg, bits: 64, value: u64::from(byte) * 0x0101_0101_0101_0101 });
                    reg
                }
                _ => Gpr::R31,
            };
            for (bytes, offset) in ops {
                let mem = match (bytes, vector) {
                    (16, Some(reg)) => Memory::Fp { bytes, reg },
                    _ => Memory::Int {
                        width: if bytes == 8 { Width::X } else { Width::W },
                        bytes,
                        sign: false,
                        reg: scalar,
                    },
                };
                let addr = self.access_at(operand(1), offset, bytes)?;
                self.ops.push(Op::I(Inst::Store { mem, addr, mmo: mmo.clone() }));
            }
            return Ok(());
        }
        let total = self.dag.constant(operand(3)).ok_or("a copy of no size")? as u64;
        let ops = pieces(total);
        let batch = if name == "LLRM_MEMMOVE_Q" { ops.len() } else { BATCH };
        for chunk in ops.chunks(batch) {
            let mut loaded = Vec::new();
            for &(bytes, offset) in chunk {
                let mem = self.piece(bytes);
                let addr = self.access_at(operand(2), offset, bytes)?;
                self.ops.push(Op::I(Inst::Load { mem, addr, mmo: mmo.clone() }));
                loaded.push(mem);
            }
            for (&(bytes, offset), mem) in chunk.iter().zip(loaded) {
                let addr = self.access_at(operand(1), offset, bytes)?;
                self.ops.push(Op::I(Inst::Store { mem, addr, mmo: mmo.clone() }));
            }
        }
        Ok(())
    }

    /// What `value`, an address, points into: a global or a frame object
    /// (`MachineMemOperand`'s pointer, which the pin's load/store optimizer
    /// asks whether two accesses may alias), read from the nodes that made
    /// the address.
    fn object_of(
        &self,
        value: SDValue,
        depth: u32,
    ) -> Option<MemObject> {
        if depth > 6 {
            return None;
        }
        let node = self.dag.node(value);
        match (&node.data, node.opcode) {
            (NodeData::Global { symbol, .. }, ISD::TargetGlobalAddress) => {
                Some(MemObject::Global(self.globals.get(*symbol)?.name.clone()))
            }
            (NodeData::PoolIndex(..), ISD::TargetConstantPool) => Some(MemObject::ConstantPool),
            (NodeData::FrameIndex(index), ISD::TargetFrameIndex | ISD::FrameIndex) if *index >= 0 => {
                Some(MemObject::Slot(*index as u32))
            }
            (_, ISD::Machine(code)) if matches!(opcode_name(code), Some("ADRP" | "ADDXri" | "MOVaddr")) => {
                node.ops.iter().find_map(|&op| self.object_of(op, depth + 1))
            }
            _ => None,
        }
    }

    /// How far `value`, an address, is into the object `object_of` names: the
    /// constants the nodes that made it add (the offset of a
    /// `MachineMemOperand`'s pointer).
    fn displacement_of(
        &self,
        value: SDValue,
        depth: u32,
    ) -> Option<i64> {
        if depth > 6 {
            return None;
        }
        let node = self.dag.node(value);
        match (&node.data, node.opcode) {
            (NodeData::Global { offset, .. }, ISD::TargetGlobalAddress) => Some(*offset),
            (NodeData::FrameIndex(index), ISD::TargetFrameIndex | ISD::FrameIndex) if *index >= 0 => Some(0),
            (_, ISD::Machine(code)) if matches!(opcode_name(code), Some("ADRP" | "MOVaddr")) => {
                self.displacement_of(*node.ops.first()?, depth + 1)
            }
            (_, ISD::Machine(code)) if opcode_name(code) == Some("ADDXri") => {
                let (base, added, shift) = (*node.ops.first()?, *node.ops.get(1)?, node.ops.get(2).copied());
                if shift.is_some_and(|shift| self.dag.constant(shift) != Some(0)) {
                    return None;
                }
                Some(self.displacement_of(base, depth + 1)? + i64::try_from(self.dag.constant(added)?).ok()?)
            }
            _ => None,
        }
    }

    /// The memory operand of the load or store `id`: its volatile flag, the
    /// object its address names and, where `at` (the offset from the address
    /// node, `None` for a form that is not a plain offset) and the node say,
    /// how far into it.
    fn memory_operand(
        &self,
        id: u32,
        store: bool,
        at: Option<i64>,
    ) -> MemOperand {
        let node = self.dag.node(SDValue { node: id, res: 0 });
        let base = node.ops.get(usize::from(store)).copied();
        let object = base.and_then(|base| self.object_of(base, 0)).unwrap_or_default();
        let offset = base.and_then(|base| Some(self.displacement_of(base, 0)? + at?));
        MemOperand { object, offset, volatile: self.dag.volatile_accesses.contains(&id) }
    }

    fn memory(
        &mut self,
        id: u32,
        name: &str,
        access: &MemoryForm,
        operands: &[Operand],
    ) -> Result<(), String> {
        let (value, fvalue, address) = match (access.store, operands) {
            (true, [Operand::Reg(value), rest @ ..]) if !access.fp => (Some(*value), None, rest),
            (true, [Operand::Fpr(value), rest @ ..]) if access.fp => (None, Some(*value), rest),
            (false, rest) => (None, None, rest),
            _ => return Err(format!("{name}: a store of something that is no register")),
        };
        let size = i64::from(access.bytes);
        // A writeback form's first result is the new base: the loaded value is
        // the second.
        let writeback = u8::from(matches!(access.form, AddressForm::Post | AddressForm::Pre));
        let addr = match (access.form, address) {
            (
                AddressForm::Register,
                [Operand::Reg(base), Operand::Reg(index), Operand::Imm(_), Operand::Imm(shifted)],
            ) => Addr::Register { base: *base, index: *index, extend: Extend::Uxtx, shifted: *shifted != 0 },
            (
                AddressForm::RegisterWord,
                [Operand::Reg(base), Operand::Reg(index), Operand::Imm(signed), Operand::Imm(shifted)],
            ) => {
                let extend = if *signed != 0 { Extend::Sxtw } else { Extend::Uxtw };
                Addr::Register { base: *base, index: *index, extend, shifted: *shifted != 0 }
            }
            (AddressForm::Register | AddressForm::RegisterWord, _) => {
                return Err(format!("{name}: a register offset that is not base and index"));
            }
            // The access updates its base register: the new base is the
            // node's first result, a copy of the old one that the access
            // then steps.
            (AddressForm::Post | AddressForm::Pre, [Operand::Reg(base), Operand::Imm(imm)]) => {
                let stepped = self.mf.gpr();
                self.ops.push(Op::Copy { dst: Reg::Gpr(stepped), src: Reg::Gpr(*base) });
                self.values.insert((id, 0), stepped);
                let imm = *imm as i64;
                if access.form == AddressForm::Post {
                    Addr::Post { base: stepped, imm }
                } else {
                    Addr::Pre { base: stepped, imm }
                }
            }
            (AddressForm::Scaled, [Operand::Reg(base), symbol @ (Operand::Symbol(..) | Operand::Pool(_))]) => {
                Addr::PageOffset { base: *base, target: self.symbol_target(symbol)?.0, got: false }
            }
            (form, [base, Operand::Imm(imm)]) => {
                let imm = *imm as i64 * if form == AddressForm::Scaled { size } else { 1 };
                match base {
                    Operand::Reg(base) => {
                        Addr::Offset { base: if self.sp_copies.contains(base) { Gpr::R31 } else { *base }, imm }
                    }
                    Operand::Slot(slot) => Addr::Slot { slot: *slot, offset: imm },
                    Operand::Fixed(at) => Addr::Slot { slot: INCOMING_ARGUMENTS, offset: at + imm },
                    _ => return Err(format!("{name}: a base that is neither register nor frame object")),
                }
            }
            _ => return Err(format!("{name}: operands are not those of its form")),
        };
        // A selected node keeps the flag of the access it folded
        // (`set_node_mem_refs`).
        let at = match &addr {
            Addr::Offset { imm, .. } | Addr::Pre { imm, .. } => Some(*imm),
            Addr::Post { .. } => Some(0),
            _ => None,
        };
        let mmo = self.memory_operand(id, access.store, at);
        if access.fp {
            let reg = match fvalue {
                Some(reg) => reg,
                None => self.mf.fpr(),
            };
            let mem = Memory::Fp { bytes: access.bytes, reg };
            self.ops.push(Op::I(if access.store {
                Inst::Store { mem, addr, mmo }
            } else {
                Inst::Load { mem, addr, mmo }
            }));
            if !access.store {
                self.fvalues.insert((id, writeback), reg);
            }
            return Ok(());
        }
        let rd = if access.store { None } else { Some(self.mf.gpr()) };
        let mem = Memory::Int {
            width: access.width,
            bytes: access.bytes,
            sign: access.sign,
            reg: value.or(rd).expect("a value"),
        };
        self.ops.push(Op::I(if access.store { Inst::Store { mem, addr, mmo } } else { Inst::Load { mem, addr, mmo } }));
        if let Some(rd) = rd {
            self.values.insert((id, writeback), rd);
        }
        Ok(())
    }

    /// `AArch64TargetLowering::EmitF128CSEL`: the block ends in `b.cc taken; b
    /// end`, `taken` is empty and falls into `end`, which begins with `dest
    /// = PHI [if_true, taken], [if_false, head]` and holds the rest of the
    /// block.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L3276>
    fn f128_select(
        &mut self,
        id: u32,
        operands: &[Operand],
    ) -> Result<(), String> {
        let [Operand::Fpr(if_true), Operand::Fpr(if_false), Operand::Imm(cond), ..] = operands else {
            return Err("F128CSEL: operands are not those of its form".into());
        };
        let (taken, end) =
            (format!(".L{}.t{}", self.function, self.splits), format!(".L{}.j{}", self.function, self.splits));
        self.splits += 1;
        let dest = self.mf.fpr();
        self.mf.wide.push(Reg::Fpr(dest));
        self.ops.push(Op::I(Inst::BranchCond { cond: cond_of(*cond)?, target: Target::Label(taken.clone()) }));
        self.ops.push(Op::I(Inst::Branch { link: false, target: Target::Label(end.clone()) }));
        let phi = Op::Phi {
            dst: Reg::Fpr(dest),
            incoming: vec![(Reg::Fpr(*if_true), taken.clone()), (Reg::Fpr(*if_false), self.segment.clone())],
        };
        self.cuts.push(Cut { at: self.ops.len(), taken, end: end.clone(), phi });
        self.segment = end;
        self.fvalues.insert((id, 0), dest);
        Ok(())
    }

    /// `AArch64FrameLowering::eliminateCallFramePseudoInstr` where the call
    /// frame is not reserved: `sub sp, sp, #n` before the call's stores,
    /// `add sp, sp, #n` after it, `n` the bytes aligned to 16
    /// (`emitFrameOffset`: 12 bits, then the next 12 shifted).
    fn adjust_stack(
        &mut self,
        bytes: u64,
        subtract: bool,
    ) {
        let mut left = bytes.next_multiple_of(16);
        for (shift12, unit) in [(true, 1u64 << 12), (false, 1)] {
            let chunk = if shift12 { (left / unit) & 0xfff } else { left & 0xfff };
            left -= chunk * unit;
            if chunk > 0 {
                self.ops.push(Op::I(Inst::AddSubImm {
                    op: AddSub { subtract, set_flags: false },
                    width: Width::X,
                    rd: Gpr::R31,
                    rn: Gpr::R31,
                    imm12: chunk as u16,
                    shift12,
                }));
            }
        }
    }

    fn node(
        &mut self,
        id: u32,
    ) -> Result<(), String> {
        let node = self.dag.node(SDValue { node: id, res: 0 });
        match node.opcode {
            // Lifetime markers make no code; stack coloring would read them.
            ISD::EntryToken | ISD::TokenFactor | ISD::Register | ISD::LifetimeStart | ISD::LifetimeEnd => Ok(()),
            ISD::CopyFromReg
                if matches!(
                    self.dag.node(node.ops[1]).data,
                    NodeData::PhysReg(id) if id == crate::reg::SP
                ) =>
            {
                let rd = self.mf.gpr();
                self.ops.push(Op::I(Inst::AddSubImm {
                    op: AddSub { subtract: false, set_flags: false },
                    width: Width::X,
                    rd,
                    rn: Gpr::R31,
                    imm12: 0,
                    shift12: false,
                }));
                self.sp_copies.insert(rd);
                self.values.insert((id, 0), rd);
                Ok(())
            }
            ISD::CopyFromReg if in_float_file(node.values[0]) => {
                let value = match self.dag.node(node.ops[1]).data {
                    NodeData::PhysReg(id) => {
                        let (dst, src) = (self.mf.fpr(), fpr_of(id)?);
                        self.ops.push(Op::Copy { dst: Reg::Fpr(dst), src: Reg::Fpr(src) });
                        if is_quad(node.values[0]) {
                            self.mf.wide.push(Reg::Fpr(dst));
                        }
                        dst
                    }
                    NodeData::Register(number) => {
                        let one = self.fvreg(number);
                        if is_quad(node.values[0]) {
                            self.mf.wide.push(Reg::Fpr(one));
                        }
                        one
                    }
                    other => return Err(format!("a register operand that is {other:?}")),
                };
                self.fvalues.insert((id, 0), value);
                Ok(())
            }
            ISD::CopyFromReg => {
                let (reg, physical) = self.register_operand(node.ops[1])?;
                let value = if physical && reg == Gpr::R31 {
                    reg
                } else if physical {
                    let dst = self.mf.gpr();
                    self.ops.push(Op::Copy { dst: Reg::Gpr(dst), src: Reg::Gpr(reg) });
                    dst
                } else {
                    reg
                };
                self.values.insert((id, 0), value);
                Ok(())
            }
            // The flags are not moved: the compare before the user set them.
            // The stack pointer is register 31 only in an add: `mov x, sp` and
            // `mov sp, x` are `add #0`, and a body that writes it has a
            // frame record (`dynamic`).
            ISD::CopyToReg
                if matches!(
                    self.dag.node(node.ops[1]).data,
                    NodeData::PhysReg(id) if id == crate::reg::SP
                ) =>
            {
                let Operand::Reg(src) = self.operand(node.ops[2])? else { return Err("a copy of a constant".into()) };
                self.mf.dynamic = true;
                self.ops.push(Op::I(Inst::AddSubImm {
                    op: AddSub { subtract: false, set_flags: false },
                    width: Width::X,
                    rd: Gpr::R31,
                    rn: src,
                    imm12: 0,
                    shift12: false,
                }));
                Ok(())
            }
            ISD::CopyToReg if self.is_flags(node.ops[1]) => Ok(()),
            ISD::CopyToReg if in_float_file(self.dag.value_type(node.ops[2])) => {
                let dst = match self.dag.node(node.ops[1]).data {
                    NodeData::PhysReg(id) => fpr_of(id)?,
                    NodeData::Register(number) => self.fvreg(number),
                    other => return Err(format!("a register operand that is {other:?}")),
                };
                // `EmitCopyToRegNode`: the copy of an undefined value is an
                // `IMPLICIT_DEF` of the register, defined as zero here as the
                // integer copy's is.
                if matches!(
                    self.dag.opcode(node.ops[2]),
                    ISD::Machine(code) if opcode_name(code) == Some("IMPLICIT_DEF")
                ) {
                    let size = match self.dag.value_type(node.ops[2]) {
                        EVT::Simple(MVT::F32) => FpSize::S,
                        EVT::Simple(MVT::F64) => FpSize::D,
                        other => return Err(format!("a copy of an undefined {other:?}")),
                    };
                    let width = if size == FpSize::D { Width::X } else { Width::W };
                    self.ops.push(Op::I(Inst::FpInt {
                        op: FpConvert::Bits { to_fp: true },
                        width,
                        size,
                        gpr: Gpr::R31,
                        fpr: dst,
                    }));
                    return Ok(());
                }
                let Operand::Fpr(src) = self.operand(node.ops[2])? else {
                    return Err("a float copy of a constant".into());
                };
                if is_quad(self.dag.value_type(node.ops[2])) {
                    self.mf.wide.extend([Reg::Fpr(dst), Reg::Fpr(src)]);
                }
                self.ops.push(Op::Copy { dst: Reg::Fpr(dst), src: Reg::Fpr(src) });
                Ok(())
            }
            // `EmitCopyToRegNode`: the copy of an undefined value is an
            // `IMPLICIT_DEF` of the register. It is defined as zero here (an
            // undefined value is any value) until the MIR has an undef.
            ISD::CopyToReg
                if matches!(
                    self.dag.opcode(node.ops[2]),
                    ISD::Machine(code) if opcode_name(code) == Some("IMPLICIT_DEF")
                ) =>
            {
                let (dst, _) = self.register_operand(node.ops[1])?;
                self.ops.push(Op::Imm { dst, bits: 32, value: 0 });
                Ok(())
            }
            ISD::CopyToReg => {
                let (dst, _) = self.register_operand(node.ops[1])?;
                match self.operand(node.ops[2])? {
                    Operand::Reg(src) => self.ops.push(Op::Copy { dst: Reg::Gpr(dst), src: Reg::Gpr(src) }),
                    // An integer the pattern left in a float register (the
                    // lane an `addv` reduced into): `fmov w, s`, the pin's
                    // cross-class `COPY`.
                    Operand::Fpr(src) => {
                        let (width, size) = if self.dag.value_type(node.ops[2]).size_in_bits() == 64 {
                            (Width::X, FpSize::D)
                        } else {
                            (Width::W, FpSize::S)
                        };
                        self.ops.push(Op::I(Inst::FpInt {
                            op: FpConvert::Bits { to_fp: false },
                            width,
                            size,
                            gpr: dst,
                            fpr: src,
                        }));
                    }
                    _ => return Err("a copy of a constant".into()),
                }
                Ok(())
            }
            ISD::Machine(code) => {
                let name = opcode_name(code).ok_or_else(|| format!("a machine node with no name (opcode {code})"))?;
                if name == "RET_ReallyLR" {
                    let uses = node
                        .ops
                        .iter()
                        .filter_map(|op| match self.dag.node(*op).data {
                            NodeData::PhysReg(id) => Some(reg_of(id)),
                            _ => None,
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    self.ops.push(Op::Return { uses, tail: None });
                    return Ok(());
                }
                match name {
                    // The outgoing area is laid out by the frame; the pseudos
                    // only size it.
                    "ADJCALLSTACKDOWN" => {
                        let Some(bytes) = node.ops.first().and_then(|op| self.dag.constant(*op)) else {
                            return Err("a call sequence of no size".into());
                        };
                        if self.moves_sp {
                            self.adjust_stack(bytes as u64, true);
                        } else {
                            self.mf.outgoing = self.mf.outgoing.max(bytes as u64);
                        }
                        return Ok(());
                    }
                    "ADJCALLSTACKUP" => {
                        if self.moves_sp
                            && let Some(bytes) = node.ops.first().and_then(|op| self.dag.constant(*op))
                        {
                            self.adjust_stack(bytes as u64, false);
                        }
                        return Ok(());
                    }
                    "LLRM_MEMCPY_Q" | "LLRM_MEMMOVE_Q" | "LLRM_MEMSET_Q" => return self.block_memory(id, name),
                    "LLRM_SAVE_Q" => return self.save_q(id),
                    // The jump through a table is one terminator, made at its
                    // `JumpTableDest32`: the debug info and the `BR` are no
                    // code.
                    "JUMP_TABLE_DEBUG_INFO" => return Ok(()),
                    "BR" if node.ops.iter().any(|&op| {
                        matches!(
                            self.dag.opcode(op),
                            ISD::Machine(code) if opcode_name(code) == Some("JumpTableDest32")
                        )
                    }) =>
                    {
                        return Ok(());
                    }
                    "BL" | "BLR" => return self.call(id, name),
                    "TCRETURNdi" | "TCRETURNri" | "TCRETURNriBTI" => return self.tail_call(id, name),
                    _ => {}
                }
                let mut operands = node
                    .ops
                    .iter()
                    .filter(|op| {
                        let vt = self.dag.value_type(**op);
                        // A block is an operand of type `Other`, a chain is
                        // not.
                        (vt != EVT::OTHER || self.dag.opcode(**op) == ISD::BasicBlock) && vt != EVT::GLUE
                    })
                    .map(|&op| self.operand(op))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|why| format!("{name}: {why}"))?;
                self.constrain_register_classes(code, node, &mut operands);
                if let Some(size) = name
                    .strip_prefix("FCMPE")
                    .or_else(|| name.strip_prefix("FCMP"))
                    .and_then(|rest| rest.get(..1))
                    .and_then(fp_size)
                {
                    let compare = match operands.as_slice() {
                        [Operand::Fpr(rn), Operand::Fpr(rm)] => {
                            Inst::FpCompare { signalling: name.starts_with("FCMPE"), size, rn: *rn, rm: Some(*rm) }
                        }
                        [Operand::Fpr(rn), ..] => {
                            Inst::FpCompare { signalling: name.starts_with("FCMPE"), size, rn: *rn, rm: None }
                        }
                        _ => return Err(format!("{name}: operands are not those of its form")),
                    };
                    self.ops.push(Op::I(compare));
                    return Ok(());
                }
                // The entry's address is read and jumped to.
                if name == "JumpTableDest32" {
                    let [Operand::Reg(table), Operand::Reg(index), Operand::Table(number)] = operands.as_slice() else {
                        return Err("JumpTableDest32: operands are not those of its form".into());
                    };
                    let blocks = self.tables.get(*number as usize).ok_or("a jump table the builder did not make")?;
                    let targets = blocks
                        .iter()
                        .map(|&block| match self.label(block)? {
                            Target::Label(label) => Ok(label),
                            _ => Err("a block without a label".to_owned()),
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    let (dst, scratch) = (self.mf.gpr(), self.mf.gpr());
                    self.ops.push(Op::JumpTable { dst, scratch, table: *table, index: *index, id: *number, targets });
                    return Ok(());
                }
                if name == "COPY_TO_REGCLASS" || name == "COPY" {
                    return self.copy_to_class(id, &operands);
                }
                if name == "F128CSEL" {
                    return self.f128_select(id, &operands);
                }
                if float_inst(name, Vreg(0), Gpr(0), &operands).is_some() {
                    // A fill of a 64-bit vector (`MOVIv2i32`) has no type of
                    // ours: it reads as an `i64`.
                    let float_result = node.values[0].is_floating_point()
                        || node.values[0].is_vector()
                        || vector_fill(name, &operands).is_some()
                        || reduction(name).is_some();
                    let (rd, gd) = if float_result { (self.mf.fpr(), Gpr(0)) } else { (Vreg(0), self.mf.gpr()) };
                    let inst = float_inst(name, rd, gd, &operands).expect("a float row")?;
                    // `FMLA` adds into the register it reads: `rd` is a copy of
                    // the accumulator that the instruction then changes.
                    if let (
                        Inst::VectorFma { .. }
                        | Inst::VectorFmaLane { .. }
                        | Inst::VectorInsGpr { .. }
                        | Inst::VectorInsLane { .. },
                        Some(Operand::Fpr(acc)),
                    ) = (&inst, operands.first())
                    {
                        self.ops.push(Op::Copy { dst: Reg::Fpr(rd), src: Reg::Fpr(*acc) });
                    }
                    self.ops.push(Op::I(inst));
                    if float_result {
                        self.fvalues.insert((id, 0), rd);
                    } else {
                        self.values.insert((id, 0), gd);
                    }
                    return Ok(());
                }
                if name == "BRK" {
                    let [Operand::Imm(imm16)] = operands.as_slice() else {
                        return Err("a BRK without an immediate".into());
                    };
                    let imm16 = u16::try_from(*imm16).map_err(|_| format!("a BRK immediate {imm16} over 16 bits"))?;
                    self.ops.push(Op::I(Inst::Brk { imm16 }));
                    return Ok(());
                }
                if name == "DMB" {
                    let barrier = match operands.as_slice() {
                        [Operand::Imm(11)] => Barrier::Ish,
                        [Operand::Imm(9)] => Barrier::IshLd,
                        [Operand::Imm(10)] => Barrier::IshSt,
                        _ => return Err("a barrier that is not ish, ishld or ishst".into()),
                    };
                    self.ops.push(Op::I(Inst::Dmb { barrier }));
                    return Ok(());
                }
                if let Some((op, size, acquire, release)) = atomic_form(name) {
                    let reg = |operand: &Operand| match operand {
                        Operand::Reg(reg) => Ok(*reg),
                        _ => Err(format!("{name}: an atomic operand that is not a register")),
                    };
                    // `LDAR p`, `STLR v, p`, `SWP/LDOP v, p`, `CAS expected,
                    // new, p`.
                    let (rs, rt, rn, result) = match (op, operands.as_slice()) {
                        (Atomic::Ldar | Atomic::Ldapr, [pointer]) => (Gpr::R31, self.mf.gpr(), reg(pointer)?, true),
                        (Atomic::Stlr, [value, pointer]) => (Gpr::R31, reg(value)?, reg(pointer)?, false),
                        (Atomic::Cas, [expected, new, pointer]) => {
                            // `cas` overwrites the expected value with the old
                            // one.
                            let old = self.mf.gpr();
                            self.ops.push(Op::Copy { dst: Reg::Gpr(old), src: Reg::Gpr(reg(expected)?) });
                            (old, reg(new)?, reg(pointer)?, true)
                        }
                        (_, [value, pointer]) => (reg(value)?, self.mf.gpr(), reg(pointer)?, true),
                        _ => return Err(format!("{name}: operands are not those of its form")),
                    };
                    self.ops.push(Op::I(Inst::Atomic { op, size, acquire, release, rs, rt, rn }));
                    if result {
                        self.values.insert((id, 0), if op == Atomic::Cas { rs } else { rt });
                    }
                    return Ok(());
                }
                // `ST1i32`: a lane of a vector stored at the address in a
                // register.
                if let Some(lanes) = name.strip_prefix("ST1i").and_then(|bits| match bits {
                    "8" => Some(Arrangement::B16),
                    "16" => Some(Arrangement::H8),
                    "32" => Some(Arrangement::S4),
                    "64" => Some(Arrangement::D2),
                    _ => None,
                }) {
                    let [Operand::Fpr(rt), Operand::Imm(index), Operand::Reg(base)] = operands.as_slice() else {
                        return Err(format!("{name}: operands are not those of its form"));
                    };
                    self.ops.push(Op::I(Inst::VectorStoreLane { lanes, rt: *rt, index: *index as u8, base: *base }));
                    return Ok(());
                }
                if let Some(access) = memory_form(name) {
                    return self.memory(id, name, &access, &operands);
                }
                if let Some(branch) = self.branch(name, &operands)? {
                    self.ops.push(Op::I(branch));
                    return Ok(());
                }
                // A 32-bit write clears the upper half, so a sub-register is
                // its register.
                match (name, operands.as_slice()) {
                    ("EXTRACT_SUBREG", [Operand::Reg(rn), _]) => {
                        self.values.insert((id, 0), *rn);
                        return Ok(());
                    }
                    // A scalar write to a vector register zeroes the rest of
                    // it.
                    ("SUBREG_TO_REG", [.., Operand::Fpr(rn), _]) => {
                        self.fvalues.insert((id, 0), *rn);
                        return Ok(());
                    }
                    // The low lane of a vector register is the scalar one.
                    ("EXTRACT_SUBREG", [Operand::Fpr(rn), _]) => {
                        self.fvalues.insert((id, 0), *rn);
                        return Ok(());
                    }
                    ("SUBREG_TO_REG", [.., Operand::Reg(rn), _]) => {
                        let dst = self.mf.gpr();
                        self.ops.push(Op::Copy { dst: Reg::Gpr(dst), src: Reg::Gpr(*rn) });
                        self.values.insert((id, 0), dst);
                        return Ok(());
                    }
                    // Into an undefined register there is no upper half to
                    // keep.
                    // A general register into the low lane of a vector one:
                    // `fmov d, x` (or `s, w`) into the undefined register.
                    ("INSERT_SUBREG", [_, Operand::Reg(rn), _])
                        if node.values[0].is_vector()
                            && matches!(
                                self.dag.opcode(node.ops[0]),
                                ISD::Machine(code) if opcode_name(code) == Some("IMPLICIT_DEF")
                            ) =>
                    {
                        let (width, size) = match self.dag.value_type(node.ops[1]).size_in_bits() {
                            64 => (Width::X, FpSize::D),
                            _ => (Width::W, FpSize::S),
                        };
                        let fpr = self.mf.fpr();
                        self.ops.push(Op::I(Inst::FpInt {
                            op: FpConvert::Bits { to_fp: true },
                            width,
                            size,
                            gpr: *rn,
                            fpr,
                        }));
                        self.fvalues.insert((id, 0), fpr);
                        return Ok(());
                    }
                    ("INSERT_SUBREG", [_, Operand::Reg(rn), _])
                        if matches!(
                            self.dag.opcode(node.ops[0]),
                            ISD::Machine(code) if opcode_name(code) == Some("IMPLICIT_DEF")
                        ) =>
                    {
                        self.values.insert((id, 0), *rn);
                        return Ok(());
                    }
                    // The same for a scalar float register into a vector one.
                    ("INSERT_SUBREG", [_, Operand::Fpr(rn), _])
                        if matches!(
                            self.dag.opcode(node.ops[0]),
                            ISD::Machine(code) if opcode_name(code) == Some("IMPLICIT_DEF")
                        ) =>
                    {
                        self.fvalues.insert((id, 0), *rn);
                        return Ok(());
                    }
                    // `BFMWri dst, src, immr, imms` inserts into a register it
                    // reads: the result is a copy of `dst` that the move then
                    // changes.
                    (
                        "BFMWri" | "BFMXri",
                        [Operand::Reg(old), Operand::Reg(rn), Operand::Imm(immr), Operand::Imm(imms)],
                    ) => {
                        let rd = self.mf.gpr();
                        self.ops.push(Op::Copy { dst: Reg::Gpr(rd), src: Reg::Gpr(*old) });
                        let width = if name == "BFMWri" { Width::W } else { Width::X };
                        self.ops.push(Op::I(Inst::BitfieldMove {
                            op: Bitfield::Bfm,
                            width,
                            rd,
                            rn: *rn,
                            immr: *immr as u8,
                            imms: *imms as u8,
                        }));
                        self.values.insert((id, 0), rd);
                        return Ok(());
                    }
                    ("IMPLICIT_DEF", []) => {
                        let dst = self.mf.gpr();
                        self.values.insert((id, 0), dst);
                        return Ok(());
                    }
                    _ => {}
                }
                // A flag-setting instruction defines a register even where
                // nothing reads it, as the pin's selection has
                // it (`$wzr` and the plain opcode come later, in `prepass`:
                // `AArch64DeadRegisterDefinitions`, `optimizeCompareInstr`), so
                // that equal ones merge.
                let rd = self.mf.gpr();
                if let Some((op, width)) = cond_select(name) {
                    // The flags arrive as a glued copy to NZCV, or as a value.
                    let [Operand::Reg(rn), Operand::Reg(rm), Operand::Imm(cond), ..] = operands.as_slice() else {
                        return Err(format!("{name}: operands are not those of a conditional select"));
                    };
                    self.ops.push(Op::I(Inst::CondSelect { op, width, rd, rn: *rn, rm: *rm, cond: cond_of(*cond)? }));
                    self.values.insert((id, 0), rd);
                    return Ok(());
                }
                if let Some((op, width)) = add_carry(name) {
                    // The carry arrives as the flags the instruction before
                    // wrote.
                    let [Operand::Reg(rn), Operand::Reg(rm), ..] = operands.as_slice() else {
                        return Err(format!("{name}: operands are not those of an add with carry"));
                    };
                    self.ops.push(Op::I(Inst::AddCarry { op, width, rd, rn: *rn, rm: *rm }));
                    self.values.insert((id, 0), rd);
                    return Ok(());
                }
                if let ("MOVi32imm" | "MOVi64imm", [Operand::Imm(value)]) = (name, operands.as_slice()) {
                    let bits = if name == "MOVi32imm" { 32 } else { 64 };
                    self.ops.push(Op::Imm { dst: rd, bits, value: *value });
                    self.values.insert((id, 0), rd);
                    return Ok(());
                }
                // `ADDXri` of a frame index is the object's address.
                if let (Some(object), "ADDXri", Some(Operand::Imm(offset))) = (operands.first(), name, operands.get(1))
                {
                    let (slot, base) = match object {
                        Operand::Slot(slot) => (Some(*slot), 0),
                        Operand::Fixed(at) => (Some(INCOMING_ARGUMENTS), *at),
                        _ => (None, 0),
                    };
                    if let Some(slot) = slot {
                        self.ops.push(Op::FrameAddress { dst: rd, slot, offset: base + *offset as i64 });
                        self.values.insert((id, 0), rd);
                        return Ok(());
                    }
                }
                match (name, operands.as_slice()) {
                    ("ADRP", [symbol @ (Operand::Symbol(..) | Operand::Pool(_))]) => {
                        let (target, _) = self.symbol_target(symbol)?;
                        self.ops.push(Op::I(Inst::Adr { page: true, rd, target, got: false }));
                    }
                    // `ADRP` and `ADDXri` of a jump table's data object.
                    ("MOVaddrJT", [Operand::Table(number), ..]) => {
                        let target =
                            Target::Symbol { name: crate::jumptable::symbol(self.function, *number), addend: 0 };
                        let page = self.mf.gpr();
                        self.ops.push(Op::I(Inst::Adr { page: true, rd: page, target: target.clone(), got: false }));
                        self.ops.push(Op::I(Inst::AddPageOffset { rd, rn: page, target }));
                    }
                    // `ADDXri page, :lo12:symbol, 0`
                    ("ADDXri", [Operand::Reg(rn), symbol @ (Operand::Symbol(..) | Operand::Pool(_)), _]) => {
                        let (target, _) = self.symbol_target(symbol)?;
                        self.ops.push(Op::I(Inst::AddPageOffset { rd, rn: *rn, target }));
                    }
                    // `ADRP` and `ADDXri` of one symbol.
                    ("MOVaddr" | "MOVaddrCP", [symbol @ (Operand::Symbol(..) | Operand::Pool(_)), _]) => {
                        let (target, _) = self.symbol_target(symbol)?;
                        let page = self.mf.gpr();
                        self.ops.push(Op::I(Inst::Adr { page: true, rd: page, target: target.clone(), got: false }));
                        self.ops.push(Op::I(Inst::AddPageOffset { rd, rn: page, target }));
                    }
                    // `ADRP` of the GOT slot and the load from it.
                    ("LOADgot", [Operand::Symbol(global, _)]) => {
                        let (target, _) = self.target(*global, 0)?;
                        let page = self.mf.gpr();
                        self.ops.push(Op::I(Inst::Adr { page: true, rd: page, target: target.clone(), got: true }));
                        self.ops.push(Op::I(Inst::Load {
                            mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: rd },
                            addr: Addr::PageOffset { base: page, target, got: true },
                            mmo: MemOperand::default(),
                        }));
                    }
                    _ => {
                        // `GPR64sp`/`GPR32sp`: register number 31 in these
                        // operands is
                        // the stack pointer, so a zero base is a register
                        // holding zero.
                        let width = add_sub(name)
                            .filter(|&(.., immediate)| immediate)
                            .map(|(_, width, _)| width)
                            .or_else(|| add_sub_extended(name).map(|(_, width)| width));
                        if let (Some(width), Some(Operand::Reg(rn))) = (width, operands.first_mut())
                            && *rn == Gpr::R31
                        {
                            let zero = self.mf.gpr();
                            let bits = if width == Width::X { 64 } else { 32 };
                            self.ops.push(Op::Imm { dst: zero, bits, value: 0 });
                            *rn = zero;
                        }
                        self.ops.push(Op::I(build(name, rd, &operands)?));
                    }
                }
                self.values.insert((id, 0), rd);
                Ok(())
            }
            other => Err(format!("node {other:?} is not selected")),
        }
    }
}

/// `blocks` of function `name` as a machine function, one block of ops each.
/// `blocks` of function `name` as a machine function, one block of ops each;
/// with `lines`, each op carries the source place of the IR instruction its
/// node was made for (`SDLoc` to `DebugLoc`).
pub fn emit(
    name: &str,
    blocks: &[Selected],
    lines: bool,
) -> Result<MFunc, String> {
    let first = blocks.first().ok_or("a function with no block")?;
    let mut emitter = Emitter {
        dag: &first.dag,
        globals: &first.globals,
        function: name,
        layout: &first.layout,
        tables: &first.tables,
        mf: MFunc::new(name),
        values: HashMap::default(),
        vregs: HashMap::default(),
        fvregs: HashMap::default(),
        fvalues: HashMap::default(),
        ops: Vec::new(),
        pool: std::cell::RefCell::new(Vec::new()),
        segment: String::new(),
        cuts: Vec::new(),
        splits: 0,
        sp_copies: rustc_hash::FxHashSet::default(),
        incoming: HashMap::default(),
        moves_sp: blocks
            .iter()
            .any(
                |block| block
                    .units
                    .iter()
                    .flat_map(|unit| &unit.nodes)
                    .any(
                        |&node| {
                            let node = block.dag.node(SDValue { node, res: 0 });
                            node.opcode == ISD::CopyToReg
                                && matches!(
                                    block.dag.node(node.ops[1]).data,
                                    NodeData::PhysReg(id) if id == crate::reg::SP
                                )
                        },
                    ),
            ),
    };
    for &(size, align) in &first.frame {
        emitter.mf.slot(size, align);
    }
    emitter.mf.declared.clone_from(&first.declared);
    // The blocks `F128CSEL` splits make, after the selector's; and where each
    // split block now ends.
    let mut joined: Vec<MBlock> = Vec::new();
    let mut tails: HashMap<String, String> = HashMap::default();
    let mut phi_counts: Vec<usize> = Vec::new();
    for (at, block) in blocks.iter().enumerate() {
        emitter.dag = &block.dag;
        emitter.values.clear();
        emitter.fvalues.clear();
        emitter.segment = block_label(name, at);
        for &(reg, vreg) in &block.live_ins {
            let dst = if is_fpr(reg) { Reg::Fpr(emitter.fvreg(vreg)) } else { Reg::Gpr(emitter.vreg(vreg)) };
            emitter.ops.push(Op::Copy { dst, src: reg_of(reg)? });
        }
        // `PHI`: the value each predecessor that branches here gives it; the
        // phis come first in the block, as the pin's ISel puts them.
        let mut phis: Vec<Op> = Vec::new();
        let mut headed: Vec<u32> = Vec::new();
        for phi in blocks.iter().flat_map(|one| one.phis.iter()).filter(|phi| Some(&phi.to) == block.layout.get(at)) {
            if headed.contains(&phi.dst) {
                continue;
            }
            headed.push(phi.dst);
            let (dst, mut incoming) = (phi_register(&mut emitter, phi.dst, phi.float, phi.quad), Vec::new());
            for (from, pred) in blocks.iter().enumerate() {
                if !pred.successors.contains(&phi.to) {
                    continue;
                }
                for one in pred.phis.iter().filter(|one| one.to == phi.to && one.dst == phi.dst) {
                    let value = phi_register(&mut emitter, one.src, one.float, one.quad);
                    if !incoming.iter().any(|(_, label): &(Reg, String)| *label == block_label(name, from)) {
                        incoming.push((value, block_label(name, from)));
                    }
                }
            }
            phis.push(Op::Phi { dst, incoming });
        }
        phi_counts.push(phis.len());
        emitter.ops.splice(0..0, phis);
        let mut places: Vec<Option<SrcLoc>> = Vec::new();
        // The notes of nothing: where they stand does not matter.
        for &(variable, _) in block.dbg_values.iter().filter(|(_, value)| value.is_none()) {
            emitter.ops.push(Op::DbgValue { variable, reg: None });
        }
        for unit in &block.units {
            for &node in unit.nodes.iter().rev() {
                places.resize(emitter.ops.len(), None);
                emitter.node(node)?;
                // `InstrEmitter::EmitDbgValue`: after the node whose value a
                // `llvm.dbg.value` names.
                for &(variable, value) in
                    block.dbg_values.iter().filter(|(_, value)| value.is_some_and(|v| v.node == node))
                {
                    let value = value.expect("filtered to a value");
                    let reg = emitter.values.get(&(value.node, value.res)).map(|&gpr| Reg::Gpr(gpr));
                    emitter.ops.push(Op::DbgValue { variable, reg });
                }
                if lines {
                    let order = block.dag.node(SDValue { node, res: 0 }).order as usize;
                    let place = block.dag.lines.get(order).copied().flatten();
                    places.resize(emitter.ops.len(), place);
                }
            }
        }
        let ops = std::mem::take(&mut emitter.ops);
        places.resize(ops.len(), None);
        let mut pairs: Vec<(Op, Option<SrcLoc>)> = ops.into_iter().zip(places).collect();
        let cuts = std::mem::take(&mut emitter.cuts);
        if cuts.is_empty() {
            let ops: OpList = pairs.into_iter().collect();
            emitter.mf.blocks.push(MBlock { label: block_label(name, at), ops });
            continue;
        }
        // The head keeps the selector's label; each join follows its empty
        // block, after the function's.
        let mut head: Vec<(Op, Option<SrcLoc>)> = pairs.drain(..cuts[0].at).collect();
        let mut rest = pairs;
        let mut consumed = cuts[0].at;
        let mut last = block_label(name, at);
        for (k, cut) in cuts.iter().enumerate() {
            let next = cuts.get(k + 1).map_or(consumed + rest.len(), |next| next.at);
            let mut segment: Vec<(Op, Option<SrcLoc>)> = vec![(cut.phi.clone(), None)];
            segment.extend(rest.drain(..next - consumed));
            consumed = next;
            joined.push(MBlock { label: cut.taken.clone(), ops: OpList::default() });
            if k + 1 == cuts.len() {
                // What fell through to the next block of the selector now has
                // to jump.
                if !matches!(
                    segment.last().map(|(op, _)| op),
                    Some(
                        Op::Return { .. }
                            | Op::JumpTable { .. }
                            | Op::I(Inst::Branch { link: false, .. } | Inst::Brk { .. })
                    )
                ) && at + 1 < blocks.len()
                {
                    segment.push((
                        Op::I(Inst::Branch { link: false, target: Target::Label(block_label(name, at + 1)) }),
                        None,
                    ));
                }
                last.clone_from(&cut.end);
            }
            joined.push(MBlock { label: cut.end.clone(), ops: segment.into_iter().collect() });
        }
        emitter
            .mf
            .blocks
            .push(MBlock { label: block_label(name, at), ops: std::mem::take(&mut head).into_iter().collect() });
        tails.insert(block_label(name, at), last);
    }
    // A phi of the selector's names the block its value comes from by the label
    // it had.
    for (block, &count) in emitter.mf.blocks.iter_mut().zip(&phi_counts) {
        for op in (&mut block.ops).into_iter().take(count) {
            if let Op::Phi { incoming, .. } = op {
                for (_, from) in incoming.iter_mut() {
                    if let Some(tail) = tails.get(from.as_str()) {
                        from.clone_from(tail);
                    }
                }
            }
        }
    }
    emitter.mf.blocks.extend(joined);
    emitter.mf.pool = emitter.pool.take();
    emitter.mf.block_ids =
        emitter.mf.blocks.iter().zip(&first.layout).map(|(block, &id)| (block.label.clone(), id)).collect();
    emitter.mf.edge_probs = first.edge_probs.clone();
    Ok(emitter.mf)
}
