//! AArch64's scalar cost model: `AArch64TTIImpl` and the `BasicTTIImplBase`
//! rules it falls to, translated from LLVM d1106deb71cc for the scalar integer,
//! float and pointer types, for the cpus `generic` and `apple-m1`.
//!
//! # How it works
//! A type is first legalized (`getTypeLegalizationCost`): an `i8` is promoted
//! to `i32`, an `i128` is two `i64`. The operation is then priced by
//! `AArch64TTIImpl`'s rules and, where they fall through, by `BasicTTIImplBase`
//! and `TargetTransformInfoImplBase`. Every row of `tti_oracle.txt` and
//! `tti_intrinsics.txt` (made by `tti_oracle.py` from the pinned `opt
//! -passes='print<cost-model>'`) is checked against these functions, on each
//! cpu and cost kind.
//!
//! # Deviations from LLVM
//! - DEVIATE 1: intrinsic prices are read from `tti_intrinsics.txt`, the pinned
//!   `opt`'s answers, as the x86 model's are; the scalar rules of
//!   `getIntrinsicInstrCost` are not translated.
//! - DEVIATE 2: the load latency of `apple-m1` is a load of latency 4 and
//!   reciprocal throughput 1/2 (what the Cyclone scheduling model gives `LDR`);
//!   the scheduling model is not read. `generic` has the formula's fixed
//!   latency 4 (`FeatureFixedLoadLatency4`).
//! - DEVIATE 3: `isLegalAddressingMode` takes the access type; the trait does
//!   not, so the access is 8 bytes, a register
//!   ([`AArch64::is_legal_addressing_mode_for`] takes the size).
//! - DEVIATE 4: `isMaskAndCmp0FoldingBeneficial` takes the mask; the trait does
//!   not, so the answer is the base class's `false`
//!   ([`AArch64Lowering::is_mask_and_cmp0_folding_beneficial_for`] takes the
//!   mask).
//! - DEVIATE 5: the compare takes no predicate, so the two compares of an `fcmp
//!   one`/`ueq` are not seen.
//! - DEVIATE 6: no instruction context (T4): an extension absorbed by a
//!   widening user, `isExtFree`, a `fneg` beside an `fmul`, an `icmp` of an
//!   `and` with zero are not seen free; an extending load is, by
//!   `cast_of_load_cost`.
//! - DEVIATE 7: the unrolling preferences that read the loop (nesting depth,
//!   exit blocks, Apple's runtime heuristics, the multi-exit search loops) are
//!   not reached through the trait.
//! - DEVIATE 8: `x86_fp80` has no AArch64 type: invalid. Vectors, half and
//!   `fp128` are not modelled (T1).
//!
//! # Example
//! `sdiv i32 %a, 8` costs `3 * add + ashr = 4` on both cpus.
//!
//! From unit test `the_arithmetic_costs_equal_the_pinned_opt_on_both_cpus`.
//!
//! # Runs
//! Read by InlineCost, the SCEV expander and loop strength reduction; nothing
//! runs it as a pass.
//!
//! # See also
//! `llrm_mir::tti`, the interface; `llrm_x86::tti`, the x86 model.

use llrm_mir::lowering::Lowering;
use llrm_mir::tti::*;

use crate::expand_imm::mov_imm;
use crate::mc::immediate;
use crate::mc::inst::Gpr;

/// The cpus whose answers differ.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cpu {
    Generic,
    AppleM1,
}

impl Cpu {
    fn name(self) -> &'static str {
        match self {
            Cpu::Generic => "generic",
            Cpu::AppleM1 => "apple-m1",
        }
    }
}

/// AArch64's scalar profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AArch64 {
    pub cpu: Cpu,
}

impl AArch64 {
    /// The profile of a `-mcpu` llrm supports.
    pub fn of_cpu(cpu: &str) -> Option<AArch64> {
        match cpu {
            "generic" => Some(AArch64 { cpu: Cpu::Generic }),
            "apple-m1" => Some(AArch64 { cpu: Cpu::AppleM1 }),
            _ => None,
        }
    }

