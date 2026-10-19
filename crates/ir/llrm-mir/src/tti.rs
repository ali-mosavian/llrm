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

/// `TTI::LSRCost`: what loop strength reduction compares formulas by.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LsrCost {
    pub insns: u32,
    pub num_regs: u32,
    pub add_rec_cost: u32,
    pub num_iv_muls: u32,
    pub num_base_adds: u32,
    pub imm_cost: u32,
    pub setup_cost: u32,
    pub scale_cost: u32,
}

/// `TTI::MemIndexedMode`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemIndexedMode {
    PreInc,
    PreDec,
    PostInc,
    PostDec,
}

/// `TTI::UnrollingPreferences`: what the unroller may do, as the target sets
/// it.
#[derive(Clone, Debug)]
pub struct UnrollingPreferences {
    pub threshold: u32,
    pub max_percent_threshold_boost: u32,
    pub opt_size_threshold: u32,
    pub partial_threshold: u32,
    pub partial_opt_size_threshold: u32,
    pub default_unroll_runtime_count: u32,
    pub max_count: u32,
    pub max_upper_bound: u32,
    pub full_unroll_max_count: u32,
    pub be_insns: u32,
    pub partial: bool,
    pub runtime: bool,
    pub allow_remainder: bool,
    pub unroll_remainder: bool,
    pub allow_expensive_trip_count: bool,
    pub force: bool,
    pub upper_bound: bool,
    pub max_iterations_count_to_analyze: u32,
}

/// The target a cost is for.
pub trait Tti {
    /// The implementation's name, for what reports which target priced a
    /// decision.
    fn name(&self) -> &'static str {
        "generic"
    }

    /// The cost of an integer-to-float cast whose operand is a load, where the
    /// target prices it apart (X86: the extension of a narrow integer to i32 is
    /// free for a load, `FILD` reads it from memory); none: as any cast (T4).
    fn cast_of_load_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Option<Cost> {
        let _ = (op, dst, src, kind);
        None
    }

    /// The cost of a store whose address is a GEP with a non-constant index,
    /// for a target that prices it apart from other stores (X86: "a store
    /// with index and scale costs 2 uops", for every kind but throughput);
    /// none: as any store (T4).
    fn indexed_store_cost(
        &self,
        kind: CostKind,
    ) -> Option<Cost> {
        let _ = kind;
        None
    }

    /// `getSchedModel().LoopMicroOpBufferSize`: the micro-ops the target's loop
    /// buffer holds; 0 where its scheduling model has none (T11: only the
    /// models of the cpus the plan page pins are known, and none has one).
    fn loop_micro_op_buffer_size(&self) -> u32 {
        0
    }

    /// `BasicTTIImplBase::getUnrollingPreferences`: partial and runtime
    /// unrolling up to the loop buffer's size, unless the loop calls something
    /// that is lowered to a call (`has_calls`).
    fn get_unrolling_preferences(
        &self,
        has_calls: bool,
        up: &mut UnrollingPreferences,
    ) {
        let max_ops = self.loop_micro_op_buffer_size();
        if max_ops == 0 || has_calls {
            return;
        }
        // Enable runtime and partial unrolling up to the specified size, and
        // the trip count upper bound.
        up.partial = true;
        up.runtime = true;
        up.upper_bound = true;
        up.partial_threshold = max_ops;
        // Avoid unrolling when optimizing for size.
        up.opt_size_threshold = 0;
        up.partial_opt_size_threshold = 0;
        // Instructions optimized when the back edge becomes a fall through.
        up.be_insns = 2;
    }

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

    /// `TargetTransformInfoImplBase::isLegalAddImmediate`: none.
    fn is_legal_add_immediate(
        &self,
        imm: i64,
    ) -> bool {
        let _ = imm;
        false
    }

    /// `TargetTransformInfoImplBase::isLegalICmpImmediate`: none.
    fn is_legal_icmp_immediate(
        &self,
        imm: i64,
    ) -> bool {
        let _ = imm;
        false
    }

    /// `getScalingFactorCost`: every legal addressing mode is free, the rest
    /// invalid (`TargetTransformInfoImplBase`'s guess).
    fn scaling_factor_cost(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> Cost {
        if self.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale) { Cost::FREE } else { Cost::INVALID }
    }

    /// `isLSRCostLess`: registers first, then the add-recurrence cost, the IV
    /// multiplies, the base adds, the scale, the immediates and the setup.
    fn is_lsr_cost_less(
        &self,
        a: &LsrCost,
        b: &LsrCost,
    ) -> bool {
        let key = |c: &LsrCost| {
            (c.num_regs, c.add_rec_cost, c.num_iv_muls, c.num_base_adds, c.scale_cost, c.imm_cost, c.setup_cost)
        };
        key(a) < key(b)
    }

    /// `isNumRegsMajorCostOfLSR`.
    fn is_num_regs_major_cost_of_lsr(&self) -> bool {
        true
    }

    /// `shouldDropLSRSolutionIfLessProfitable`.
    fn should_drop_lsr_solution_if_less_profitable(&self) -> bool {
        false
    }

    /// `canMacroFuseCmp`: a compare fuses with the branch that reads it.
    fn can_macro_fuse_cmp(&self) -> bool {
        false
    }

    /// `isIndexedLoadLegal` and `isIndexedStoreLegal`: a pre- or post-indexed
    /// access of the mode.
    fn is_indexed_access_legal(
        &self,
        mode: MemIndexedMode,
        ty: Ty,
    ) -> bool {
        let _ = (mode, ty);
        false
    }

    /// `getNumberOfRegisters` of the general-purpose class.
    fn number_of_gpr_registers(&self) -> u32 {
        8
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

    /// `isLoweredToCall` of a named function that is not an intrinsic
    /// (`TargetTransformInfoImplBase`): the libm functions that become one
    /// selection node, and `abs`, `pow`... that are likely to be optimized
    /// into something smaller, are not calls.
    fn is_lowered_to_call(
        &self,
        name: Option<&str>,
        local_linkage: bool,
    ) -> bool {
        let Some(name) = name else { return true };
        if local_linkage {
            return true;
        }
        // A libm function that becomes one selection node, and the functions
        // "likely to be optimized into something smaller"; the suffixes are
        // LLVM's, `exp10`'s lacking `l`.. as listed there.
        const ONE_NODE: &[&str] = &[
            "copysign",
            "copysignf",
            "copysignl",
            "fabs",
            "fabsf",
            "fabsl",
            "fmin",
            "fminf",
            "fminl",
            "fmax",
            "fmaxf",
            "fmaxl",
            "sin",
            "sinf",
            "sinl",
            "cos",
            "cosf",
            "cosl",
            "tan",
            "tanf",
            "tanl",
            "asin",
            "asinf",
            "asinl",
            "acos",
            "acosf",
            "acosl",
            "atan",
            "atanf",
            "atanl",
            "atan2",
            "atan2f",
            "atan2l",
            "sinh",
            "sinhf",
            "sinhl",
            "cosh",
            "coshf",
            "coshl",
            "tanh",
            "tanhf",
            "tanhl",
            "sqrt",
            "sqrtf",
            "sqrtl",
            "exp10",
            "exp10l",
            "exp10f",
            "pow",
            "powf",
            "powl",
            "exp2",
        ];
        const SMALLER: &[&str] = &["exp2l", "exp2f", "floor", "floorf", "ceil", "round", "ffs", "ffsl"];
        const MORE: &[&str] = &["abs", "labs", "llabs"];
        !(ONE_NODE.contains(&name) || SMALLER.contains(&name) || MORE.contains(&name))
    }

    /// `getIntrinsicInstrCost` of the call `name(args)` returning `ret`
    /// (none for `void`): invalid where the target has no answer (T7).
    fn intrinsic_cost(
        &self,
        name: &str,
        ret: Option<Ty>,
        args: &[Ty],
        kind: CostKind,
    ) -> Cost {
        let _ = (ret, args, kind);
        if is_free_intrinsic(name) { Cost::FREE } else { Cost::INVALID }
    }
}

