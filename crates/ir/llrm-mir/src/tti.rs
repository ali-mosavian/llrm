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

/// A fixed-width vector as the vector cost hooks see it: `count` lanes of a
/// scalar type. The scalar [`Ty`] stays scalar (T1); a target that prices
/// vectors answers the `vector_*` hooks of [`Tti`], the others keep their
/// defaults ("no vectors").
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VectorTy {
    pub lane: Ty,
    pub count: u32,
}

/// `TargetTransformInfo::RegisterClass`, by the pin's class id: 0 scalar, 1
/// vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum RegisterClass {
    Scalar,
    Vector,
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
    /// The constant, sign-extended from its width, where a target prices the
    /// size of an immediate by it; the pin's `OperandValueInfo` has none, and
    /// no target but the m68k reads it.
    pub value: Option<i128>,
}

impl OperandInfo {
    pub const ANY: OperandInfo =
        OperandInfo { constant: false, power_of_two: false, negated_power_of_two: false, value: None };

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
            value: Some(if width >= 128 || width == 0 {
                value
            } else {
                let shift = 128 - width;
                (((value as u128) << shift) as i128) >> shift
            }),
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

/// How a load or store names its memory, for the bytes its address takes in
/// code (`Tti::access_address_cost`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddressForm {
    pub base: AddressBase,
    /// The offset from the base is a constant of this many bytes' encoding:
    /// 0 none, 1 a byte, 4 a word.
    pub displacement_bytes: u8,
    /// A register scaled into the address.
    pub indexed: bool,
}

/// What an address starts from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressBase {
    /// A global's own address: the access names it absolutely.
    Global,
    /// A stack slot of the function.
    Frame,
    /// A pointer in a register.
    Register,
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

/// One operation of the code LSR's choice leaves behind: its encoded bytes
/// and its clocks, in thousandths (the clocks are read where a level prices
/// both axes; 0 = unstated).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OpCost {
    pub bytes: u32,
    pub clocks_milli: u32,
}

impl OpCost {
    pub const fn bytes(bytes: u32) -> Self {
        Self { bytes, clocks_milli: 0 }
    }
}

/// DEVIATE LSR-SIZE (owner w12): the target's byte table for the code LSR's
/// formulae leave behind, in the place of the numbers `lsr_use.rs` held.
/// `None` from [`Tti::lsr_size_model`] is the pin's order.
#[derive(Clone, Copy, Debug)]
pub struct LsrSizeModel {
    /// A displacement from a global's address.
    pub disp_global: OpCost,
    /// A frame slot's displacement, laid out after the pass.
    pub disp_frame: OpCost,
    /// A displacement that fits a signed byte.
    pub disp_short: OpCost,
    /// Any other nonzero displacement. A zero one is free.
    pub disp_long: OpCost,
    /// The add or lea for each register not folded into the address, with
    /// its immediate.
    pub base_add: OpCost,
    /// The bytes of the step added each trip to a `bits`-wide counter, by
    /// the step where it is a constant.
    pub step: fn(bits: u32, constant: Option<i64>) -> u32,
    /// The bytes of loading a counter's constant start in the preheader, where
    /// the target prices it by the value (a zero, a byte, a dword). `None`:
    /// `setup` for each, as the pin counts them.
    pub constant_start: Option<fn(bits: u32, value: i64) -> u32>,
    /// A `mov` or `lea` of a register's start in the preheader.
    pub setup: OpCost,
    /// Setup instructions counted for one formula, at most.
    pub setup_cap: u32,
    /// DEVIATE LSR-DERIVED: a recurrence the loop as written derives from its
    /// counter in one instruction a trip has no setup to count
    /// ([`Tuning::lsr_derived_setup_free`]).
    pub derived_setup_free: bool,
    /// Ordering by the model's bytes applies at -Os only; at -O1 the pin's
    /// order stands (LG3's byte test included).
    pub size_only: bool,
    /// The `cmp` an exit test costs when the counter does not end at zero
    /// (a count down to zero takes its flags from the step): its bytes for a
    /// compare of `bits` against an immediate, or a register where `None`.
    pub exit_compare: fn(bits: u32, immediate: Option<i64>) -> u32,
}

impl LsrSizeModel {
    /// The bytes of x86's `[reg + disp]`, as `lsr_use.rs` counted them; one
    /// number (3) for an add, lea or mov with its immediate.
    pub const X86: Self = Self {
        disp_global: OpCost::bytes(4),
        disp_frame: OpCost::bytes(2),
        disp_short: OpCost::bytes(1),
        disp_long: OpCost::bytes(4),
        base_add: OpCost::bytes(3),
        step: |_, _| 3,
        constant_start: None,
        setup: OpCost::bytes(3),
        setup_cap: 4,
        derived_setup_free: false,
        size_only: false,
        exit_compare: |_, _| 0,
    };

    /// The bytes of one address displacement.
    pub fn displacement(
        &self,
        offset: i64,
        global: bool,
        frame: bool,
    ) -> u32 {
        if global {
            self.disp_global.bytes
        } else if frame {
            self.disp_frame.bytes
        } else if offset == 0 {
            0
        } else if i8::try_from(offset).is_ok() {
            self.disp_short.bytes
        } else {
            self.disp_long.bytes
        }
    }
}

/// The bytes of the instructions LSR's setup and exit test are made of.
#[derive(Clone, Copy, Debug)]
pub struct LsrBytes {
    /// A constant's opcode, before its LEB128.
    pub constant: u32,
    /// Reading a local.
    pub local_get: u32,
    /// Writing a local.
    pub local_set: u32,
    /// An add or a compare.
    pub op: u32,
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
    /// DEVIATE LU13: frame words a trip reloads or stores for the registers
    /// kept across a call that clobbers them (0 unless the target says so).
    pub spills: u32,
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

/// `TTI::MemCmpExpansionOptions`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MemCmpExpansionOptions {
    /// Load sizes in bytes, largest first.
    pub load_sizes: Vec<u32>,
    pub max_num_loads: u32,
    pub num_loads_per_block: u32,
    pub allow_overlapping_loads: bool,
    /// Sizes a run of loads may be merged into (three-way compares only).
    pub allowed_tail_expansions: Vec<u32>,
}

/// The fields of `HardwareLoopInfo` a target sets in
/// `isHardwareLoopProfitable`; the pass keeps the rest (the exit block, branch
/// and count it finds).
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfo.h#L104>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HardwareLoopTarget {
    /// `CountType`'s width: the pin's default is 32
    /// (`hardware-loop-counter-bitwidth`).
    pub count_bits: u32,
    /// `LoopDecrement`: what `llvm.loop.decrement` takes off the counter.
    pub loop_decrement: u64,
    /// `IsNestingLegal`: a loop inside another may be a hardware loop.
    pub is_nesting_legal: bool,
    /// `CounterInReg`: the counter is a register value carried round the loop
    /// by a phi (`loop.decrement.reg`), not state the target keeps.
    pub counter_in_reg: bool,
    /// `PerformEntryTest`: the loop's entry guard may be folded into the
    /// counter's set (`test.set.loop.iterations`).
    pub perform_entry_test: bool,
    /// Beyond the pin (PPC's `checkCallInLoop` is its own check): the counter
    /// is lost across a call, so a loop that calls is left alone.
    pub rejects_calls: bool,
    /// Beyond the pin: the counter is one more register, so a loop is
    /// converted only where its own counter dies, replaced by this one.
    pub requires_dead_counter: bool,
}

impl Default for HardwareLoopTarget {
    /// `HardwareLoopInfo(L)`'s defaults.
    fn default() -> Self {
        HardwareLoopTarget {
            count_bits: 32,
            loop_decrement: 1,
            is_nesting_legal: false,
            counter_in_reg: false,
            perform_entry_test: false,
            rejects_calls: false,
            requires_dead_counter: false,
        }
    }
}

/// An argument of a call, as the call sequence sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CallArgument {
    /// The value's type; none for an aggregate or a vector.
    pub ty: Option<Ty>,
    /// The bytes copied onto the stack, for a `byval` argument.
    pub byval_bytes: Option<u64>,
}

/// Rule 1 of the inliner's policy: an `optsize` caller's inline of a callee
/// that stays must not grow the code (SZ1).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SizeRule {
    /// The fewest calls a callee has in the module for the rule to hold its
    /// sites (a callee called once is the pin's: the copy meets constants the
    /// cost model follows, and folds with the sites around it).
    pub min_sites: i64,
    /// Whether the rule stands down where the pin's own cost, net of the
    /// instructions it folds, is below zero: the pin counts that inline as
    /// shrinking the caller (a one-instruction wrapper, a callee whose body
    /// folds away).
    pub yields_to_negative_cost: bool,
}

/// What a target states of the inliner's verdict beyond the pin's analyzer.
/// The inliner reads this and nothing else of the target's inline hooks:
/// `Tti::inline_policy` is the one place a target says it.
///
/// DEVIATE: the pin's `getInlineCost` is `cost < threshold` alone (the default
/// policy states nothing). Reason: a small platform's call is 3 bytes and its
/// cycles are few; the two rules of the user's 11-04 ruling are bytes against
/// the call at `-Os`, and bytes against clocks on a hot path at `-O2`/`-O3`.
/// Exit: none; step 4 of `handoff/inline-cost-model-design.md`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlinePolicy {
    /// Rule 1, at `-Os` and `-Oz`; none: not held.
    pub by_size: Option<SizeRule>,
    /// Rule 2, at `-O2` and `-O3`: what a byte of growth is worth in
    /// thousandths of a clock against the call's latency, weighted by the
    /// site's frequency. 0: the pin's threshold alone.
    pub by_rate: i64,
}

impl InlinePolicy {
    /// The pin's verdict: neither rule.
    pub const PIN: InlinePolicy = InlinePolicy { by_size: None, by_rate: 0 };
}

/// A call site: its arguments and the static frame its callee builds.
#[derive(Clone, Debug, PartialEq)]
pub struct CallShape {
    pub arguments: Vec<CallArgument>,
    /// The bytes of the callee's static allocas.
    pub callee_frame_bytes: u64,
    /// The bytes of the caller's static allocas: a caller with none has no
    /// frame, and an inlined callee's moves into it instead of vanishing.
    pub caller_frame_bytes: u64,
}

/// A change in cost on both axes a target prices: positive is cheaper.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Saving {
    pub clocks: i32,
    pub bytes: i32,
}

impl std::ops::Add for Saving {
    type Output = Saving;

    fn add(
        self,
        other: Saving,
    ) -> Saving {
        Saving { clocks: self.clocks + other.clocks, bytes: self.bytes + other.bytes }
    }
}

impl std::ops::Sub for Saving {
    type Output = Saving;

