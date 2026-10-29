//! `Inst` as the 32-bit word `AArch64MCCodeEmitter` writes for it
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/MCTargetDesc/AArch64MCCodeEmitter.cpp>),
//! the fields laid out as `AArch64InstrFormats.td` states them. A branch or
//! address aimed at a symbol or a label leaves its field zero and returns the
//! `Fixup` that fills it.

use std::fmt;

use super::immediate;
use super::inst::*;

/// What fills an instruction's immediate field after encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixupKind {
    /// `b`, `bl`: 26 bits of words.
    Branch26,
    /// `b.cond`, `cbz`, `cbnz`: 19 bits of words.
    Branch19,
    /// `tbz`, `tbnz`: 14 bits of words.
    Branch14,
    /// `adr`: 21 bits of bytes.
    Adr21,
    /// `adrp`: 21 bits of 4 KiB pages.
    Page21,
    /// The 12-bit offset in a page of an `add` (`access` None) or of a load or
    /// store of `1 << access` bytes.
    PageOffset12 {
        access: Option<u8>,
    },
    GotPage21,
    GotPageOffset12,
}

impl FixupKind {
    /// The object's kind of a field a label of the function fills.
    pub fn label_kind(self) -> Option<llrm_object::FixupKind> {
        use super::fixup;
        Some(match self {
            FixupKind::Branch26 => fixup::BRANCH26,
            FixupKind::Branch19 => fixup::BRANCH19,
            FixupKind::Branch14 => fixup::BRANCH14,
            FixupKind::Adr21 => fixup::ADR21,
            _ => return None,
        })
    }

    /// The relocation the object format holds, for the kinds it has one for.
    pub fn relocation(self) -> Option<llrm_object::Fix> {
        use super::fixup;
        Some(match self {
            FixupKind::Branch26 => fixup::BRANCH26_FIX,
            FixupKind::Page21 => fixup::PAGE21_FIX,
            FixupKind::PageOffset12 { access } => fixup::page_offset12(access),
            FixupKind::GotPage21 => fixup::GOT_PAGE21,
            FixupKind::GotPageOffset12 => fixup::GOT_PAGE_OFFSET12,
            FixupKind::Branch19 | FixupKind::Branch14 | FixupKind::Adr21 => return None,
        })
    }
}

/// A field left to fill: where it points and how the value goes in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fixup {
    pub kind: FixupKind,
    /// A `Symbol` or a `Label`, never an `Offset`.
    pub target: Target,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Encoded {
    pub word: u32,
    pub fixup: Option<Fixup>,
}

/// What the encoder cannot say.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(
        &self,
        out: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

fn error<T>(text: impl Into<String>) -> Result<T, Error> {
    Err(Error(text.into()))
}

fn reg(register: Gpr) -> Result<u32, Error> {
    if register.0 > 31 { error(format!("x{} is no machine register", register.0)) } else { Ok(register.0) }
}

fn vreg(register: Vreg) -> Result<u32, Error> {
    if register.0 > 31 { error(format!("v{} is no machine register", register.0)) } else { Ok(register.0) }
}

fn field(
    what: &str,
    value: i64,
    bits: u32,
    signed: bool,
) -> Result<u32, Error> {
    let fits = if signed {
        let limit = 1i64 << (bits - 1);
        (-limit..limit).contains(&value)
    } else {
        (0..1i64 << bits).contains(&value)
    };
    if !fits {
        return error(format!("{what} {value} does not fit {bits} bits"));
    }
    Ok((value as u64 & ((1u64 << bits) - 1)) as u32)
}

/// A pc-relative field of `bits` bits counted in `1 << scale` bytes.
fn pc_field(
    what: &str,
    offset: i64,
    bits: u32,
    scale: u32,
) -> Result<u32, Error> {
    if offset & ((1 << scale) - 1) != 0 {
        return error(format!("{what} {offset} is not a multiple of {}", 1 << scale));
    }
    field(what, offset >> scale, bits, true)
}

