//! An A64 instruction as the machine states it, before it is encoded: what
//! LLVM's `MCInst` carries for the `AArch64` target. One variant per encoding
//! class of `AArch64InstrFormats.td`
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64InstrFormats.td>),
//! its opcode a field, so a new mnemonic of a class is a new value, not a new
//! branch.

use std::fmt;

/// The register width of an integer instruction: the `sf` bit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum MemObject {
    #[default]
    Unknown,
    /// A global, by symbol.
    Global(String),
    /// A frame object.
    Slot(u32),
}

/// What a load or store reaches (`MachineMemOperand`): its object and the byte
/// offset in it, where both are known.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MemOperand {
    pub object: MemObject,
    pub offset: Option<i64>,
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
                MemObject::Unknown => None,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Target {
    /// Bytes from the instruction itself.
    Offset(i64),
    /// A symbol and an addend: left for the linker.
    Symbol { name: String, addend: i64 },
    /// A label of the same function: the object builder resolves it.
    Label(String),
}

/// `add`/`sub` and their flag-setting forms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddSub {
    pub subtract: bool,
    pub set_flags: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Logic {
    And,
    Orr,
    Eor,
    Ands,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wide {
    /// `movn`
    Not,
    /// `movz`
    Zero,
    /// `movk`
    Keep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bitfield {
    Sbfm,
    Bfm,
    Ubfm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Div {
    Udiv,
    Sdiv,
    Lslv,
    Lsrv,
    Asrv,
    Rorv,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unary {
    Rbit,
    Rev16,
    /// `rev`: reverses the register's bytes.
    Rev,
    Clz,
    Cls,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Select {
    Csel,
    Csinc,
    Csinv,
    Csneg,
}

/// How a load or store forms its address.
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Memory {
    /// An integer register: `bytes` 1, 2, 4 or 8; `sign` extends a narrower
    /// load to `width`.
    Int { width: Width, bytes: u8, sign: bool, reg: Gpr },
    /// A floating-point or vector register of `bytes` 1, 2, 4, 8 or 16.
    Fp { bytes: u8, reg: Vreg },
}

/// What an [`Inst::Atomic`] does: `ldar`, `ldapr`, `stlr`, `swp`, `ldadd`,
/// `ldclr`, `ldeor`, `ldset`, `ldsmax`, `ldsmin`, `ldumax`, `ldumin`, `cas`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Barrier {
    Ish,
    IshLd,
    IshSt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fill {
    Zero,
    Byte(u8),
    /// The low byte of a general register.
    Gpr(Gpr),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FpUnary {
    Mov,
    Abs,
    Neg,
    Sqrt,
    /// `frintn`, `frintp`, `frintm`, `frintz`, `frinta`, `frintx`, `frinti`.
    Rint(Rounding),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rounding {
    Nearest,
    Up,
    Down,
    Zero,
    Away,
    Exact,
    Current,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FpFused {
    Madd,
    Msub,
    Nmadd,
    Nmsub,
}

/// A conversion between a general-purpose register and a floating-point one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FpConvert {
    /// `scvtf` / `ucvtf` from an integer.
    ToFp { signed: bool },
    /// `fcvtzs` / `fcvtzu` to an integer.
    ToIntZero { signed: bool },
    /// `fmov` between a general register and the same-size float register.
    Bits { to_fp: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameMark {
    PrologueEnd,
    EpilogueBegin,
}

impl Inst {
    /// A pseudo-instruction: it encodes to no word.
    pub fn is_directive(&self) -> bool {
        matches!(self, Inst::Cfi(_) | Inst::Mark(_))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Inst {
    /// Where the prologue ends or an epilogue begins: no bytes, it tells the
    /// line table (`prologue_end`, `epilogue_begin`).
    Mark(FrameMark),
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
    /// A 16-byte vector with every byte `fill`: `movi v.2d, #0`, `movi v.16b,
    /// #imm`, `dup v.16b, w`.
    VectorFill {
        rd: Vreg,
        fill: Fill,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