    fn sub(
        self,
        other: Saving,
    ) -> Saving {
        Saving { clocks: self.clocks - other.clocks, bytes: self.bytes - other.bytes }
    }
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

    /// `getRegisterBitWidth(RGK_Scalar)`: the bits of a scalar register, 0
    /// where the target does not say.
    fn scalar_register_bits(&self) -> u32 {
        0
    }

    /// `getRegisterBitWidth(FixedWidthVector)`: the bits of the widest vector
    /// register, 0 where the target has none (and the SLP vectorizer is off).
    fn vector_register_bits(&self) -> u32 {
        0
    }

    /// `getNumberOfRegisters(ClassID)`: 0 where the target has no vector
    /// registers.
    fn number_of_registers(
        &self,
        _class: RegisterClass,
    ) -> u32 {
        0
    }

    /// `getMaxInterleaveFactor(VF, HasUnorderedReductions)`.
    fn max_interleave_factor(
        &self,
        _vf: u32,
        _has_unordered_reductions: bool,
    ) -> u32 {
        1
    }

    /// `shouldMaximizeVectorBandwidth(RGK_FixedWidthVector)`: size the widest
    /// VF by the smallest type in the loop, not the widest.
    fn should_maximize_vector_bandwidth(&self) -> bool {
        false
    }

    /// `enableAggressiveInterleaving(HasReductions)`: interleave a loop that is
    /// not small.
    fn enable_aggressive_interleaving(&self) -> bool {
        false
    }

    /// `getMemoryOpCost` of a vector access of `align` bytes. `None`: the
    /// target does not price (so does not take) vector accesses of this type.
    fn vector_memory_op_cost(
        &self,
        _op: MemoryOp,
        _ty: VectorTy,
        _align: u64,
        _kind: CostKind,
    ) -> Option<Cost> {
        None
    }

    /// `getArithmeticInstrCost` of a vector operation. `None`: not priced.
    fn vector_arithmetic_cost(
        &self,
        _op: ArithOp,
        _ty: VectorTy,
        _kind: CostKind,
    ) -> Option<Cost> {
        None
    }

    /// `getArithmeticReductionCost` / `getMinMaxReductionCost` of the
    /// horizontal step of a vector reduction (`llvm.vector.reduce.*` of `ty`);
    /// `ordered`: a float add or multiply in lane order (no `reassoc`). `None`:
    /// not priced.
    fn vector_reduction_cost(
        &self,
        _kind: crate::intrinsics::ReduceKind,
        _ty: VectorTy,
        _ordered: bool,
        _cost_kind: CostKind,
    ) -> Option<Cost> {
        None
    }

    /// `getCastInstrCost` between fixed vectors of one lane count. `None`: not
    /// priced, which is not scalarized: the pin's cost is one this port does
    /// not know.
    fn vector_cast_cost(
        &self,
        _kind: CastKind,
        _dst: VectorTy,
        _src: VectorTy,
        _cost_kind: CostKind,
    ) -> Option<Cost> {
        None
    }

    /// `getCmpSelInstrCost` of a vector compare, or of a select of `ty` by a
    /// vector of conditions; `predicate` is the compare's, or the one that
    /// feeds the select. `None`: not priced.
    fn vector_cmp_sel_cost(
        &self,
        _op: CmpSel,
        _ty: VectorTy,
        _predicate: Option<Predicate>,
        _cost_kind: CostKind,
    ) -> Option<Cost> {
        None
    }

    /// `getScalarizationOverhead` of every lane of `ty`: the inserts that build
    /// it from scalars and the extracts that take it apart. `None`: not priced.
    fn scalarization_overhead(
        &self,
        _ty: VectorTy,
        _insert: bool,
        _extract: bool,
        _cost_kind: CostKind,
    ) -> Option<Cost> {
        None
    }

    /// Two compares joined by `and`/`or` stay two branches at every level, as
    /// the pin does at optsize (the target's `split_merged_conditions`);
    /// false: the pin's cost rule decides.
    fn splits_merged_conditions(&self) -> bool {
        false
    }

