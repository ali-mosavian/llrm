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
//! - T6: the 16-bit target reads the addressing hooks and the register count
//!   from its description: `getScalingFactorCost` is the cost of the address
//!   form that holds the scale (the prefix and its stall beyond the native `bx
//!   + si` form), `getNumberOfRegisters` the registers a value may sit in
//!   (six). The pin's X86TTI prices every second register 1 and has eight.
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct X86 {
    pub sse1: bool,
    pub sse2: bool,
    /// `TuningMacroFusion` (Sandy Bridge and newer tunings, and `core2`'s as
    /// X86.td lists it); `branchfusion` is a Bulldozer-and-newer feature none
    /// of the profiles has.
    pub macro_fusion: bool,
    /// Long mode: the integer registers are 64 bits wide (`is64Bit`).
    pub quad: bool,
    /// What real mode's addressing and register file make of the hooks LSR
    /// reads (T6); none: the 32-bit mode's.
    pub real_mode: Option<RealMode>,
    /// The target's tuning table (`Machine::tuning`); the pin's by default.
    pub tuning: llrm_mir::target::Tuning,
}

/// T6: the 16-bit target's address forms and registers, from its description
/// (`Machine::address_forms`, `Machine::registers`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealMode {
    /// What an address `base + scale * index` costs above the native form,
    /// for each scale some address form holds: the legal scales.
    pub scale_costs: Vec<(i64, i64)>,
    /// General registers a value may sit in.
    pub registers: u32,
    /// What a scaled index costs in instructions, once for each scaled
    /// register: the address-size prefix and the stall it takes.
    pub scaled_index_insns: u32,
    /// The space of huge pointers: an offset there carries into the selector,
    /// so an address is one register and no more.
    pub huge_space: Option<u32>,
    /// The target's `-Os` growth guard for inlining (SZ1, D11): a callee that
    /// stays is inlined into an `optsize` caller only if its code does not
    /// outgrow the call. The pin has none (its `OptSizeThreshold` 50 counts
    /// instructions, not bytes); 16-bit code names it so a size row never
    /// grows.
    pub inline_size_guard: bool,
    /// What an instruction that touches a float takes in code, in percent of
    /// its `CodeSize` units: x87 moves each operand through the register
    /// stack (`qmove`: the model tied where the code grew 2 bytes).
    pub float_code_size_percent: i64,
    /// (trips, bar): an `optsize` loop of at most that many trips is fully
    /// unrolled under that unrolled size (DEVIATE U1). (0, 0): the pin's.
    pub unroll_size_few_trips: (u32, u32),
    /// DEVIATE REC-1: (depth, callee size) of the recursion taken at speed;
    /// none: the pin's.
    pub recursive_inlining: Option<(u32, i64)>,
}

impl RealMode {
    fn scale_cost(
        &self,
        scale: i64,
    ) -> Option<i64> {
        self.scale_costs.iter().find(|(held, _)| *held == scale).map(|(_, cost)| *cost)
    }
}

impl X86 {
    /// The bits of the integer registers.
    pub fn native(&self) -> u32 {
        if self.quad { 64 } else { 32 }
    }

    /// The profile of the baseline cpu of `triple`'s architecture: the 32-bit
    /// ones by their `-march` (`i686` is `pentiumpro`), `x86_64` long mode.
    pub fn of_triple(triple: &str) -> Option<X86> {
        let cpu = match triple.split('-').next()? {
            "x86_64" => "x86-64",
            "i386" => "i386",
            "i486" => "i486",
            "i586" => "pentium",
            "i686" => "pentiumpro",
            _ => return None,
        };
        X86::of_cpu(cpu)
    }

