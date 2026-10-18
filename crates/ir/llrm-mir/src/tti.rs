//! `TargetTransformInfo`'s interface: what an instruction costs on the target,
//! for the readers that price code (InlineCost, the SCEV expander, loop
//! strength reduction, unrolling). A target implements [`Tti`]; the X86 one is
//! `llrm_x86::tti`, translated from `BasicTTIImplBase` and `X86TTIImpl`.
//!
//! # How it works
//! Every query answers a [`Cost`] for one instruction of a scalar type under a
//! [`CostKind`], as the pinned `opt -passes='print<cost-model>'` does for the
//! target (the target crate checks its rows against `opt`). Vector types have
//! no [`Ty`] (T1).
//!
//! # See also
//! `llrm_x86::tti`; the plan page `tti-plan` in docs/architecture.

/// `InstructionCost`: a number of units, or invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cost {
    pub value: i64,
    pub valid: bool,
}

impl Cost {
    pub const FREE: Cost = Cost { value: 0, valid: true };
    pub const BASIC: Cost = Cost { value: 1, valid: true };
    pub const EXPENSIVE: Cost = Cost { value: 4, valid: true };
    pub const INVALID: Cost = Cost { value: 0, valid: false };

    pub const fn of(value: i64) -> Cost {
        Cost { value, valid: true }
    }
}

impl std::ops::Add for Cost {
    type Output = Cost;

    fn add(
        self,
        other: Cost,
    ) -> Cost {
        Cost { value: self.value + other.value, valid: self.valid && other.valid }
    }
}

impl std::ops::Mul<i64> for Cost {
    type Output = Cost;

    fn mul(
        self,
        count: i64,
    ) -> Cost {
        Cost { value: self.value * count, valid: self.valid }
    }
}

/// `TTI::TargetCostKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CostKind {
    RecipThroughput,
    Latency,
    CodeSize,
    SizeAndLatency,
}

impl CostKind {
    pub fn index(self) -> usize {
        self as usize
    }
}

/// A scalar type as the cost model sees it (T1: vectors are not modelled).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ty {
    Int(u32),
    Float,
    Double,
    X87,
    Pointer,
}

impl Ty {
    /// The cost model's view of an MIR type; none for a vector, aggregate or
    /// other (T1).
    pub fn of(
        types: &crate::types::Types,
        ty: crate::types::TypeId,
    ) -> Option<Ty> {
        use crate::types::{FloatKind, Type};
        match types.get(ty) {
            Type::Int(bits) => Some(Ty::Int(*bits)),
            Type::Float(FloatKind::Float) => Some(Ty::Float),
            Type::Float(FloatKind::Double) => Some(Ty::Double),
            Type::Float(FloatKind::X86Fp80) => Some(Ty::X87),
            Type::Pointer(_) => Some(Ty::Pointer),
            _ => None,
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, Ty::Float | Ty::Double | Ty::X87)
    }
}

/// `TTI::OperandValueInfo`: what is known of an operand's value.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OperandInfo {
    pub constant: bool,
    pub power_of_two: bool,
    pub negated_power_of_two: bool,
}

impl OperandInfo {
    pub const ANY: OperandInfo = OperandInfo { constant: false, power_of_two: false, negated_power_of_two: false };

    /// A constant operand of this value at `width` bits.
    pub fn constant(
        value: i128,
        width: u32,
    ) -> OperandInfo {
        let bits = if width >= 128 { value as u128 } else { (value as u128) & ((1u128 << width) - 1) };
        let negated = if width >= 128 {
            (value.wrapping_neg()) as u128
        } else {
            ((value.wrapping_neg()) as u128) & ((1u128 << width) - 1)
        };
        OperandInfo {
            constant: true,
            power_of_two: bits != 0 && bits & (bits - 1) == 0,
            negated_power_of_two: negated != 0 && negated & (negated - 1) == 0,
        }
    }
}