    /// `AArch64TargetLowering::isLegalAddressingMode` for an access of
    /// `num_bytes` (0: not a power of two):
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L20254>
    pub fn is_legal_addressing_mode_for(
        num_bytes: u64,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        // No global is ever allowed as a base.
        if base_gv {
            return false;
        }
        // No reg+reg+imm addressing.
        if has_base_reg && base_offset != 0 && scale != 0 {
            return false;
        }
        // Canonicalise `1*ScaledReg + imm` into `BaseReg + imm` and
        // `2*ScaledReg` into `BaseReg + ScaledReg`.
        let (mut has_base_reg, mut scale) = (has_base_reg, scale);
        if scale != 0 && !has_base_reg {
            match scale {
                1 => (has_base_reg, scale) = (true, 0),
                2 => (has_base_reg, scale) = (true, 1),
                _ => return false,
            }
        }
        // A base register is required in all addressing modes.
        has_base_reg && instruction_addressing_mode_is_legal(num_bytes, base_offset, scale)
    }
}

/// `AArch64InstrInfo::isLegalAddressingMode`:
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64InstrInfo.cpp#L11809>
fn instruction_addressing_mode_is_legal(
    num_bytes: u64,
    offset: i64,
    scale: i64,
) -> bool {
    // The parameter is an `unsigned`.
    let scale = scale as u32;
    if offset != 0 && scale != 0 {
        return false;
    }
    if scale == 0 {
        // 9-bit signed offset.
        if (-256..256).contains(&offset) {
            return true;
        }
        // 12-bit unsigned offset, a multiple of the access size.
        return num_bytes != 0
            && offset > 0
            && offset as u64 / num_bytes <= (1 << 12) - 1
            && offset as u64 % num_bytes == 0;
    }
    // reg1 + SIZE_IN_BYTES * reg2 and reg1 + reg2.
    scale == 1 || u64::from(scale) == num_bytes
}

/// The access size the trait's addressing-mode queries assume (DEVIATE 3).
const ACCESS_BYTES: u64 = 8;

/// A type after `getTypeLegalizationCost`: the number of parts (`first`) and
/// the width and class of one (`second`).
#[derive(Clone, Copy)]
struct Legal {
    count: i64,
    bits: u32,
    float: bool,
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1047
/// Integers narrower than 32 bits are promoted to `i32`, wider than 64 are
/// expanded to halves until `i64`.
fn legalize(ty: Ty) -> Option<Legal> {
    let int = |bits| Some(Legal { count: 1, bits, float: false });
    match ty {
        Ty::Int(bits) if bits <= 32 => int(32),
        Ty::Int(bits) if bits <= 64 => int(64),
        Ty::Int(bits) => {
            Some(Legal { count: i64::from(bits.div_ceil(64).next_power_of_two()), bits: 64, float: false })
        }
        Ty::Pointer => int(64),
        Ty::Float => Some(Legal { count: 1, bits: 32, float: true }),
        Ty::Double => Some(Legal { count: 1, bits: 64, float: true }),
        Ty::X87 => None,
    }
}

/// `DL.isLegalInteger`: the data layout's `n32:64`.
fn is_legal_integer(bits: u32) -> bool {
    bits == 32 || bits == 64
}

fn bits_of(ty: Ty) -> u32 {
    match ty {
        Ty::Int(bits) => bits,
        Ty::Float => 32,
        Ty::Double | Ty::Pointer => 64,
        Ty::X87 => 80,
    }
}

/// `TargetTransformInfoImplBase::getArithmeticInstrCost`:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfoImpl.h#L733
fn impl_base_arithmetic_cost(
    op: ArithOp,
    ty: Ty,
    kind: CostKind,
) -> Cost {
    use ArithOp::*;
    // FIXME: Unlikely to be true for CodeSize.
    if matches!(op, FDiv | FRem | SDiv | SRem | UDiv | URem) {
        return Cost::EXPENSIVE;
    }
    // Assume a 3cy latency for fp arithmetic ops.
    if kind == CostKind::Latency && ty.is_float() {
        return Cost::of(3);
    }
    Cost::BASIC
}

/// `TargetTransformInfoImplBase::getCastInstrCost`:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfoImpl.h#L789
fn impl_base_cast_cost(
    op: CastKind,
    dst: Ty,
    src: Ty,
) -> Cost {
    let free = match op {
        CastKind::IntToPtr => is_legal_integer(bits_of(src)) && bits_of(src) <= 64,
        CastKind::PtrToInt => is_legal_integer(bits_of(dst)) && bits_of(dst) >= 64,
        CastKind::BitCast => dst == src,
        // trunc to a native type is free (assuming the target has compare and
        // shift-right of the same width).
        CastKind::Trunc => is_legal_integer(bits_of(dst)),
        _ => false,
    };
    if free { Cost::FREE } else { Cost::BASIC }
}

/// The chunk of `getIntImmCost(int64_t)`:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L390
fn int_imm_chunk(value: i64) -> i64 {
    // Check if the immediate can be encoded within an instruction.
    if value == 0 || immediate::logical(value as u64, 64).is_some() {
        return 0;
    }
    let value = if value < 0 { !value } else { value };
    // How many moves we will need to materialize this constant.
    mov_imm(value as u64, 64, Gpr(Gpr::FIRST_VIRTUAL)).len() as i64
}

/// `isLegalArithImmed`:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/MCTargetDesc/AArch64AddressingModes.h#L355
fn is_legal_arith_immediate(c: u64) -> bool {
    c >> 12 == 0 || (c & 0xfff == 0 && c >> 24 == 0)
}

/// The cost-model name of a cost kind in the oracle's tables.
fn kind_name(kind: CostKind) -> &'static str {
    match kind {
        CostKind::RecipThroughput => "throughput",
        CostKind::Latency => "latency",
        CostKind::CodeSize => "code-size",
        CostKind::SizeAndLatency => "size-latency",
    }
}

