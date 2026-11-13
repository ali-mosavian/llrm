//! An A64 instruction as the machine states it, before it is encoded: what
//! LLVM's `MCInst` carries for the `AArch64` target. One variant per encoding
//! class of `AArch64InstrFormats.td`
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64InstrFormats.td>),
//! its opcode a field, so a new mnemonic of a class is a new value, not a new
//! branch.

use std::fmt;

/// The register width of an integer instruction: the `sf` bit.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Width {
    W,
    X,
}

impl Width {
    pub fn bits(self) -> u32 {
        match self {
            Width::W => 32,
            Width::X => 64,
        }
    }

    pub(super) fn sf(self) -> u32 {
        u32::from(self == Width::X)
    }
}

use llrm_analysis::aa::AliasResult;
use llrm_analysis::placealias::{Object, alias_of};

/// The object an access is relative to, as far as telling two accesses apart:
/// what `MachinePointerInfo` names.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub enum MemObject {
    #[default]
    Unknown,
    /// A global, by symbol.
    Global(String),
    /// A frame object.
    Slot(u32),
    /// The function's constant pool: memory nothing writes.
    ConstantPool,
}

/// What a load or store reaches (`MachineMemOperand`): its object and the byte
/// offset in it, where both are known.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct MemOperand {
    pub object: MemObject,
    pub offset: Option<i64>,
    /// `MachineMemOperand::isVolatile`: the access is not made a writeback
    /// form (`AArch64LoadStoreOpt::tryToMergeLdStUpdate`).
    pub volatile: bool,
}

impl MemOperand {
    /// Whether an access of `bytes` here and one of `other_bytes` at `other`
    /// cannot touch the same byte (`AliasAnalysis`' `NoAlias`, as
    /// `BaseIndexOffset::computeAliasing` reads it from the objects): two
    /// different objects never overlap; one object's accesses do not when their
    /// ranges do not.
    pub fn disjoint(
        &self,
        bytes: i64,
        other: &MemOperand,
        other_bytes: i64,
    ) -> bool {
        fn object(memory: &MemOperand) -> Option<Object<&str>> {
            match &memory.object {
                MemObject::Unknown | MemObject::ConstantPool => None,
                MemObject::Global(name) => Some(Object::Global(name.as_str())),
                MemObject::Slot(slot) => Some(Object::Frame { id: i64::from(*slot), fixed: false }),
            }
        }
        let distance = (self.object == other.object).then(|| Some(other.offset? - self.offset?)).flatten();
        alias_of(
            distance,
            object(self).as_ref(),
            u64::try_from(bytes).ok(),
            object(other).as_ref(),
            u64::try_from(other_bytes).ok(),
        ) == AliasResult::NoAlias
    }
}

/// A general-purpose register, `x0`..`x30`, or number 31, which is the zero
/// register or the stack pointer as the operand's role says.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gpr(pub u32);

impl Gpr {
    /// Number 31.
    pub const R31: Gpr = Gpr(31);
    pub const FP: Gpr = Gpr(29);
    pub const LR: Gpr = Gpr(30);
    /// The first number of a virtual register: below it are the 32 machine
    /// registers, as LLVM numbers its own (`Register::isVirtualRegister`).
    pub const FIRST_VIRTUAL: u32 = 64;

    pub fn is_virtual(self) -> bool {
        self.0 >= Self::FIRST_VIRTUAL
    }
}

/// A floating-point or vector register, `v0`..`v31`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Vreg(pub u32);

impl Vreg {
    pub fn is_virtual(self) -> bool {
        self.0 >= Gpr::FIRST_VIRTUAL
    }
}

/// The size of a scalar floating-point operand: the `type` field.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FpSize {
    S,
    D,
    H,
}

impl FpSize {
    pub(super) fn field(self) -> u32 {
        match self {
            FpSize::S => 0,
            FpSize::D => 1,
            FpSize::H => 3,
        }
    }