/// `inst` as its word and the fixup it leaves.
pub fn encode(inst: &Inst) -> Result<Encoded, Error> {
    let mut fixup = None;
    let word = match inst {
        Inst::AddSubImm { op, width, rd, rn, imm12, shift12 } => {
            let imm = field("an add/sub immediate", i64::from(*imm12), 12, false)?;
            add_sub_bits(*op)
                | 0b100010 << 23
                | u32::from(*shift12) << 22
                | imm << 10
                | width.sf() << 31
                | place(*rn, *rd)?
        }
        Inst::AddSubShifted { op, width, rd, rn, rm, shift, amount } => {
            if *shift == Shift::Ror {
                return error("add/sub has no ror shift");
            }
            let amount = field("a shift amount", i64::from(*amount), 6, false)?;
            if width.bits() == 32 && amount > 31 {
                return error(format!("a shift of {amount} does not fit a w register"));
            }
            add_sub_bits(*op)
                | 0b01011 << 24
                | shift.field() << 22
                | reg(*rm)? << 16
                | amount << 10
                | width.sf() << 31
                | place(*rn, *rd)?
        }
        Inst::AddSubExtended { op, width, rd, rn, rm, extend, amount } => {
            let amount = field("an extend shift", i64::from(*amount), 3, false)?;
            if amount > 4 {
                return error(format!("an extend shift of {amount} is over 4"));
            }
            add_sub_bits(*op)
                | 0b01011 << 24
                | 1 << 21
                | reg(*rm)? << 16
                | extend.field() << 13
                | amount << 10
                | width.sf() << 31
                | place(*rn, *rd)?
        }
        Inst::AddPageOffset { rd, rn, target } => {
            fixup = Some(fixup_for(FixupKind::PageOffset12 { access: None }, target)?);
            0x9100_0000 | place(*rn, *rd)?
        }
        Inst::LogicImm { op, width, rd, rn, imm } => {
            let Some(code) = immediate::logical(*imm, width.bits()) else {
                return error(format!("{imm:#x} is no logical immediate of {} bits", width.bits()));
            };
            let n = code >> 12 & 1;
            if width.bits() == 32 && n != 0 {
                return error("a w register takes no 64-bit element");
            }
            logic_opc(*op) << 29
                | 0b100100 << 23
                | n << 22
                | (code >> 6 & 0x3F) << 16
                | (code & 0x3F) << 10
                | width.sf() << 31
                | place(*rn, *rd)?
        }
        Inst::LogicShifted { op, invert, width, rd, rn, rm, shift, amount } => {
            let amount = field("a shift amount", i64::from(*amount), 6, false)?;
            if width.bits() == 32 && amount > 31 {
                return error(format!("a shift of {amount} does not fit a w register"));
            }
            logic_opc(*op) << 29
                | 0b01010 << 24
                | shift.field() << 22
                | u32::from(*invert) << 21
                | reg(*rm)? << 16
                | amount << 10
                | width.sf() << 31
                | place(*rn, *rd)?
        }
        Inst::MoveWide { op, width, rd, imm16, lsl } => {
            if lsl % 16 != 0 || u32::from(*lsl) >= width.bits() {
                return error(format!("a move-wide shift of {lsl} for a {}-bit register", width.bits()));
            }
            let opc = match op {
                Wide::Not => 0,
                Wide::Zero => 2,
                Wide::Keep => 3,
            };
            opc << 29
                | 0b100101 << 23
                | u32::from(lsl / 16) << 21
                | u32::from(*imm16) << 5
                | width.sf() << 31
                | reg(*rd)?
        }
        Inst::BitfieldMove { op, width, rd, rn, immr, imms } => {
            let (immr, imms) = (u32::from(*immr), u32::from(*imms));
            if immr >= width.bits() || imms >= width.bits() {
                return error(format!("bitfield positions {immr}, {imms} for a {}-bit register", width.bits()));
            }
            let opc = match op {
                Bitfield::Sbfm => 0,
                Bitfield::Bfm => 1,
                Bitfield::Ubfm => 2,
            };
            opc << 29
                | 0b100110 << 23
                | width.sf() << 22
                | immr << 16
                | imms << 10
                | width.sf() << 31
                | place(*rn, *rd)?
        }
        Inst::Extract { width, rd, rn, rm, lsb } => {
            if u32::from(*lsb) >= width.bits() {
                return error(format!("an extract position of {lsb}"));
            }
            0b00100111 << 23
                | width.sf() << 22
                | reg(*rm)? << 16
                | u32::from(*lsb) << 10
                | width.sf() << 31
                | place(*rn, *rd)?
        }
        Inst::Divide { op, width, rd, rn, rm } => {
            let opcode = match op {
                Div::Udiv => 0b000010,
                Div::Sdiv => 0b000011,
                Div::Lslv => 0b001000,
                Div::Lsrv => 0b001001,
                Div::Asrv => 0b001010,
                Div::Rorv => 0b001011,
            };
            0b0011010110 << 21 | reg(*rm)? << 16 | opcode << 10 | width.sf() << 31 | place(*rn, *rd)?
        }
        Inst::OneSource { op, width, rd, rn } => {
            let opcode = match (op, width) {
                (Unary::Rbit, _) => 0b000000,
                (Unary::Rev16, _) => 0b000001,
                (Unary::Rev, Width::W) => 0b000010,
                (Unary::Rev, Width::X) => 0b000011,
                (Unary::Clz, _) => 0b000100,
                (Unary::Cls, _) => 0b000101,
            };
            1 << 30 | 0b011010110 << 21 | opcode << 10 | width.sf() << 31 | place(*rn, *rd)?
        }
        Inst::ThreeSource { op, width, rd, rn, rm, ra } => {
            // `madd` and `msub` take either width in their `sf`; the long and
            // high multiplies are 64-bit.
            let (op31, o0, sf) = match op {
                Mul3::Madd => (0b000, 0, width.sf()),
                Mul3::Msub => (0b000, 1, width.sf()),
                Mul3::Smaddl => (0b001, 0, 1),
                Mul3::Smsubl => (0b001, 1, 1),
                Mul3::Smulh => (0b010, 0, 1),
                Mul3::Umaddl => (0b101, 0, 1),
                Mul3::Umsubl => (0b101, 1, 1),
                Mul3::Umulh => (0b110, 0, 1),
            };
            sf << 31 | 0b11011 << 24 | op31 << 21 | reg(*rm)? << 16 | o0 << 15 | reg(*ra)? << 10 | place(*rn, *rd)?
        }
        Inst::CondSelect { op, width, rd, rn, rm, cond } => {
            let (invert, increment) = match op {
                Select::Csel => (0, 0),
                Select::Csinc => (0, 1),
                Select::Csinv => (1, 0),
                Select::Csneg => (1, 1),
            };
            invert << 30
                | 0b11010100 << 21
                | reg(*rm)? << 16
                | cond.field() << 12
                | increment << 10
                | width.sf() << 31
                | place(*rn, *rd)?
        }
        Inst::CondCompare { negate, width, rn, rm_or_imm, nzcv, cond } => {
            let (second, immediate) = match rm_or_imm {
                CcOperand::Reg(rm) => (reg(*rm)?, 0),
                CcOperand::Imm5(imm) => (field("a compare immediate", i64::from(*imm), 5, false)?, 1),
            };
            u32::from(!*negate) << 30
                | 1 << 29
                | 0b11010010 << 21
                | second << 16
                | cond.field() << 12
                | immediate << 11
                | reg(*rn)? << 5
                | field("nzcv", i64::from(*nzcv), 4, false)?
                | width.sf() << 31
        }
        Inst::Branch { link, target } => {
            let base = if *link { 0x9400_0000 } else { 0x1400_0000 };
            base | immediate_field(target, FixupKind::Branch26, 26, 2, &mut fixup)?
        }
        Inst::BranchCond { cond, target } => {
            0x5400_0000 | immediate_field(target, FixupKind::Branch19, 19, 2, &mut fixup)? << 5 | cond.field()
        }
        Inst::CompareBranch { nonzero, width, rt, target } => {
            0b011010 << 25
                | u32::from(*nonzero) << 24
                | immediate_field(target, FixupKind::Branch19, 19, 2, &mut fixup)? << 5
                | width.sf() << 31
                | reg(*rt)?
        }
        Inst::TestBranch { nonzero, rt, bit, target } => {
            let bit = field("a bit number", i64::from(*bit), 6, false)?;
            0b011011 << 25
                | u32::from(*nonzero) << 24
                | (bit & 0x1F) << 19
                | immediate_field(target, FixupKind::Branch14, 14, 2, &mut fixup)? << 5
                | (bit >> 5) << 31
                | reg(*rt)?
        }
        Inst::BranchReg { link, rn } => (if *link { 0xD63F_0000 } else { 0xD61F_0000 }) | reg(*rn)? << 5,
        Inst::Ret { rn } => 0xD65F_0000 | reg(*rn)? << 5,
        Inst::Load { mem, addr, .. } => load_store(mem, addr, true, &mut fixup)?,
        Inst::Store { mem, addr, .. } => load_store(mem, addr, false, &mut fixup)?,
        Inst::LoadPair { first, second, addr } => pair(first, second, addr, true)?,
        Inst::StorePair { first, second, addr } => pair(first, second, addr, false)?,
        Inst::Adr { page, rd, target, got } => {
            let kind = match (page, got) {
                (true, true) => FixupKind::GotPage21,
                (true, false) => FixupKind::Page21,
                (false, _) => FixupKind::Adr21,
            };
            let scale = if *page { 12 } else { 0 };
            let value = immediate_field(target, kind, 21, scale, &mut fixup)?;
            u32::from(*page) << 31 | (value & 3) << 29 | 0b10000 << 24 | (value >> 2) << 5 | reg(*rd)?
        }
        Inst::FpUnary { op, size, rd, rn } => {
            let opcode = match op {
                FpUnary::Mov => 0b000000,
                FpUnary::Abs => 0b000001,
                FpUnary::Neg => 0b000010,
                FpUnary::Sqrt => 0b000011,
                FpUnary::Rint(Rounding::Nearest) => 0b001000,
                FpUnary::Rint(Rounding::Up) => 0b001001,
                FpUnary::Rint(Rounding::Down) => 0b001010,
                FpUnary::Rint(Rounding::Zero) => 0b001011,
                FpUnary::Rint(Rounding::Away) => 0b001100,
                FpUnary::Rint(Rounding::Exact) => 0b001110,
                FpUnary::Rint(Rounding::Current) => 0b001111,
            };
            0x1E20_4000 | size.field() << 22 | opcode << 15 | vreg(*rn)? << 5 | vreg(*rd)?
        }
        Inst::VectorMov { rd, rn } => 0x4EA0_1C00 | vreg(*rn)? << 16 | vreg(*rn)? << 5 | vreg(*rd)?,
        Inst::FpBinary { op, size, rd, rn, rm } => {
            let opcode = match op {
                FpBinary::Mul => 0b0000,
                FpBinary::Div => 0b0001,
                FpBinary::Add => 0b0010,
                FpBinary::Sub => 0b0011,
                FpBinary::Max => 0b0100,
                FpBinary::Min => 0b0101,
                FpBinary::MaxNm => 0b0110,
                FpBinary::MinNm => 0b0111,
                FpBinary::Nmul => 0b1000,
            };
            0x1E20_0800 | size.field() << 22 | vreg(*rm)? << 16 | opcode << 12 | vreg(*rn)? << 5 | vreg(*rd)?
        }
        Inst::FpFused { op, size, rd, rn, rm, ra } => {
            let (o1, o0) = match op {
                FpFused::Madd => (0, 0),
                FpFused::Msub => (0, 1),
                FpFused::Nmadd => (1, 0),
                FpFused::Nmsub => (1, 1),
            };
            0x1F00_0000
                | size.field() << 22
                | o1 << 21
                | vreg(*rm)? << 16
                | o0 << 15
                | vreg(*ra)? << 10
                | vreg(*rn)? << 5
                | vreg(*rd)?
        }
        Inst::FpCompare { signalling, size, rn, rm } => {
            let (second, zero) = match rm {
                Some(rm) => (vreg(*rm)?, 0),
                None => (0, 0b01000),
            };
            0x1E20_2000 | size.field() << 22 | second << 16 | vreg(*rn)? << 5 | u32::from(*signalling) << 4 | zero
        }
        Inst::FpSelect { size, rd, rn, rm, cond } => {
            0x1E20_0C00 | size.field() << 22 | vreg(*rm)? << 16 | cond.field() << 12 | vreg(*rn)? << 5 | vreg(*rd)?
        }
        Inst::VectorFill { rd, fill } => match fill {
            Fill::Zero => 0x6F00_E400 | vreg(*rd)?,
            Fill::Byte(byte) => 0x4F00_E400 | u32::from(byte >> 5) << 16 | u32::from(byte & 31) << 5 | vreg(*rd)?,
            Fill::Gpr(source) => 0x4E01_0C00 | reg(*source)? << 5 | vreg(*rd)?,
        },
        Inst::FpMoveImm { size, rd, imm8 } => 0x1E20_1000 | size.field() << 22 | u32::from(*imm8) << 13 | vreg(*rd)?,
        Inst::FpToFp { to, from, rd, rn } => {
            // `fcvt`: the source size is `type`, the destination's code `opc`.
            let opc = match to {
                FpSize::S => 0b00,
                FpSize::D => 0b01,
                FpSize::H => 0b11,
            };
            if to == from {
                return error("fcvt between one size and itself");
            }
            0x1E22_4000 | from.field() << 22 | opc << 15 | vreg(*rn)? << 5 | vreg(*rd)?
        }
        Inst::FpInt { op, width, size, gpr, fpr } => {
            let (rmode, opcode, to_fp) = match op {
                FpConvert::ToFp { signed } => (0b00, if *signed { 0b010 } else { 0b011 }, true),
                FpConvert::ToIntZero { signed } => (0b11, if *signed { 0b000 } else { 0b001 }, false),
                FpConvert::Bits { to_fp } => (0b00, if *to_fp { 0b111 } else { 0b110 }, *to_fp),
            };
            let (source, destination) = if to_fp { (reg(*gpr)?, vreg(*fpr)?) } else { (vreg(*fpr)?, reg(*gpr)?) };
            0x1E20_0000 | width.sf() << 31 | size.field() << 22 | rmode << 19 | opcode << 16 | source << 5 | destination
        }
        Inst::FpFixed { op, width, size, gpr, fpr, fbits } => {
            let (rmode, opcode, to_fp) = match op {
                FpConvert::ToFp { signed } => (0b00, if *signed { 0b010 } else { 0b011 }, true),
                FpConvert::ToIntZero { signed } => (0b11, if *signed { 0b000 } else { 0b001 }, false),
                FpConvert::Bits { .. } => return error("a fixed-point move of bits"),
            };
            if *fbits == 0 || u32::from(*fbits) > width.bits() {
                return error(format!("{fbits} fractional bits in a {}-bit register", width.bits()));
            }
            let (source, destination) = if to_fp { (reg(*gpr)?, vreg(*fpr)?) } else { (vreg(*fpr)?, reg(*gpr)?) };
            let scale = 64 - u32::from(*fbits);
            0x1E00_0000
                | width.sf() << 31
                | size.field() << 22
                | rmode << 19
                | opcode << 16
                | scale << 10
                | source << 5
                | destination
        }
        Inst::Svc { imm16 } => 0xD400_0001 | u32::from(*imm16) << 5,
        Inst::Atomic { op, size, acquire, release, rs, rt, rn } => {
            let (size, a, l) = (u32::from(*size), u32::from(*acquire), u32::from(*release));
            if size > 3 {
                return error(format!("an atomic access of 2^{size} bytes"));
            }
            let (rs, rt, rn) = (reg(*rs)?, reg(*rt)?, reg(*rn)?);
            let memory = |opc: u32, o3: u32| {
                0x3820_0000 | size << 30 | a << 23 | l << 22 | rs << 16 | o3 << 15 | opc << 12 | rn << 5 | rt
            };
            match op {
                Atomic::Ldar => 0x08DF_FC00 | size << 30 | rn << 5 | rt,
                Atomic::Stlr => 0x089F_FC00 | size << 30 | rn << 5 | rt,
                Atomic::Ldapr => 0x38BF_C000 | size << 30 | rn << 5 | rt,
                Atomic::Cas => 0x08A0_7C00 | size << 30 | a << 22 | l << 15 | rs << 16 | rn << 5 | rt,
                Atomic::Swp => memory(0, 1),
                Atomic::Ldadd => memory(0, 0),
                Atomic::Ldclr => memory(1, 0),
                Atomic::Ldeor => memory(2, 0),
                Atomic::Ldset => memory(3, 0),
                Atomic::Ldsmax => memory(4, 0),
                Atomic::Ldsmin => memory(5, 0),
                Atomic::Ldumax => memory(6, 0),
                Atomic::Ldumin => memory(7, 0),
            }
        }
        Inst::Dmb { barrier } => match barrier {
            Barrier::Ish => 0xD503_3BBF,
            Barrier::IshLd => 0xD503_39BF,
            Barrier::IshSt => 0xD503_3ABF,
        },
        Inst::Brk { imm16 } => 0xD420_0000 | u32::from(*imm16) << 5,
        Inst::Nop => 0xD503_201F,
        Inst::Prefetch { op, base } => 0xF980_0000 | reg(*base)? << 5 | u32::from(*op),
    };
    Ok(Encoded { word, fixup })
}

