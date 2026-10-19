//! X86's scalar cost model: `X86TTIImpl` and the `BasicTTIImplBase` rules it
//! falls to, translated from LLVM d1106deb71cc for the scalar types.
//!
//! # How it works
//! A type is first legalized (`getTypeLegalizationCost`): an `i64` on a 32-bit
//! target is two `i32` operations. The operation is then looked up in LLVM's
//! scalar tables (`ScalarVarDivCostTable`, `X86CostTbl`, the SSE tables when
//! the CPU has SSE) before the generic rules answer. Every row of
//! `tti_oracle.txt` (made by `tti_oracle.py` from the pinned `opt
//! -passes='print<cost-model>'`) is checked against these functions, on each
//! supported CPU and cost kind.
//!
//! # Deviations from LLVM
//! - T3: the SSE tables are read for `core2` only, the one supported CPU with
//!   SSE; llrm's back end emits x87 for scalar floats whatever the CPU says.
//! - T4: no instruction context, so a `zext` folded into a GEP index and an
//!   extending load are not seen free.
//! - T5: the constant costs (`int_imm_cost*`) and `is_legal_addressing_mode`
//!   are read off the source; the cost model has no printer for them.
//!
//! # Example
//! `sdiv i32 %a, 8` costs `2 * ashr + lshr + add = 4` and `sdiv i32 %a, %b`
//! costs 25 on every x87 CPU (the `ScalarVarDivCostTable` entry).
//!
//! From unit test `the_arithmetic_costs_equal_the_pinned_opt_on_every_cpu`.
//!
//! # Runs
//! Read by InlineCost, the SCEV expander and loop strength reduction; nothing
//! runs it as a pass.
//!
//! # See also
//! `llrm_mir::tti`, the interface; the plan page `tti-plan` in
//! docs/architecture.

use llrm_mir::tti::*;

/// X86's scalar profile: which tables its CPU reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct X86 {
    pub sse1: bool,
    pub sse2: bool,
}

impl X86 {
    /// The profile of a `-march` llrm supports.
    pub fn of_cpu(cpu: &str) -> Option<X86> {
        match cpu {
            "i386" | "i486" | "pentium" | "pentiumpro" | "k5" | "k6" | "athlon" => {
                Some(X86 { sse1: false, sse2: false })
            }
            "core2" => Some(X86 { sse1: true, sse2: true }),
            _ => None,
        }
    }
}

/// `getTypeLegalizationCost` for the scalar types on a 32-bit target: how many
/// operations of what type a value takes:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1039
fn legalized(ty: Ty) -> Option<(i64, Ty)> {
    match ty {
        Ty::Int(bits) if bits <= 8 => Some((1, Ty::Int(8))),
        Ty::Int(16) => Some((1, Ty::Int(16))),
        Ty::Int(bits) if bits <= 32 => Some((1, Ty::Int(32))),
        Ty::Int(bits) => Some((i64::from(bits.div_ceil(32).next_power_of_two()), Ty::Int(32))),
        Ty::Pointer => Some((1, Ty::Int(32))),
        other => Some((1, other)),
    }
}

/// `CostKindTblEntry`: the four costs of a table row, absent where the row
/// states one only (`{ 1 }` is throughput alone).
type Row = [Option<i64>; 4];

const fn all(
    throughput: i64,
    latency: i64,
    size: i64,
    size_latency: i64,
) -> Row {
    [Some(throughput), Some(latency), Some(size), Some(size_latency)]
}

const fn throughput(value: i64) -> Row {
    [Some(value), None, None, None]
}

/// `ScalarVarDivCostTable`:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L757
fn scalar_variable_divide(
    op: ArithOp,
    ty: Ty,
) -> Option<Row> {
    if !matches!(
        op,
        ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem
    ) {
        return None;
    }
    match ty {
        Ty::Int(8) => Some(all(15, 20, 2, 4)),
        Ty::Int(16) => Some(all(17, 20, 2, 4)),
        Ty::Int(32) => Some(all(25, 22, 2, 4)),
        Ty::Int(64) => Some(all(41, 24, 2, 4)),
        _ => None,
    }
}