    pub(super) fn letter(self) -> char {
        match self {
            FpSize::S => 's',
            FpSize::D => 'd',
            FpSize::H => 'h',
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Shift {
    Lsl,
    Lsr,
    Asr,
    Ror,
}

impl Shift {
    pub(super) fn field(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Extend {
    Uxtb,
    Uxth,
    Uxtw,
    Uxtx,
    Sxtb,
    Sxth,
    Sxtw,
    Sxtx,
}

impl Extend {
    pub(super) fn field(self) -> u32 {
        self as u32
    }
}

/// A condition code, in the order of its 4-bit field.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Cond {
    Eq,
    Ne,
    Hs,
    Lo,
    Mi,
    Pl,
    Vs,
    Vc,
    Hi,
    Ls,
    Ge,
    Lt,
    Gt,
    Le,
    Al,
    Nv,
}

impl Cond {
    pub(super) fn field(self) -> u32 {
        self as u32
    }

    pub fn inverted(self) -> Cond {
        const ALL: [Cond; 16] = [
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
        ALL[(self as usize) ^ 1]
    }
}

/// What a branch, an `adr` or a page offset is aimed at.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Target {
    /// Bytes from the instruction itself.
    Offset(i64),
    /// A symbol and an addend: left for the linker.
    Symbol { name: String, addend: i64 },
    /// A label of the same function: the object builder resolves it.
    Label(String),
}

/// `add`/`sub` and their flag-setting forms.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AddSub {
    pub subtract: bool,
    pub set_flags: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Logic {
    And,
    Orr,
    Eor,
    Ands,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Wide {
    /// `movn`
    Not,
    /// `movz`
    Zero,
    /// `movk`
    Keep,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Bitfield {
    Sbfm,
    Bfm,
    Ubfm,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Div {
    Udiv,
    Sdiv,
    Lslv,
    Lsrv,
    Asrv,
    Rorv,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Unary {
    Rbit,
    Rev16,
    /// `rev`: reverses the register's bytes.
    Rev,
    Clz,
    Cls,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Mul3 {
    Madd,
    Msub,
    Smaddl,
    Smsubl,
    Smulh,
    Umaddl,
    Umsubl,
    Umulh,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Select {
    Csel,
    Csinc,
    Csinv,
    Csneg,
}

/// How a load or store forms its address.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Addr {
    /// `[base, #imm]`: scaled by the access size when `imm` is a multiple of it
    /// and in range, otherwise the unscaled form (`ldur`).
    Offset { base: Gpr, imm: i64 },
    /// `[base, #imm]!`
    Pre { base: Gpr, imm: i64 },
    /// `[base], #imm`
    Post { base: Gpr, imm: i64 },
    /// `[base, index{, extend #shift}]`: `shifted` scales the index by the
    /// size.
    Register { base: Gpr, index: Gpr, extend: Extend, shifted: bool },
    /// `[base, #:lo12:symbol]`
    PageOffset { base: Gpr, target: Target, got: bool },
    /// A frame object, before the frame is laid out: frame lowering turns it
    /// into `Offset` from the stack or frame pointer (LLVM's frame index
    /// operand).
    Slot { slot: u32, offset: i64 },
}

/// The register a load or store moves and how much of it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Memory {
    /// An integer register: `bytes` 1, 2, 4 or 8; `sign` extends a narrower
    /// load to `width`.
    Int { width: Width, bytes: u8, sign: bool, reg: Gpr },
    /// A floating-point or vector register of `bytes` 1, 2, 4, 8 or 16.
    Fp { bytes: u8, reg: Vreg },
}

/// What an [`Inst::Atomic`] does: `ldar`, `ldapr`, `stlr`, `swp`, `ldadd`,
/// `ldclr`, `ldeor`, `ldset`, `ldsmax`, `ldsmin`, `ldumax`, `ldumin`, `cas`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Atomic {
    Ldar,
    Ldapr,
    Stlr,
    Swp,
    Ldadd,
    Ldclr,
    Ldeor,
    Ldset,
    Ldsmax,
    Ldsmin,
    Ldumax,
    Ldumin,
    Cas,
}

/// The domain and kind of a `dmb`: `ish`, `ishld`, `ishst`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Barrier {
    Ish,
    IshLd,
    IshSt,
}

/// The lane layout of a 128-bit vector: `16b`, `8h`, `4s`, `2d`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Arrangement {
    B16,
    H8,
    S4,
    D2,
}

impl Arrangement {
    /// `imm5` of `dup` (general): the lane size as the position of its lowest
    /// set bit.
    pub(super) fn imm5(self) -> u32 {
        match self {
            Arrangement::B16 => 0b00001,
            Arrangement::H8 => 0b00010,
            Arrangement::S4 => 0b00100,
            Arrangement::D2 => 0b01000,
        }
    }

    /// The arrangement of the lanes half as wide (`xtn`'s result, `sshll`'s
    /// source): the low half's, or the whole register's when `high`.
    pub fn narrowed(
        self,
        high: bool,
    ) -> &'static str {
        match (self, high) {
            (Arrangement::H8, false) => "8b",
            (Arrangement::H8, true) => "16b",
            (Arrangement::S4, false) => "4h",
            (Arrangement::S4, true) => "8h",
            (Arrangement::D2, false) => "2s",
            (Arrangement::D2, true) => "4s",
            (Arrangement::B16, _) => unreachable!("no lane is narrower than a byte"),
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Arrangement::B16 => "16b",
            Arrangement::H8 => "8h",
            Arrangement::S4 => "4s",
            Arrangement::D2 => "2d",
        }
    }
}

/// The three-register integer vector operations (the "three same" class).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VectorOp {
    Add,
    Sub,
    Mul,
    And,
    Bic,
    Orr,
    Orn,
    Eor,
    /// `ushl`/`sshl`: a shift by each lane's own (signed) amount.
    Ushl,
    Sshl,
}

impl VectorOp {
    /// (`U`, `opcode`, `size` for the logical forms) of the encoding.
    pub(super) fn fields(self) -> (u32, u32, Option<u32>) {
        match self {
            VectorOp::Add => (0, 0b10000, None),
            VectorOp::Sub => (1, 0b10000, None),
            VectorOp::Mul => (0, 0b10011, None),
            VectorOp::Ushl => (1, 0b01000, None),
            VectorOp::Sshl => (0, 0b01000, None),
            VectorOp::And => (0, 0b00011, Some(0)),
            VectorOp::Bic => (0, 0b00011, Some(1)),
            VectorOp::Orr => (0, 0b00011, Some(2)),
            VectorOp::Orn => (0, 0b00011, Some(3)),
            VectorOp::Eor => (1, 0b00011, Some(0)),
        }
    }

    pub fn mnemonic(self) -> &'static str {
        match self {
            VectorOp::Add => "add",
            VectorOp::Sub => "sub",
            VectorOp::Mul => "mul",
            VectorOp::And => "and",
            VectorOp::Bic => "bic",
            VectorOp::Orr => "orr",
            VectorOp::Orn => "orn",
            VectorOp::Eor => "eor",
            VectorOp::Ushl => "ushl",
            VectorOp::Sshl => "sshl",
        }
    }

    /// Whether the operation is the bitwise `.16b` form whatever the lanes.
    pub fn is_logical(self) -> bool {
        matches!(
            self,
            VectorOp::And | VectorOp::Bic | VectorOp::Orr | VectorOp::Orn | VectorOp::Eor
        )
    }
}

/// The three-register float vector operations: `fadd v.4s, v.4s, v.4s`. The
/// lanes are `S4` or `D2`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VectorFpOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl VectorFpOp {
    /// (`U`, `a` (bit 23), `opcode`) of the "three same" float encoding.
    pub(super) fn fields(self) -> (u32, u32, u32) {
        match self {
            VectorFpOp::Add => (0, 0, 0b11010),
            VectorFpOp::Sub => (0, 1, 0b11010),
            VectorFpOp::Mul => (1, 0, 0b11011),
            VectorFpOp::Div => (1, 0, 0b11111),
        }
    }

    pub fn mnemonic(self) -> &'static str {
        match self {
            VectorFpOp::Add => "fadd",
            VectorFpOp::Sub => "fsub",
            VectorFpOp::Mul => "fmul",
            VectorFpOp::Div => "fdiv",
        }
    }
}

