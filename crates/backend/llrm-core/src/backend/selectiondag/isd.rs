//! Node opcodes ([`ISD`]) and condition codes ([`CondCode`]), the pin's
//! `ISDOpcodes.h` for the scalar subset.

/// Generic opcodes; `Target` carries the X86ISD ones (`>= BUILTIN_OP_END`
/// in the pin) and `Machine` an instruction chosen by selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ISD {
    EntryToken,
    TokenFactor,
    Constant,
    TargetConstant,
    ConstantFP,
    TargetConstantFP,
    Undef,
    Poison,
    Register,
    GlobalAddress,
    TargetGlobalAddress,
    ExternalSymbol,
    FrameIndex,
    TargetFrameIndex,
    ConstantPool,
    BasicBlock,
    ValueType,
    CondCodeNode,
    MergeValues,
    CopyToReg,
    CopyFromReg,

    Add,
    Sub,
    Mul,
    SDiv,
    UDiv,
    SRem,
    URem,
    MulHS,
    MulHU,
    SMulLoHi,
    UMulLoHi,
    SDivRem,
    UDivRem,
    AddC,
    AddE,
    SubC,
    SubE,
    UAddO,
    USubO,
    SAddO,
    SSubO,
    UMulO,
    SMulO,
    UAddOCarry,
    USubOCarry,
    SAddSat,
    UAddSat,
    SSubSat,
    USubSat,
    SMin,
    SMax,
    UMin,
    UMax,
    Abs,
    PtrAdd,
    And,
    Or,
    Xor,
    Shl,
    Srl,
    Sra,
    Rotl,
    Rotr,
    FShl,
    FShr,
    BSwap,
    BitReverse,
    CtPop,
    CtLz,
    CtLzZeroPoison,
    CtTz,
    CtTzZeroPoison,
    Parity,
    CtLs,

    Truncate,
    ZeroExtend,
    SignExtend,
    AnyExtend,
    SignExtendInReg,
    BitCast,
    BuildPair,
    ExtractElement,
    SIntToFp,
    UIntToFp,
    FpToSInt,
    FpToUInt,
    FpExtend,
    FpRound,
    FAdd,
    FSub,
    FMul,
    FDiv,
    FRem,
    FNeg,
    FAbs,
    FSqrt,
    FCopySign,

    SetCC,
    Select,
    SelectCC,
    BrCC,
    BrCond,
    Br,
    BrJT,
    BrInd,

    Load,
    Store,
    CallSeqStart,
    CallSeqEnd,
    DynamicStackAlloc,
    Trap,

    Target(u16),
    Machine(u32),
}

impl ISD {
    /// `TargetLowering::isCommutativeBinOp` for the generic opcodes.
    pub fn is_commutative_binop(self) -> bool {
        use ISD::*;
        matches!(
            self,
            Add | Mul | MulHU | MulHS | SMulLoHi | UMulLoHi | And | Or | Xor | SMin | SMax | UMin | UMax | SAddSat | UAddSat | FAdd | FMul | UAddO | SAddO | UMulO | SMulO | AddC | AddE
        )
    }
}

/// `ISD::CondCode`, with the pin's values: the bit layout is relied on by
/// [`CondCode::swapped_operands`], [`CondCode::true_when_equal`] and the
/// inverse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum CondCode {
    SetFalse,
    SetOEq,
    SetOGt,
    SetOGe,
    SetOLt,
    SetOLe,
    SetONe,
    SetO,
    SetUo,
    SetUEq,
    SetUGt,
    SetUGe,
    SetULt,
    SetULe,
    SetUNe,
    SetTrue,
    SetFalse2,
    SetEq,
    SetGt,
    SetGe,
    SetLt,
    SetLe,
    SetNe,
    SetTrue2,
}

impl CondCode {
    const ALL: [CondCode; 24] = {
        use CondCode::*;
        [
            SetFalse, SetOEq, SetOGt, SetOGe, SetOLt, SetOLe, SetONe, SetO, SetUo, SetUEq, SetUGt, SetUGe, SetULt, SetULe, SetUNe, SetTrue, SetFalse2, SetEq, SetGt, SetGe, SetLt,
            SetLe, SetNe, SetTrue2,
        ]
    };

    fn from_bits(bits: u8) -> CondCode {
        CondCode::ALL[bits as usize]
    }

    /// `ISD::isTrueWhenEqual`.
    pub fn true_when_equal(self) -> bool {
        self as u8 & 1 != 0
    }

    /// `ISD::getSetCCSwappedOperands`: the L and G bits swap.
    pub fn swapped_operands(self) -> CondCode {
        let bits = self as u8;
        let (l, g) = ((bits >> 2) & 1, (bits >> 1) & 1);
        CondCode::from_bits((bits & !6) | (l << 1) | (g << 2))
    }

    /// `ISD::getSetCCInverse` for an integer type.
    pub fn inverse_integer(self) -> CondCode {
        let mut bits = self as u8 ^ 7;
        if bits > CondCode::SetTrue2 as u8 {
            bits &= !8;
        }
        CondCode::from_bits(bits)
    }

    pub fn is_signed_int(self) -> bool {
        matches!(self, CondCode::SetGt | CondCode::SetGe | CondCode::SetLt | CondCode::SetLe)
    }

    pub fn is_unsigned_int(self) -> bool {
        matches!(self, CondCode::SetUGt | CondCode::SetUGe | CondCode::SetULt | CondCode::SetULe)
    }
}