type Prices = llrm_support::hash::HashMap<(&'static str, &'static str, &'static str), Option<i64>>;

/// Intrinsic prices by (cpu, kind, callee name), as the pinned `opt` gives them
/// (DEVIATE 1).
fn intrinsic_prices() -> &'static Prices {
    static PRICES: std::sync::OnceLock<Prices> = std::sync::OnceLock::new();
    PRICES.get_or_init(|| {
        include_str!("tti_intrinsics.txt")
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| {
                let mut words = line.split(' ');
                let (cpu, kind, name, cost) = (words.next()?, words.next()?, words.next()?, words.next()?);
                Some(((cpu, kind, name), cost.parse().ok()))
            })
            .collect()
    })
}

impl Tti for AArch64 {
    fn name(&self) -> &'static str {
        "AArch64"
    }

    /// A `sext`/`zext` of a load is the extending load (`ldrsb`, `ldrh`, ...)
    /// when the loaded type is a byte, half or word and both types are one
    /// register (`BasicTTIImplBase::getCastInstrCost`, BasicTTIImpl.h:1297).
    fn cast_of_load_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        _kind: CostKind,
    ) -> Option<Cost> {
        self.ext_load_free(op, dst, src).then_some(Cost::FREE)
    }

    fn ext_load_free(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
    ) -> bool {
        matches!(op, CastKind::SExt | CastKind::ZExt)
            && matches!(src, Ty::Int(8 | 16 | 32))
            && matches!(dst, Ty::Int(bits) if bits > bits_of(src))
            && matches!(
                (legalize(src), legalize(dst)),
                (Some(a), Some(b)) if a.count == b.count
            )
    }

    fn is_type_legal(
        &self,
        ty: Ty,
    ) -> bool {
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp
        // W and X registers hold i32 and i64 (narrower integers are promoted);
        // there is no x87.
        matches!(ty, Ty::Int(32 | 64) | Ty::Pointer | Ty::Float | Ty::Double)
    }

    fn intrinsic_cost(
        &self,
        name: &str,
        ret: Option<Ty>,
        args: &[Ty],
        kind: CostKind,
    ) -> Cost {
        let _ = (ret, args);
        match intrinsic_prices().get(&(self.cpu.name(), kind_name(kind), name)) {
            Some(Some(cost)) => Cost::of(*cost),
            Some(None) => Cost::INVALID,
            None if is_free_intrinsic(name) => Cost::FREE,
            // A scalar intrinsic with no case of its own (`stacksave`) is
            // "turned into a library call":
            // BasicTTIImplBase::getTypeBasedIntrinsicInstrCost's
            // `SingleCallCost`.
            // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L2323
            None => base_intrinsic_cost(name).unwrap_or(Cost::of(if kind == CostKind::CodeSize { 1 } else { 10 })),
        }
    }

    /// `AArch64TTIImpl::getUnrollingPreferences`, as far as the trait reaches
    /// (DEVIATE 7). Neither cpu's scheduling model has a loop buffer, so
    /// `BasicTTIImplBase`'s part sets nothing (the default of this trait);
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L6225
    fn get_unrolling_preferences(
        &self,
        has_calls: bool,
        up: &mut UnrollingPreferences,
    ) {
        // Enable partial unrolling and runtime unrolling.
        let max_ops = self.loop_micro_op_buffer_size();
        if max_ops != 0 && !has_calls {
            up.partial = true;
            up.runtime = true;
            up.partial_threshold = max_ops;
            up.opt_size_threshold = 0;
            up.be_insns = 2;
        }
        up.upper_bound = true;
        // Disable partial & runtime unrolling on -Os.
        up.partial_opt_size_threshold = 0;
    }

    /// `AArch64TTIImpl::getCmpSelInstrCost`, scalars:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L5363
    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost {
        let Some(lt) = legalize(ty) else { return Cost::INVALID };
        if op == CmpSel::FCmp {
            // Some comparison operators require expanding to multiple compares
            // + or (DEVIATE 5): factor 1.
            return Cost::of(if kind == CostKind::Latency { 2 } else { lt.count });
        }
        // BasicTTIImplBase: the operation is legal, one per legalized part:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1454
        Cost::of(lt.count)
    }

    /// `AArch64TTIImpl::getMemoryOpCost`, scalars:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L5664
    fn memory_op_cost(
        &self,
        op: MemoryOp,
        ty: Ty,
        kind: CostKind,
        stored_constant: bool,
    ) -> Cost {
        let _ = stored_constant;
        let Some(lt) = legalize(ty) else { return Cost::INVALID };
        // TODO: consider latency as well for TCK_SizeAndLatency.
        if matches!(kind, CostKind::CodeSize | CostKind::SizeAndLatency) {
            return Cost::of(lt.count);
        }
        if kind == CostKind::Latency {
            // Latency doesn't make much sense for stores, so just return 1.
            if op == MemoryOp::Store {
                return Cost::BASIC;
            }
            // The latency of the last of LT.first loads, and the time it gets
            // to get there (DEVIATE 2).
            return Cost::of(match self.cpu {
                Cpu::Generic => lt.count - 1 + 4,
                Cpu::AppleM1 => (lt.count - 1) / 2 + 4,
            });
        }
        Cost::of(lt.count)
    }

    /// `AArch64TTIImpl::getCFInstrCost`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L4636
    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: CostKind,
    ) -> Cost {
        if kind != CostKind::RecipThroughput {
            return if phi { Cost::FREE } else { Cost::BASIC };
        }
        // Branches are assumed to be predicted.
        Cost::FREE
    }

    /// `AArch64TTIImpl::getIntImmCost(const APInt &, Type *)`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L406
    fn int_imm_cost(
        &self,
        imm: i128,
        bits: u32,
    ) -> Cost {
        if bits == 0 {
            return Cost::of(i64::from(u32::MAX));
        }
        // Sign-extend all constants to a multiple of 64-bit, and cost each
        // 64-bit chunk.
        let extended = if bits >= 128 { imm } else { (imm << (128 - bits)) >> (128 - bits) };
        let total: i64 = (0..bits).step_by(64).map(|shift| int_imm_chunk((extended >> shift.min(127)) as i64)).sum();
        // We need at least one instruction to materialze the constant.
        Cost::of(total.max(1))
    }

    /// `AArch64TTIImpl::getIntImmCostInst`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L431
    fn int_imm_cost_inst(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost {
        // There is no cost model for constants with a bit size of 0.
        if bits == 0 {
            return Cost::FREE;
        }
        let immediate = match op {
            ImmUser::Unknown => return Cost::FREE,
            // Always hoist the base address of a GetElementPtr.
            ImmUser::GetElementPtr => return if index == 0 { Cost::of(2) } else { Cost::FREE },
            ImmUser::Store => Some(0),
            ImmUser::Add
            | ImmUser::Sub
            | ImmUser::Mul
            | ImmUser::Divide
            | ImmUser::And
            | ImmUser::Or
            | ImmUser::Xor
            | ImmUser::ICmp => Some(1),
            // Always return TCC_Free for the shift value of a shift
            // instruction.
            ImmUser::Shift => {
                if index == 1 {
                    return Cost::FREE;
                }
                None
            }
            ImmUser::Other => None,
        };
        let cost = self.int_imm_cost(imm, bits);
        if immediate == Some(index) {
            let constants = i64::from(bits.div_ceil(64));
            return if cost.value <= constants { Cost::FREE } else { cost };
        }
        cost
    }

    /// `AArch64TargetLowering::isTruncateFree`: truncations from a 64-bit GPR
    /// to a 32-bit one are free:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L18942
    fn is_truncate_free(
        &self,
        from: Ty,
        to: Ty,
    ) -> bool {
        matches!((from, to), (Ty::Int(a), Ty::Int(b)) if a > b)
    }

    fn is_legal_addressing_mode(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        AArch64::is_legal_addressing_mode_for(ACCESS_BYTES, base_gv, base_offset, has_base_reg, scale)
    }

    fn is_legal_addressing_mode_for(
        &self,
        access_bytes: u64,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        AArch64::is_legal_addressing_mode_for(access_bytes, base_gv, base_offset, has_base_reg, scale)
    }

    /// `isLegalAddImmediate`: 12-bit optionally shifted immediates, the same
    /// encoding for add and sub:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L20177
    fn is_legal_add_immediate(
        &self,
        imm: i64,
    ) -> bool {
        imm != i64::MIN && is_legal_arith_immediate(imm.unsigned_abs())
    }

    /// Integer comparisons are implemented with ADDS/SUBS:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L20248
    fn is_legal_icmp_immediate(
        &self,
        imm: i64,
    ) -> bool {
        self.is_legal_add_immediate(imm)
    }

    /// `AArch64TTIImpl::getScalingFactorCost`: "scale represents reg2 * scale,
    /// thus account for 1 if it is not equal to 0 or 1":
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L7420
    fn scaling_factor_cost(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> Cost {
        if self.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale) {
            Cost::of(i64::from(scale != 0 && scale != 1))
        } else {
            Cost::INVALID
        }
    }

    /// `AArch64TTIImpl::isLSRCostLess`: the number of instructions is added to
    /// the comparison, though not first, and the base additions come before
    /// the add-recurrence cost:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L7461
    fn is_lsr_cost_less(
        &self,
        a: &LsrCost,
        b: &LsrCost,
    ) -> bool {
        let key = |c: &LsrCost| {
            (
                c.num_regs,
                c.insns,
                c.num_base_adds,
                c.add_rec_cost,
                c.num_iv_muls,
                c.scale_cost,
                c.imm_cost,
                c.setup_cost,
            )
        };
        key(a) < key(b)
    }

    /// The indexed load and store actions of the `AArch64TargetLowering`
    /// constructor: every mode, for the register types (`i8`..`i64`, `f32`,
    /// `f64`; a pointer is an `i64`):
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L1157
    fn is_indexed_access_legal(
        &self,
        mode: MemIndexedMode,
        ty: Ty,
    ) -> bool {
        let _ = mode;
        matches!(
            ty,
            Ty::Int(8 | 16 | 32 | 64) | Ty::Float | Ty::Double | Ty::Pointer
        )
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.h#L126
    fn number_of_gpr_registers(&self) -> u32 {
        31
    }

    /// `AArch64TTIImpl::getCastInstrCost`, scalars (DEVIATE 6):
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L3886
    fn cast_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Cost {
        let _ = kind;
        let (Some(src_lt), Some(dst_lt)) = (legalize(src), legalize(dst)) else { return Cost::INVALID };
        // ConversionTbl:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L4112
        if matches!(
            (op, dst, src),
            (CastKind::FPExt, Ty::Double, Ty::Float) | (CastKind::FPTrunc, Ty::Float, Ty::Double)
        ) {
            return Cost::BASIC;
        }
        // BasicTTIImplBase::getCastInstrCost:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1257
        if impl_base_cast_cost(op, dst, src) == Cost::FREE {
            return Cost::FREE;
        }
        let int_or_ptr = |ty: Ty| matches!(ty, Ty::Int(_) | Ty::Pointer);
        let truncate_free = !src_lt.float && !dst_lt.float && src_lt.bits > dst_lt.bits;
        // Bitcast between types that are legalized to the same type are free,
        // and so is the trunc that is one.
        let same_type =
            src_lt.count == dst_lt.count && int_or_ptr(src) == int_or_ptr(dst) && src_lt.bits == dst_lt.bits;
        match op {
            CastKind::Trunc if truncate_free || same_type => return Cost::FREE,
            CastKind::BitCast if same_type => return Cost::FREE,
            // All 32-bit GPR operations implicitly zero the high-half of the
            // 64-bit GPR:
            // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L18991
            CastKind::ZExt if !src_lt.float && !dst_lt.float && src_lt.bits == 32 && dst_lt.bits == 64 => {
                return Cost::FREE;
            }
            _ => {}
        }
        // Every scalar conversion between legal types is a legal or custom
        // operation: one instruction per part (the part count when both
        // sides split alike).
        if src_lt.count == dst_lt.count { Cost::of(src_lt.count) } else { Cost::BASIC }
    }

    /// `AArch64TTIImpl::getArithmeticInstrCost`, scalars (DEVIATE 6):
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L4926
    fn arithmetic_cost(
        &self,
        op: ArithOp,
        ty: Ty,
        kind: CostKind,
        first: OperandInfo,
        second: OperandInfo,
    ) -> Cost {
        use ArithOp::*;
        let Some(lt) = legalize(ty) else { return Cost::INVALID };
        let none = OperandInfo::ANY;
        let count = Cost::of(lt.count);
        // TODO: Handle more cost kinds for floating point operations.
        if matches!(op, FAdd | FSub | FMul | FDiv | FRem | FNeg) && kind != CostKind::RecipThroughput {
            return impl_base_arithmetic_cost(op, ty, kind);
        }
        match op {
            Add | Sub | And | Or | Xor | Shl | LShr | AShr => count,
            Mul => {
                // i128 multiply is umulh + 2*madd + mul and grows
                // ~O(Bitwidth^2).
                if bits_of(ty) > 64 { count * lt.count } else { count }
            }
            SDiv | SRem if second.constant && matches!(ty, Ty::Int(bits) if bits <= 64) => {
                let add = self.arithmetic_cost(Add, ty, kind, none, none);
                let asr = self.arithmetic_cost(AShr, ty, kind, none, none);
                let mul = self.arithmetic_cost(Mul, ty, kind, none, none);
                if second.power_of_two || second.negated_power_of_two {
                    if op == SDiv { add * 3 + asr } else { asr * 3 + add }
                } else {
                    mul + asr + add * 2
                }
            }
            SDiv | SRem | UDiv | URem => {
                if second.constant {
                    if op == UDiv && second.power_of_two {
                        return self.arithmetic_cost(LShr, ty, kind, none, none);
                    }
                    if op == URem && second.power_of_two {
                        return self.arithmetic_cost(And, ty, kind, none, none);
                    }
                    if matches!(op, UDiv | URem) {
                        let has_mulh = ty == Ty::Int(64);
                        let mul = self.arithmetic_cost(Mul, ty, kind, none, none);
                        let add = self.arithmetic_cost(Add, ty, kind, none, none);
                        let shr = self.arithmetic_cost(AShr, ty, kind, none, none);
                        // UMULL/UMULH, the UMULL shift, and the rest.
                        let umull_shift = if has_mulh { Cost::FREE } else { shr };
                        let div = mul + umull_shift + add * 2 + shr;
                        return if op == URem { div + mul + add } else { div };
                    }
                }
                if bits_of(ty) > 64 {
                    // getCallInstrCost: a library call.
                    return Cost::of(10);
                }
                self.base_divide_cost(op, ty, kind, first, second)
            }
            FNeg | FAdd | FSub => count,
            FMul | FDiv => count * 2,
            // getCallInstrCost: a library call.
            FRem => Cost::of(10),
        }
    }
}

impl AArch64 {
    /// `BasicTTIImplBase::getArithmeticInstrCost` for the divisions of a type
    /// of at most 64 bits:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1092
    fn base_divide_cost(
        &self,
        op: ArithOp,
        ty: Ty,
        kind: CostKind,
        first: OperandInfo,
        second: OperandInfo,
    ) -> Cost {
        use ArithOp::*;
        // TODO: Handle more cost kinds.
        if kind != CostKind::RecipThroughput {
            return impl_base_arithmetic_cost(op, ty, kind);
        }
        match op {
            // The operation is legal (or promoted). Assume it costs 1.
            SDiv | UDiv => Cost::of(legalize(ty).map_or(1, |lt| lt.count)),
            // An 'Expand' of URem and SRem expands into X % Y -> X-(X/Y)*Y.
            _ => {
                let divide = if op == SRem { SDiv } else { UDiv };
                let none = OperandInfo::ANY;
                self.arithmetic_cost(divide, ty, kind, first, second)
                    + self.arithmetic_cost(Mul, ty, kind, none, none)
                    + self.arithmetic_cost(Sub, ty, kind, none, none)
            }
        }
    }
}

/// AArch64's `TargetLowering` facts.
pub struct AArch64Lowering;

impl AArch64Lowering {
    /// `AArch64TargetLowering::isMaskAndCmp0FoldingBeneficial`: only sink an
    /// `and` that masks a single bit, as it is likely to fold the
    /// and/cmp/br into one `tbz`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L33646
    pub fn is_mask_and_cmp0_folding_beneficial_for(mask: Option<u128>) -> bool {
        mask.is_some_and(u128::is_power_of_two)
    }
}

impl Lowering for AArch64Lowering {}