/// `fmla` and `fmls`: `rd += rn * rm` and `rd -= rn * rm`, every lane.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VectorFma {
    Add,
    Sub,
}

impl VectorFma {
    pub fn mnemonic(self) -> &'static str {
        match self {
            VectorFma::Add => "fmla",
            VectorFma::Sub => "fmls",
        }
    }
}

/// The one-register float vector operations: `fneg v.4s, v.4s`, and the
/// conversions between a float vector and an integer one of the same lane
/// width (`scvtf v.4s, v.4s`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VectorFpUnary {
    Neg,
    Abs,
    Scvtf,
    Ucvtf,
    Fcvtzs,
    Fcvtzu,
}

impl VectorFpUnary {
    /// (`U`, bit 23, `opcode`) of the "two-register misc" float encoding.
    pub(super) fn fields(self) -> (u32, u32, u32) {
        match self {
            VectorFpUnary::Neg => (1, 1, 0b01111),
            VectorFpUnary::Abs => (0, 1, 0b01111),
            VectorFpUnary::Scvtf => (0, 0, 0b11101),
            VectorFpUnary::Ucvtf => (1, 0, 0b11101),
            VectorFpUnary::Fcvtzs => (0, 1, 0b11011),
            VectorFpUnary::Fcvtzu => (1, 1, 0b11011),
        }
    }

