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
    RegisterMask,
    JumpTable,
    TargetExternalSymbolNode,
    LifetimeStart,
    LifetimeEnd,
    VAStart,
    VAEnd,
    VACopy,
    /// `SRCVALUE`: the IR pointer an access came from, for the target.
    SrcValue,
    GlobalAddress,
    TargetGlobalAddress,
    ExternalSymbol,
    FrameIndex,
    TargetFrameIndex,
    ConstantPool,
    TargetConstantPool,
    TargetJumpTable,
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
    SetCCCarry,
    SAddSat,
    UAddSat,
    SSubSat,
    USubSat,
    SMin,
    SMax,
    UMin,
    UMax,
    Abs,
    /// `ASSERT_ZEXT` / `ASSERT_SEXT`: the value's bits above the type in
    /// operand 1 are zero / copies of its sign bit. Selection drops them.
    AssertZext,
    AssertSext,
    /// `ABS_MIN_POISON`: `abs` whose minimum value is poison.
    AbsMinPoison,
    PtrAdd,
    And,
    Or,
    Xor,
    Shl,
    Srl,
    Sra,
    Rotl,
    Rotr,
    ShlParts,
    SraParts,
    SrlParts,
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
    /// `FREEZE`: stops undef and poison propagating.
    Freeze,
    SCmp,
    UCmp,
    SMulFix,
    UMulFix,
    SMulFixSat,
    UMulFixSat,
    SDivFix,
    SShlSat,
    UShlSat,
    StackSave,
    StackRestore,
    FSin,
    FCos,
    FAtan,
    FLog,
    FLog2,
    FExp,
    FExp2,
    FRint,
    LRint,
    LLRint,
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
    /// `LOAD`/`STORE` with an indexed addressing mode (`MemIndexedMode`), made
    /// after legalization (`getIndexedLoad`/`getIndexedStore`): `PRE_INC`
    /// writes back `base + offset` before the access, `POST_INC` after it.
    /// A load's values are (value, new base, chain) and its operands (chain,
    /// base, offset); a store's values are (new base, chain) and its
    /// operands (chain, value, base, offset).
    LoadPre,
    LoadPost,
    StorePre,
    StorePost,
    /// `ATOMIC_SWAP`: `(chain, ptr, value)` to the old value and a chain.
    AtomicSwap,
    /// `ATOMIC_CMP_SWAP`: `(chain, ptr, expected, new)` to the old value and a
    /// chain.
    AtomicCmpSwap,
    /// `ATOMIC_CMP_SWAP_WITH_SUCCESS`: `(chain, ptr, expected, new)` to the old
    /// value, a success flag and a chain.
    AtomicCmpSwapWithSuccess,
    /// `ATOMIC_LOAD_ADD` and its siblings: `(chain, ptr, value)` to the old
    /// value and a chain.
    AtomicLoadAdd,
    AtomicLoadSub,
    AtomicLoadAnd,
    AtomicLoadOr,
    AtomicLoadXor,
    /// `ATOMIC_LOAD_CLR`: and with the complement.
    AtomicLoadClr,
    /// `ATOMIC_LOAD` and `ATOMIC_STORE`.
    AtomicLoad,
    AtomicStore,
    /// `ATOMIC_FENCE`: `(chain, ordering, scope)` as target constants, to a
    /// chain.
    AtomicFence,
    CallSeqStart,
    CallSeqEnd,
    DynamicStackAlloc,
    Trap,
    /// `INTRINSIC_WO_CHAIN`: `(id, args...)`; `INTRINSIC_W_CHAIN`:
    /// `(chain, id, args...)` to the result and a chain; `INTRINSIC_VOID`:
    /// `(chain, id, args...)` to a chain. `id` is a target constant the
    /// target numbers.
    IntrinsicWoChain,
    IntrinsicWChain,
    IntrinsicVoid,

    Target(u16),
    Machine(u32),
}