    /// The profile of a `-march` llrm supports.
    pub fn of_cpu(cpu: &str) -> Option<X86> {
        match cpu {
            "i386" | "i486" | "pentium" | "pentiumpro" | "k5" | "k6" | "athlon" => Some(X86 {
                sse1: false,
                sse2: false,
                macro_fusion: false,
                quad: false,
                real_mode: None,
                tuning: llrm_mir::target::Tuning::PIN,
            }),
            "core2" => Some(X86 {
                sse1: true,
                sse2: true,
                macro_fusion: true,
                quad: false,
                real_mode: None,
                tuning: llrm_mir::target::Tuning::PIN,
            }),
            // `-march=x86-64`: the baseline of long mode, SSE2
            // and `TuningMacroFusion` (X86.td `X86_64V1Tuning`).
            "x86-64" => Some(X86 {
                sse1: true,
                sse2: true,
                macro_fusion: true,
                quad: true,
                real_mode: None,
                tuning: llrm_mir::target::Tuning::PIN,
            }),
            _ => None,
        }
    }

    /// The model a target description names (`tti` and `llvm_cpu` of its
    /// `datalayout.toml`): none for a name no model here has.
    pub fn described(
        tti: &str,
        llvm_cpu: &str,
    ) -> Option<X86> {
        match tti {
            "X86" => X86::of_cpu(llvm_cpu),
            _ => None,
        }
    }

    /// The profile of 16-bit code (T6).
    pub fn in_real_mode(
        &self,
        real_mode: RealMode,
    ) -> X86 {
        X86 { real_mode: Some(real_mode), ..self.clone() }
    }

    /// The target's tuning table.
    pub fn with_tuning(
        &self,
        tuning: llrm_mir::target::Tuning,
    ) -> X86 {
        X86 { tuning, ..self.clone() }
    }
}