/// `word` with the pc-relative field of `kind` filled for a target `offset`
/// bytes from the instruction (a label of the same function).
pub fn resolve(
    word: u32,
    kind: FixupKind,
    offset: i64,
) -> Result<u32, Error> {
    Ok(word
        | match kind {
            FixupKind::Branch26 => pc_field("a branch offset", offset, 26, 2)?,
            FixupKind::Branch19 => pc_field("a branch offset", offset, 19, 2)? << 5,
            FixupKind::Branch14 => pc_field("a test-branch offset", offset, 14, 2)? << 5,
            FixupKind::Adr21 => {
                let value = pc_field("an adr offset", offset, 21, 0)?;
                (value & 3) << 29 | (value >> 2) << 5
            }
            other => return error(format!("{other:?} is not a pc-relative label field")),
        })
}

/// `Rn` and `Rd`, fields 9:5 and 4:0.
fn place(
    rn: Gpr,
    rd: Gpr,
) -> Result<u32, Error> {
    Ok(reg(rn)? << 5 | reg(rd)?)
}

/// `op` and `S` of an add or subtract.
fn add_sub_bits(op: AddSub) -> u32 {
    u32::from(op.subtract) << 30 | u32::from(op.set_flags) << 29
}

fn logic_opc(op: Logic) -> u32 {
    match op {
        Logic::And => 0,
        Logic::Orr => 1,
        Logic::Eor => 2,
        Logic::Ands => 3,
    }
}