impl ISD {
    /// `SDNode::getOperationName`, as the dump prints it.
    pub fn name(self) -> String {
        use ISD::*;
        let name = match self {
            EntryToken => "EntryToken",
            TokenFactor => "TokenFactor",
            Constant => "Constant",
            TargetConstant => "TargetConstant",
            ConstantFP => "ConstantFP",
            TargetConstantFP => "TargetConstantFP",
            Undef => "undef",
            Poison => "poison",
            Register => "Register",
            RegisterMask => "RegisterMask",
            JumpTable => "JumpTable",
            TargetExternalSymbolNode => "TargetExternalSymbol",
            LifetimeStart => "lifetime.start",
            LifetimeEnd => "lifetime.end",
            VAStart => "vastart",
            VAEnd => "vaend",
            VACopy => "vacopy",
            SrcValue => "SrcValue",
            GlobalAddress => "GlobalAddress",
            TargetGlobalAddress => "TargetGlobalAddress",
            ExternalSymbol => "ExternalSymbol",
            FrameIndex => "FrameIndex",
            TargetFrameIndex => "TargetFrameIndex",
            ConstantPool => "ConstantPool",
            TargetConstantPool => "TargetConstantPool",
            TargetJumpTable => "TargetJumpTable",
            BasicBlock => "BasicBlock",
            ValueType => "ValueType",
            CondCodeNode => "CONDCODE",
            MergeValues => "merge_values",
            CopyToReg => "CopyToReg",
            CopyFromReg => "CopyFromReg",
            Add => "add",
            Sub => "sub",
            Mul => "mul",
            SDiv => "sdiv",
            UDiv => "udiv",
            SRem => "srem",
            URem => "urem",
            MulHS => "mulhs",
            MulHU => "mulhu",
            SMulLoHi => "smul_lohi",
            UMulLoHi => "umul_lohi",
            SDivRem => "sdivrem",
            UDivRem => "udivrem",
            AddC => "addc",
            AddE => "adde",
            SubC => "subc",
            SubE => "sube",
            UAddO => "uaddo",
            USubO => "usubo",
            SAddO => "saddo",
            SSubO => "ssubo",
            UMulO => "umulo",
            SMulO => "smulo",
            UAddOCarry => "uaddo_carry",
            USubOCarry => "usubo_carry",
            SetCCCarry => "setcccarry",
            SAddSat => "saddsat",
            UAddSat => "uaddsat",
            SSubSat => "ssubsat",
            USubSat => "usubsat",
            SMin => "smin",
            SMax => "smax",
            UMin => "umin",
            UMax => "umax",
            Abs => "abs",
            AssertZext => "assertzext",
            AssertSext => "assertsext",
            AbsMinPoison => "abs_min_poison",
            PtrAdd => "ptradd",
            And => "and",
            Or => "or",
            Xor => "xor",
            Shl => "shl",
            Srl => "srl",
            Sra => "sra",
            Rotl => "rotl",
            Rotr => "rotr",
            ShlParts => "shl_parts",
            SraParts => "sra_parts",
            SrlParts => "srl_parts",
            FShl => "fshl",
            FShr => "fshr",
            BSwap => "bswap",
            BitReverse => "bitreverse",
            CtPop => "ctpop",
            CtLz => "ctlz",
            CtLzZeroPoison => "ctlz_zero_poison",
            CtTz => "cttz",
            CtTzZeroPoison => "cttz_zero_poison",
            Parity => "parity",
            CtLs => "ctls",
            Truncate => "truncate",
            ZeroExtend => "zero_extend",
            SignExtend => "sign_extend",
            AnyExtend => "any_extend",
            SignExtendInReg => "sign_extend_inreg",
            BitCast => "bitcast",
            BuildPair => "build_pair",
            ExtractElement => "extract_element",
            SIntToFp => "sint_to_fp",
            UIntToFp => "uint_to_fp",
            FpToSInt => "fp_to_sint",
            FpToUInt => "fp_to_uint",
            FpExtend => "fp_extend",
            FpRound => "fp_round",
            FAdd => "fadd",
            FSub => "fsub",
            FMul => "fmul",
            FDiv => "fdiv",
            FRem => "frem",
            FNeg => "fneg",
            FAbs => "fabs",
            FSqrt => "fsqrt",
            Freeze => "freeze",
            SCmp => "scmp",
            UCmp => "ucmp",
            SMulFix => "smulfix",
            UMulFix => "umulfix",
            SMulFixSat => "smulfixsat",
            UMulFixSat => "umulfixsat",
            SShlSat => "sshlsat",
            UShlSat => "ushlsat",
            SDivFix => "sdivfix",
            StackSave => "stacksave",
            StackRestore => "stackrestore",
            FSin => "fsin",
            FCos => "fcos",
            FAtan => "fatan",
            FLog => "flog",
            FLog2 => "flog2",
            FExp => "fexp",
            FExp2 => "fexp2",
            FRint => "frint",
            LRint => "lrint",
            LLRint => "llrint",
            FCopySign => "fcopysign",
            SetCC => "setcc",
            Select => "select",
            SelectCC => "select_cc",
            BrCC => "br_cc",
            BrCond => "brcond",
            Br => "br",
            BrJT => "br_jt",
            BrInd => "brind",
            Load => "load",
            Store => "store",
            LoadPre | LoadPost => "load<indexed>",
            StorePre | StorePost => "store<indexed>",
            AtomicSwap => "atomic_swap",
            AtomicCmpSwap => "atomic_cmp_swap",
            AtomicCmpSwapWithSuccess => "atomic_cmp_swap_with_success",
            AtomicLoadAdd => "atomic_load_add",
            AtomicLoadSub => "atomic_load_sub",
            AtomicLoadAnd => "atomic_load_and",
            AtomicLoadOr => "atomic_load_or",
            AtomicLoadXor => "atomic_load_xor",
            AtomicLoadClr => "atomic_load_clr",
            AtomicLoad => "atomic_load",
            AtomicStore => "atomic_store",
            AtomicFence => "atomic_fence",
            CallSeqStart => "callseq_start",
            CallSeqEnd => "callseq_end",
            DynamicStackAlloc => "dynamic_stackalloc",
            Trap => "trap",
            IntrinsicWoChain => "intrinsic_wo_chain",
            IntrinsicWChain => "intrinsic_w_chain",
            IntrinsicVoid => "intrinsic_void",
            Target(code) => return super::x86isd::name(code),
            Machine(code) => return format!("<<Unknown Machine Node #{code}>>"),
        };
        name.to_string()
    }