/// The binary operations a cost is asked of, `FNeg` included
/// (`getArithmeticInstrCost`'s opcodes).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    SDiv,
    UDiv,
    SRem,
    URem,
    Shl,
    LShr,
    AShr,
    And,
    Or,
    Xor,
    FAdd,
    FSub,
    FMul,
    FDiv,
    FRem,
    FNeg,
}

/// The casts a cost is asked of (`getCastInstrCost`'s opcodes).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastKind {
    Trunc,
    ZExt,
    SExt,
    FPTrunc,
    FPExt,
    FPToUI,
    FPToSI,
    UIToFP,
    SIToFP,
    PtrToInt,
    IntToPtr,
    BitCast,
}

/// A comparison or a select (`getCmpSelInstrCost`'s opcodes).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CmpSel {
    ICmp,
    FCmp,
    Select,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryOp {
    Load,
    Store,
}

/// The instruction a constant is an operand of (`getIntImmCostInst`'s opcodes).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmUser {
    GetElementPtr,
    Store,
    ICmp,
    And,
    Add,
    Sub,
    Divide,
    Mul,
    Or,
    Xor,
    Shift,
    /// A cast, `phi`, call, select, `ret` or load.
    Other,
    /// Anything else: free.
    Unknown,
}

/// `TTI::LSRCost`: what loop strength reduction counts of a candidate solution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LsrCost {
    pub insns: u32,
    pub num_regs: u32,
    pub add_rec_cost: u32,
    pub num_iv_muls: u32,
    pub num_base_adds: u32,
    pub scale_cost: u32,
    pub imm_cost: u32,
    pub setup_cost: u32,
}

/// The target a cost is for.
pub trait Tti {
    /// `getCmpSelInstrCost`.
    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost;

    /// `getMemoryOpCost`; `stored_constant`: the stored value is a constant.
    fn memory_op_cost(
        &self,
        op: MemoryOp,
        ty: Ty,
        kind: CostKind,
        stored_constant: bool,
    ) -> Cost;

    /// `getCFInstrCost` of a `phi` or a branch.
    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: CostKind,
    ) -> Cost;

    /// `getIntImmCost`: materializing an integer constant.
    fn int_imm_cost(
        &self,
        imm: i128,
        bits: u32,
    ) -> Cost;

    /// `getIntImmCostInst`: the constant at operand `index` of an instruction.
    fn int_imm_cost_inst(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost;

    /// `isTruncateFree`.
    fn is_truncate_free(
        &self,
        from: Ty,
        to: Ty,
    ) -> bool;

    /// `isLegalAddressingMode`: `base_gv + base_offset + base_reg + scale *
    /// index`.
    fn is_legal_addressing_mode(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool;

    /// `getScalingFactorCost`: what the scale of an addressing mode costs;
    /// invalid where the mode is not legal.
    fn scaling_factor_cost(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> Cost;

    /// `isLSRCostLess`.
    fn is_lsr_cost_less(
        &self,
        first: &LsrCost,
        second: &LsrCost,
    ) -> bool;

    /// `canMacroFuseCmp`: a compare and its branch issue as one.
    fn can_macro_fuse_cmp(&self) -> bool;

    /// `isLegalAddImmediate`.
    fn is_legal_add_immediate(
        &self,
        imm: i64,
    ) -> bool;

    /// `isLegalICmpImmediate`.
    fn is_legal_icmp_immediate(
        &self,
        imm: i64,
    ) -> bool;

    /// `getNumberOfRegisters` of the general register class.
    fn general_registers(&self) -> u32;

    /// `isNumRegsMajorCostOfLSR`: the base's `true` (X86 does not override).
    fn is_num_regs_major_cost_of_lsr(&self) -> bool {
        true
    }

    /// `getCastInstrCost`.
    fn cast_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Cost;

    /// `getArithmeticInstrCost`.
    fn arithmetic_cost(
        &self,
        op: ArithOp,
        ty: Ty,
        kind: CostKind,
        first: OperandInfo,
        second: OperandInfo,
    ) -> Cost;
}