fn fixup_for(
    kind: FixupKind,
    target: &Target,
) -> Result<Fixup, Error> {
    match target {
        Target::Offset(_) => error("an offset is encoded, not fixed up"),
        other => Ok(Fixup { kind, target: other.clone() }),
    }
}

/// The field of a pc-relative operand: its value now, or zero and a fixup.
fn immediate_field(
    target: &Target,
    kind: FixupKind,
    bits: u32,
    scale: u32,
    fixup: &mut Option<Fixup>,
) -> Result<u32, Error> {
    match target {
        Target::Offset(bytes) => {
            let paged = matches!(kind, FixupKind::Page21 | FixupKind::GotPage21);
            let value = if paged { bytes >> 12 } else { *bytes };
            pc_field("a branch offset", value, bits, if paged { 0 } else { scale })
        }
        symbolic => {
            *fixup = Some(fixup_for(kind, symbolic)?);
            Ok(0)
        }
    }
}

/// The `size` field and the bytes a memory operand moves.
fn memory(mem: &Memory) -> Result<(u32, u32, u32, u32), Error> {
    // (size field, V, opc, log2 bytes)
    Ok(match *mem {
        Memory::Int { width, bytes, sign, reg: register } => {
            reg(register)?;
            let log = match bytes {
                1 => 0,
                2 => 1,
                4 => 2,
                8 => 3,
                other => return error(format!("an integer access of {other} bytes")),
            };
            let opc = match (sign, width, bytes) {
                (false, _, _) => 0b01,
                (true, Width::X, 1 | 2 | 4) => 0b10,
                (true, Width::W, 1 | 2) => 0b11,
                _ => return error(format!("a signed {bytes}-byte load into a {} register", width.bits())),
            };
            (log, 0, opc, log)
        }
        Memory::Fp { bytes, reg: register } => {
            vreg(register)?;
            match bytes {
                1 => (0, 1, 0b01, 0),
                2 => (1, 1, 0b01, 1),
                4 => (2, 1, 0b01, 2),
                8 => (3, 1, 0b01, 3),
                16 => (0, 1, 0b11, 4),
                other => return error(format!("a floating-point access of {other} bytes")),
            }
        }
    })
}

