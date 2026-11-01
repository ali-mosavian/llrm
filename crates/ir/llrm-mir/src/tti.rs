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

/// `TTI::AddressingModeKind`: the indexed addressing modes loop strength
/// reduction favors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressingMode {
    None,
    PreIndexed,
    PostIndexed,
    All,
}

impl AddressingMode {
    /// `AMK & AMK_PostIndexed`.
    pub fn has_post_indexed(self) -> bool {
        matches!(self, AddressingMode::PostIndexed | AddressingMode::All)
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

/// What `getUnrollingPreferences` reads of a loop beyond its cost: AArch64 sets
/// its thresholds by nesting depth and exit count.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LoopShape {
    /// It calls something that is lowered to a call.
    pub has_calls: bool,
    /// The loop's nesting depth, 1 for an outermost loop.
    pub depth: u32,
    /// The blocks outside the loop it can exit to, and the blocks inside that
    /// exit.
    pub exit_blocks: u32,
    pub exiting_blocks: u32,
}

/// The predicate of a comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Predicate {
    Int(crate::opcode::IntPredicate),
    Float(crate::opcode::FloatPredicate),
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
    /// `UnrollAndJam`: unroll and jam the loop nest (the pin's default: no).
    pub unroll_and_jam: bool,
    /// `UnrollAndJamInnerLoopThreshold`: the size the jammed inner loop may
    /// have (the pin's default: 60).
    pub unroll_and_jam_inner_loop_threshold: u32,
}

