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
//! DEVIATE: the float arithmetic, compare, select, conversion, move and `LDR`/`STR` `ui` rows, and the integer
//! `ADD`/`SUB`/logic, variable shift, `SBFM`/`UBFM`, `MADD`, divide and
//! one-source forms, `MOVi32imm`/`MOVi64imm`, a frame address, the integer `LDR`/`STR` (`ui`, `UR`, `roX`, `roW`)
//! forms and the sub-register pseudos only (no `BFM`, no insert into a defined register), with no
//! memory operand kept (`MemOperand::default`). Reason: each family adds rows
//! to [`build`] with an encoder byte check. Exit: C2..Cn.

use std::collections::HashMap;

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{NodeData, SDValue, SelectionDAG};
use llrm_dagisel::generated::aarch64::aarch64_apple_m1::MACHINE_OPCODES;
use llrm_lir::registers::RegId;

use super::function::Selected;
use super::globals::Globals;
use crate::REGISTER_INFO;
use crate::mc::immediate::decode_logical;
use crate::mc::inst::{
    FpBinary, FpConvert, FpFused, FpSize, FpUnary, Rounding, Vreg, Addr, AddSub, Bitfield, Cond, Div, Extend, Gpr, Inst, Logic, Memory, MemOperand, Mul3, Select, Shift, Target, Unary,
    Width,
};
use crate::mc::visit::Reg;
use crate::mfunc::{Callee, INCOMING_ARGUMENTS, MBlock, MFunc, Op, block_label};

/// The machine opcode's name.
fn opcode_name(code: u32) -> Option<&'static str> {
    MACHINE_OPCODES.iter().find(|(v, _)| u32::from(*v) == code).map(|(_, name)| *name)
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
    /// A block by id, as a branch names it.
    Block(u32),
    /// An incoming argument at this offset into the caller's outgoing area.
    Fixed(i64),
    /// The flags a compare set: the instruction before this one wrote them.
    Flags,
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

/// `ANDWri`, `ORNXrs`, `BICSWrs` and the rest of the logic family: (operation, inverted
/// operand, width, form).
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
}