fn memory_register(mem: &Memory) -> u32 {
    match mem {
        Memory::Int { reg, .. } => reg.0,
        Memory::Fp { reg, .. } => reg.0,
    }
}

fn load_store(
    mem: &Memory,
    addr: &Addr,
    load: bool,
    fixup: &mut Option<Fixup>,
) -> Result<u32, Error> {
    let (size, v, load_opc, log) = memory(mem)?;
    // A store is opc 00, or 10 for a 128-bit register; a load's is the table's.
    let opc = if load {
        load_opc
    } else if matches!(mem, Memory::Fp { bytes: 16, .. }) {
        0b10
    } else {
        0b00
    };
    let rt = memory_register(mem);
    let base = |base: &Gpr| reg(*base);
    let head = size << 30 | 0b111 << 27 | v << 26;
    Ok(match addr {
        Addr::Offset { base: b, imm } => {
            let scale = i64::from(1u32 << log);
            if *imm >= 0 && imm % scale == 0 && imm / scale < 4096 {
                head | 0b01 << 24 | opc << 22 | ((imm / scale) as u32) << 10 | base(b)? << 5 | rt
            } else {
                // The unscaled form: a signed byte offset.
                head | opc << 22 | field("an unscaled offset", *imm, 9, true)? << 12 | base(b)? << 5 | rt
            }
        }
        Addr::Pre { base: b, imm } => {
            head | opc << 22 | field("a pre-index offset", *imm, 9, true)? << 12 | 0b11 << 10 | base(b)? << 5 | rt
        }
        Addr::Post { base: b, imm } => {
            head | opc << 22 | field("a post-index offset", *imm, 9, true)? << 12 | 0b01 << 10 | base(b)? << 5 | rt
        }
        Addr::Register { base: b, index, extend, shifted } => {
            if !matches!(
                extend,
                Extend::Uxtw | Extend::Uxtx | Extend::Sxtw | Extend::Sxtx
            ) {
                return error("a register offset extends with uxtw, lsl, sxtw or sxtx");
            }
            head | opc << 22
                | 1 << 21
                | reg(*index)? << 16
                | extend.field() << 13
                | u32::from(*shifted) << 12
                | 0b10 << 10
                | base(b)? << 5
                | rt
        }
        Addr::Slot { .. } => return error("a frame object is addressed after the frame is laid out"),
        Addr::PageOffset { base: b, target, got } => {
            let kind =
                if *got { FixupKind::GotPageOffset12 } else { FixupKind::PageOffset12 { access: Some(log as u8) } };
            *fixup = Some(fixup_for(kind, target)?);
            head | 0b01 << 24 | opc << 22 | base(b)? << 5 | rt
        }
    })
}