    pub fn mnemonic(self) -> &'static str {
        match self {
            VectorFpUnary::Neg => "fneg",
            VectorFpUnary::Abs => "fabs",
            VectorFpUnary::Scvtf => "scvtf",
            VectorFpUnary::Ucvtf => "ucvtf",
            VectorFpUnary::Fcvtzs => "fcvtzs",
            VectorFpUnary::Fcvtzu => "fcvtzu",
        }
    }
}

/// The across-lanes reductions of one vector register into a scalar:
/// `addv`, `smaxv`, `sminv`, `umaxv`, `uminv`, and `addp` of the two `d` lanes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VectorReduce {
    Addv,
    Smaxv,
    Sminv,
    Umaxv,
    Uminv,
    Addp,
}

impl VectorReduce {
    pub fn mnemonic(self) -> &'static str {
        match self {
            VectorReduce::Addv => "addv",
            VectorReduce::Smaxv => "smaxv",
            VectorReduce::Sminv => "sminv",
            VectorReduce::Umaxv => "umaxv",
            VectorReduce::Uminv => "uminv",
            VectorReduce::Addp => "addp",
        }
    }

    /// `(U, opcode)` of the "across lanes" class; `addp` is the "pairwise"
    /// scalar class, which has the `addv` opcode.
    pub(super) fn fields(self) -> (u32, u32) {
        match self {
            VectorReduce::Addv | VectorReduce::Addp => (0, 0b11011),
            VectorReduce::Smaxv => (0, 0b01010),
            VectorReduce::Sminv => (0, 0b11010),
            VectorReduce::Umaxv => (1, 0b01010),
            VectorReduce::Uminv => (1, 0b11010),
        }
    }
}

/// The shifts of a vector by an immediate.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VectorShift {
    Shl,
    Ushr,
    Sshr,
}

/// The lane of the narrow operand of a long or wide vector operation: the
/// result lanes are twice as wide (`8b` to `8h`, `4h` to `4s`, `2s` to `2d`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NarrowLane {
    B,
    H,
    S,
}

impl NarrowLane {
    /// The narrow lane size in bits.
    pub fn bits(self) -> u32 {
        match self {
            NarrowLane::B => 8,
            NarrowLane::H => 16,
            NarrowLane::S => 32,
        }
    }

    /// `size` of the encodings.
    pub(super) fn size(self) -> u32 {
        match self {
            NarrowLane::B => 0,
            NarrowLane::H => 1,
            NarrowLane::S => 2,
        }
    }

    /// The arrangement of the narrow operand, `8b` or, for the `2` forms,
    /// `16b`.
    pub fn narrow_suffix(
        self,
        high: bool,
    ) -> &'static str {
        match (self, high) {
            (NarrowLane::B, false) => "8b",
            (NarrowLane::B, true) => "16b",
            (NarrowLane::H, false) => "4h",
            (NarrowLane::H, true) => "8h",
            (NarrowLane::S, false) => "2s",
            (NarrowLane::S, true) => "4s",
        }
    }

    /// The arrangement of the result and of the wide operand.
    pub fn wide_suffix(self) -> &'static str {
        match self {
            NarrowLane::B => "8h",
            NarrowLane::H => "4s",
            NarrowLane::S => "2d",
        }
    }
}

/// The vector operations whose result lanes are twice as wide as a narrow
/// operand's: a shift left (`ushll`, `sshll`), an add of a narrow operand
/// to a wide one (`uaddw`, `saddw`) and a multiply, with or without the
/// accumulate (`umull`, `smull`, `umlal`, `smlal`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VectorLongOp {
    Ushll,
    Sshll,
    Uaddw,
    Saddw,
    Umull,
    Smull,
    Umlal,
    Smlal,
}