/// `LDRWui`, `STURXi`, `LDRSBWroX` and the rest of the integer load/store family.
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
    } else {
        match rest.strip_suffix("roX") {
            Some(element) => (element, AddressForm::Register),
            None => (rest.strip_suffix("roW")?, AddressForm::RegisterWord),
        }
    };
    let (width, bytes, sign) = match element {
        // `LDRSui` and `LDRDui` move a float
        "S" | "D" => (Width::W, if element == "S" { 4 } else { 8 }, false),
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
    Some(MemoryForm { width, bytes, sign, store, form, fp: matches!(element, "S" | "D") })
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

/// The `AArch64CC` number as the condition field.
fn cond_of(code: u64) -> Result<Cond, String> {
    use Cond::*;
    const ALL: [Cond; 16] = [Eq, Ne, Hs, Lo, Mi, Pl, Vs, Vc, Hi, Ls, Ge, Lt, Gt, Le, Al, Nv];
    ALL.get(code as usize).copied().ok_or_else(|| format!("condition code {code}"))
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
            // `ADDWrs rn, rm, shifter`: the shift type in bits 6.., the amount below
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
    let long = ["SMADDL", "SMSUBL", "UMADDL", "UMSUBL"].iter().find_map(|stem| name.strip_prefix(stem).map(|rest| (*stem, rest)));
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

/// A float machine node as an instruction: the float arithmetic, compare, select,
/// conversion and move rows. `None` for any other opcode. The destination is a float
/// register, or a general one for a conversion to an integer.
fn float_inst(
    name: &str,
    rd: Vreg,
    gd: Gpr,
    operands: &[Operand],
) -> Option<Result<Inst, String>> {
    let wrong = || Err(format!("{name}: operands are not those of its form"));
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
    let fused = [("FMADD", FpFused::Madd), ("FMSUB", FpFused::Msub), ("FNMADD", FpFused::Nmadd), ("FNMSUB", FpFused::Nmsub)];
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
    // `SCVTFUWDri`: unscaled, from a `W`, to a `D`.
    for (stem, signed) in [("SCVTFU", true), ("UCVTFU", false)] {
        if let Some(rest) = name.strip_prefix(stem).and_then(|rest| rest.strip_suffix("ri")) {
            let (width, size) = (gpr_width(&rest[..1])?, fp_size(rest.get(1..)?)?);
            return Some(match operands {
                [Operand::Reg(gpr)] => Ok(Inst::FpInt { op: FpConvert::ToFp { signed }, width, size, gpr: *gpr, fpr: rd }),
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

struct Emitter<'a> {
    dag: &'a SelectionDAG,
    globals: &'a Globals,
    function: &'a str,
    layout: &'a [u32],
    /// Blocks made for critical edges so far: their labels are numbered.
    edges_made: usize,
    mf: MFunc,
    /// The virtual register each value is in.
    values: HashMap<(u32, u8), Gpr>,
    /// The machine register each of the builder's virtual registers became.
    vregs: HashMap<u32, Gpr>,
    /// The same for the float registers, and the values in them.
    fvregs: HashMap<u32, Vreg>,
    fvalues: HashMap<(u32, u8), Vreg>,
    ops: Vec<Op>,
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

    fn operand(
        &self,
        value: SDValue,
    ) -> Result<Operand, String> {
        let node = self.dag.node(value);
        if value.res == 1 && matches!(node.opcode, ISD::Machine(_)) {
            return Ok(Operand::Flags);
        }
        // `FCMP` has no result but the flags.
        if matches!(node.opcode, ISD::Machine(code) if opcode_name(code).is_some_and(|name| name.starts_with("FCMP"))) {
            return Ok(Operand::Flags);
        }
        match node.data {
            NodeData::Const { bits, .. } if matches!(node.opcode, ISD::TargetConstant | ISD::Constant) => {
                Ok(Operand::Imm(bits as u64))
            }
            NodeData::Block(id) => Ok(Operand::Block(id)),
            // The zero register as an operand of `CSINC` and the like.
            NodeData::PhysReg(id) if gpr_of(id).is_ok_and(|g| g == Gpr::R31) => Ok(Operand::Reg(Gpr::R31)),
            NodeData::Global { symbol, offset } if node.opcode == ISD::TargetGlobalAddress => {
                Ok(Operand::Symbol(symbol, offset))
            }
            NodeData::FrameIndex(index) if node.opcode == ISD::TargetFrameIndex && index < 0 => {
                self.dag.fixed_offsets.get(&index).map(|&offset| Operand::Fixed(offset)).ok_or_else(|| format!("fixed object {index} has no offset"))
            }
            NodeData::FrameIndex(index) if node.opcode == ISD::TargetFrameIndex => Ok(Operand::Slot(index as u32)),
            _ => self
                .values
                .get(&(value.node, value.res))
                .map(|&g| Operand::Reg(g))
                .or_else(|| self.fvalues.get(&(value.node, value.res)).map(|&f| Operand::Fpr(f)))
                .ok_or_else(|| format!("operand {:?} has no register", node.opcode)),
        }
    }

    /// `BL symbol` or `BLR reg`: the registers it reads are the `Register` operands
    /// (the arguments), those it writes the physical registers a glued `CopyFromReg`
    /// after its `CALLSEQ_END` reads (the results).
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
        // The glue out of the call goes to `CALLSEQ_END`, and from it to the result copies.
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
    /// `x16`/`x17` itself.
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

    /// `COPY_TO_REGCLASS`: a copy, across the general and float classes by `fmov`.
    fn copy_to_class(
        &mut self,
        id: u32,
        operands: &[Operand],
    ) -> Result<(), String> {
        let to = self.dag.value_type(SDValue { node: id, res: 0 });
        let bits = to.size_in_bits();
        let (width, size) = if bits == 64 { (Width::X, FpSize::D) } else { (Width::W, FpSize::S) };
        match (operands.first(), to.is_floating_point()) {
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
        matches!(self.dag.node(register).data, NodeData::PhysReg(id) if id == llrm_codegen::backend::selectiondag::dagisel_adapter::EFLAGS_ID)
    }

    /// The copies a phi takes from the edges of `block` (the block at `at`), placed
    /// as phi elimination does: at the end of the block when one successor has
    /// phis, else in a block of its own on each edge that has them
    /// (a critical edge). Returns the edge blocks.
    fn phi_edges(
        &mut self,
        block: &Selected,
        ops: &mut Vec<Op>,
    ) -> Result<Vec<MBlock>, String> {
        let with_phis: Vec<u32> =
            block.successors.iter().copied().filter(|to| block.phis.iter().any(|p| p.to == *to)).collect();
        if with_phis.is_empty() {
            return Ok(Vec::new());
        }
        let is_branch = |op: &Op| {
            matches!(op, Op::I(Inst::Branch { .. } | Inst::BranchCond { .. } | Inst::CompareBranch { .. } | Inst::TestBranch { .. }))
        };
        let first_branch = ops.iter().rposition(|op| !is_branch(op)).map_or(0, |last| last + 1);
        let sequence = |this: &mut Self, to: u32| {
            let moves = block
                .phis
                .iter()
                .filter(|p| p.to == to)
                .map(|p| {
                    if p.float {
                        (Reg::Fpr(this.fvreg(p.dst)), Reg::Fpr(this.fvreg(p.src)))
                    } else {
                        (Reg::Gpr(this.vreg(p.dst)), Reg::Gpr(this.vreg(p.src)))
                    }
                })
                .filter(|(dst, src)| dst != src)
                .collect();
            let mf = &mut this.mf;
            crate::mfunc::sequence_copies(moves, |like| match like {
                Reg::Fpr(_) => Reg::Fpr(mf.fpr()),
                Reg::Gpr(_) => Reg::Gpr(mf.gpr()),
            })
        };
        if block.successors.len() == 1 {
            let copies = sequence(self, with_phis[0]);
            ops.splice(first_branch..first_branch, copies);
            return Ok(Vec::new());
        }
        let mut edges = Vec::new();
        for to in with_phis {
            let label = self.label(to)?;
            let Target::Label(label) = label else { unreachable!("a block label") };
            let edge = format!(".L{}.e{}", self.function, self.edges_made);
            self.edges_made += 1;
            let mut body: Vec<Op> = sequence(self, to);
            body.push(crate::mfunc::branch_to(&label));
            edges.push(MBlock { label: edge.clone(), ops: body });
            // Retarget the branches to `to`; a fall-through becomes a branch.
            let mut retargeted = false;
            for op in ops.iter_mut() {
                if let Op::I(Inst::Branch { target, .. } | Inst::BranchCond { target, .. } | Inst::CompareBranch { target, .. } | Inst::TestBranch { target, .. }) = op {
                    if *target == Target::Label(label.clone()) {
                        *target = Target::Label(edge.clone());
                        retargeted = true;
                    }
                }
            }
            if !retargeted {
                ops.push(crate::mfunc::branch_to(&edge));
            }
        }
        Ok(edges)
    }

    /// The label of block `id`.
    fn label(
        &self,
        id: u32,
    ) -> Result<Target, String> {
        let at = self.layout.iter().position(|&one| one == id).ok_or("a branch to a block outside the function")?;
        Ok(Target::Label(block_label(self.function, at)))
    }

    /// `B`, `Bcc`, `CBZW`..`CBNZX` and `TBZW`..`TBNZX`; `None` for any other opcode.
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
            (_, [Operand::Reg(rt), Operand::Imm(bit), Operand::Block(to)]) if name.starts_with("TB") && name.len() <= 6 => {
                let nonzero = name.starts_with("TBNZ");
                Inst::TestBranch { nonzero, rt: *rt, bit: *bit as u8, target: self.label(*to)? }
            }
            _ => return Ok(None),
        }))
    }

    /// The relocation target of global `id` at `addend`, and whether it is a GOT slot.
    fn target(
        &self,
        id: u32,
        addend: i64,
    ) -> Result<(Target, bool), String> {
        let global = self.globals.get(id).ok_or("a global the module does not have")?;
        Ok((Target::Symbol { name: global.name.clone(), addend }, global.got))
    }

    /// A load or store: operands are `[value,] base, offset...` without chains.
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
        let addr = match (access.form, address) {
            (AddressForm::Register, [Operand::Reg(base), Operand::Reg(index), Operand::Imm(_), Operand::Imm(shifted)]) => {
                Addr::Register { base: *base, index: *index, extend: Extend::Uxtx, shifted: *shifted != 0 }
            }
            (AddressForm::RegisterWord, [Operand::Reg(base), Operand::Reg(index), Operand::Imm(signed), Operand::Imm(shifted)]) => {
                let extend = if *signed != 0 { Extend::Sxtw } else { Extend::Uxtw };
                Addr::Register { base: *base, index: *index, extend, shifted: *shifted != 0 }
            }
            (AddressForm::Register | AddressForm::RegisterWord, _) => return Err(format!("{name}: a register offset that is not base and index")),
            (AddressForm::Scaled, [Operand::Reg(base), Operand::Symbol(id, offset)]) => {
                Addr::PageOffset { base: *base, target: self.target(*id, *offset)?.0, got: false }
            }
            (form, [base, Operand::Imm(imm)]) => {
                let imm = *imm as i64 * if form == AddressForm::Scaled { size } else { 1 };
                match base {
                    Operand::Reg(base) => Addr::Offset { base: *base, imm },
                    Operand::Slot(slot) => Addr::Slot { slot: *slot, offset: imm },
                    Operand::Fixed(at) => Addr::Slot { slot: INCOMING_ARGUMENTS, offset: at + imm },
                    _ => return Err(format!("{name}: a base that is neither register nor frame object")),
                }
            }
            _ => return Err(format!("{name}: operands are not those of its form")),
        };
        let mmo = MemOperand::default();
        if access.fp {
            let reg = match fvalue {
                Some(reg) => reg,
                None => self.mf.fpr(),
            };
            let mem = Memory::Fp { bytes: access.bytes, reg };
            self.ops.push(Op::I(if access.store { Inst::Store { mem, addr, mmo } } else { Inst::Load { mem, addr, mmo } }));
            if !access.store {
                self.fvalues.insert((id, 0), reg);
            }
            return Ok(());
        }
        let rd = if access.store { None } else { Some(self.mf.gpr()) };
        let mem = Memory::Int { width: access.width, bytes: access.bytes, sign: access.sign, reg: value.or(rd).expect("a value") };
        self.ops.push(Op::I(if access.store { Inst::Store { mem, addr, mmo } } else { Inst::Load { mem, addr, mmo } }));
        if let Some(rd) = rd {
            self.values.insert((id, 0), rd);
        }
        Ok(())
    }

    fn node(
        &mut self,
        id: u32,
    ) -> Result<(), String> {
        let node = self.dag.node(SDValue { node: id, res: 0 });
        match node.opcode {
            // Lifetime markers make no code; stack coloring would read them.
            ISD::EntryToken | ISD::TokenFactor | ISD::Register | ISD::LifetimeStart | ISD::LifetimeEnd => Ok(()),
            ISD::CopyFromReg if node.values[0].is_floating_point() => {
                let value = match self.dag.node(node.ops[1]).data {
                    NodeData::PhysReg(id) => {
                        let (dst, src) = (self.mf.fpr(), fpr_of(id)?);
                        self.ops.push(Op::Copy { dst: Reg::Fpr(dst), src: Reg::Fpr(src) });
                        dst
                    }
                    NodeData::Register(number) => self.fvreg(number),
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
            ISD::CopyToReg if self.is_flags(node.ops[1]) => Ok(()),
            ISD::CopyToReg if self.dag.value_type(node.ops[2]).is_floating_point() => {
                let dst = match self.dag.node(node.ops[1]).data {
                    NodeData::PhysReg(id) => fpr_of(id)?,
                    NodeData::Register(number) => self.fvreg(number),
                    other => return Err(format!("a register operand that is {other:?}")),
                };
                let Operand::Fpr(src) = self.operand(node.ops[2])? else { return Err("a float copy of a constant".into()) };
                self.ops.push(Op::Copy { dst: Reg::Fpr(dst), src: Reg::Fpr(src) });
                Ok(())
            }
            ISD::CopyToReg => {
                let (dst, _) = self.register_operand(node.ops[1])?;
                let Operand::Reg(src) = self.operand(node.ops[2])? else { return Err("a copy of a constant".into()) };
                self.ops.push(Op::Copy { dst: Reg::Gpr(dst), src: Reg::Gpr(src) });
                Ok(())
            }
            ISD::Machine(code) => {
                let name = opcode_name(code).ok_or("a machine node with no name")?;
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
                    // The outgoing area is laid out by the frame; the pseudos only size it.
                    "ADJCALLSTACKDOWN" => {
                        let Some(bytes) = node.ops.first().and_then(|op| self.dag.constant(*op)) else {
                            return Err("a call sequence of no size".into());
                        };
                        self.mf.outgoing = self.mf.outgoing.max(bytes as u64);
                        return Ok(());
                    }
                    "ADJCALLSTACKUP" => return Ok(()),
                    "BL" | "BLR" => return self.call(id, name),
                    "TCRETURNdi" | "TCRETURNri" | "TCRETURNriBTI" => return self.tail_call(id, name),
                    _ => {}
                }
                let operands = node
                    .ops
                    .iter()
                    .filter(|op| {
                        let vt = self.dag.value_type(**op);
                        // A block is an operand of type `Other`, a chain is not.
                        (vt != EVT::OTHER || self.dag.opcode(**op) == ISD::BasicBlock) && vt != EVT::GLUE
                    })
                    .map(|&op| self.operand(op))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|why| format!("{name}: {why}"))?;
                if let Some(size) = name.strip_prefix("FCMPE").or_else(|| name.strip_prefix("FCMP")).and_then(|rest| rest.get(..1)).and_then(fp_size) {
                    let compare = match operands.as_slice() {
                        [Operand::Fpr(rn), Operand::Fpr(rm)] => Inst::FpCompare { signalling: name.starts_with("FCMPE"), size, rn: *rn, rm: Some(*rm) },
                        [Operand::Fpr(rn), ..] => Inst::FpCompare { signalling: name.starts_with("FCMPE"), size, rn: *rn, rm: None },
                        _ => return Err(format!("{name}: operands are not those of its form")),
                    };
                    self.ops.push(Op::I(compare));
                    return Ok(());
                }
                if name == "COPY_TO_REGCLASS" {
                    return self.copy_to_class(id, &operands);
                }
                if float_inst(name, Vreg(0), Gpr(0), &operands).is_some() {
                    let float_result = node.values[0].is_floating_point();
                    let (rd, gd) = if float_result { (self.mf.fpr(), Gpr(0)) } else { (Vreg(0), self.mf.gpr()) };
                    let inst = float_inst(name, rd, gd, &operands).expect("a float row")?;
                    self.ops.push(Op::I(inst));
                    if float_result {
                        self.fvalues.insert((id, 0), rd);
                    } else {
                        self.values.insert((id, 0), gd);
                    }
                    return Ok(());
                }
                if let Some(access) = memory_form(name) {
                    return self.memory(id, name, &access, &operands);
                }
                if let Some(branch) = self.branch(name, &operands)? {
                    self.ops.push(Op::I(branch));
                    return Ok(());
                }
                // A 32-bit write clears the upper half, so a sub-register is its register.
                match (name, operands.as_slice()) {
                    ("EXTRACT_SUBREG", [Operand::Reg(rn), _]) => {
                        self.values.insert((id, 0), *rn);
                        return Ok(());
                    }
                    ("SUBREG_TO_REG", [.., Operand::Reg(rn), _]) => {
                        let dst = self.mf.gpr();
                        self.ops.push(Op::Copy { dst: Reg::Gpr(dst), src: Reg::Gpr(*rn) });
                        self.values.insert((id, 0), dst);
                        return Ok(());
                    }
                    // Into an undefined register there is no upper half to keep.
                    ("INSERT_SUBREG", [_, Operand::Reg(rn), _])
                        if matches!(self.dag.opcode(node.ops[0]), ISD::Machine(code) if opcode_name(code) == Some("IMPLICIT_DEF")) =>
                    {
                        self.values.insert((id, 0), *rn);
                        return Ok(());
                    }
                    ("IMPLICIT_DEF", []) => {
                        let dst = self.mf.gpr();
                        self.values.insert((id, 0), dst);
                        return Ok(());
                    }
                    _ => {}
                }
                let dead = self.dag.users(SDValue { node: id, res: 0 }).next().is_none();
                let sets_flags = ["ADDS", "SUBS", "ANDS", "BICS"].iter().any(|prefix| name.starts_with(prefix));
                // `optimizeCompareInstr` / `convertToNonFlagSettingOpc`: an add or sub whose flags nothing
                // reads (the pin selects `sub` as `SUBS*` with a dead `$nzcv`) is the plain instruction.
                // A dead value too is a compare (`cmp`), which keeps the flags.
                let flags_unread = (1..node.values.len() as u8).all(|res| self.dag.users(SDValue { node: id, res }).next().is_none());
                let plain;
                let name = if sets_flags && !dead && flags_unread && (name.starts_with("ADDS") || name.starts_with("SUBS")) {
                    plain = format!("{}{}", &name[..3], &name[4..]);
                    plain.as_str()
                } else {
                    name
                };
                // A compare writes the zero register: `cmp` is `subs xzr`.
                let rd = if sets_flags && dead { Gpr::R31 } else { self.mf.gpr() };
                if let Some((op, width)) = cond_select(name) {
                    // The flags arrive as a glued copy to NZCV, or as a value.
                    let [Operand::Reg(rn), Operand::Reg(rm), Operand::Imm(cond), ..] = operands.as_slice() else {
                        return Err(format!("{name}: operands are not those of a conditional select"));
                    };
                    self.ops.push(Op::I(Inst::CondSelect { op, width, rd, rn: *rn, rm: *rm, cond: cond_of(*cond)? }));
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
                if let (Some(object), "ADDXri", Some(Operand::Imm(offset))) = (operands.first(), name, operands.get(1)) {
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
                    ("ADRP", [Operand::Symbol(global, addend)]) => {
                        let (target, _) = self.target(*global, *addend)?;
                        self.ops.push(Op::I(Inst::Adr { page: true, rd, target, got: false }));
                    }
                    // `ADDXri page, :lo12:symbol, 0`
                    ("ADDXri", [Operand::Reg(rn), Operand::Symbol(global, addend), _]) => {
                        let (target, _) = self.target(*global, *addend)?;
                        self.ops.push(Op::I(Inst::AddPageOffset { rd, rn: *rn, target }));
                    }
                    // `ADRP` and `ADDXri` of one symbol.
                    ("MOVaddr", [Operand::Symbol(global, addend), _]) => {
                        let (target, _) = self.target(*global, *addend)?;
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
pub fn emit(
    name: &str,
    blocks: &[Selected],
) -> Result<MFunc, String> {
    let first = blocks.first().ok_or("a function with no block")?;
    let mut emitter = Emitter {
        dag: &first.dag,
        globals: &first.globals,
        function: name,
        layout: &first.layout,
        edges_made: 0,
        mf: MFunc::new(name),
        values: HashMap::new(),
        vregs: HashMap::new(),
        fvregs: HashMap::new(),
        fvalues: HashMap::new(),
        ops: Vec::new(),
    };
    for &(size, align) in &first.frame {
        emitter.mf.slot(size, align);
    }
    let mut edges: Vec<MBlock> = Vec::new();
    for (at, block) in blocks.iter().enumerate() {
        emitter.dag = &block.dag;
        emitter.values.clear();
        emitter.fvalues.clear();
        for &(reg, vreg) in &block.live_ins {
            let dst = if is_fpr(reg) { Reg::Fpr(emitter.fvreg(vreg)) } else { Reg::Gpr(emitter.vreg(vreg)) };
            emitter.ops.push(Op::Copy { dst, src: reg_of(reg)? });
        }
        for unit in &block.units {
            for &node in unit.nodes.iter().rev() {
                emitter.node(node)?;
            }
        }
        let mut ops = std::mem::take(&mut emitter.ops);
        edges.extend(emitter.phi_edges(block, &mut ops)?);
        emitter.mf.blocks.push(MBlock { label: block_label(name, at), ops });
    }
    // The blocks made for critical edges lie after the function's own.
    emitter.mf.blocks.extend(edges);
    Ok(emitter.mf)
}