/// `X86CostTbl`, the 32-bit rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1879
fn x86_table(
    op: ArithOp,
    ty: Ty,
) -> Option<Row> {
    use ArithOp::*;
    match (op, ty) {
        (Add | Sub, Ty::Int(8 | 16 | 32)) => Some(throughput(1)),
        (Mul, Ty::Int(8)) => Some(all(3, 4, 1, 1)),
        (Mul, Ty::Int(16)) => Some(all(2, 4, 1, 1)),
        (Mul, Ty::Int(32)) => Some(all(1, 4, 1, 1)),
        (FNeg, Ty::Double) => Some(all(2, 2, 1, 3)),
        (FAdd | FSub, Ty::Double) => Some(all(2, 3, 1, 1)),
        (FMul, Ty::Double) => Some(all(2, 5, 1, 1)),
        (FDiv, Ty::Double) => Some(all(38, 38, 1, 1)),
        _ => None,
    }
}

/// `SSE2CostTable`, the scalar rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1776
fn sse2_table(
    op: ArithOp,
    ty: Ty,
) -> Option<Row> {
    use ArithOp::*;
    match (op, ty) {
        (FDiv, Ty::Float) => Some(all(23, 23, 1, 1)),
        (FDiv, Ty::Double) => Some(all(38, 38, 1, 1)),
        (FNeg, Ty::Float | Ty::Double) => Some(all(1, 1, 1, 1)),
        (FAdd | FSub, Ty::Float | Ty::Double) => Some(all(2, 3, 1, 1)),
        (FMul, Ty::Double) => Some(all(2, 5, 1, 1)),
        _ => None,
    }
}

/// `SSE1CostTable`, the scalar rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1846
fn sse1_table(
    op: ArithOp,
    ty: Ty,
) -> Option<Row> {
    use ArithOp::*;
    match (op, ty) {
        (FDiv, Ty::Float) => Some(all(17, 18, 1, 1)),
        (FNeg, Ty::Float) => Some(all(2, 2, 1, 2)),
        (FAdd | FSub, Ty::Float) => Some(all(1, 3, 1, 1)),
        (FMul, Ty::Float) => Some(all(2, 5, 1, 1)),
        _ => None,
    }
}

/// `TargetTransformInfoImplBase::getArithmeticInstrCost`, which the generic
/// rules fall to for every kind but throughput:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfoImpl.h#L733
fn base_arithmetic(
    op: ArithOp,
    ty: Ty,
    kind: CostKind,
) -> Cost {
    use ArithOp::*;
    if matches!(op, FDiv | FRem | SDiv | SRem | UDiv | URem) {
        return Cost::EXPENSIVE;
    }
    if kind == CostKind::Latency && ty.is_float() {
        return Cost::of(3);
    }
    Cost::BASIC
}

/// `BasicTTIImplBase::getArithmeticInstrCost`:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1092
fn generic_arithmetic(
    op: ArithOp,
    ty: Ty,
    kind: CostKind,
    first: OperandInfo,
    second: OperandInfo,
    profile: &X86,
) -> Cost {
    use ArithOp::*;
    if kind != CostKind::RecipThroughput {
        return base_arithmetic(op, ty, kind);
    }
    let Some((count, _)) = legalized(ty) else { return Cost::INVALID };
    let unit = if ty.is_float() { 2 } else { 1 };
    // X86's operation actions on the scalar types: the divisions and
    // remainders expand (onto DIVREM, which is custom), `frem` is a libcall,
    // the rest are legal.
    match op {
        SDiv | UDiv => Cost::of(unit),
        SRem | URem => {
            let divide = if op == SRem { SDiv } else { UDiv };
            profile.arithmetic_cost(divide, ty, kind, first, second)
                + profile.arithmetic_cost(Mul, ty, kind, OperandInfo::ANY, OperandInfo::ANY)
                + profile.arithmetic_cost(Sub, ty, kind, OperandInfo::ANY, OperandInfo::ANY)
        }
        FRem => Cost::of(count * 2 * unit),
        _ => Cost::of(count * unit),
    }
}