impl VectorLongOp {
    /// (`U`, the base of the encoding) of the three-register forms and of the
    /// shift.
    pub(super) fn fields(self) -> (u32, u32) {
        match self {
            VectorLongOp::Ushll => (1, 0x0F00_A400),
            VectorLongOp::Sshll => (0, 0x0F00_A400),
            VectorLongOp::Uaddw => (1, 0x0E20_1000),
            VectorLongOp::Saddw => (0, 0x0E20_1000),
            VectorLongOp::Umull => (1, 0x0E20_C000),
            VectorLongOp::Smull => (0, 0x0E20_C000),
            VectorLongOp::Umlal => (1, 0x0E20_8000),
            VectorLongOp::Smlal => (0, 0x0E20_8000),
        }
    }

    pub fn mnemonic(self) -> &'static str {
        match self {
            VectorLongOp::Ushll => "ushll",
            VectorLongOp::Sshll => "sshll",
            VectorLongOp::Uaddw => "uaddw",
            VectorLongOp::Saddw => "saddw",
            VectorLongOp::Umull => "umull",
            VectorLongOp::Smull => "smull",
            VectorLongOp::Umlal => "umlal",
            VectorLongOp::Smlal => "smlal",
        }
    }

    pub fn is_shift(self) -> bool {
        matches!(self, VectorLongOp::Ushll | VectorLongOp::Sshll)
    }

    /// `smlal`, `umlal`: the product is added to `rd`.
    pub fn accumulates(self) -> bool {
        matches!(self, VectorLongOp::Umlal | VectorLongOp::Smlal)
    }

    /// `uaddw`, `saddw`: the first operand is wide.
    pub fn adds_wide(self) -> bool {
        matches!(self, VectorLongOp::Uaddw | VectorLongOp::Saddw)
    }
}

impl Arrangement {
    /// The lane size in bits.
    pub fn lane_bits(self) -> u32 {
        match self {
            Arrangement::B16 => 8,
            Arrangement::H8 => 16,
            Arrangement::S4 => 32,
            Arrangement::D2 => 64,
        }
    }

    /// `size` of the three-same encodings.
    pub(super) fn size(self) -> u32 {
        match self {
            Arrangement::B16 => 0,
            Arrangement::H8 => 1,
            Arrangement::S4 => 2,
            Arrangement::D2 => 3,
        }
    }
}

/// How a `movi`/`mvni` places its byte: `lsl` into a halfword or word lane, or
/// `msl` (shifting ones in).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImmShape {
    /// `.8h`, `lsl #0` or `#8`.
    H8(u8),
    /// `.4s`, `lsl #0`, `#8`, `#16` or `#24`.
    S4(u8),
    /// `.2s`, the same shifts: a 64-bit register (`Q` clear).
    S2(u8),
    /// `.4s`, `msl #8` or `#16`.
    S4Msl(u8),
}

impl ImmShape {
    /// `cmode` of the modified-immediate encoding.
    pub(super) fn cmode(self) -> u32 {
        match self {
            ImmShape::H8(shift) => 0b1000 | u32::from(shift / 8) << 1,
            ImmShape::S4(shift) | ImmShape::S2(shift) => u32::from(shift / 8) << 1,
            ImmShape::S4Msl(shift) => 0b1100 | u32::from(shift == 16),
        }
    }
}