/// The target a cost is for.
pub trait Tti {
    /// The implementation's name, for what reports which target priced a
    /// decision.
    fn name(&self) -> &'static str {
        "generic"
    }

    /// `TargetLowering::convertSelectOfConstantsToMath`: whether a select
    /// between two scalar integer constants may become arithmetic on its
    /// condition (X86: yes for every scalar integer).
    fn convert_select_of_constants_to_math(&self) -> bool {
        true
    }

    /// Whether a `sext` or `zext` of a simple load is free: the target has the
    /// extending load and both types legalize alike
    /// (`BasicTTIImplBase::getCastInstrCost`, BasicTTIImpl.h:1298-1311, which
    /// `cast_cost` cannot see without the instruction).
    fn ext_load_free(
        &self,
        _op: CastKind,
        _dst: Ty,
        _src: Ty,
    ) -> bool {
        false
    }

    /// `getRegisterBitWidth(FixedWidthVector)`: the bits of the widest vector
    /// register, 0 where the target has none (and the SLP vectorizer is off).
    fn vector_register_bits(&self) -> u32 {
        0
    }

    /// `getJumpConditionMergingParams`: the budget for keeping the compares
    /// of a branch on `a && b` or `a || b` one condition. Split always, where
    /// the target says nothing.
    fn jump_condition_merging_params(
        &self,
        query: &crate::jumpmerge::JumpMergeQuery,
    ) -> crate::jumpmerge::CondMerging {
        let _ = query;
        crate::jumpmerge::CondMerging::SPLIT
    }

    /// Whether an `optsize` caller's inline of a callee that stays must not
    /// grow the code: the callee's `CodeSize` over the instructions the
    /// inline cost did not fold, against the call and the pushes of its
    /// arguments (SZ1). The pin's `OptSizeThreshold` 50 allows growth.
    fn opt_size_inline_by_code_size(&self) -> bool {
        false
    }

    /// The same bound for a caller optimized for speed (its tuning table).
    fn speed_inline_by_code_size(&self) -> bool {
        false
    }

    /// How many percent of its `CodeSize` units a call and the pushes of its
    /// arguments take in code, where they are bigger than the average
    /// instruction (X86: a far call is 5 bytes and a push 1 to 4, against 2 to
    /// 3 for an ALU instruction); 100: as counted.
    fn call_code_size_percent(&self) -> i64 {
        100
    }

    /// How many percent of its `CodeSize` units an instruction that touches a
    /// float takes in code, where the target's unit undercounts it (16-bit
    /// x87: every operand moves through the register stack, a load or store
    /// each side of an operation the unit counts once); 100: as counted.
    fn float_code_size_percent(&self) -> i64 {
        100
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

    /// What a byte of code is worth in thousandths of a clock at speed, where
    /// the target trades bytes for clocks; 0: it does not.
    fn speed_milliclocks_per_byte(&self) -> i64 {
        0
    }

    /// `Tuning::multiply_by_clocks`.
    fn multiply_by_clocks(&self) -> bool {
        false
    }

    /// `Subtarget.canUseCMOV()`: false where a `select` is lowered to a branch.
    fn has_cmov(&self) -> bool {
        true
    }

    /// DEVIATE U1: the bar (unrolled size) an `optsize` loop of at most this
    /// many trips is fully unrolled under, where the target says so: its
    /// body's tests fold on the trip, and few copies cost little; none: the
    /// pin's.
    fn unroll_size_few_trips(&self) -> Option<(u32, u32)> {
        None
    }

    /// The target's `DefaultThreshold` for the inliner (its tuning table);
    /// none: the pass's own.
    fn inline_threshold_override(&self) -> Option<i64> {
        None
    }

    /// DEVIATE REC-1: the bounds of the recursion the target takes by default
    /// when optimizing for speed: (depth of a function's copies of itself,
    /// the largest recursive body in `gcc_insns` that is copied); none: the
    /// pin's inliner, which copies no recursive callee (`-finline-recursive`
    /// asks for gcc's).
    fn recursive_inlining(&self) -> Option<(u32, i64)> {
        None
    }

    /// The target's `Threshold` and `MaxPercentThresholdBoost` for the full
    /// unroll (its tuning table); none: the pass's own.
    fn unroll_threshold_overrides(&self) -> (Option<u32>, Option<u32>) {
        (None, None)
    }

    /// `getUnrollingPreferences` given the loop's shape, which targets read
    /// (nesting depth, exits); by default the loop-blind
    /// `get_unrolling_preferences`.
    fn get_unrolling_preferences_for(
        &self,
        shape: &LoopShape,
        up: &mut UnrollingPreferences,
    ) {
        self.get_unrolling_preferences(shape.has_calls, up)
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

    /// `getCmpSelInstrCost` with the comparison's predicate, which some targets
    /// price by (AArch64: an `fcmp one` or `ueq` takes two conditions). Callers
    /// that know the predicate ask this; by default it is `cmp_sel_cost`.
    fn cmp_sel_cost_for(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
        predicate: Option<Predicate>,
    ) -> Cost {
        let _ = predicate;
        self.cmp_sel_cost(op, ty, kind)
    }

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

    /// `hasDivRemOp(iN, IsSigned)`: whether the target has one divide that
    /// makes the quotient and the remainder. The base class has none.
    fn has_div_rem_op(
        &self,
        bits: u32,
        signed: bool,
    ) -> bool {
        let _ = (bits, signed);
        false
    }

    /// `isLegalAddressingMode`: `base_gv + base_offset + base_reg + scale *
    /// index`.
    fn is_legal_addressing_mode(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool;

    /// `isLegalAddressingMode` in address space `addr_space` (LLVM's `AS`): a
    /// space can have fewer forms than the machine's, as a huge pointer's
    /// offset carries into its selector and takes no index. By default every
    /// space has the target's forms.
    fn is_legal_addressing_mode_in(
        &self,
        addr_space: u32,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        let _ = addr_space;
        self.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale)
    }

    /// `isLegalAddressingMode` in `addr_space` for an access of `access_bytes`
    /// (0: unknown): the two together, as LSR asks it of a use.
    fn is_legal_addressing_mode_at(
        &self,
        addr_space: u32,
        access_bytes: u64,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        let _ = access_bytes;
        self.is_legal_addressing_mode_in(addr_space, base_gv, base_offset, has_base_reg, scale)
    }

    /// `isLegalAddressingMode` for an access of `access_bytes` (LLVM's
    /// `AccessTy`): an immediate offset's legality depends on the access
    /// size on AArch64 (`ldr x` takes an offset `ldr w` does not). By
    /// default the size is not asked: `is_legal_addressing_mode`.
    fn is_legal_addressing_mode_for(
        &self,
        access_bytes: u64,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        let _ = access_bytes;
        self.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale)
    }

    /// `getScalingFactorCost` for an access of `access_bytes` (0: unknown), as
    /// LSR asks it of a use; by default the size is not asked.
    fn scaling_factor_cost_at(
        &self,
        access_bytes: u64,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> Cost {
        let _ = access_bytes;
        self.scaling_factor_cost(base_gv, base_offset, has_base_reg, scale)
    }

    /// `getScalingFactorCost` for an access of `access_bytes`.
    fn scaling_factor_cost_for(
        &self,
        access_bytes: u64,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> Cost {
        if self.is_legal_addressing_mode_for(access_bytes, base_gv, base_offset, has_base_reg, scale) {
            Cost::FREE
        } else {
            Cost::INVALID
        }
    }

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
        if self.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale) {
            Cost::FREE
        } else {
            Cost::INVALID
        }
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

    /// `getPreferredAddressingMode`: which indexed addressing LSR favors.
    fn preferred_addressing_mode(&self) -> AddressingMode {
        AddressingMode::None
    }

    /// `isIndexedLoadLegal(MIM_PostInc, ty)`: a load that steps its pointer by
    /// the access size after it (`(An)+`).
    fn is_post_inc_load_legal(
        &self,
        _ty: Ty,
    ) -> bool {
        false
    }

    /// `isIndexedStoreLegal(MIM_PostInc, ty)`.
    fn is_post_inc_store_legal(
        &self,
        _ty: Ty,
    ) -> bool {
        false
    }

    /// Beyond the pin: whether a strict exit compare of a unit-stride counter
    /// that never passes its bound should become `!=` (`exitne`). Counting to
    /// zero and testing `!=` is cheap where a decrement sets the flags and one
    /// branch follows (m68k `subq; bne`, `dbra`), not where the compare is
    /// free anyway (x86: the rewrite moved `bq32` rows the wrong way). Off by
    /// default; a target opts in where it measured a win.
    fn prefers_exit_ne(&self) -> bool {
        false
    }

    /// `isNumRegsMajorCostOfLSR`.
    fn is_num_regs_major_cost_of_lsr(&self) -> bool {
        true
    }

    /// `shouldDropLSRSolutionIfLessProfitable`.
    fn should_drop_lsr_solution_if_less_profitable(&self) -> bool {
        false
    }

    /// DEVIATE LU3: whether a rewrite the cost model rates worse than the loop
    /// as written is dropped when it needs more registers than exist.
    fn lsr_drops_spilling_solution(&self) -> bool {
        false
    }

    /// DEVIATE LU5: whether a formula whose base register is a non-affine
    /// addrec of an enclosing loop is charged the induction variables its
    /// expansion keeps live across that loop (one for each order past the
    /// first), not one register. The pin charges one and survives on 7
    /// registers; with 6 they spill every outer iteration.
    fn lsr_charges_outer_nonaffine_ivs(&self) -> bool {
        false
    }

    /// Beyond the pin (which treats a counter only some trips step as no
    /// induction variable, `createSimpleAffineAddRec`): such a counter gets a
    /// pointer that steps with it, where the address takes no scale. False
    /// keeps the pin's loops. Exit: none, a pass of its own.
    fn mirrors_conditional_counters(&self) -> bool {
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

    /// Instructions a scaled index (`base + scale * index`, scale above 1)
    /// costs beyond the address itself, once for each scaled register: the
    /// backend forms it once and every use of the register shares it. 0
    /// where the scale is part of the address form, as at the pin (DEVIATE
    /// LU1, `lsr_use.rs`).
    fn scaled_index_insns(&self) -> u32 {
        0
    }

    /// Whether the target adds registers and an immediate in one instruction
    /// whatever the use (x86's `lea`): an unfolded offset beside two or more
    /// registers then costs no add of its own. DEVIATE LU2 (`lsr_use.rs`).
    fn address_form_adds_offsets(&self) -> bool {
        false
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

    /// `isTypeLegal`: whether the target holds a value of `ty` in a register
    /// (`TargetLowering::isTypeLegal`). The default is a 32-bit target
    /// with no wider integer.
    fn is_type_legal(
        &self,
        ty: Ty,
    ) -> bool {
        matches!(
            ty,
            Ty::Int(8 | 16 | 32) | Ty::Float | Ty::Double | Ty::Pointer
        )
    }

    /// `shouldBuildLookupTables`: whether the target jumps through a table
    /// (X86 and AArch64: `BRIND` is legal).
    fn should_build_lookup_tables(&self) -> bool {
        true
    }

    /// `getMinimumLookupTableEntryBitWidth`: the narrowest cell of a lookup
    /// table, so that a load of it is a native one.
    fn minimum_lookup_table_entry_bit_width(&self) -> u32 {
        8
    }

    /// `getEstimatedNumberOfCaseClusters`
    /// ([BasicTTIImpl.h:584](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L584)):
    /// the clusters a switch on `cases` (sign-extended values, `index_bits`
    /// wide) is estimated to lower to, and the size of its jump table where it
    /// gets one (0: none). The generic answer, [`basic_case_clusters`]; a
    /// target with its own jump table limits or bit tests overrides it
    /// (T1).
    fn estimated_number_of_case_clusters(
        &self,
        cases: &[i128],
        index_bits: u32,
        for_size: bool,
    ) -> (u64, u64) {
        basic_case_clusters(cases, index_bits, for_size)
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
        if is_free_intrinsic(name) { Cost::FREE } else { base_intrinsic_cost(name).unwrap_or(Cost::INVALID) }
    }
}

/// `BasicTTIImplBase::getEstimatedNumberOfCaseClusters`, T1: bit tests
/// (`isSuitableForBitTests`) are not translated; a jump table is allowed with
/// `getMinimumJumpTableEntries()` = 4, density 10% (40% for size), at most 64K
/// entries. The clusters, and the jump table's size, if one.
pub fn basic_case_clusters(
    cases: &[i128],
    index_bits: u32,
    for_size: bool,
) -> (u64, u64) {
    let count = cases.len() as u64;
    let _ = index_bits;
    if count < 4 {
        return (count, 0);
    }
    let (low, high) = (cases.iter().min().copied().unwrap_or(0), cases.iter().max().copied().unwrap_or(0));
    let range = (high - low) as u64 + 1;
    let density = if for_size { 40 } else { 10 };
    if range <= 1 << 16 && count * 100 >= range * density {
        return (1, range);
    }
    (count, 0)
}

/// An intrinsic that makes no code: `assume`, `lifetime`, the scope marks of
/// `noalias` (`getIntrinsicInstrCost` returns `TCC_Free` for them).
/// The intrinsics `BasicTTIImplBase::getTypeBasedIntrinsicInstrCost` prices
/// itself, whatever the target and the kind: `memset.pattern` is 4, "set to
/// match the cost of the `memset_pattern16` libcall" (BasicTTIImpl.h:2658). A
/// loop that holds one is not small to the unroller.
pub fn base_intrinsic_cost(name: &str) -> Option<Cost> {
    const MEMSET_PATTERN: &str = "llvm.experimental.memset.pattern";
    (name == MEMSET_PATTERN || name.strip_prefix(MEMSET_PATTERN).is_some_and(|rest| rest.starts_with('.')))
        .then(|| Cost::of(4))
}

pub fn is_free_intrinsic(name: &str) -> bool {
    const FREE: [&str; 8] = [
        "llvm.assume",
        "llvm.lifetime.start",
        "llvm.lifetime.end",
        "llvm.invariant.start",
        "llvm.invariant.end",
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

#[cfg(test)]
mod access_tests {
    use super::*;

    /// A target that states none of the access-aware questions answers them as
    /// the older ones: nothing existing changes when a caller asks the new.
    #[test]
    fn the_questions_that_take_more_default_to_the_older_ones() {
        let basic = Basic;
        assert_eq!(
            basic.cmp_sel_cost_for(CmpSel::FCmp, Ty::Double, CostKind::RecipThroughput, None),
            basic.cmp_sel_cost(CmpSel::FCmp, Ty::Double, CostKind::RecipThroughput)
        );
        assert_eq!(
            basic.is_legal_addressing_mode_for(4, false, 8, true, 1),
            basic.is_legal_addressing_mode(false, 8, true, 1)
        );
        assert_eq!(basic.scaling_factor_cost_for(8, false, 0, true, 2), basic.scaling_factor_cost(false, 0, true, 2));
        struct Counts(Basic);
        impl Tti for Counts {
            fn get_unrolling_preferences(
                &self,
                has_calls: bool,
                up: &mut UnrollingPreferences,
            ) {
                up.max_count = if has_calls { 7 } else { 9 };
            }

            fn cmp_sel_cost(
                &self,
                op: CmpSel,
                ty: Ty,
                kind: CostKind,
            ) -> Cost {
                self.0.cmp_sel_cost(op, ty, kind)
            }

            fn memory_op_cost(
                &self,
                op: MemoryOp,
                ty: Ty,
                kind: CostKind,
                stored_constant: bool,
            ) -> Cost {
                self.0.memory_op_cost(op, ty, kind, stored_constant)
            }

            fn cf_instr_cost(
                &self,
                phi: bool,
                kind: CostKind,
            ) -> Cost {
                self.0.cf_instr_cost(phi, kind)
            }

            fn int_imm_cost(
                &self,
                imm: i128,
                bits: u32,
            ) -> Cost {
                self.0.int_imm_cost(imm, bits)
            }

            fn int_imm_cost_inst(
                &self,
                op: ImmUser,
                index: u32,
                imm: i128,
                bits: u32,
            ) -> Cost {
                self.0.int_imm_cost_inst(op, index, imm, bits)
            }

            fn is_truncate_free(
                &self,
                from: Ty,
                to: Ty,
            ) -> bool {
                self.0.is_truncate_free(from, to)
            }

            fn is_legal_addressing_mode(
                &self,
                gv: bool,
                offset: i64,
                base: bool,
                scale: i64,
            ) -> bool {
                self.0.is_legal_addressing_mode(gv, offset, base, scale)
            }

            fn is_legal_addressing_mode_in(
                &self,
                addr_space: u32,
                gv: bool,
                offset: i64,
                base: bool,
                scale: i64,
            ) -> bool {
                self.0.is_legal_addressing_mode_in(addr_space, gv, offset, base, scale)
            }

            fn scaling_factor_cost_at(
                &self,
                access_bytes: u64,
                gv: bool,
                offset: i64,
                base: bool,
                scale: i64,
            ) -> Cost {
                self.0.scaling_factor_cost_at(access_bytes, gv, offset, base, scale)
            }

            fn is_legal_addressing_mode_at(
                &self,
                addr_space: u32,
                access_bytes: u64,
                gv: bool,
                offset: i64,
                base: bool,
                scale: i64,
            ) -> bool {
                self.0.is_legal_addressing_mode_at(addr_space, access_bytes, gv, offset, base, scale)
            }

            fn cast_cost(
                &self,
                op: CastKind,
                dst: Ty,
                src: Ty,
                kind: CostKind,
            ) -> Cost {
                self.0.cast_cost(op, dst, src, kind)
            }

            fn arithmetic_cost(
                &self,
                op: ArithOp,
                ty: Ty,
                kind: CostKind,
                a: OperandInfo,
                b: OperandInfo,
            ) -> Cost {
                self.0.arithmetic_cost(op, ty, kind, a, b)
            }
        }
        let mut up = UnrollingPreferences {
            threshold: 0,
            max_percent_threshold_boost: 0,
            opt_size_threshold: 0,
            partial_threshold: 0,
            partial_opt_size_threshold: 0,
            default_unroll_runtime_count: 0,
            max_count: 0,
            max_upper_bound: 0,
            full_unroll_max_count: 0,
            be_insns: 0,
            partial: false,
            runtime: false,
            allow_remainder: false,
            unroll_remainder: false,
            allow_expensive_trip_count: false,
            force: false,
            upper_bound: false,
            max_iterations_count_to_analyze: 0,
            unroll_and_jam: false,
            unroll_and_jam_inner_loop_threshold: 0,
        };
        Counts(Basic).get_unrolling_preferences_for(&LoopShape { has_calls: true, ..LoopShape::default() }, &mut up);
        assert_eq!(up.max_count, 7);
    }
}