    /// DEVIATE LSR-OUTER: whether reducing a multiply of the outer loop's
    /// counter (by `multiplier`, none: not a constant) to a counter stepped by
    /// it pays: the clocks of the cheaper of the multiply and its shift-and-add
    /// chain against what a counter held across the inner loop costs each trip.
    /// false: the pin's, which leaves a loop with a subloop alone.
    fn reduces_outer_multiply(
        &self,
        multiplier: Option<u128>,
    ) -> bool {
        let _ = multiplier;
        false
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

    /// The inliner's policy for this target. The default is built from the
    /// hooks below that a target already states, so a target may keep stating
    /// those or return a policy of its own.
    fn inline_policy(&self) -> InlinePolicy {
        InlinePolicy {
            by_size: self
                .opt_size_inline_by_code_size()
                .then(
                    || SizeRule {
                        min_sites: self.opt_size_inline_guard_min_sites(),
                        yields_to_negative_cost: self.opt_size_inline_guard_yields_to_negative_cost(),
                    },
                ),
            by_rate: self.speed_inline_milliclocks_per_byte(),
        }
    }

    /// Whether isel selects a load or store of an integer whose byte size is no
    /// register or power of two (an `i24`: SROA cuts a three-byte slice
    /// into one, as the pin does). DEVIATE (default no): SROA keeps such a
    /// slice whole, where a target's isel has no lowering for
    /// the width (the x87 `long double` slices). Exit: every target that lowers
    /// odd widths says yes.
    fn selects_odd_sized_integers(&self) -> bool {
        false
    }

    /// Whether a callee that allocates may be copied into a recursive caller,
    /// as the pin's `getInlineCost` allows up to 1024 bytes of it
    /// (`TotalAllocaSizeRecursiveCaller`). DEVIATE (default no): a recursion's
    /// depth times the callee's frame overflows a small platform's stack.
    /// Exit: a target that says yes.
    fn inline_frames_into_recursive_callers(&self) -> bool {
        false
    }

    /// Whether an `optsize` caller's inline of a callee that stays must not
    /// grow the code: the callee's `CodeSize` over the instructions the
    /// inline cost did not fold, against the call and the pushes of its
    /// arguments (SZ1). The pin's `OptSizeThreshold` 50 allows growth.
    fn opt_size_inline_by_code_size(&self) -> bool {
        false
    }

    /// The cost model the inliner counts a callee's size with, where the
    /// target's `CodeSize` is not bytes: the same model with `CodeSize` in
    /// bytes of code. None: `CodeSize` as it is. The other passes keep
    /// `CodeSize` in the pin's units, which their thresholds are in.
    fn inline_size_view(&self) -> Option<std::rc::Rc<dyn Tti>> {
        None
    }

    /// [`Self::inline_size_view`] for a caller lowered for `speed` (-O2,
    /// -O3), where the target's lowering of an operation is larger than a
    /// size caller's. The default is the one view.
    fn inline_size_view_at(
        &self,
        _speed: bool,
    ) -> Option<std::rc::Rc<dyn Tti>> {
        self.inline_size_view()
    }

    /// What the address of a load or store adds to its `CodeSize`, beyond
    /// the access itself (`memory_op_cost`): the bytes of a global's absolute
    /// address, a frame slot's, a displacement or a scaled index. Free: the
    /// access is priced alone, as the pin does.
    fn access_address_cost(
        &self,
        form: AddressForm,
        kind: CostKind,
    ) -> Cost {
        let _ = (form, kind);
        Cost::FREE
    }

    /// What a `memcpy`, `memmove` or `memset` of `length` bytes (none: not a
    /// constant) costs under `kind`, where the target expands it inline or
    /// into a call by its length. None: `intrinsic_cost` answers.
    fn memory_intrinsic_cost(
        &self,
        family: &str,
        length: Option<u64>,
        kind: CostKind,
    ) -> Option<Cost> {
        let _ = (family, length, kind);
        None
    }

    /// What computing an address that no addressing mode holds costs (a
    /// `lea` or an add): one unit, as the pin prices it.
    fn address_computation_cost(
        &self,
        kind: CostKind,
    ) -> Cost {
        let _ = kind;
        Cost::BASIC
    }

    /// What a call of a function the target lowers to a call, with
    /// `arguments` arguments, costs: one unit for the call and one per
    /// argument, as the pin's `TargetTransformInfoImplBase` prices it.
    fn direct_call_cost(
        &self,
        arguments: i64,
        kind: CostKind,
    ) -> Cost {
        let _ = kind;
        Cost::BASIC * (arguments + 1)
    }

    /// What one call costs the caller and the callee together, in `kind`:
    /// the call and return, the arguments' setup (a by-value copy, a push
    /// past the argument registers), and the frame the callee builds and tears
    /// down. Inlining removes all of it, so the inliner's size and rate bounds
    /// read it in place of the call instruction's flat price. None: the pin's
    /// flat price (the call as one instruction and one per argument).
    fn call_sequence_cost(
        &self,
        shape: &CallShape,
        kind: CostKind,
    ) -> Option<Cost> {
        let _ = (shape, kind);
        None
    }

    /// What building and tearing down the callee's frame costs in `kind`,
    /// once per call executed. Inlining removes it from the callee, which then
    /// stays or dies: in bytes it is saved once, when the callee dies, and
    /// only where the caller already has a frame. None: no frame term.
    fn callee_frame_cost(
        &self,
        shape: &CallShape,
        kind: CostKind,
    ) -> Option<Cost> {
        let _ = (shape, kind);
        None
    }

    /// What a byte of growth is worth, in thousandths of a clock, to a speed
    /// caller's inline (D11 at speed): the growth is refused when it costs
    /// more than the call's latency saves, weighted by the site's frequency.
    /// 0: the pin's threshold alone.
    fn speed_inline_milliclocks_per_byte(&self) -> i64 {
        0
    }

    /// Whether a speed caller's inline that gives a loop of the callee a
    /// constant trip count is exempt from the byte rate: the copy unrolls
    /// and folds, which the call's latency does not show. No pin
    /// counterpart (the pin's InlineCost only adds `LoopPenalty` per loop).
    fn inline_hints_known_loop_bound(&self) -> bool {
        false
    }

    /// DEVIATE OB1 (`unrollbound.rs`): the most trips the unroller takes from
    /// the size of the object a loop's accesses run through, as a bound it
    /// unrolls completely by; none: the pin's bound alone.
    fn unroll_object_bound(&self) -> Option<u32> {
        None
    }

    /// Whether a priced inline trial keeps a copy that leaves the module no
    /// larger and saves clocks though no callee goes with it. False: the
    /// callee's credit decides (arm64 -O3 jpeg `decode`: 5,828 -> 7,324 B and
    /// +1.2M instructions when the copies were kept).
    fn inline_trial_keeps_shrinking_copy(&self) -> bool {
        false
    }

    /// DEVIATE SZ2-TRIPS: whether the priced SZ2 trial counts clocks at each
    /// block's frequency over the entry's (a loop's trips), and lets the byte
    /// rate alone decide a copy whose callee stays. False: each instruction
    /// once, and the callee must go with the copy or the site be hot.
    fn inline_trial_by_trips(&self) -> bool {
        false
    }

    /// The largest caller, in instructions, whose refused sites SZ2 tries
    /// inline: a trial runs the simplifier over the whole caller once per
    /// callee tried.
    fn inline_trial_caller_limit(&self) -> usize {
        300
    }

    /// Whether SZ2 trials simplify their copy with the cleanup list, not the
    /// whole one: a trial only counts instructions. False: the whole list.
    fn inline_trial_light_simplify(&self) -> bool {
        false
    }

    /// Whether SZ2 tries all the sites of a callee that stays in one caller as
    /// one unit, not one site at a time: the pin inlines each site on its
    /// cost and lets later passes merge the copies (three `report(const)`
    /// calls sink into one), which a trial of one site cannot see. False: a
    /// site alone. Part of DEVIATE SZ1 (the pin has no SZ2 trial): wasm, exit
    /// 2026-11-20.
    fn inline_trial_groups_sites(&self) -> bool {
        false
    }

    /// Whether a later round of the inliner over one caller skips a callee SZ2
    /// rejected in an earlier round: a round costs the simplifier over the
    /// caller per callee. False: tried again.
    fn inline_trial_skips_rejected(&self) -> bool {
        false
    }

    /// Whether the inliner's size view is measured in encoded bytes: it also
    /// prices what the pin counts free (phis, jumps, the first return, casts)
    /// and the call by its whole sequence, and it serves speed callers only,
    /// a size caller keeping the units its bound was fitted in (DEVIATE
    /// INL-O1 SIZE). False: the view, where there is one, prices every
    /// caller as it did.
    fn inline_size_view_is_measured(&self) -> bool {
        false
    }

    /// Whether the -O1 pipeline walks the call graph a second time after the
    /// dead globals and loops are gone, repricing a callee the folds emptied
    /// (`Options::second_inline`, which a driver may turn on at every level).
    fn inline_second_round_at_o1(&self) -> bool {
        false
    }

    /// Whether a callee the -O1 size bound refused is tried inline as at -Os
    /// (SZ2: the copy stays if the caller comes out smaller), where -O1
    /// prices no byte in clocks. False: the bound's verdict stands.
    fn inline_trials_at_o1(&self) -> bool {
        false
    }

    /// Whether `speed_inline_milliclocks_per_byte` also binds at -O1. False
    /// (the default; -O1 is strict): -O1 prices no byte in clocks and keeps
    /// the pin's threshold alone.
    /// `Tuning::sdiv_by_magnitude`.
    fn sdiv_by_magnitude(&self) -> bool {
        false
    }

    fn speed_inline_rate_at_o1(&self) -> bool {
        false
    }

    /// DEVIATE INL-O1-HOT (owner w10, exit 2026-11-24): where -O1 holds a
    /// growing inline (`by_size`), a site the pin calls locally hot
    /// (`HotCallSiteRelFreq`, 60 times the caller's entry) still inlines when
    /// its growth is at most this percent of the call sequence removed. None:
    /// the guard holds every growing inline.
    fn inline_o1_hot_growth_percent(&self) -> Option<i64> {
        None
    }

    /// What a call to a callee whose peak register pressure is `peak` values
    /// costs in clocks to save and restore the registers it must preserve (a
    /// register the callee takes beyond the scratch ones is pushed and popped
    /// once per call). Inlining removes it; the trial prices it. 0: not priced.
    fn callee_saved_clocks(
        &self,
        peak: usize,
    ) -> i64 {
        let _ = peak;
        0
    }

    /// INL-HOT: the least site frequency (over the caller's entry) at which a
    /// call the pin's threshold alone refused is tried inlined at -O2/-O3 and
    /// kept where the rate pays. Whole units of relative frequency. None: the
    /// threshold is final.
    fn inline_hot_trial_frequency(&self) -> Option<i64> {
        None
    }

    /// DEVIATE INL-SPEED-OS (owner w11, exit 2026-12-05): at a size build an
    /// SZ2 trial the byte count refuses is kept where its sites run at
    /// least this many times per call of the caller and the copies save clocks,
    /// for at most `INLINE_SPEED_BYTES_PER_SITE` bytes a site. The pin's
    /// `OptSizeThreshold` and gcc's `-Os` inline a small callee in a hot
    /// loop (jpeg_single -Os: `clamp`, 1.18 M calls, 7% of the instructions).
    /// None: bytes alone.
    fn inline_trial_speed_at_size(&self) -> Option<i64> {
        None
    }

    /// Whether INL-HOT's trials run at -O3 too (false: -O2 only).
    fn inline_hot_trial_at_o3(&self) -> bool {
        false
    }

    /// Whether a full unroll's saving is counted over `ASSUMED_TRIPS` runs
    /// of each unknown-count loop around it, as the pin's branch weights
    /// say (up to two). False: such a loop counts once, the runs the
    /// compiler can show.
    fn unroll_assumes_unknown_loops_run(&self) -> bool {
        true
    }

    /// Whether a call to a function of the module destroys what that function
    /// writes, not every caller-saved register (`-enable-ipra`; off in the
    /// pin, on in GCC from -O1).
    fn interprocedural_register_use(&self) -> bool {
        false
    }

    /// DEVIATE IPRA-DSO-LOCAL: see `Tuning::ipra_binds_local`.
    fn ipra_binds_local(&self) -> bool {
        false
    }

    /// Whether one wider store of constants may start off its own size
    /// (`allowsMemoryAccess` calls the unaligned store fast). False: it starts
    /// at a multiple of its size.
    fn merges_unaligned_stores(&self) -> bool {
        false
    }

    /// Whether a size build's loop rotation copies a header only when the copy
    /// is no bigger than the branches the rotation removes: the loop-closing
    /// jump and the header's own exit test, in `CodeSize`. False: the pass's
    /// header limit alone.
    fn rotation_header_priced_at_size(&self) -> bool {
        false
    }

    /// `Tuning::size_rotation_header_limit`.
    fn size_rotation_header_limit(&self) -> Option<u32> {
        None
    }

    /// Whether a priced full unroll counts a promoted local array's accesses
    /// as free only while its elements fit the registers (UP1).
    fn unroll_promotion_fits_registers(&self) -> bool {
        false
    }

    /// The signed bits of the immediate a store takes; 0: none (the stored
    /// constant needs a register).
    fn unroll_store_immediate_bits(&self) -> u32 {
        0
    }

    /// The same for the floats of an x87 target, against the stack's slots.
    fn unroll_promotion_fits_x87_stack(&self) -> bool {
        false
    }

    /// DEVIATE ARM64-UNROLL-PRICE: whether a loop of a nest of known trips
    /// that reads no counter of the loops around it is priced like any other
    /// full unroll instead of following the nest's decision (unrolling the
    /// nest folds nothing there), and a priced unroll counts the constant trips
    /// of the loops around it, not 32 each. False: the nest bypass holds for
    /// every such loop, the pin's threshold alone, and 32 a loop. A load's
    /// address does not count as a counter read (2026-11-07, owner w15, exit
    /// 2026-11-30: tile -O2 364 B and 2,155 instructions, rolled 68 B and
    /// 4,181).
    fn unroll_prices_nest_loops_without_outer_counter(&self) -> bool {
        false
    }

    /// DEVIATE ARM64-NEST-O3-INNER (owner arm64, exit 2026-11-24): at -O3 an
    /// innermost loop directly inside a loop of known trips is unrolled by the
    /// pin's threshold alone, unpriced (see `full_unroll_pays`). False: it is
    /// priced like any other loop.
    fn unroll_pin_decides_inner_of_known_trips(&self) -> bool {
        false
    }

    /// DEVIATE PHICONST-2 (owner x87, exit 2026-11-24): a phi of constants is
    /// folded into the users that fold, in a loop, and through a phi of
    /// constants behind a diamond. A move per edge the fold adds is paid by
    /// the operation it removes, which pays only where a byte is cheap against
    /// a clock: x86 m32 (x_cordic -O3 -800 clk); m16 grew 5 to 11 B (rate 2 B
    /// a clock).
    fn folds_phi_constant_chains(&self) -> bool {
        false
    }

    /// Whether a size build's loop rotation copies a header only when its
    /// exit test folds in the preheader (RS1). False: the pass's header limit
    /// alone.
    fn rotation_guard_must_fold_at_size(&self) -> bool {
        false
    }

    /// DEVIATE ROTATION-TRIAL (beyond the pin, owner greedy-os-causes, exit
    /// 2026-12-05): at a size build a counted loop RS1 still rotates is
    /// also tried unrotated in a second compile, and the smaller object is
    /// kept. Needs [`Self::rotation_guard_must_fold_at_size`]. False: no
    /// trial. DEVIATE ROTATION-HOT (beyond the pin, owner x86-m32, exit
    /// 2026-11-24): at a size build with the rotation trial, a loop RS1
    /// leaves (unfolded guard, trips not counted) is also tried rotated as the
    /// pin does, and kept where the finished code executes less for a few
    /// bytes a loop. False: RS1 alone.
    fn rotation_hot_at_size(&self) -> bool {
        false
    }

    /// Bytes a loop ROTATION-HOT rotates may add where the finished code
    /// executes less. 0: the object may not grow (-Os strict on bytes).
    fn rotation_hot_bytes_per_loop(&self) -> i64 {
        4
    }

    /// DEVIATE GVN-PRE-OPTSIZE: GVN's load PRE inserts no load into a
    /// predecessor in a function marked `optsize` or `minsize` (gcc's
    /// `tree-ssa-pre.cc` inserts at -Os only on a hot edge, and none is
    /// hot). The pin's `PerformLoadPRE` has no size gate.
    fn load_pre_off_at_size(&self) -> bool {
        false
    }

    fn rotation_trial_at_size(&self) -> bool {
        false
    }

    /// DEVIATE LICM-FRAME (size builds, target opt-in, owner w15, exit
    /// 2026-11-30): the speculative LICM does not hoist an instruction the loop
    /// might never execute into the entry block of a function when the loop
    /// calls: the value would live across the call in a callee-saved register
    /// and the frame could no longer start after the guard (shrink wrapping).
    /// False: the pin's rule.
    fn licm_speculation_keeps_frame_late(&self) -> bool {
        false
    }

    /// DEVIATE RL-EXITING-LATCH: whether `LoopRotate` also rotates a loop whose latch already exits when the copy of
    /// its header folds away in the preheader (the pin: only when a header phi dies or the latch's exit count
    /// becomes computable). gcc's header copy does it. False: the pin's.
    fn rotation_of_exiting_latch_when_guard_folds(&self) -> bool {
        false
    }

    /// DEVIATE Z80-ROT: whether -Oz rotates a loop whose copied header folds
    /// away in the preheader, the pin's no-copy limit notwithstanding.
    /// False: the pin.
    fn rotation_free_guard_at_minsize(&self) -> bool {
        false
    }

    /// The fewest calls a callee has in the module for the bound above to hold
    /// its sites (a callee called once is the pin's: the copy
    /// meets constants the cost model follows, and folds with the sites around
    /// it).
    /// Whether an -Os caller's SZ2 trial also inlines the sites its splice
    /// brings (`Tuning::inline_chain_trial`).
    /// An -Os caller's SZ2 trial leaves a callee that stays, with two or more
    /// calls left, to the by-size guard
    /// (`Tuning::inline_trial_defers_to_shared_growth`).
    fn inline_trial_defers_to_shared_growth(&self) -> bool {
        false
    }

    /// `visitADD`'s or-disjoint for adds of operands that share no bits
    /// (`Tuning::add_to_or_disjoint`).
    fn add_to_or_disjoint(&self) -> bool {
        false
    }

    fn opt_size_inline_chain_trial(&self) -> bool {
        false
    }

    /// Whether an -Os caller's SZ2 trial prices the dying callee once and
    /// refuses copies that grow the bytes where the callee's credit alone
    /// made the count accept. Only where `function_bytes` is calibrated
    /// against objects (m68k, `size_calibration.rs`); the x86 model is not, and
    /// a truthful credit refuses trials that shrink the build there.
    fn opt_size_inline_byte_gate(&self) -> bool {
        false
    }

    /// Whether an -Os caller's SZ2 trial is decided by the bytes alone: the
    /// copies stay when they leave the module smaller, the dying callee
    /// counted once. Only where `function_bytes` is calibrated against objects
    /// and the unit count (instructions of the pin's x86) means nothing: an
    /// 8-bit target's instruction is one to four bytes of work.
    fn opt_size_inline_trial_by_bytes(&self) -> bool {
        false
    }

    fn opt_size_inline_guard_min_sites(&self) -> i64 {
        1
    }

    /// Whether the bound above stands down where the pin's own cost, net of
    /// the instructions it folds, is below zero: the pin counts that inline as
    /// shrinking the caller (a one-instruction wrapper, a callee whose body
    /// folds away). False: the bound holds whatever the cost.
    fn opt_size_inline_guard_yields_to_negative_cost(&self) -> bool {
        false
    }

    /// Whether a constant address offset may become a phi: false where the
    /// access folds it as a displacement and the target says to keep it. A
    /// boolean as the pin's `replacingOperandWithVariableIsCheap` is: a
    /// register offset costs an add and a register at m16 (6 GPRs, base+index
    /// already taken), nothing at m32.
    fn constant_offset_phi_is_cheap(&self) -> bool {
        true
    }

    /// DEVIATE LU30: whether a static alloca in an address is the frame
    /// register and a displacement, which costs no register (`[bp+si+disp]`).
    fn frame_slot_is_free_base(&self) -> bool {
        false
    }

    /// DEVIATE LU31: whether LSR's over-complex search keeps, for every
    /// address use, only the formulae of the fewest registers (the pin does
    /// it for post-indexed targets only): where registers are scarcer than
    /// the winner-register heuristic assumes (x86-m16: six).
    fn lsr_address_fewest_registers(&self) -> bool {
        false
    }

    /// Whether a constant operand of an arithmetic instruction may become a phi
    /// in `sinkLastInstruction`: false where the join then costs a `mov` and a
    /// taken jump per use (DEVIATE SINK-CONST-PHI). Never at size.
    fn constant_arith_phi_is_cheap(
        &self,
        _optsize: bool,
    ) -> bool {
        true
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
    /// buffer holds; 0 where its scheduling model has none (X86: 28 for the
    /// `SandyBridgeModel` cpus; the others' models are not known here).
    /// The program's code is (not) position independent: addressing modes
    /// differ (`isPositionIndependent`). Set once per program.
    fn set_position_independent(
        &self,
        _position_independent: bool,
    ) {
    }

    fn loop_micro_op_buffer_size(&self) -> u32 {
        0
    }

    /// `getSchedModel().IssueWidth`; 0: the model states none.
    fn issue_width(&self) -> u32 {
        0
    }

    /// `Tuning::unroll_recurrence_veto`.
    fn vetoes_recurrence_bound_unroll(&self) -> bool {
        false
    }

    /// What a byte of code is worth in thousandths of a clock at speed, where
    /// the target trades bytes for clocks; 0: it does not.
    fn speed_milliclocks_per_byte(&self) -> i64 {
        0
    }

    /// Whether a full unroll is credited a `move` for each read, in the loops
    /// inside it, of a value its copies make constant: true where an immediate
    /// operand saves a register copy (two-address or stack machines). False
    /// where it saves nothing (3-address arm64: `cmp x, #n` = `cmp x, y`).
    /// Credited, a nested read counts a run of the inner loop's assumed trips
    /// (shellsort -O3 at 20 clocks a byte: 12,491 clocks "saved", 6,177 ->
    /// 7,090 instructions run, 164 -> 560 B).
    fn unroll_credits_immediate_operands(&self) -> bool {
        true
    }

    /// DEVIATE UP1: what a code byte a full unroll adds is worth in
    /// thousandths of a clock at -O2/-O3, where the unroll must save that many
    /// clocks to be made (`loopunrollprice`); 0: the pin's threshold alone.
    fn unroll_milliclocks_per_byte(&self) -> i64 {
        0
    }

    /// DEVIATE UNROLL-TRIAL (beyond the pin; owner 6502, exit 2026-12-05): the
    /// unrolled size, in the unroll units, up to which a loop the threshold
    /// refused is unrolled on trial: the function is simplified and priced
    /// whole, and the unroll stays only if it pays. `None`: no trial.
    fn unroll_trial_threshold(&self) -> Option<u32> {
        None
    }

    /// DEVIATE OS-INLINE-TRIAL: the limits of the Os inline trial, which prices
    /// a copy the byte model refuses by the finished code. `(band,
    /// instructions)`: a refusal is tried if it is within `band` percent of the
    /// caller's bytes, in a module of at most `instructions` instructions (it
    /// compiles the mid-end twice). None: no trial, as on every target but
    /// the 6502.
    fn opt_size_inline_trial_limits(&self) -> Option<(i64, usize)> {
        None
    }

    /// DEVIATE PEEL-OS-TRIAL: whether the -Os trial also peels the loops
    /// `computePeelCount` would at the `unroll_trial_at_size` bar (the pin's
    /// optsize threshold of 0 never does).
    fn peels_on_trial_at_size(&self) -> bool {
        false
    }

    /// DEVIATE UNROLL-OS-TRIAL: see `Tuning::unroll_trial_at_size`.
    fn unroll_trial_at_size(&self) -> Option<u32> {
        None
    }

    /// `Tuning::unroll_trial_bytes_strict`.
    fn unroll_trial_bytes_strict(&self) -> bool {
        false
    }

    /// `Tuning::unroll_trial_nests_only`.
    fn unroll_trial_nests_only(&self) -> bool {
        false
    }

    /// DEVIATE UNROLL-NEST-TRIAL: at -O2 and -O3 the loops with a subloop that
    /// the target's own `unroll_threshold_overrides` refused but the pin's
    /// threshold admits are noted, and the driver compiles the module again
    /// with them unrolled and keeps that where the finished code prices
    /// better at the CPU's byte rate. The pin never judges such a loop by its
    /// savings (`analyzeLoopUnrollCost` skips a loop with a subloop), only
    /// by size.
    fn unroll_trial_nests_at_speed(&self) -> bool {
        false
    }

    /// `Tuning::inline_dying_credit_at_os` (D13).
    fn inline_dying_credit_at_os(&self) -> bool {
        true
    }

    /// `Tuning::unroll_never_grows_at_o1`.
    fn unroll_never_grows_at_o1(&self) -> bool {
        false
    }

    /// Whether the -O1 unroll walk may peel a loop (the pin does, under
    /// `-funroll-loops`). A peel copies the loop body, which -O1 on a
    /// bytes-strict target cannot pay for.
    fn peels_at_o1(&self) -> bool {
        true
    }

    /// `Tuning::second_ipsccp`.
    fn second_ipsccp(&self) -> bool {
        false
    }

    /// DEVIATE UP1-CALL at -O1: the size-neutral rule above prices a call the
    /// copies keep as one instruction with its operands, where the pin's
    /// analysis gives up on a call (gcc's `estimate_num_insns` prices it).
    fn unroll_o1_prices_calls(&self) -> bool {
        false
    }

    /// `Tuning::unroll_o1_analyzed_trips`.
    fn unroll_o1_analyzed_trips(&self) -> Option<u32> {
        None
    }

    /// `Tuning::pops_arguments_at_size`.
    fn pops_arguments_at_size(&self) -> bool {
        true
    }

    /// `Tuning::libc_memcpy`.
    fn libc_memcpy(&self) -> bool {
        false
    }

    /// `Tuning::multiply_by_clocks`.
    fn multiply_by_clocks(&self) -> bool {
        false
    }

    /// `Tuning::keeps_sahf_where_bytes_are_held`.
    fn keeps_sahf_where_bytes_are_held(&self) -> bool {
        false
    }

    /// DEVIATE DIV-LIVE (owner x86-m32, exit 2026-11-24):
    /// `Tuning::narrow_divide_live_limit`.
    fn narrow_divide_live_limit(&self) -> u32 {
        0
    }

    /// `Tuning::narrow_divide_live_max_bits`.
    fn narrow_divide_live_max_bits(&self) -> u32 {
        31
    }

    /// `Tuning::narrow_divide_live_at_size`.
    fn narrow_divide_live_at_size(&self) -> bool {
        true
    }

    /// DEVIATE INL-O1: `Tuning::inline_never_grows_without_rate`.
    fn inline_never_grows_without_rate(&self) -> bool {
        false
    }

    /// DEVIATE BFS-1: `Tuning::cold_site_ignores_sinks`.
    fn cold_site_ignores_sinks(&self) -> bool {
        false
    }

    /// `Tuning::inline_o1_weighs_entry`.
    fn inline_o1_weighs_entry(&self) -> bool {
        false
    }

    /// `Tuning::inline_o1_unit_bytes_milli`.
    fn inline_o1_unit_bytes_milli(&self) -> i64 {
        1000
    }

    /// DEVIATE INL-O1: `Tuning::inline_o1_milliclocks_per_byte`.
    fn inline_o1_milliclocks_per_byte(&self) -> i64 {
        0
    }

    /// DEVIATE VASTART-PROMOTE (owner x86-m32, exit 2026-11-27; users: x86-m32,
    /// m68k with `va_arg_slot` = 4): `Tuning::promote_va_list`.
    fn promote_va_list(&self) -> bool {
        false
    }

    /// DEVIATE VASTART-PROMOTE: the slot every stack argument takes where
    /// `va_arg` is left in the IR as an instruction (clang writes it for
    /// targets without a `va_arg` lowering of its own) and `va_list` is one
    /// pointer: each read loads the pointer, loads the value, steps the
    /// pointer by the type's size (alignment capped to the slot) and stores
    /// it back, so SROA can keep the pointer in a register. `None`: `va_arg`
    /// stays an instruction.
    fn va_arg_slot(&self) -> Option<u32> {
        None
    }

    /// DEVIATE SETCC-TREE-BRANCH: `Tuning::branch_compare_trees_at_size`.
    fn branch_compare_trees_at_size(&self) -> bool {
        false
    }

    /// DEVIATE SINK-EXT (owner x86-m32, exit 2026-11-24):
    /// `Tuning::sink_spares_ranges`.
    fn sink_spares_ranges(&self) -> bool {
        false
    }

    /// `Tuning::divide_by_bytes`.
    fn divide_by_bytes(&self) -> bool {
        false
    }

    /// `Tuning::immediate_forms_at_size`.
    fn immediate_forms_at_size(&self) -> bool {
        false
    }

    /// `Tuning::price_pin_multiplies`.
    fn price_pin_multiplies(&self) -> bool {
        false
    }

    /// `Tuning::lftr_any_stride` (DEVIATE LFTR1).
    fn lftr_any_stride(&self) -> bool {
        false
    }

    /// DEVIATE MUL-HOT: see `Tuning::mul_chains_hot_at_size`.
    fn mul_chains_hot_at_size(&self) -> bool {
        false
    }

    /// `Tuning::imul_at_size`.
    fn imul_at_size(&self) -> bool {
        false
    }

    /// `Tuning::merge_functions_at_size`.
    fn merge_functions_at_size(&self) -> bool {
        false
    }

    /// Drop the local functions that are the same code at the levels that
    /// allow no trade between bytes and clocks, and at speed (`MergeLocalFunctions`);
    /// no thunk is written.
    fn merge_local_functions_at_speed(&self) -> bool {
        false
    }

    /// `Tuning::sdiv_pow2_stays_at_size`.
    fn sdiv_pow2_stays_at_size(&self) -> bool {
        false
    }

    /// `Tuning::rep_movs_at_size`.
    fn rep_movs_at_size(&self) -> bool {
        false
    }

    /// `Tuning::rep_movs_at_size_max`.
    fn rep_movs_at_size_max(&self) -> u64 {
        128
    }

    /// `Tuning::rep_stos_at_size_max`.
    fn rep_stos_at_size_max(&self) -> u64 {
        128
    }

    /// `Tuning::rep_stos_at_size`.
    fn rep_stos_at_size(&self) -> bool {
        false
    }

    /// `Tuning::rep_stos_priced_unit`.
    fn rep_stos_priced_unit(&self) -> bool {
        false
    }

    /// `Subtarget.canUseCMOV()`: false where a `select` is lowered to a branch.
    fn has_cmov(&self) -> bool {
        true
    }

    /// DEVIATE D5 (`twoentryphi.rs`): what each select costs beyond its own
    /// price when folding a two-entry phi (a select is a branch and a move
    /// where there is no conditional move). 0: as at the pin, which does not
    /// count the selects. `optsize`: the function is a size build.
    fn select_branch_extra(
        &self,
        optsize: bool,
    ) -> i64 {
        let _ = optsize;
        0
    }

    /// `Tuning::carry_slot_indexes`.
    fn carries_slot_indexes(&self) -> bool {
        true
    }

    /// `Tuning::sink_select_arms`.
    fn sinks_select_arms(&self) -> bool {
        false
    }

    /// `Tuning::branch_constant_arm_selects`.
    fn branches_constant_arm_selects(&self) -> bool {
        false
    }

    /// Whether `sinks_select_arms` also holds in a size build: the branch form
    /// leaves out the copy of the other arm, so it is not longer.
    fn sinks_select_arms_for_size(&self) -> bool {
        false
    }

    /// `Tuning::inline_variable_memcpy`.
    fn inlines_variable_memcpy(&self) -> bool {
        false
    }

    /// `Tuning::size_copy_price`.
    fn copy_price_for_size(&self) -> Option<crate::target::CopyPrice> {
        None
    }

    /// DEVIATE LG3: the bytes of the instructions a loop's setup and exit test
    /// are made of, where the target's code size is what its cost is (wasm).
    /// LSR then breaks a tie between the end-biased solution and the pin's by
    /// bytes. None: the pin's order stands. owner: w12, exit: 2026-11-20.
    fn lsr_bytes(&self) -> Option<LsrBytes> {
        None
    }

    /// DEVIATE LG1, LG2: `GenerateICmpZeroScales` scales a compare against zero
    /// by a factor whose product may wrap, where it is still a zero test
    /// (`lsr_generate`). False: `getExactSDiv`'s no-overflow check alone, as
    /// the pin has it (wasm). owner: wasm, exit: 2026-12-04.
    fn lsr_scales_zero_compare_past_overflow(&self) -> bool {
        true
    }

    /// `Tuning::ipsccp_widen_keeps_sign`.
    fn ipsccp_widen_keeps_sign(&self) -> bool {
        false
    }

    /// `Tuning::ipsccp_widen_keeps_sign_at_size`.
    fn ipsccp_widen_keeps_sign_at_size(&self) -> bool {
        false
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

    /// DEVIATE IPACP-O3: whether a function is copied for the constants its
    /// callers pass at -O2 and -O3 (gcc: -O3 only), as gcc's `-fipa-cp-clone`
    /// (on at -O3) does and the pin's pipeline does not;
    /// `-f[no-]ipa-cp-clone` decides when given.
    fn ipa_cp_clones_at_speed(&self) -> bool {
        false
    }

    /// DEVIATE PL6: whether a loop's first iteration is peeled where its entry
    /// constants fold enough of it to pay the copy at the speed rate.
    fn peels_folded_entry(&self) -> bool {
        false
    }

    /// DEVIATE REC-2: gcc's growth bar of the recursive edge at (-O2, -O3),
    /// in `gcc_insns` of the callee;
    /// none: no bar.
    fn recursive_edge_growth(&self) -> Option<(i64, i64)> {
        None
    }

    /// DEVIATE REC-2: what the copies of a recursive function in itself may add
    /// at (-O2, -O3), in `gcc_insns`; none: the pass's own.
    fn recursive_inlining_growth(&self) -> Option<(i64, i64)> {
        None
    }

    /// DEVIATE REC-1 (arm64): the least share (numerator, denominator) of a
    /// recursion's body clocks that its calls to itself spend on the call,
    /// return and argument words for its copies to be made: a copy removes
    /// those and nothing else. None: no bound.
    fn recursive_inlining_call_share(&self) -> Option<(i64, i64)> {
        None
    }

    /// DEVIATE REC-9: whether a linear recursion (one call of itself) called
    /// from another function is copied into the caller and peeled there, as
    /// gcc does, without the pin's `recursive` refusal or the
    /// bytes-against-clocks rule (gcc prices no clocks; the peeling's
    /// own caps bound it). false: REC-1's bounds and the price, as for a tree.
    fn peels_linear_recursion(&self) -> bool {
        false
    }

    /// DEVIATE REC-7: the depth of the copies at (-O2, -O3); none: the depth
    /// of `recursive_inlining`.
    fn recursive_inlining_depth(&self) -> Option<(u32, u32)> {
        None
    }

    /// DEVIATE REC-2: whether, at (-O2, -O3), a tree recursion's tail calls are
    /// eliminated before its copies of itself are made.
    fn recursive_tre_first(&self) -> (bool, bool) {
        (false, false)
    }

    /// DEVIATE REC-1: the ceilings (depth, callee size in `gcc_insns`) within
    /// which a tree-recursive function's copies of itself are chosen by
    /// price: each level of copies is made while the clocks it saves, at
    /// `speed_milliclocks_per_byte`, are worth the bytes it adds. None: the
    /// bounds of `recursive_inlining` are the rule.
    fn priced_self_inlining(&self) -> Option<(u32, i64)> {
        None
    }

    /// The target's `Threshold` and `MaxPercentThresholdBoost` for the full
    /// unroll (its tuning table); none: the pass's own.
    fn unroll_threshold_overrides(&self) -> (Option<u32>, Option<u32>) {
        (None, None)
    }

    /// How many values the x87 stack holds where this subtarget keeps floats
    /// there (no SSE2 floating point); none: floats are in registers a
    /// pressure rule need not count.
    fn x87_stack_slots(&self) -> Option<u32> {
        None
    }

    /// Whether a product by a constant and one by its negation are made once
    /// on the x87 stack (`Tuning::x87_share_negated_products`).
    fn x87_share_negated_products(&self) -> bool {
        false
    }

    /// Whether a zero fill is stored as zero doubles through the FPU at `-Os`
    /// (`Tuning::x87_zero_fill_at_size`).
    fn x87_zero_fill_at_size(&self) -> bool {
        false
    }

    /// DEVIATE FPTOINT-CHECKED: the width of the integer a float is converted
    /// into in one instruction (the x87's `fistp` stores a dword or a qword,
    /// and NaN and overflow come out as its most negative value) when the
    /// result is `bits` wide; none: the range of the integer is checked on the
    /// float.
    fn wide_float_to_int(
        &self,
        bits: u32,
    ) -> Option<u32> {
        let _ = bits;
        None
    }

    /// `Tuning::size_fill`: the encoded bytes of a constant fill at size.
    fn size_fill(&self) -> Option<crate::target::SizeFill> {
        None
    }

    /// The target's `PartialThreshold` (its tuning table); none: the pass's
    /// own.
    fn unroll_partial_threshold_override(&self) -> Option<u32> {
        None
    }

    /// The `PartialThreshold` of the unroll-and-jam's outer loop (its tuning
    /// table); none: `unroll_partial_threshold_override`'s.
    fn unroll_jam_partial_threshold_override(&self) -> Option<u32> {
        None
    }

    /// Tuning `unroll_jam_exact_count`.
    fn unroll_jam_exact_count(&self) -> bool {
        false
    }

    /// Tuning `constant_hoisting`.
    fn constant_hoisting(&self) -> bool {
        false
    }

    /// `Tuning::min_jump_table_density`.
    fn min_jump_table_density(&self) -> Option<u64> {
        None
    }

    /// The target's `-unswitch-threshold` (its tuning table); none: the pin's
    /// 50.
    fn unswitch_threshold_override(&self) -> Option<i64> {
        None
    }

    /// `isHardwareLoopProfitable`: whether the target counts loops in hardware
    /// (`djnz`, `dbra`, `loop`), and how, in `info`. The hook sees no loop
    /// yet: no target reads one (DEVIATE 3 of `hardwareloops`). Off by
    /// default; the pass does not run for a target that says no.
    fn is_hardware_loop_profitable(
        &self,
        _info: &mut HardwareLoopTarget,
    ) -> bool {
        false
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

    /// `int_imm_cost_inst` for a pass that also knows whether the function is
    /// built for size (`ConstantHoisting`'s `OptForSize`). The pin's cost
    /// does not depend on it; a target's DEVIATE may (x86 ZIMM).
    fn int_imm_cost_inst_for_size(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
        _for_size: bool,
    ) -> Cost {
        self.int_imm_cost_inst(op, index, imm, bits)
    }

    /// `int_imm_cost_inst` as the pin prices it, before any DEVIATE of the
    /// target's own (x86 ZIMM): what `ConstantHoisting` asks to keep a
    /// constant a candidate.
    fn int_imm_cost_inst_pin(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost {
        self.int_imm_cost_inst(op, index, imm, bits)
    }

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

    /// `enableMemCmpExpansion(OptSize, IsZeroCmp)`: how a `memcmp` may be
    /// expanded to loads and compares; none (the base class) leaves the call.
    fn enable_mem_cmp_expansion(
        &self,
        opt_size: bool,
        is_zero_cmp: bool,
    ) -> Option<MemCmpExpansionOptions> {
        let _ = (opt_size, is_zero_cmp);
        None
    }

    /// `allowsMisalignedMemoryAccesses` of an `bits`-wide integer access in
    /// `address_space` at `align` bytes. The base class has none.
    fn allows_misaligned_memory_accesses(
        &self,
        bits: u32,
        address_space: u32,
        align: u64,
    ) -> bool {
        let _ = (bits, address_space, align);
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

    /// `getIntImmCodeSizeCost`: the code size of the constant at operand
    /// `index` of an `op`; the base class has none.
    fn int_imm_code_size_cost(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost {
        let _ = (op, index, imm, bits);
        Cost::FREE
    }

    /// `getIntImmCostIntrin`: the constant at operand `index` of a call to the
    /// intrinsic `name`; the base class makes it free.
    fn int_imm_cost_intrin(
        &self,
        name: &str,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost {
        let _ = (name, index, imm, bits);
        Cost::FREE
    }

    /// `preferToKeepConstantsAttached` (`BasicTTIImpl`): a divide or remainder
    /// by a constant keeps it unless `isIntDivCheap`, which the base class
    /// says it is not. `min_size`: the function is `minsize`.
    fn prefer_to_keep_constants_attached(
        &self,
        div_rem_by_constant: bool,
        min_size: bool,
    ) -> bool {
        let _ = min_size;
        div_rem_by_constant
    }

    /// DEVIATE ZIMM (M64-ZIMM-CRC): a zero-extended 32-bit constant that is
    /// one arm of a `select` beside zero. The target turns it into a mask
    /// `and` with the constant as an immediate, so hoisting it only holds a
    /// register. The pin's `preferToKeepConstantsAttached` has no such case.
    fn keeps_select_mask_attached(
        &self,
        bits: u128,
        width: u32,
    ) -> bool {
        let _ = (bits, width);
        false
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

    /// DEVIATE LSR-SIZE (owner w12, exit 2026-11-24): LSR ranks formulae by
    /// the bytes of this table first when the function is built for size;
    /// none is the pin's order at every level.
    fn lsr_size_model(&self) -> Option<LsrSizeModel> {
        None
    }

    /// DEVIATE CIV1 (owner alloc-small, exit 2026-11-30): `replaceCongruentIVs`
    /// maps each wide counter's truncation to the type of the header's last
    /// phi (`Phis.back()`, ScalarEvolutionExpander.cpp#L1965), which is a
    /// pointer phi when the loop has one (pointers sort to the back), and
    /// `isTruncateFree` to a pointer is false: a loop that steps a pointer
    /// keeps a narrow counter beside the wide one it truncates to. True
    /// takes the narrowest integer phi. The divergence is our real-mode
    /// `huge_space` addressing mode (a bare base register only), which the
    /// pin has no counterpart for: under the pin's ordinary p3 modes LSR
    /// shares the wide counter (GenerateTruncates) and `llc
    /// -print-after=loop-reduce` leaves no duplicate i16 phi. m16 huge bq16
    /// -O1: 357 -> 339 B.
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1965
    fn congruent_ivs_truncate_to_narrowest_integer(&self) -> bool {
        false
    }

    /// DEVIATE LU13 (owner z80, exit 2026-11-30: a per-class register model in
    /// the LSR cost): a call clobbers every register, so each register the
    /// loop keeps across one is a frame slot, reloaded each trip and, for a
    /// recurrence, stored each trip. The pin has no call term in
    /// `LoopStrengthReduce.cpp` (it reads `getNumberOfRegisters` at L1613,
    /// `isLSRCostLess` at L1663 and `isNumRegsMajorCostOfLSR` at L6385 only).
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1613
    fn lsr_call_clobbers_registers(&self) -> bool {
        false
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

    /// Beyond the pin (AArch64 and ARM step by any small immediate): whether a
    /// post-increment access of `access_bytes` may step its pointer by `step`
    /// bytes. m68k's `(An)+` steps by the access size and no other amount.
    fn is_post_inc_step_legal(
        &self,
        _access_bytes: i64,
        _step: i64,
    ) -> bool {
        true
    }

    /// Beyond the pin: whether a size build keeps a loop whose closed form
    /// (`rewriteLoopExitValues`) costs more code than the loop, as the 16-bit
    /// targets do. m68k's wide multiplies are library calls.
    fn sizes_exit_values(&self) -> bool {
        false
    }

    /// Beyond the pin: whether `LoopIdiomRecognize` turns a shift loop into a
    /// `ctlz`/`cttz` count (`recognizeAndInsertFFS`, `recognizeShiftUntil*`).
    /// The pin does on every target; one that expands `ctlz` to a long
    /// sequence, or weighs bytes against the loop, keeps the loop and says
    /// false until its `Tti` prices the intrinsics.
    fn shift_loop_counts(&self) -> bool {
        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfoImpl.h#L569
    /// `getPopcntSupport(bits) == PSK_FastHardware`: whether `ctpop` of an
    /// integer of `bits` is one fast instruction. The base class says
    /// software; `LoopIdiomRecognize` forms `ctpop` only where it is fast.
    fn popcnt_support_fast(
        &self,
        _bits: u32,
    ) -> bool {
        false
    }

    /// Beyond the pin: whether a function built for size turns a store or
    /// copy loop into a `memset` or `memcpy` call (`LoopIdiomRecognize`). The
    /// pin does; a target whose call setup and library routine are more bytes
    /// than the loop (m68k: three pushes, `jsr`, `lea` against a four
    /// instruction loop) says false.
    fn memory_idioms_at_optsize(&self) -> bool {
        true
    }

    /// Beyond the pin: whether a shift by a variable amount needs the count
    /// in one fixed register (x86's `cl`), which costs a move and holds the
    /// register. A size build then keeps `mul X, select(c, 8, 2)` rather than
    /// the pin's `shl X, select(c, 3, 1)` (`visitMul`'s `tryGetLog2`).
    fn variable_shift_count_in_fixed_register(&self) -> bool {
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

    /// Beyond the pin: whether a counter of an integer width the target does
    /// not hold natively (`DataLayout::legal_integer`) whose range provably
    /// fits a native width becomes a counter of that width (`narrowiv`). LLVM
    /// only widens counters; `for (long i = 0; i < 12; ++i)` keeps four bytes a
    /// compare on an 8-bit target. Off by default; a target opts in where it
    /// measured a win. DEVIATE IV-NARROW (owner z80-b, review 2026-12-20).
    fn narrows_counters(&self) -> bool {
        false
    }

    /// Beyond the pin: whether [`Tti::narrows_counters`] also applies when the
    /// function is not sized for bytes. A narrower counter is fewer bytes of
    /// code but can cost clocks (a scaled byte index is a shift chain the
    /// pointer recurrence did not need); a target that has not priced that
    /// says no and narrows at `-Os` only.
    fn narrows_counters_at_speed(&self) -> bool {
        true
    }

    /// Beyond the pin: whether a cycle through a phi whose values only matter
    /// at a narrower native width (read through `and` masks, `trunc`s and
    /// values known zero above it) is carried at that width (`narrowcycle`),
    /// when [`Tti::narrowing_gain`] prices every operation of it no dearer, the
    /// whole cheaper and no axis dearer, the extensions at its uses counted.
    /// `TruncInstCombine` starts from a `trunc` and wants every value of its
    /// graph unused outside it; a phi cycle read by a mask and a compare of
    /// masked bits has neither. Off by default.
    /// DEVIATE NARROW-CYCLE (owner m68k, exit 2026-11-20).
    fn narrows_demanded_cycles(&self) -> bool {
        false
    }

    /// What `op` at `narrow_bits` saves against `wide_bits` (negative: dearer);
    /// `constant` is its constant operand, `source_dies` whether the other
    /// operand has no use after it (a two-address target copies it
    /// otherwise); `narrow_bits` 0 is the operation gone. Zero where the
    /// target does not price it apart.
    fn narrowing_gain(
        &self,
        _op: crate::opcode::BinaryOp,
        _wide_bits: u32,
        _narrow_bits: u32,
        _constant: Option<u64>,
        _source_dies: bool,
    ) -> Saving {
        Saving::default()
    }

    /// What a cycle at `narrow_bits` saves per iteration when it doubles a
    /// value (`shl x, 1`) and tests that value's top bit (`and x, 1 <<
    /// (narrow_bits - 1)`): the shift leaves the bit in the carry at that
    /// width only, so the target reads it there and drops the copy and the
    /// test (m68k `cmpcarry`). Zero where the target has no such flag.
    /// DEVIATE NARROW-CYCLE (owner m68k, exit 2026-11-20).
    fn shift_out_test_gain(
        &self,
        _narrow_bits: u32,
    ) -> Saving {
        Saving::default()
    }

    /// What reading a value of `narrow_bits` where `wide_bits` is wanted costs
    /// (the zero extension), as a positive saving lost.
    fn extension_cost(
        &self,
        _narrow_bits: u32,
        _wide_bits: u32,
    ) -> Saving {
        Saving::default()
    }

    /// Whether `IndVarSimplify` runs `sinkUnusedInvariants`. The pin runs it
    /// on every target.
    /// DEVIATE (2026-11-07, owner wasm, exit 2026-11-21): off by default;
    /// on, 17 other-target cells grew (6502 jpeg, arm64 jpeg, x86_32 grep).
    /// Flip the default when those are fixed.
    /// [pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp)
    fn sinks_unused_invariants(&self) -> bool {
        false
    }

    /// Whether InstCombine makes `scmp`/`ucmp` of a select chain
    /// (`foldSelectToCmp`). The pin does so on every target.
    /// DEVIATE (2026-11-07, owner wasm, exit 2026-11-21): off by default; on,
    /// 62 other-target cells grew (arm64 sfm16 -Os +500 B, m68k -O2 +320 B,
    /// x86_32 -O2 +28 B, 27 z80 cells): the compare's expansion and the
    /// inline cost of the callee that holds it. Flip the default when fixed.
    /// [pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSelect.cpp#L4172)
    fn folds_select_to_cmp(&self) -> bool {
        false
    }

    /// Whether InstCombine folds `icmp (phi C1, C2), C` into a phi of the
    /// folded compares (`foldICmpWithConstant`). The pin does so on every
    /// target. DEVIATE ICMPPHI-1 (2026-11-12, owner wasm, exit 2026-11-24):
    /// on for wasm only. Elsewhere the exit test of a counted loop becomes
    /// a live i1 value the lowering handles badly (m68k, z80, 6502: 31
    /// cells up +295 B; arm64, x86: 18 of 21 cells up +128 B). Flip when
    /// their lowering does.
    /// [pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L1369)
    fn folds_icmp_of_constant_phi(&self) -> bool {
        false
    }

    /// Whether InstCombine leaves `icmp (udiv X, C), K` alone while the
    /// quotient has another use. The pin folds it into a range test of `X`
    /// on every target (`foldICmpDivConstant`). DEVIATE ICMPDIV-1 (owner
    /// llrm-target-6502, exit 2026-11-24): where a divide is a library call,
    /// the fold keeps the dividend live across it beside the quotient the
    /// loop carries (digits Os: two callee-saved cells, 12 B of push and pop).
    /// [pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L2857)
    fn keeps_icmp_of_shared_division(&self) -> bool {
        false
    }

    /// Whether InstCombine leaves `icmp eq/ne (select c, a, b), (select c, b, a)` alone. The pin
    /// compares the arms, `icmp eq/ne a, b` (`matchSymmetricPair`), on every target. DEVIATE
    /// SYMPAIR-1 (owner llrm-target-wasm, exit 2026-11-24): at m16 a 64-bit arm compare is a
    /// longer sequence than the two selects it replaces (+4 B in 24 sfm16_float objects).
    /// [pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp)
    fn keeps_icmp_of_swapped_selects(&self) -> bool {
        false
    }

    /// A target's value of the pin's `cl::opt` `SCEVCheapExpansionBudget`
    /// (default 4), for a function built for size or not: the operations
    /// `IndVarSimplify`'s test replacement may spend on a loop's exit limit.
    /// [pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L41)
    fn lftr_expansion_budget(
        &self,
        _optsize: bool,
    ) -> u32 {
        4
    }

    /// `isNumRegsMajorCostOfLSR`.
    fn is_num_regs_major_cost_of_lsr(&self) -> bool {
        true
    }

    /// `shouldDropLSRSolutionIfLessProfitable`.
    fn should_drop_lsr_solution_if_less_profitable(&self) -> bool {
        false
    }

    /// DEVIATE LU15 (owner z80, exit 2026-12-15: a baseline that prices the
    /// loop as written): with `should_drop_lsr_solution_if_less_profitable`,
    /// a loop that has an address use keeps LSR's solution. The baseline
    /// rates every address as one pointer register, so a solution that adds
    /// the pointer beside a count always looks worse, and dropping it keeps
    /// `base + i` (strlen: +17 B, a frame). The pin has no drop here.
    fn lsr_dropped_solution_spares_addresses(&self) -> bool {
        false
    }

    /// Instructions the exit test of a counter that ends at zero costs beyond
    /// the step: 0 where the step's flags (or a branch on the register) end
    /// the loop, 1 where the zero has to be tested by its own instruction
    /// (z80's 16-bit `inc`). A counter that does not end at zero pays the
    /// compare instead.
    fn zero_test_cost(&self) -> u32 {
        0
    }

    /// The registers an address can be formed from by the target's cheapest
    /// address form (m16: bx, si, di); the general registers where any
    /// register serves.
    fn address_registers(&self) -> u32 {
        self.number_of_gpr_registers()
    }

    /// DEVIATE LG3: instructions one base register that names a global's
    /// address costs in the preheader (arm64: `adrp` + `add`, and a symbol
    /// plus a constant is a new pair, not the pair the pin's bases already
    /// hold). LSR keeps the pin's solution over the end-biased one when
    /// those new bases outweigh the loop instructions the latter saves over
    /// the trips the loop is known to run. 0: bases are not priced (the
    /// pin's rule).
    fn lsr_symbol_base_setup_insns(&self) -> u32 {
        0
    }

    /// Whether a one-register formula of the first use makes its register a
    /// loser for the later uses (`VisitedRegs`, `SolveRecurse` L5600): the
    /// pin's rule. DEVIATE LS1 turns it off where the target has too few
    /// registers for the narrower search to hide a counter that serves two
    /// uses.
    fn lsr_visited_registers_lose(&self) -> bool {
        true
    }

    /// DEVIATE LU14: whether `Solve` also searches the uses in the opposite
    /// order and keeps the cheaper solution (a tie keeps the pin's order).
    /// `SolveRecurse` requires of a use the registers the uses before it
    /// chose, so a cheaper combination that starts with a later use is never
    /// rated. The pin searches one order.
    fn lsr_solves_both_use_orders(&self) -> bool {
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

    /// Whether `LoopTermFold` runs after LSR for this target, for a function
    /// built for size or not: the pin enables it for RISCV, AVR and Mips only
    /// (`EnableLoopTermFold`,
    /// [TargetPassConfig.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/TargetPassConfig.cpp#L875)).
    fn enables_loop_term_fold(
        &self,
        _optsize: bool,
    ) -> bool {
        false
    }

    /// `TargetTransformInfo::isProfitableToHoist` (the pin's `TargetLowering`
    /// hook, overridden by AArch64 and PowerPC to keep a fused multiply-add or
    /// a load pattern): whether `inst`, identical in every arm of a branch,
    /// should run once before it. Asked by SimplifyCFG's
    /// `hoistCommonCodeFromSuccessors`.
    fn is_profitable_to_hoist(
        &self,
        _context: &crate::context::Context,
        _function: &crate::module::Function,
        _inst: crate::module::InstId,
    ) -> bool {
        true
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

    /// The values LICM may hold across a loop before it refuses a promotion
    /// (DEVIATE LP1, `licm/pressure.rs` in llrm-transforms): the register count
    /// by default; `None` where registers are not what a value is held in and
    /// the pin promotes without a cap.
    fn licm_register_limit(&self) -> Option<u32> {
        Some(self.number_of_gpr_registers())
    }

    /// DEVIATE IND-MASK (beyond the pin; exit 2026-11-30): whether `indvars`
    /// also widens a counter of an illegal narrow width whose chain is
    /// masked by a constant and then extended (`(i*5+3)&1023` indexing a
    /// table, `short i`). The pin sees only an extension of the counter's own
    /// recurrence, so the chain stays narrow and `LSR` skips it.
    fn indvars_widens_narrow_counter_through_masks(&self) -> bool {
        false
    }

    /// DEVIATE IND-PREFIX (beyond the pin; exit 2026-11-30): an integer
    /// width `indvars_widens_narrow_counter_through_masks` also covers
    /// although it is legal, because every operation on it pays more than the
    /// register's width (the 486's operand-size prefix on 16 bits).
    fn narrow_counter_is_taxed(
        &self,
        _bits: u32,
    ) -> bool {
        false
    }

    /// DEVIATE LSR-N: integer widths LSR takes as native induction variables
    /// besides the datalayout's `n`. The pin's `IVUsers` skips every type
    /// `isLegalInteger` refuses, so on a datalayout of `n8` no 16- or 32-bit
    /// counter, product or pointer is reduced. None: the datalayout's.
    /// Held at `-Os` too. Owner llrm-target-6502; exit: a byte-aware LSR cost
    /// (`lsr_size_model`) for the 6502, 2026-11-24.
    fn lsr_native_integer_widths(&self) -> &'static [u32] {
        &[]
    }

    /// DEVIATE IU1: whether `IVUsers` meets the users of a value in use-list
    /// order, as the pin does, where the default is descending program order.
    /// LSR's ties follow it: wasm32 memory_expansion -O2 was 1307 B over the
    /// pin and is 367 B under. Other targets state it when their cells agree.
    /// Owner llrm-target-wasm; exit: every target judged, 2026-11-24.
    fn lsr_users_in_use_list_order(&self) -> bool {
        false
    }

    /// DEVIATE LSR-PHI (owner x87, exit 2026-11-24): whether `IVUsers` makes a
    /// use of each incoming value of a phi that is a user, where the pin's
    /// `Processed` check (`IVUsers.cpp` `AddUsersImpl`, "do not recurse into a
    /// phi twice") keeps the first and drops the rest, so `i*300+21472` and
    /// `i*300-81472` merging in one phi leave one of the two a multiply chain.
    /// x_cordic m32 -O3: 100 trips of a 5-instruction chain gcc does not pay.
    fn lsr_phi_operand_uses(&self) -> bool {
        false
    }

    /// DEVIATE IU1 (owner llrm-target-x86-m64, exit 2026-11-24): with use-list
    /// order, whether `IVUsers` meets a phi before the other users of a value.
    /// The pin keeps the one operand of a phi it meets first (`Processed`, "do
    /// not recurse into a phi twice"); use-list order is edit history, so
    /// which operand that is changes with edits. A phi first is the order
    /// program order gives. div_constants m64 -O1: 4334 B -> 4173 B and
    /// 6960 B -> 6854 B, no cell larger by more than 3 B.
    fn lsr_phi_users_first(&self) -> bool {
        false
    }

    /// DEVIATE FMULADD-SPLIT (owner x87-c, exit 2026-11-24): whether LICM
    /// splits an `llvm.fmuladd` of a loop whose factors the loop does not
    /// write into `fmul` and `fadd`, the product then leaving the loop;
    /// true where the target has no fused multiply-add to make of it.
    /// x_ld_to_ulong m32 -O1: `r * 67108864` made on every trip of the
    /// inner loop, gcc holds it.
    fn splits_invariant_fmuladd(&self) -> bool {
        false
    }

    /// The registers LSR keeps back from `number_of_gpr_registers` when it
    /// also counts what the loop keeps live (DEVIATE LU3c); the pin's one.
    fn lsr_reserved_registers(&self) -> u32 {
        1
    }

    /// `TCC_Basic` in this target's cost units for `kind` (DEVIATE TB1; the
    /// pin's is the constant 1): the SCEV expansion budgets
    /// (`isHighCostExpansion`, 4 of them) are counted in it. A target whose
    /// costs are not about one per instruction (the Z80 prices in machine
    /// cycles) says what a basic instruction costs.
    fn tcc_basic(
        &self,
        _kind: CostKind,
    ) -> Cost {
        Cost::BASIC
    }

    /// DEVIATE LU12: whether the adds of a use whose fixups all lie after the
    /// loop are rated as run once, not every trip (the pin rates them every
    /// trip). Off: they cost the loop an add each, as in the pin.
    fn lsr_exit_use_adds_run_once(&self) -> bool {
        false
    }

    /// Whether a stack slot's address is a displacement off the frame register
    /// in an addressing mode, so as the base of an address it costs no
    /// register of its own.
    fn frame_address_is_free(&self) -> bool {
        false
    }

    /// DEVIATE EXT-LOAD-COST: whether a load that widens costs clocks over
    /// the plain load, so CGP's `optimizeExt` leaves an extension in the
    /// block that reads it. The pin has no such cost: it moves it.
    fn ext_load_costs_extra(&self) -> bool {
        false
    }

    /// The bits one general-purpose register holds: an `i32` takes one at 32,
    /// two at 16 (the 386 registers of a 16-bit target are 32 wide).
    fn gpr_bits(&self) -> u32 {
        32
    }

    /// Instructions a scaled index (`base + scale * index`, scale above 1)
    /// costs beyond the address itself, once for each scaled register: the
    /// backend forms it once and every use of the register shares it. 0
    /// where the scale is part of the address form, as at the pin (DEVIATE
    /// LU1, `lsr_use.rs`).
    fn scaled_index_insns(&self) -> u32 {
        0
    }

    /// DEVIATE GEP-BYTE-INDEX (owner llrm-target-6502, exit 2026-11-30): the
    /// largest object offset a byte index reaches in one address instruction
    /// (the 6502's `abs,x`: 255). An `inbounds` GEP into a global no larger
    /// than this has a byte offset (`gepbyteindex`). None: the pass is off.
    fn address_index_max(&self) -> Option<u64> {
        None
    }

    /// The width of the narrow divide instruction, where an unsigned divide or
    /// remainder whose operands ScalarEvolution proves that narrow is done
    /// in the narrow type (`narrowrange`); none: the pass does not run.
    fn range_narrowing_width(&self) -> Option<u32> {
        None
    }

    /// Whether the loop as written keeps the bound of its exit test live next
    /// to the counter. LSR rates an exit test against a variable bound as one
    /// add-recurrence, `{-bound,+,1}` compared with zero (`getUse`, ICmpZero:
    /// `N - other`), so the baseline holds one register where the written
    /// loop holds two (`i` and `bound`) and any rewrite that keeps the bound
    /// in a register looks one register dearer; with `should_drop_lsr_
    /// solution_if_less_profitable` the tie then drops the rewrite. The rating
    /// of the baseline adds a register for it. DEVIATE LU20 (`lsr_use.rs`);
    /// false where a register is not the scarce thing.
    fn lsr_written_exit_bound_is_live(&self) -> bool {
        false
    }

    /// Multiplies the loop as written does to form an induction variable that
    /// steps by `step` (`bits` wide) and is not a phi of its own: `i * 54`
    /// where the loop only has `i`. LSR's baseline sees one add-recurrence
    /// either way; the pin's note on `AddRecCost` is "This should be a
    /// function of the stride" (LoopStrengthReduce.cpp, RateRegister). The
    /// rating of the baseline adds this to `num_iv_muls`. DEVIATE LU17
    /// (`lsr_use.rs`); 0 where multiplying is cheap.
    fn lsr_written_step_multiplies(
        &self,
        _step: i64,
        _bits: u32,
    ) -> u32 {
        0
    }

    /// DEVIATE LU24 (`lsr_solve.rs`): the search's required-register rule
    /// ("formulae that do not use a register the solution so far could
    /// share are ignored") is left off for address uses, as the pin does for
    /// post-indexed targets: the cost model weighs a pointer that steps
    /// against the index and base add it replaces.
    fn lsr_address_left_to_cost_model(&self) -> bool {
        false
    }

    /// DEVIATE LU23 (`lsr_use.rs`): in the baseline, an address `gep p, n` of
    /// a pointer in a register and a counter that is the only phi is a base
    /// add each trip (`num_base_adds`), not a recurrence of its own. False
    /// where the add folds into the access (the pin's x86).
    fn lsr_written_pointer_add_is_base_add(&self) -> bool {
        false
    }

    /// DEVIATE LU22 (`lsr_use.rs`): an addrec of another loop that is the
    /// extension of a phi of that loop (`zext i8 j`) is as free as the phi
    /// itself. Without it an outer loop's reduction (LSR-OUTER) rates every
    /// formula naming the inner loop's narrowed counter a loser.
    fn lsr_extension_of_phi_exists(&self) -> bool {
        false
    }

    /// Whether an offset LSR leaves unfolded in an address that has one base
    /// register and no scale is folded into that address after LSR (the 6502's
    /// `(zp),y` with a constant `y`): then it costs no add. DEVIATE LU16
    /// (`lsr_use.rs`); the pin counts it as a base add.
    fn unfolded_address_offsets_fold(&self) -> bool {
        false
    }

    /// Whether the target adds registers and an immediate in one instruction
    /// whatever the use (x86's `lea`): an unfolded offset beside two or more
    /// registers then costs no add of its own, for a value of `bits` bits.
    /// DEVIATE LU2 (`lsr_use.rs`).
    fn address_form_adds_offsets(
        &self,
        _bits: u32,
    ) -> bool {
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

    /// `getFPOpCost(ty) == TCC_Expensive`
    /// ([BasicTTIImpl.h:698](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L698)):
    /// no legal `FADD` on `ty`, so a float operation or cast on it is a library
    /// call. Default: legal (an FPU, or the x87).
    fn fp_op_expensive(
        &self,
        _ty: Ty,
    ) -> bool {
        false
    }

    /// `haveFastSqrt`: a legal `FSQRT` on `ty`
    /// ([BasicTTIImpl.h:674](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L674)).
    /// Default: none, for a target without an FPU or a sqrt instruction.
    fn have_fast_sqrt(
        &self,
        _ty: Ty,
    ) -> bool {
        false
    }

    /// `isFCmpOrdCheaperThanFCmpZero`
    /// ([TargetTransformInfoImpl.h:581](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfoImpl.h#L581)):
    /// whether `fcmp ord v, v` is cheaper than `fcmp oge v, 0.0` (X86: no).
    fn is_fcmp_ord_cheaper_than_fcmp_zero(
        &self,
        _ty: Ty,
    ) -> bool {
        true
    }

    /// `Tuning::partial_libcalls_skip_at_size`.
    fn partial_libcalls_skip_at_size(&self) -> bool {
        false
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

    /// DEVIATE (owner m68k, exit 2026-11-27): the code size, in the unit of the
    /// inliner's size view, of a switch's compares and jump table beyond its
    /// one branch; `clusters` and `table` as
    /// [`Self::estimated_number_of_case_clusters`]. The pin counts a switch
    /// as one control-flow instruction in `CodeSize`, so a callee with a
    /// 7-compare switch looked to SZ1 like a branch. 0: the branch alone.
    fn switch_code_size(
        &self,
        _clusters: u64,
        _table: u64,
    ) -> i64 {
        0
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

    /// `intrinsic_cost` of a `ctlz` or `cttz` whose `is_zero_poison` argument
    /// is the constant true: some targets price it apart
    /// (`X86TTIImpl::getIntrinsicInstrCost`, `CTLZ_ZERO_POISON`). The same
    /// price as `intrinsic_cost` where a target has no such rows.
    fn zero_poison_intrinsic_cost(
        &self,
        name: &str,
        ret: Option<Ty>,
        args: &[Ty],
        kind: CostKind,
    ) -> Cost {
        self.intrinsic_cost(name, ret, args, kind)
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

            fn int_imm_cost_inst_for_size(
                &self,
                op: ImmUser,
                index: u32,
                imm: i128,
                bits: u32,
                for_size: bool,
            ) -> Cost {
                self.0.int_imm_cost_inst_for_size(op, index, imm, bits, for_size)
            }

            fn int_imm_cost_inst_pin(
                &self,
                op: ImmUser,
                index: u32,
                imm: i128,
                bits: u32,
            ) -> Cost {
                self.0.int_imm_cost_inst_pin(op, index, imm, bits)
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

#[cfg(test)]
mod size_model_tests {
    use super::*;

    /// A table whose every number differs: a displacement read from the wrong
    /// field, or a literal left behind, shows as the wrong value.
    const DISTINCT: LsrSizeModel = LsrSizeModel {
        disp_global: OpCost::bytes(40),
        disp_frame: OpCost::bytes(20),
        disp_short: OpCost::bytes(10),
        disp_long: OpCost::bytes(70),
        base_add: OpCost::bytes(5),
        step: |_, _| 6,
        constant_start: None,
        setup: OpCost::bytes(7),
        setup_cap: 2,
        derived_setup_free: false,
        size_only: false,
        exit_compare: |bits, _| bits,
    };

    /// Each displacement form is read from its own field; zero is free.
    #[test]
    fn a_displacement_is_priced_by_its_form_from_the_table() {
        let m = DISTINCT;
        assert_eq!(m.displacement(123, true, false), 40);
        assert_eq!(m.displacement(0, false, true), 20);
        assert_eq!(m.displacement(0, false, false), 0);
        assert_eq!(m.displacement(-128, false, false), 10);
        assert_eq!(m.displacement(128, false, false), 70);
    }

    /// x86's table is the numbers `lsr_use.rs` held as literals (0/1/4, 2 for a
    /// frame slot, 3 per add, step and setup, at most 4 setups): moving them
    /// into the table changes no x86 object.
    #[test]
    fn the_x86_table_is_the_numbers_lsr_held() {
        let m = LsrSizeModel::X86;
        assert_eq!((m.disp_global.bytes, m.disp_frame.bytes, m.disp_short.bytes, m.disp_long.bytes), (4, 2, 1, 4));
        assert_eq!((m.base_add.bytes, (m.step)(32, Some(1)), m.setup.bytes, m.setup_cap), (3, 3, 3, 4));
    }
}