impl Fill {
    /// Whether it writes all 128 bits; `movi d` and the `.2s` forms write 64.
    pub fn is_quad(&self) -> bool {
        !matches!(
            self,
            Fill::MaskD(_) | Fill::Imm { shape: ImmShape::S2(_), .. }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Fill {
    Zero,
    Byte(u8),
    /// `dup`: the low lane bits of a general register in every lane.
    Dup(Arrangement, Gpr),
    /// `movi`/`mvni v.<shape>, #imm8`: the byte placed by the shape, inverted
    /// by `mvni`.
    Imm {
        invert: bool,
        shape: ImmShape,
        imm8: u8,
    },
    /// `movi v.2d, #mask`: each bit of `imm8` is a whole byte of ones or zeros.
    Mask(u8),
    /// `movi d, #mask`: the same, into a double register.
    MaskD(u8),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FpUnary {
    Mov,
    Abs,
    Neg,
    Sqrt,
    /// `frintn`, `frintp`, `frintm`, `frintz`, `frinta`, `frintx`, `frinti`.
    Rint(Rounding),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Rounding {
    Nearest,
    Up,
    Down,
    Zero,
    Away,
    Exact,
    Current,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FpBinary {
    Mul,
    Div,
    Add,
    Sub,
    Max,
    Min,
    MaxNm,
    MinNm,
    Nmul,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FpFused {
    Madd,
    Msub,
    Nmadd,
    Nmsub,
}

/// A conversion between a general-purpose register and a floating-point one.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FpConvert {
    /// `scvtf` / `ucvtf` from an integer.
    ToFp { signed: bool },
    /// `fcvtzs` / `fcvtzu` to an integer.
    ToIntZero { signed: bool },
    /// `fmov` between a general register and the same-size float register.
    Bits { to_fp: bool },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FrameMark {
    PrologueEnd,
    EpilogueBegin,
    /// From here the source variable is the value in `reg`, or in none
    /// (`DBG_VALUE`): no code.
    Value {
        variable: llrm_mir::MetadataId,
        reg: Option<Gpr>,
    },
}

impl Inst {
    /// A pseudo-instruction: it encodes to no word.
    pub fn is_directive(&self) -> bool {
        matches!(self, Inst::Cfi(_) | Inst::Mark(_) | Inst::Label(_))
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Inst {
    /// Where the prologue ends or an epilogue begins: no bytes, it tells the
    /// line table (`prologue_end`, `epilogue_begin`).
    Mark(FrameMark),
    /// A label inside a block (the base of a jump table's offsets): no bytes.
    Label(String),
    /// A `.cfi_*` directive (`CFI_INSTRUCTION`): no bytes, it states what the
    /// instructions before it did to the frame.
    Cfi(llrm_object::assembler::Cfi),
    AddSubImm {
        op: AddSub,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        imm12: u16,
        shift12: bool,
    },
    AddSubShifted {
        op: AddSub,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        rm: Gpr,
        shift: Shift,
        amount: u8,
    },
    AddSubExtended {
        op: AddSub,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        rm: Gpr,
        extend: Extend,
        amount: u8,
    },
    /// `add rd, rn, #:lo12:symbol`, aimed at a symbol.
    AddPageOffset {
        rd: Gpr,
        rn: Gpr,
        target: Target,
    },
    LogicImm {
        op: Logic,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        imm: u64,
    },
    LogicShifted {
        op: Logic,
        invert: bool,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        rm: Gpr,
        shift: Shift,
        amount: u8,
    },
    MoveWide {
        op: Wide,
        width: Width,
        rd: Gpr,
        imm16: u16,
        lsl: u8,
    },
    BitfieldMove {
        op: Bitfield,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        immr: u8,
        imms: u8,
    },
    Extract {
        width: Width,
        rd: Gpr,
        rn: Gpr,
        rm: Gpr,
        lsb: u8,
    },
    Divide {
        op: Div,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        rm: Gpr,
    },
    OneSource {
        op: Unary,
        width: Width,
        rd: Gpr,
        rn: Gpr,
    },
    ThreeSource {
        op: Mul3,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        rm: Gpr,
        ra: Gpr,
    },
    CondSelect {
        op: Select,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        rm: Gpr,
        cond: Cond,
    },
    /// `adc`, `adcs`, `sbc`, `sbcs`: `rn +/- rm` and the carry flag.
    AddCarry {
        op: AddSub,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        rm: Gpr,
    },
    CondCompare {
        negate: bool,
        width: Width,
        rn: Gpr,
        rm_or_imm: CcOperand,
        nzcv: u8,
        cond: Cond,
    },
    Branch {
        link: bool,
        target: Target,
    },
    BranchCond {
        cond: Cond,
        target: Target,
    },
    CompareBranch {
        nonzero: bool,
        width: Width,
        rt: Gpr,
        target: Target,
    },
    TestBranch {
        nonzero: bool,
        rt: Gpr,
        bit: u8,
        target: Target,
    },
    BranchReg {
        link: bool,
        rn: Gpr,
    },
    Ret {
        rn: Gpr,
    },
    Load {
        mem: Memory,
        addr: Addr,
        mmo: MemOperand,
    },
    Store {
        mem: Memory,
        addr: Addr,
        mmo: MemOperand,
    },
    LoadPair {
        first: Memory,
        second: Memory,
        addr: Addr,
    },
    StorePair {
        first: Memory,
        second: Memory,
        addr: Addr,
    },
    Adr {
        page: bool,
        rd: Gpr,
        target: Target,
        /// `adrp` of the target's GOT slot (`@GOTPAGE`): the load that follows
        /// reads it at `@GOTPAGEOFF`.
        got: bool,
    },
    FpUnary {
        op: FpUnary,
        size: FpSize,
        rd: Vreg,
        rn: Vreg,
    },
    /// `mov vd.16b, vn.16b` (`orr`): all 128 bits, where `FpUnary::Mov` copies
    /// 64.
    VectorMov {
        rd: Vreg,
        rn: Vreg,
    },
    FpBinary {
        op: FpBinary,
        size: FpSize,
        rd: Vreg,
        rn: Vreg,
        rm: Vreg,
    },
    FpFused {
        op: FpFused,
        size: FpSize,
        rd: Vreg,
        rn: Vreg,
        rm: Vreg,
        ra: Vreg,
    },
    FpCompare {
        signalling: bool,
        size: FpSize,
        rn: Vreg,
        rm: Option<Vreg>,
    },
    FpSelect {
        size: FpSize,
        rd: Vreg,
        rn: Vreg,
        rm: Vreg,
        cond: Cond,
    },
    FpMoveImm {
        size: FpSize,
        rd: Vreg,
        imm8: u8,
    },
    FpToFp {
        to: FpSize,
        from: FpSize,
        rd: Vreg,
        rn: Vreg,
    },
    /// `scvtf`/`ucvtf s, s` (`d, d`): the integer in a vector register's low
    /// lane to a float of the same size.
    FpLaneToFp {
        signed: bool,
        size: FpSize,
        rd: Vreg,
        rn: Vreg,
    },
    FpInt {
        op: FpConvert,
        width: Width,
        size: FpSize,
        gpr: Gpr,
        fpr: Vreg,
    },
    /// `scvtf`/`ucvtf` from an integer or `fcvtzs`/`fcvtzu` to one, with
    /// `fbits` fractional bits (the value scaled by 2^fbits); `op` is
    /// `ToFp` or `ToIntZero`.
    FpFixed {
        op: FpConvert,
        width: Width,
        size: FpSize,
        gpr: Gpr,
        fpr: Vreg,
        fbits: u8,
    },
    /// A 16-byte vector from an immediate or a general register: `movi v.2d,
    /// #0`, `movi v.16b, #imm`, `dup v.8h, w`, `movi v.8h, #imm, lsl #8`.
    VectorFill {
        rd: Vreg,
        fill: Fill,
    },
    /// A 16-byte integer vector operation on every lane: `add v.8h, v.8h,
    /// v.8h`. The logical operations ignore `lanes` (they are `.16b`).
    VectorBinary {
        op: VectorOp,
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
        rm: Vreg,
    },
    /// A 16-byte integer vector shifted by an immediate: `shl v.8h, v.8h, #3`.
    VectorShift {
        op: VectorShift,
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
        amount: u8,
    },
    /// `ushll v.8h, v.8b, #0` (`extend`), `ushll2 v.8h, v.16b, #0`: the low or
    /// high half of the 16-byte `rn` (the 8-byte register for the low half)
    /// shifted left, lanes twice as wide.
    VectorLongShift {
        op: VectorLongOp,
        lane: NarrowLane,
        high: bool,
        rd: Vreg,
        rn: Vreg,
        amount: u8,
    },
    /// `uaddw v.8h, v.8h, v.8b`, `umull v.8h, v.8b, v.8b` and their `2` forms:
    /// the narrow `rm` (the high half of it with `high`), and `rn` (narrow
    /// too for `umull`), into wide lanes.
    VectorLong {
        op: VectorLongOp,
        lane: NarrowLane,
        high: bool,
        rd: Vreg,
        rn: Vreg,
        rm: Vreg,
    },
    /// `not v.16b` (`mvn`).
    VectorNot {
        rd: Vreg,
        rn: Vreg,
    },
    /// A 16-byte float vector operation on every lane: `fadd v.4s, v.4s,
    /// v.4s`; `lanes` is `S4` or `D2`.
    VectorFpBinary {
        op: VectorFpOp,
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
        rm: Vreg,
    },
    /// `fmla v.4s, v.4s, v.4s`: `rd` is read and written; `lanes` is `S4` or
    /// `D2`.
    VectorFma {
        op: VectorFma,
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
        rm: Vreg,
    },
    /// `dup v.4s, v.s[index]`: one lane of `rn` in every lane of `rd` (a
    /// scalar float register is lane 0 of its vector register).
    VectorDupLane {
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
        index: u8,
    },
    /// `addv s, v.4s`: every lane of `rn` reduced into the scalar register
    /// `rd` of the lane's size (`ADDVv4i32v`); `addp d, v.2d` for `D2`
    /// (`ADDPv2i64p`).
    VectorReduce {
        op: VectorReduce,
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
    },
    /// `ins v.d[index], x`: a general register into one lane of `rd`, the
    /// others kept (`INSvi64gpr`; a `w` register for the narrower lanes).
    VectorInsGpr {
        lanes: Arrangement,
        rd: Vreg,
        rn: Gpr,
        index: u8,
    },
    /// `fmla v.4s, v.4s, v.s[index]`: `rd` is read and written; `lanes` is
    /// `S4` or `D2`.
    VectorFmaLane {
        op: VectorFma,
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
        rm: Vreg,
        index: u8,
    },
    /// `mov d0, v1.d[index]` (`DUPi64`; `s`, `h`, `b` for the others): one
    /// lane of `rn` as a scalar.
    VectorDupScalar {
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
        index: u8,
    },
    /// `uzp1 v.8h, v.8h, v.8h` (`UZP1v8i16`; `uzp2`): the even (`second`: the
    /// odd) lanes of `rn`, then of `rm`.
    VectorUnzip {
        second: bool,
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
        rm: Vreg,
    },
    /// `xtn{2} v.8b, v.8h`: each lane of `rn` (`wide`'s lanes) halved, into
    /// the low half of `rd` or, `high`, its top half (`rd` is then read too).
    VectorNarrow {
        high: bool,
        wide: Arrangement,
        rd: Vreg,
        rn: Vreg,
    },
    /// `fmul v.4s, v.4s, v.s[index]` (`FMULv4i32_indexed`): `lanes` is `S4` or
    /// `D2`; nothing of `rd` is read.
    VectorFmulLane {
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
        rm: Vreg,
        index: u8,
    },
    /// `mul v.4s, v.4s, v.s[index]` (`MULv4i32_indexed`): nothing of `rd` is
    /// read. DEVIATE MUL5 (owner llrm-arm64-vec, exit 2026-11-27): the `.8h`
    /// form takes `rm` from `v0`..`v15`, which the allocator does not keep to.
    VectorMulLane {
        rd: Vreg,
        rn: Vreg,
        rm: Vreg,
        index: u8,
    },
    /// `st1 { v.s }[index], [base]`: one lane of `rt` stored at `[base]`
    /// (`ST1i32`).
    VectorStoreLane {
        lanes: Arrangement,
        rt: Vreg,
        index: u8,
        base: Gpr,
    },
    /// `umov w, v.s[index]`: one lane of `rn` zero-extended into a general
    /// register (`UMOVvi32`; an `x` register for the 64-bit lane).
    VectorUmov {
        lanes: Arrangement,
        rd: Gpr,
        rn: Vreg,
        index: u8,
    },
    /// `ins v.s[index], v.s[src]`: lane `src` of `rn` into lane `index` of
    /// `rd`, the others kept (`INSvi32lane`).
    VectorInsLane {
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
        index: u8,
        src: u8,
    },
    /// `fneg v.4s, v.4s` and `fabs`; `lanes` is `S4` or `D2`.
    VectorFpUnary {
        op: VectorFpUnary,
        lanes: Arrangement,
        rd: Vreg,
        rn: Vreg,
    },
    Svc {
        imm16: u16,
    },
    /// An atomic access through `[rn]`: a load-acquire or store-release, or one
    /// of the LSE read-modify-writes and `cas`, which order as `acquire`
    /// and `release` say. `rs` is the operand (the value to combine, or
    /// `cas`'s expected value, which it overwrites with the old one) and `rt`
    /// the register the old value comes to (the data of a store-release;
    /// `cas`'s new value).
    Atomic {
        op: Atomic,
        /// log2 of the access's bytes.
        size: u8,
        acquire: bool,
        release: bool,
        rs: Gpr,
        rt: Gpr,
        rn: Gpr,
    },
    /// `dmb`.
    Dmb {
        barrier: Barrier,
    },
    Brk {
        imm16: u16,
    },
    Nop,
    /// `prfm <op>, [<base>]`: a hint that the line at `base` is wanted; `op` is
    /// the 5-bit prefetch operation.
    Prefetch {
        op: u8,
        base: Gpr,
    },
}

/// The second operand of a conditional compare.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CcOperand {
    Reg(Gpr),
    Imm5(u8),
}

impl fmt::Display for Cond {
    fn fmt(
        &self,
        out: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        const NAMES: [&str; 16] =
            ["eq", "ne", "hs", "lo", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le", "al", "nv"];
        out.write_str(NAMES[*self as usize])
    }
}