/// `SSE2ConversionTbl`, the scalar rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L3526
fn sse2_conversion(
    op: CastKind,
    dst: Ty,
    src: Ty,
) -> Option<Row> {
    use CastKind::*;
    let narrow = |ty: Ty| matches!(ty, Ty::Int(32) | Ty::Int(64) | Ty::Float | Ty::Double);
    if !narrow(dst) || !narrow(src) {
        return None;
    }
    let cost = match (op, dst, src) {
        (SIToFP, Ty::Float | Ty::Double, Ty::Int(32 | 64)) => 3,
        (UIToFP, Ty::Float | Ty::Double, Ty::Int(32)) => 3,
        (UIToFP, Ty::Float, Ty::Int(64)) => 8,
        (UIToFP, Ty::Double, Ty::Int(64)) => 9,
        (FPToSI, Ty::Int(32 | 64), Ty::Float | Ty::Double) => 4,
        (FPToUI, Ty::Int(32), Ty::Float | Ty::Double) => 4,
        (FPToUI, Ty::Int(64), Ty::Float) => 4,
        (FPToUI, Ty::Int(64), Ty::Double) => 15,
        _ => return None,
    };
    Some(all(cost, 1, 1, 1))
}

/// Whether `BITCAST` to `ty` is `Expand` in the lowering (X86ISelLowering): on
/// the x87 targets everything but `f64` is, with SSE2 nothing.
fn bitcast_expands(
    profile: &X86,
    ty: Ty,
) -> bool {
    !profile.sse2 && !matches!(ty, Ty::Double)
}

/// `BasicTTIImplBase::getCastInstrCost` for scalars:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1257
/// (the free ones, first, are `TargetTransformInfoImplBase`'s:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfoImpl.h#L789).
fn generic_cast(
    profile: &X86,
    op: CastKind,
    dst: Ty,
    src: Ty,
    kind: CostKind,
) -> Cost {
    use CastKind::*;
    let adjust = |cost: Cost| {
        if kind == CostKind::RecipThroughput {
            cost
        } else if cost.value == 0 {
            cost
        } else {
            Cost::BASIC
        }
    };
    // TargetTransformInfoImplBase::getCastInstrCost: the free ones.
    let free = match op {
        IntToPtr => matches!(src, Ty::Int(8 | 16 | 32)),
        PtrToInt => matches!(dst, Ty::Int(32 | 64)) && matches!(dst, Ty::Int(32)),
        BitCast => dst == src,
        Trunc => matches!(dst, Ty::Int(8 | 16 | 32)),
        _ => false,
    };
    if free {
        return Cost::FREE;
    }
    let (Some((source_count, source)), Some((dest_count, dest))) = (legalized(src), legalized(dst)) else {
        return Cost::INVALID;
    };
    let is_integer = |ty: Ty| matches!(ty, Ty::Int(_) | Ty::Pointer);
    let bits = |ty: Ty| match ty {
        Ty::Int(bits) => bits,
        Ty::Pointer | Ty::Float => 32,
        Ty::Double => 64,
        Ty::X87 => 80,
    };
    match op {
        Trunc if bits(source) > bits(dest) && is_integer(source) && is_integer(dest) => return Cost::FREE,
        Trunc | BitCast
            if source_count == dest_count && is_integer(src) == is_integer(dst) && bits(source) == bits(dest) =>
        {
            return Cost::FREE;
        }
        _ => {}
    }
    // Legal or promoted at one count: the count; else a scalar conversion
    // costs 1 unless its operation expands, then 4.
    let expands = matches!(op, BitCast | PtrToInt | IntToPtr) && bitcast_expands(profile, dest);
    // `BITCAST` to an i8 or i16 is legal (promoted); to the other types it
    // expands on the x87 targets, but `f64`'s is custom.
    let legal = !matches!(op, BitCast | PtrToInt | IntToPtr) || profile.sse2 || matches!(dest, Ty::Int(8 | 16));
    adjust(if source_count == dest_count && legal {
        Cost::of(source_count)
    } else if !expands {
        Cost::BASIC
    } else {
        Cost::EXPENSIVE
    })
}