    /// `TargetLowering::isCommutativeBinOp` for the generic opcodes.
    pub fn is_commutative_binop(self) -> bool {
        use ISD::*;
        matches!(
            self,
            Add | Mul
                | MulHU
                | MulHS
                | SMulLoHi
                | UMulLoHi
                | And
                | Or
                | Xor
                | SMin
                | SMax
                | UMin
                | UMax
                | SAddSat
                | UAddSat
                | FAdd
                | FMul
                | UAddO
                | SAddO
                | UMulO
                | SMulO
                | AddC
                | AddE
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
    /// The codes by their number, as `ISD::CondCode` numbers them.
    pub const ALL_CODES: [CondCode; 24] = Self::ALL;

    const ALL: [CondCode; 24] = {
        use CondCode::*;
        [
            SetFalse, SetOEq, SetOGt, SetOGe, SetOLt, SetOLe, SetONe, SetO, SetUo, SetUEq, SetUGt, SetUGe, SetULt,
            SetULe, SetUNe, SetTrue, SetFalse2, SetEq, SetGt, SetGe, SetLt, SetLe, SetNe, SetTrue2,
        ]
    };

    fn from_bits(bits: u8) -> CondCode {
        CondCode::ALL[bits as usize]
    }

    /// `ISD::isSignedOp` of an integer code: bit 0 signed, bit 1 unsigned.
    fn signedness(self) -> u8 {
        match self {
            CondCode::SetLt | CondCode::SetLe | CondCode::SetGt | CondCode::SetGe => 1,
            CondCode::SetULt | CondCode::SetULe | CondCode::SetUGt | CondCode::SetUGe => 2,
            _ => 0,
        }
    }

    /// `ISD::getSetCCOrOperation`: the code true when either is; `None` is
    /// `SETCC_INVALID`.
    pub fn or_operation(
        self,
        other: CondCode,
        integer: bool,
    ) -> Option<CondCode> {
        if integer && (self.signedness() | other.signedness()) == 3 {
            return None;
        }
        let mut bits = self as u8 | other as u8;
        if bits > CondCode::SetTrue2 as u8 {
            bits &= !16;
        }
        if integer && bits == CondCode::SetUNe as u8 {
            bits = CondCode::SetNe as u8;
        }
        Some(CondCode::from_bits(bits))
    }

    /// `ISD::getSetCCAndOperation`: the code true when both are.
    pub fn and_operation(
        self,
        other: CondCode,
        integer: bool,
    ) -> Option<CondCode> {
        if integer && (self.signedness() | other.signedness()) == 3 {
            return None;
        }
        let mut bits = self as u8 & other as u8;
        if bits > CondCode::SetTrue2 as u8 {
            bits &= !16;
        }
        let mut code = CondCode::from_bits(bits);
        if integer {
            code = match code {
                CondCode::SetUo => CondCode::SetFalse,
                CondCode::SetOEq | CondCode::SetUEq => CondCode::SetEq,
                CondCode::SetOLt => CondCode::SetULt,
                CondCode::SetOGt => CondCode::SetUGt,
                other => other,
            };
        }
        Some(code)
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

    /// `ISD::getSetCCInverse` for a floating-point type: every condition bit
    /// flips, the unordered one too.
    pub fn inverse_float(self) -> CondCode {
        let mut bits = self as u8 ^ 15;
        if bits > CondCode::SetTrue2 as u8 {
            bits &= !8;
        }
        CondCode::from_bits(bits)
    }

    /// The name the dump prints for a `CONDCODE` operand.
    pub fn name(self) -> &'static str {
        use CondCode::*;
        match self {
            SetFalse => "setfalse",
            SetOEq => "setoeq",
            SetOGt => "setogt",
            SetOGe => "setoge",
            SetOLt => "setolt",
            SetOLe => "setole",
            SetONe => "setone",
            SetO => "seto",
            SetUo => "setuo",
            SetUEq => "setueq",
            SetUGt => "setugt",
            SetUGe => "setuge",
            SetULt => "setult",
            SetULe => "setule",
            SetUNe => "setune",
            SetTrue => "settrue",
            SetFalse2 => "setfalse2",
            SetEq => "seteq",
            SetGt => "setgt",
            SetGe => "setge",
            SetLt => "setlt",
            SetLe => "setle",
            SetNe => "setne",
            SetTrue2 => "settrue2",
        }
    }

    pub fn is_signed_int(self) -> bool {
        matches!(
            self,
            CondCode::SetGt | CondCode::SetGe | CondCode::SetLt | CondCode::SetLe
        )
    }

    pub fn is_unsigned_int(self) -> bool {
        matches!(
            self,
            CondCode::SetUGt | CondCode::SetUGe | CondCode::SetULt | CondCode::SetULe
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The atomic read-modify-write nodes have names of their own, which the
    /// DAG dump and a selector's refusals print.
    #[test]
    fn the_atomic_nodes_are_named() {
        assert_eq!(ISD::AtomicLoadAdd.name(), "atomic_load_add");
        assert_eq!(ISD::AtomicCmpSwap.name(), "atomic_cmp_swap");
        assert_eq!(ISD::AtomicCmpSwapWithSuccess.name(), "atomic_cmp_swap_with_success");
        assert_eq!(ISD::AtomicStore.name(), "atomic_store");
        assert_eq!(ISD::AtomicFence.name(), "atomic_fence");
    }
}