/// `getTypeLegalizationCost` for the scalar types: how many operations of what
/// type a value takes, the integer registers being `native` bits wide (32, or
/// 64 in long mode):
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1039
fn legalized(
    ty: Ty,
    native: u32,
) -> Option<(i64, Ty)> {
    match ty {
        Ty::Int(bits) if bits <= 8 => Some((1, Ty::Int(8))),
        Ty::Int(16) => Some((1, Ty::Int(16))),
        Ty::Int(bits) if bits <= 32 => Some((1, Ty::Int(32))),
        Ty::Int(bits) if native == 64 && bits <= 64 => Some((1, Ty::Int(64))),
        Ty::Int(bits) => Some((i64::from(bits.div_ceil(native).next_power_of_two()), Ty::Int(native))),
        Ty::Pointer => Some((1, Ty::Int(native))),
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

/// `X64CostTbl`, the 64-bit targets' rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1868
fn x64_table(
    op: ArithOp,
    ty: Ty,
) -> Option<Row> {
    use ArithOp::*;
    match (op, ty) {
        (Add | Sub, Ty::Int(64)) => Some(throughput(1)),
        (Mul, Ty::Int(64)) => Some(all(2, 6, 1, 2)),
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
    let Some((count, _)) = legalized(ty, profile.native()) else { return Cost::INVALID };
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
        IntToPtr => matches!(src, Ty::Int(8 | 16 | 32)) || (profile.quad && src == Ty::Int(64)),
        PtrToInt => dst == Ty::Int(profile.native()),
        BitCast => dst == src,
        Trunc => matches!(dst, Ty::Int(8 | 16 | 32)) || (profile.quad && dst == Ty::Int(64)),
        _ => false,
    };
    if free {
        return Cost::FREE;
    }
    let (Some((source_count, source)), Some((dest_count, dest))) =
        (legalized(src, profile.native()), legalized(dst, profile.native()))
    else {
        return Cost::INVALID;
    };
    // `X86TargetLowering::isZExtFree`: a 32-bit result is zero-extended into
    // the quadword already.
    if op == ZExt && profile.quad && source == Ty::Int(32) && dest == Ty::Int(64) {
        return Cost::FREE;
    }
    let is_integer = |ty: Ty| matches!(ty, Ty::Int(_) | Ty::Pointer);
    let bits = |ty: Ty| match ty {
        Ty::Int(bits) => bits,
        Ty::Pointer => profile.native(),
        Ty::Float => 32,
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

const INTRINSIC_PRICES: &str = include_str!("tti_intrinsics.txt");

/// The price of `name` under (profile, kind) in the pinned `opt`'s table (T7):
/// `None` when the table has no row, `Some(None)` for an invalid price. The
/// text is sorted by byte, so a lookup bisects it where it lies; building a
/// map on first use cost 1.06 M Ir on d_faces -O2.
fn intrinsic_price(
    profile: &str,
    kind: &str,
    name: &str,
) -> Option<Option<i64>> {
    let key = || profile.bytes().chain([b' ']).chain(kind.bytes()).chain([b' ']).chain(name.bytes()).chain([b' ']);
    let key_len = profile.len() + kind.len() + name.len() + 3;
    let (mut low, mut high) = (0, INTRINSIC_PRICES.len());
    while low < high {
        let middle = low + (high - low) / 2;
        let start = INTRINSIC_PRICES[low..middle].rfind('\n').map_or(low, |at| low + at + 1);
        let end = INTRINSIC_PRICES[start..].find('\n').map_or(INTRINSIC_PRICES.len(), |at| start + at);
        let line = &INTRINSIC_PRICES[start..end];
        match line.bytes().take(key_len).cmp(key()) {
            std::cmp::Ordering::Less => low = end + 1,
            std::cmp::Ordering::Greater => high = start,
            std::cmp::Ordering::Equal => return Some(line[key_len..].trim_end().parse().ok()),
        }
    }
    None
}

impl Tti for X86 {
    fn name(&self) -> &'static str {
        "X86"
    }

    fn inline_threshold_override(&self) -> Option<i64> {
        self.tuning.inline_threshold
    }

    fn size_milliclocks_per_byte(&self) -> i64 {
        self.tuning.size_milliclocks_per_byte
    }

    fn unroll_threshold_overrides(&self) -> (Option<u32>, Option<u32>) {
        (self.tuning.unroll_threshold, self.tuning.unroll_max_percent_threshold_boost)
    }

    fn recursive_inlining(&self) -> Option<(u32, i64)> {
        self.real_mode.as_ref()?.recursive_inlining
    }

    fn unroll_size_few_trips(&self) -> Option<(u32, u32)> {
        let real = self.real_mode.as_ref().filter(|real| real.unroll_size_few_trips.0 != 0)?;
        Some(real.unroll_size_few_trips)
    }

    /// SZ1: an inline that stays must not grow the code, where the target
    /// says so (`RealMode::inline_size_guard`): at m32 -Os follows the pin
    /// (crc, tile and recchop grow 52, 42 and 15 bytes there).
    fn opt_size_inline_by_code_size(&self) -> bool {
        self.real_mode.as_ref().is_some_and(|real| real.inline_size_guard)
    }

    /// A call is 5 bytes and the push of an argument 1 to 4, against 2 to 3 for
    /// an ALU instruction. With 100 the 28 calls of `_savegame_read_block`
    /// stayed (+408 bytes of the module); 150 inlines them and still keeps the
    /// 14-instruction loop of `idioms.bas` out of ten sites.
    fn call_code_size_percent(&self) -> i64 {
        150
    }

    fn float_code_size_percent(&self) -> i64 {
        self.real_mode.as_ref().map_or(100, |real| real.float_code_size_percent)
    }

    // X86TTIImpl::hasDivRemOp (X86TargetTransformInfo.cpp#L7421) asks
    // `isOperationLegal(SDIVREM/UDIVREM, VT)`; X86ISelLowering sets the scalar
    // divides to `Expand` (L340) and leaves the two-result ones at the base
    // class's `Legal`, so every register type has one.
    fn has_div_rem_op(
        &self,
        bits: u32,
        _signed: bool,
    ) -> bool {
        self.is_type_legal(Ty::Int(bits))
    }

    fn is_type_legal(
        &self,
        ty: Ty,
    ) -> bool {
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp
        // The GPR classes hold i8, i16, i32 and, in long mode, i64; x87 holds
        // every scalar float.
        match ty {
            Ty::Int(8 | 16 | 32) | Ty::Pointer => true,
            Ty::Int(64) => self.quad,
            Ty::Int(_) => false,
            Ty::Float | Ty::Double | Ty::X87 => true,
        }
    }

    fn cast_of_load_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Option<Cost> {
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1297
        // A `sext`/`zext` of a load is the extending load (movsx, movzx) when
        // both types legalize to one register each.
        if let (CastKind::SExt | CastKind::ZExt, Ty::Int(to), Ty::Int(from)) = (op, dst, src)
            && from >= 8
            && let (Some((to_count, _)), Some((from_count, _))) =
                (legalized(Ty::Int(to), self.native()), legalized(Ty::Int(from), self.native()))
            && to_count == from_count
        {
            return Some(Cost::FREE);
        }
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

    /// X86's `getJumpConditionMergingParams`: `x86-br-merging-base-cost` 2,
    /// one more for `a == b && a == c` (a fast pattern); an `or` of two `==`
    /// splits (no CCMP on these targets). Likely bias 0, unlikely bias -1.
    fn jump_condition_merging_params(
        &self,
        query: &llrm_mir::jumpmerge::JumpMergeQuery,
    ) -> llrm_mir::jumpmerge::CondMerging {
        use llrm_mir::jumpmerge::{CondMerging, Logic};
        use llrm_mir::module::{Operand, ValueDef};
        use llrm_mir::opcode::{IntPredicate, Opcode};
        let is_eq = |operand: Operand| match operand {
            Operand::Value(value) => match query.function.value(value).def {
                ValueDef::Instruction(inst) => {
                    matches!(
                        query.function.instruction(inst).opcode,
                        Opcode::ICmp(IntPredicate::Eq)
                    )
                }
                _ => false,
            },
            _ => false,
        };
        let both_eq = is_eq(query.lhs) && is_eq(query.rhs);
        match query.logic {
            Logic::Or if both_eq => CondMerging::SPLIT,
            Logic::And if both_eq => CondMerging { base_cost: 3, likely_bias: 0, unlikely_bias: -1 },
            _ => CondMerging { base_cost: 2, likely_bias: 0, unlikely_bias: -1 },
        }
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
        match intrinsic_price(profile, kind, name) {
            Some(Some(cost)) => Cost::of(cost),
            Some(None) => Cost::INVALID,
            None if is_free_intrinsic(name) => Cost::FREE,
            None => base_intrinsic_cost(name).unwrap_or(Cost::INVALID),
        }
    }

    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost {
        let Some((count, _)) = legalized(ty, self.native()) else { return Cost::INVALID };
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
        let Some((count, _)) = legalized(ty, self.native()) else { return Cost::INVALID };
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

    fn is_legal_addressing_mode_in(
        &self,
        addr_space: u32,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        if self.real_mode.as_ref().is_some_and(|real| real.huge_space == Some(addr_space)) {
            return !base_gv && has_base_reg && base_offset == 0 && scale == 0;
        }
        self.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale)
    }

    /// `X86TargetLowering::isLegalAddressingMode`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L36621,
    /// Small code model; position independent in long mode, not in 32-bit.
    fn is_legal_addressing_mode(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        // A displacement is a signed 32-bit immediate, below 16 MB beside a
        // symbol.
        if let Some(real) = &self.real_mode {
            // 16-bit addresses: a 16-bit displacement and a scale a primary
            // form holds (0 is no index).
            return i16::try_from(base_offset).is_ok() && (scale == 0 || real.scale_cost(scale).is_some());
        }
        if i32::try_from(base_offset).is_err() || (base_gv && base_offset >= 16 * 1024 * 1024) {
            return false;
        }
        // Long mode, position independent (the PIE a default clang makes): a
        // symbol is `[rip + symbol]`, which takes no displacement
        // beside it and no index (the `isPositionIndependent() && is64Bit()`
        // rule).
        if self.quad && base_gv && (base_offset != 0 || scale > 1) {
            return false;
        }
        match scale {
            0 | 1 | 2 | 4 | 8 => true,
            // Formed with base + scale register: only without a base register.
            3 | 5 | 9 => !has_base_reg,
            _ => false,
        }
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L36734
    /// `isLegalAddImmediate`: a signed 32-bit immediate (a negative one is a
    /// `sub`).
    fn is_legal_add_immediate(
        &self,
        imm: i64,
    ) -> bool {
        i32::try_from(imm).is_ok()
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L36730
    fn is_legal_icmp_immediate(
        &self,
        imm: i64,
    ) -> bool {
        i32::try_from(imm).is_ok()
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L8045
    /// `getScalingFactorCost`: a second register in the address is one, a
    /// plain address nothing; an illegal mode is invalid.
    fn scaling_factor_cost(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> Cost {
        if self.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale) {
            // T6: real mode's native form (bx + si/di) is free at scale 1; the
            // others take the address-size prefix and its stall.
            if let Some(cost) = self.real_mode.as_ref().and_then(|real| real.scale_cost(scale)) {
                return Cost::of(cost);
            }
            Cost::of(i64::from(scale != 0))
        } else {
            Cost::INVALID
        }
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7202
    /// `isLSRCostLess`: "instruction number 1st priority".
    fn is_lsr_cost_less(
        &self,
        a: &LsrCost,
        b: &LsrCost,
    ) -> bool {
        let key = |c: &LsrCost| {
            (
                c.insns,
                c.num_regs,
                c.add_rec_cost,
                c.num_iv_muls,
                c.num_base_adds,
                c.scale_cost,
                c.imm_cost,
                c.setup_cost,
            )
        };
        key(a) < key(b)
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7211
    fn can_macro_fuse_cmp(&self) -> bool {
        self.macro_fusion
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L173
    /// Eight general registers, sixteen in long mode (`getNumberOfRegisters`).
    fn number_of_gpr_registers(&self) -> u32 {
        self.real_mode.as_ref().map_or(if self.quad { 16 } else { 8 }, |real| real.registers)
    }

    /// `lea` forms `base + index + displacement` in one instruction.
    fn address_form_adds_offsets(&self) -> bool {
        true
    }

    /// DEVIATE LU4: real mode keeps the loop as written where LSR's own model
    /// rates its rewrite worse (`shouldDropLSRSolutionIfLessProfitable`, which
    /// the pin's X86 leaves off). Six registers and a prefix on every scaled
    /// index make the model's best answer a loss in bytes and clocks too.
    fn should_drop_lsr_solution_if_less_profitable(&self) -> bool {
        self.real_mode.is_some()
    }

    fn lsr_charges_outer_nonaffine_ivs(&self) -> bool {
        self.real_mode.is_some()
    }

    fn mirrors_conditional_counters(&self) -> bool {
        self.real_mode.is_some()
    }

    /// Real mode forms a scaled index with an address-size prefix.
    fn lsr_drops_spilling_solution(&self) -> bool {
        self.real_mode.is_some()
    }

    fn scaled_index_insns(&self) -> u32 {
        self.real_mode.as_ref().map_or(0, |real| real.scaled_index_insns)
    }

    /// i8 and i16 extending loads are legal, and free when the result
    /// legalizes to as many registers as the loaded type does.
    fn ext_load_free(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
    ) -> bool {
        matches!(op, CastKind::ZExt | CastKind::SExt)
            && matches!(
                (src, dst),
                (Ty::Int(8), Ty::Int(16 | 32)) | (Ty::Int(16), Ty::Int(32))
            )
            && matches!(
                (legalized(src, self.native()), legalized(dst, self.native())),
                (Some((one, _)), Some((same, _))) if one == same
            )
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
        let (Some((source_count, source)), Some((dest_count, dest))) =
            (legalized(src, self.native()), legalized(dst, self.native()))
        else {
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
        let Some((count, legal)) = legalized(ty, self.native()) else { return Cost::INVALID };
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
        let tables: [(bool, fn(ArithOp, Ty) -> Option<Row>); 4] =
            [(self.sse2, sse2_table), (self.sse1, sse1_table), (self.quad, x64_table), (true, x86_table)];
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