/// `SSE2CostTbl` and `SSE1CostTbl` of `getCmpSelInstrCost`, the scalar rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L4058
/// and
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L4075
fn sse_cmp_sel(
    profile: &X86,
    op: CmpSel,
    ty: Ty,
) -> Option<Row> {
    match (op, ty) {
        (CmpSel::ICmp | CmpSel::FCmp, Ty::Double) if profile.sse2 => Some(all(1, 5, 1, 1)),
        (CmpSel::Select, Ty::Double) if profile.sse2 => Some(all(2, 2, 3, 3)),
        (CmpSel::ICmp | CmpSel::FCmp, Ty::Float) if profile.sse1 => Some(all(1, 5, 1, 1)),
        (CmpSel::Select, Ty::Float) if profile.sse1 => Some(all(2, 2, 3, 3)),
        _ => None,
    }
}

fn int_imm_chunk(value: i64) -> i64 {
    if value == 0 {
        0
    } else if i32::try_from(value).is_ok() {
        1
    } else {
        2
    }
}

/// Intrinsic prices by (profile, kind, callee name), as the pinned `opt` gives
/// them (T7).
fn intrinsic_prices() -> &'static llrm_support::hash::HashMap<(&'static str, &'static str, &'static str), Option<i64>> {
    static PRICES: std::sync::OnceLock<
        llrm_support::hash::HashMap<(&'static str, &'static str, &'static str), Option<i64>>,
    > = std::sync::OnceLock::new();
    PRICES.get_or_init(|| {
        include_str!("tti_intrinsics.txt")
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| {
                let mut words = line.split(' ');
                let (profile, kind, name, cost) = (words.next()?, words.next()?, words.next()?, words.next()?);
                Some(((profile, kind, name), cost.parse().ok()))
            })
            .collect()
    })
}