fn pair(
    first: &Memory,
    second: &Memory,
    addr: &Addr,
    load: bool,
) -> Result<u32, Error> {
    let (opc, v, log) = match (first, second) {
        (
            Memory::Int { width: Width::W, bytes: 4, sign: false, .. },
            Memory::Int { width: Width::W, bytes: 4, sign: false, .. },
        ) => (0b00, 0, 2),
        (
            Memory::Int { width: Width::X, bytes: 8, sign: false, .. },
            Memory::Int { width: Width::X, bytes: 8, sign: false, .. },
        ) => (0b10, 0, 3),
        (Memory::Fp { bytes: 4, .. }, Memory::Fp { bytes: 4, .. }) => (0b00, 1, 2),
        (Memory::Fp { bytes: 8, .. }, Memory::Fp { bytes: 8, .. }) => (0b01, 1, 3),
        (Memory::Fp { bytes: 16, .. }, Memory::Fp { bytes: 16, .. }) => (0b10, 1, 4),
        _ => return error("a register pair of two registers of one size"),
    };
    let (kind, base, imm) = match addr {
        Addr::Offset { base, imm } => (0b010, base, imm),
        Addr::Pre { base, imm } => (0b011, base, imm),
        Addr::Post { base, imm } => (0b001, base, imm),
        _ => return error("a pair takes an offset, pre-index or post-index address"),
    };
    let scale = 1i64 << log;
    if imm % scale != 0 {
        return error(format!("a pair offset {imm} is not a multiple of {scale}"));
    }
    Ok(opc << 30
        | 0b101 << 27
        | v << 26
        | kind << 23
        | u32::from(load) << 22
        | field("a pair offset", imm / scale, 7, true)? << 15
        | memory_register(second) << 10
        | reg(*base)? << 5
        | memory_register(first))
}