/// An intrinsic that makes no code: `assume`, `lifetime`, the scope marks of
/// `noalias` (`getIntrinsicInstrCost` returns `TCC_Free` for them).
pub fn is_free_intrinsic(name: &str) -> bool {
    const FREE: [&str; 6] = [
        "llvm.assume",
        "llvm.lifetime.start",
        "llvm.lifetime.end",
        "llvm.noalias.scope.decl",
        "llvm.experimental.noalias.scope.decl",
        "llrm.noalias.scope.decl",
    ];
    FREE.iter().any(|one| name == *one || name.strip_prefix(one).is_some_and(|rest| rest.starts_with('.')))
}

/// The generic profile: `TargetTransformInfoImplBase`'s defaults, what a target
/// with no cost model of its own is priced by (T10): one unit for an operation,
/// nothing for a `phi` (unless throughput is asked), a constant, or a
/// truncation; no addressing mode folds.
#[derive(Clone, Copy, Debug, Default)]
pub struct Basic;

impl Tti for Basic {
    fn cmp_sel_cost(
        &self,
        _: CmpSel,
        _: Ty,
        _: CostKind,
    ) -> Cost {
        Cost::BASIC
    }

    fn memory_op_cost(
        &self,
        _: MemoryOp,
        _: Ty,
        _: CostKind,
        _: bool,
    ) -> Cost {
        Cost::BASIC
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: CostKind,
    ) -> Cost {
        if phi && kind != CostKind::RecipThroughput { Cost::FREE } else { Cost::BASIC }
    }

    fn int_imm_cost(
        &self,
        _: i128,
        _: u32,
    ) -> Cost {
        Cost::BASIC
    }

    fn int_imm_cost_inst(
        &self,
        _: ImmUser,
        _: u32,
        _: i128,
        _: u32,
    ) -> Cost {
        Cost::FREE
    }

    fn is_truncate_free(
        &self,
        _: Ty,
        _: Ty,
    ) -> bool {
        false
    }

    fn is_legal_addressing_mode(
        &self,
        _: bool,
        _: i64,
        _: bool,
        _: i64,
    ) -> bool {
        false
    }

    fn cast_cost(
        &self,
        _: CastKind,
        _: Ty,
        _: Ty,
        _: CostKind,
    ) -> Cost {
        Cost::BASIC
    }

    fn arithmetic_cost(
        &self,
        _: ArithOp,
        _: Ty,
        _: CostKind,
        _: OperandInfo,
        _: OperandInfo,
    ) -> Cost {
        Cost::BASIC
    }
}