impl Tti for X86 {
    fn name(&self) -> &'static str {
        "X86"
    }

    fn cast_of_load_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Option<Cost> {
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L3805
        // For a scalar load the extend would be free.
        (matches!(op, CastKind::SIToFP | CastKind::UIToFP) && matches!(src, Ty::Int(bits) if (2..32).contains(&bits)))
            .then(|| self.cast_cost(CastKind::SIToFP, dst, Ty::Int(32), kind))
    }

    fn indexed_store_cost(
        &self,
        kind: CostKind,
    ) -> Option<Cost> {
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L5818
        (kind != CostKind::RecipThroughput).then(|| Cost::of(2))
    }

    fn intrinsic_cost(
        &self,
        name: &str,
        ret: Option<Ty>,
        args: &[Ty],
        kind: CostKind,
    ) -> Cost {
        let _ = (ret, args);
        let profile = if self.sse2 { "sse2" } else { "nosse" };
        let kind = match kind {
            CostKind::RecipThroughput => "throughput",
            CostKind::Latency => "latency",
            CostKind::CodeSize => "code-size",
            CostKind::SizeAndLatency => "size-latency",
        };
        match intrinsic_prices().get(&(profile, kind, name)) {
            Some(Some(cost)) => Cost::of(*cost),
            Some(None) => Cost::INVALID,
            None if is_free_intrinsic(name) => Cost::FREE,
            None => Cost::INVALID,
        }
    }

    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost {
        let Some((count, _)) = legalized(ty) else { return Cost::INVALID };
        if let Some(each) = sse_cmp_sel(self, op, ty).and_then(|row| row[kind.index()]) {
            return Cost::of(count * each);
        }
        // Assume a 3 cycle latency for a floating point select:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L4133
        if kind == CostKind::Latency && op == CmpSel::Select && ty.is_float() {
            return Cost::of(3);
        }
        // BasicTTIImplBase: legal, one per legalized part.
        Cost::of(count)
    }

    /// `X86TTIImpl::getMemoryOpCost`, scalars:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L5806
    fn memory_op_cost(
        &self,
        op: MemoryOp,
        ty: Ty,
        kind: CostKind,
        stored_constant: bool,
    ) -> Cost {
        // Load latency is not modelled here: the base answers 4.
        if op == MemoryOp::Load && kind == CostKind::Latency {
            return Cost::of(4);
        }
        if kind != CostKind::RecipThroughput {
            return Cost::BASIC;
        }
        let Some((count, _)) = legalized(ty) else { return Cost::INVALID };
        // A constant stored through a floating point register is loaded first.
        let extra = if op == MemoryOp::Store && stored_constant && ty.is_float() { count } else { 0 };
        Cost::of(extra + count)
    }

    /// `X86TTIImpl::getCFInstrCost`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7076
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

    /// `X86TTIImpl::getIntImmCost(const APInt &, Type *)`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L6892
    fn int_imm_cost(
        &self,
        imm: i128,
        bits: u32,
    ) -> Cost {
        if bits == 0 {
            return Cost::of(i64::from(u32::MAX));
        }
        if bits > 128 || imm == 0 {
            return Cost::FREE;
        }
        // Sign-extended to a multiple of 64 bits, costed 64 bits at a time.
        let extended = if bits >= 128 { imm } else { (imm << (128 - bits)) >> (128 - bits) };
        let mut total = 0;
        let mut shift = 0;
        while shift < bits {
            total += int_imm_chunk((extended >> shift) as i64);
            shift += 64;
        }
        Cost::of(total.max(1))
    }

    /// `X86TTIImpl::getIntImmCostInst`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L6927
    fn int_imm_cost_inst(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost {
        if bits == 0 {
            return Cost::FREE;
        }
        let zext = if bits >= 128 { imm as u128 } else { (imm as u128) & ((1u128 << bits) - 1) };
        let immediate = match op {
            ImmUser::Unknown => return Cost::FREE,
            ImmUser::GetElementPtr => return if index == 0 { Cost::of(2) } else { Cost::FREE },
            ImmUser::Store => Some(0),
            ImmUser::ICmp => {
                if index == 1 && bits == 64 && (zext == 0x1_0000_0000 || zext == 0xffff_ffff) {
                    return Cost::FREE;
                }
                Some(1)
            }
            ImmUser::And => {
                if index == 1 && bits == 64 && zext <= u128::from(u32::MAX) {
                    return Cost::FREE;
                }
                Some(1)
            }
            ImmUser::Add | ImmUser::Sub => {
                if index == 1 && bits == 64 && zext == 0x8000_0000 {
                    return Cost::FREE;
                }
                Some(1)
            }
            ImmUser::Divide => return Cost::FREE,
            ImmUser::Mul | ImmUser::Or | ImmUser::Xor => Some(1),
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

    /// `X86TargetLowering::isTruncateFree`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L36743
    fn is_truncate_free(
        &self,
        from: Ty,
        to: Ty,
    ) -> bool {
        match (from, to) {
            (Ty::Int(a), Ty::Int(b)) => a > b,
            _ => false,
        }
    }

    /// `X86TargetLowering::isLegalAddressingMode`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L36621,
    /// 32-bit, small code model, not position independent.
    fn is_legal_addressing_mode(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        // A displacement is a signed 32-bit immediate, below 16 MB beside a
        // symbol.
        if i32::try_from(base_offset).is_err() || (base_gv && base_offset >= 16 * 1024 * 1024) {
            return false;
        }
        match scale {
            0 | 1 | 2 | 4 | 8 => true,
            // Formed with base + scale register: only without a base register.
            3 | 5 | 9 => !has_base_reg,
            _ => false,
        }
    }

    /// `X86TTIImpl::getCastInstrCost`, scalars:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L2772
    /// (T4: no instruction context, so the `zext` folded into a GEP index and
    /// the extending load are not seen).
    fn cast_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Cost {
        use CastKind::*;
        if self.sse2 {
            if let Some(each) = sse2_conversion(op, dst, src).and_then(|row| row[kind.index()]) {
                return Cost::of(each);
            }
        }
        let (Some((source_count, source)), Some((dest_count, dest))) = (legalized(src), legalized(dst)) else {
            return Cost::INVALID;
        };
        if op == Trunc && source == dest {
            return Cost::FREE;
        }
        if self.sse2 {
            if let Some(each) = sse2_conversion(op, dest, source).and_then(|row| row[kind.index()]) {
                return Cost::of(source_count.max(dest_count) * each);
            }
        }
        // i8 and i16 integers convert through i32:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L3805
        if matches!(op, SIToFP | UIToFP) && matches!(src, Ty::Int(bits) if (2..32).contains(&bits)) {
            let extend = if op == SIToFP { SExt } else { ZExt };
            return self.cast_cost(extend, Ty::Int(32), src, kind) + self.cast_cost(SIToFP, dst, Ty::Int(32), kind);
        }
        if matches!(op, FPToSI | FPToUI) && matches!(dst, Ty::Int(bits) if (2..32).contains(&bits)) {
            return self.cast_cost(FPToSI, Ty::Int(32), src, kind) + self.cast_cost(Trunc, dst, Ty::Int(32), kind);
        }
        generic_cast(self, op, dst, src, kind)
    }

    /// `X86TTIImpl::getArithmeticInstrCost`, scalars:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L255
    fn arithmetic_cost(
        &self,
        op: ArithOp,
        ty: Ty,
        kind: CostKind,
        first: OperandInfo,
        second: OperandInfo,
    ) -> Cost {
        use ArithOp::*;
        let Some((count, legal)) = legalized(ty) else { return Cost::INVALID };
        let none = OperandInfo::ANY;
        // Multiply by a power of two (or its negation) is a shift:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L334
        if op == Mul && second.constant && (second.power_of_two || second.negated_power_of_two) {
            let mut cost = self.arithmetic_cost(Shl, ty, kind, none, none);
            if second.negated_power_of_two {
                cost = cost + self.arithmetic_cost(Sub, ty, kind, none, none);
            }
            return cost;
        }
        // Signed divide by a power of two:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L346
        if matches!(op, SDiv | SRem) && second.constant && second.power_of_two {
            let mut cost = self.arithmetic_cost(AShr, ty, kind, none, none) * 2
                + self.arithmetic_cost(LShr, ty, kind, none, none)
                + self.arithmetic_cost(Add, ty, kind, none, none);
            if op == SRem {
                cost = cost
                    + self.arithmetic_cost(Mul, ty, kind, none, none)
                    + self.arithmetic_cost(Sub, ty, kind, none, none);
            }
            return cost;
        }
        // Unsigned divide by a power of two:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L371
        if matches!(op, UDiv | URem) && second.constant && second.power_of_two {
            return if op == UDiv {
                self.arithmetic_cost(LShr, ty, kind, none, none)
            } else {
                self.arithmetic_cost(And, ty, kind, none, none)
            };
        }
        // A scalar divide by a constant is a multiply-high sequence:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L382
        if second.constant && !second.negated_power_of_two && matches!(op, UDiv | SDiv | URem | SRem) {
            let mut cost = if matches!(op, URem | SRem) { 6 } else { 5 };
            if matches!(kind, CostKind::CodeSize | CostKind::SizeAndLatency) {
                cost += 2;
            }
            return Cost::of(count * cost);
        }
        if !second.constant {
            if let Some(row) = scalar_variable_divide(op, legal) {
                if let Some(each) = row[kind.index()] {
                    return Cost::of(count * each);
                }
            }
        }
        let tables: [(bool, fn(ArithOp, Ty) -> Option<Row>); 3] =
            [(self.sse2, sse2_table), (self.sse1, sse1_table), (true, x86_table)];
        for (present, table) in tables {
            if present {
                if let Some(each) = table(op, legal).and_then(|row| row[kind.index()]) {
                    return Cost::of(count * each);
                }
            }
        }
        // Single-instruction code size cases:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1918
        if kind == CostKind::CodeSize && matches!(op, FAdd | FSub | FMul | FDiv | FNeg | And | Or | Xor) {
            return Cost::of(count);
        }
        generic_arithmetic(op, ty, kind, first, second, self)
    }
}
