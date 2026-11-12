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
//! - DEVIATE 3: `isLegalAddressingMode` takes the access type; the trait's
//!   typed queries (`*_at`, which LSR asks of a use) take its size in bytes,
//!   but the untyped ones (`is_legal_addressing_mode`) assume 8 bytes, a
//!   register ([`AArch64::is_legal_addressing_mode_for`] takes the size).
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

use llrm_mir::lowering::{ExtUser, Isd, LoadExt, Lowering};
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

/// What a code byte costs a full unroll at -O2/-O3, in thousandths of a clock.
/// A byte executed cold costs the refill of its 64 B line from L2, about 16
/// clocks: 0.25 a byte, the floor (a Cortex-A72/M1-class core fetches 16 B a
/// clock and runs 3-4 instructions a clock, so an instruction is about 0.3).
/// The saving the model counts is the loop control and the folded operands
/// only, so the price is set at twice the floor: 0.5 keeps every unroll that
/// saves 0.5-23 clocks a byte, which the pin makes by size (150) alone
/// (fpbench -O2 -33% instructions). It was 250, the median of the cells where
/// gcc and clang differed, against a saving counted over assumed runs.
///
/// DEVIATE UP1 (exit 2026-11-20), cells that grow bytes for little, against 1.0
/// (2026-11-08): nbody_fixed -O2 +32 B for -37 instructions and -O3 +20 B for
/// -45; shellsort -O3 +704 B for -139; jpeg_fixed -O3 +152 B and jpeg_single
/// +148 B for -722 and -168 instructions. A wash at the floor, and they run few
/// times. jpeg -O2: +28 and +8 B for -33 and -42 instructions of 11.65 M and
/// 8.49 M (measured, not 0): the 3-trip component loops of the header parse
/// (`id[c] == want`, 16 B for 11 clocks a run) run once a program, so the
/// saving is real and one-off. The model prices them right at 1.2 instructions
/// a byte, above the 0.8 break-even; it cannot know the code is cold.
const UNROLL_MILLICLOCKS_PER_BYTE: i64 = 500;

/// Bytes, in thousandths, one inliner cost unit (an instruction) stands for.
const BYTES_PER_COST_UNIT_MILLI: i64 = 4000;

/// Thousandths of a clock one unit of inline growth costs at -O2/-O3 (250
/// clocks).
const INLINE_MILLICLOCKS_PER_UNIT: i64 = 250_000;

/// AArch64's scalar profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AArch64 {
    pub cpu: Cpu,
    /// `unroll_milliclocks_per_byte`; the tests' knob.
    pub unroll_rate: i64,
}

impl AArch64 {
    /// The profile of a `-mcpu` llrm supports.
    pub fn of_cpu(cpu: &str) -> Option<AArch64> {
        match cpu {
            "generic" => Some(AArch64 { cpu: Cpu::Generic, unroll_rate: UNROLL_MILLICLOCKS_PER_BYTE }),
            "apple-m1" => Some(AArch64 { cpu: Cpu::AppleM1, unroll_rate: UNROLL_MILLICLOCKS_PER_BYTE }),
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
pub(crate) fn is_legal_arith_immediate(c: u64) -> bool {
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

thread_local! {
    static INDEXED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many rows of the intrinsic table this thread has indexed.
pub fn intrinsic_rows_indexed() -> usize {
    INDEXED.with(std::cell::Cell::get)
}

type Section = llrm_support::hash::HashMap<&'static str, Option<i64>>;

/// Intrinsic prices by (cpu, kind, callee name), as the pinned `opt` gives them
/// (DEVIATE 1). A compile reads the rows of one cpu and a few kinds, which the
/// file keeps together: each such section is found in the text and indexed
/// when a price of it is first asked for, not the 1081 rows of all of them.
fn intrinsic_price(
    cpu: &'static str,
    kind: &'static str,
    name: &str,
) -> Option<Option<i64>> {
    static SECTIONS: std::sync::Mutex<Vec<(&str, &str, &Section)>> = std::sync::Mutex::new(Vec::new());
    let mut sections = SECTIONS.lock().unwrap();
    let known = sections.iter().find(|(c, k, _)| *c == cpu && *k == kind).map(|(_, _, section)| *section);
    let section = known.unwrap_or_else(|| {
        let section: &'static Section = Box::leak(Box::new(index_section(cpu, kind)));
        sections.push((cpu, kind, section));
        section
    });
    section.get(name).copied()
}

/// The rows `cpu kind name cost` of one section of the table, by name.
fn index_section(
    cpu: &str,
    kind: &str,
) -> Section {
    const TABLE: &str = include_str!("tti_intrinsics.txt");
    let prefix = format!("{cpu} {kind} ");
    let Some(start) = TABLE.find(&format!("\n{prefix}")).map(|at| at + 1) else { return Section::default() };
    let rows = TABLE[start..].lines().take_while(|line| line.starts_with(&prefix));
    let mut section = Section::default();
    for line in rows {
        INDEXED.with(|indexed| indexed.set(indexed.get() + 1));
        let mut words = line[prefix.len()..].split(' ');
        if let (Some(name), Some(cost)) = (words.next(), words.next()) {
            section.insert(name, cost.parse().ok());
        }
    }
    section
}

impl Cpu {
    /// `ST->getMaxInterleaveFactor()`: 2, 4 with `FeatureMaxInterleaveFactor4`
    /// (`TuneAppleA14`, which `apple-m1` tunes for, AArch64Processors.td#L422).
    fn max_interleave_factor(self) -> u32 {
        match self {
            Cpu::Generic => 2,
            Cpu::AppleM1 => 4,
        }
    }

    /// `ST->isMisaligned128StoreSlow()`: only the pre-A14 Apple cores and the
    /// Exynos set it; neither cpu here does.
    fn misaligned_128_store_slow(self) -> bool {
        false
    }
}

/// `getTypeLegalizationCost(<count x lane>).first`: the number of NEON
/// registers the vector takes, and its legal lane count per register. A lane
/// of 8, 16, 32 or 64 bits and a power-of-two count only (DEVIATE VT1: other
/// shapes are widened or split by `TypeLegalizer` and are not priced).
pub(crate) fn vector_pieces(ty: VectorTy) -> Option<(i64, u32)> {
    let lane = match ty.lane {
        Ty::Int(bits @ (8 | 16 | 32 | 64)) => bits,
        Ty::Float => 32,
        Ty::Double => 64,
        _ => return None,
    };
    if !ty.count.is_power_of_two() {
        return None;
    }
    let total = u64::from(lane) * u64::from(ty.count);
    // A type of at most 128 bits is one register; wider ones split in halves.
    let pieces = total.div_ceil(128).max(1) as i64;
    Some((pieces, lane))
}

/// `AArch64TTIImpl::getMemoryOpCost` for a fixed vector, the throughput
/// kinds: L5664 (DEVIATE VT2: `TCK_Latency` reads the scheduling model and is
/// not priced).
fn vector_memory_op_cost(
    cpu: Cpu,
    op: MemoryOp,
    ty: VectorTy,
    align: u64,
    kind: CostKind,
) -> Option<Cost> {
    let (first, lane) = vector_pieces(ty)?;
    match kind {
        CostKind::Latency => return None,
        CostKind::CodeSize | CostKind::SizeAndLatency => return Some(Cost::of(first)),
        CostKind::RecipThroughput => {}
    }
    // Unaligned 128-bit stores, on the cpus that set the feature.
    if cpu.misaligned_128_store_slow()
        && op == MemoryOp::Store
        && u64::from(lane) * u64::from(ty.count) == 128
        && align < 16
    {
        const AMORTIZATION_COST: i64 = 6;
        return Some(Cost::of(first * 2 * AMORTIZATION_COST));
    }
    // `useNeonVector`: an element of a power of two between 8 and 64 bits, and
    // an access that is not byte-aligned, costs the registers it takes.
    if align != 1 {
        return Some(Cost::of(first));
    }
    // Byte-aligned: broken into the power-of-two pieces `ld1`/`st1` take.
    if ty.count == 3 && lane == 8 {
        return Some(Cost::of(first));
    }
    let per_register = 128 / lane;
    let mut cost = i64::from(ty.count / per_register);
    let remainder = ty.count % per_register;
    if remainder != 0 {
        let mut work = vec![(remainder, 0u32)];
        while let Some((current, offset)) = work.pop() {
            if current.is_power_of_two() {
                cost += 1 + i64::from(offset != 0);
                continue;
            }
            let previous = current.next_power_of_two() / 2;
            work.push((previous, offset));
            work.push((current - previous, offset + previous));
        }
    }
    Some(Cost::of(cost))
}

/// `AArch64TTIImpl::getArithmeticInstrCost` for a fixed vector, throughput:
/// `add`/`sub`/`mul` and the bitwise and shift operations of a legal type are
/// one instruction per register (L4985-4990, then `BaseT`). DEVIATE VT3: a
/// `mul` of `<2 x i64>` (14 in the pin: no `mul.2d`) and the integer divisions
/// are not priced, so a loop that has one is not vectorized; `frem` of a vector
/// (24 to 52 in the pin: libcalls) is not priced either.
fn vector_arithmetic_cost(
    op: ArithOp,
    ty: VectorTy,
    kind: CostKind,
) -> Option<Cost> {
    if kind != CostKind::RecipThroughput {
        return None;
    }
    let (first, lane) = vector_pieces(ty)?;
    let float = ty.lane.is_float();
    match op {
        ArithOp::Add | ArithOp::Sub | ArithOp::And | ArithOp::Or | ArithOp::Xor if !float => Some(Cost::of(first)),
        ArithOp::Shl | ArithOp::LShr | ArithOp::AShr if !float => Some(Cost::of(first)),
        ArithOp::Mul if !float && lane < 64 => Some(Cost::of(first)),
        // L5283-5306: `fadd`, `fsub` and `fneg` are one per register, `fmul`
        // and `fdiv` two (marked custom to lower them to SVE, at no added
        // cost).
        ArithOp::FAdd | ArithOp::FSub | ArithOp::FNeg if float => Some(Cost::of(first)),
        ArithOp::FMul | ArithOp::FDiv if float => Some(Cost::of(2 * first)),
        _ => None,
    }
}

impl Tti for AArch64 {
    /// DEVIATE IPACP-O2 (owner w10, exit 2026-11-24): gcc's ipa-cp copies
    /// at speed; queens -O3 113741 -> 106011 instructions.
    fn ipa_cp_clones_at_speed(&self) -> bool {
        true
    }

    /// DEVIATE LFTR1 (owner w12, review 2026-12-20): any constant
    /// non-wrapping stride counts; the pin's step-one rule costs bytes here.
    fn lftr_any_stride(&self) -> bool {
        true
    }

    /// SZ1: at -Os an inline that stays must not grow the code, as gcc's `-Os`
    /// inlines only where the module shrinks. The pin's `OptSizeThreshold` 50
    /// inlines jpeg_fixed's `put32` (a 4-trip store loop) into eight sites
    /// and `message` into nine: +176 B against gcc.
    fn opt_size_inline_by_code_size(&self) -> bool {
        true
    }

    /// DEVIATE INL-SPEED-OS (owner w11, exit 2026-12-05): see
    /// `Tti::inline_trial_speed_at_size`.
    fn inline_trial_speed_at_size(&self) -> Option<i64> {
        Some(16)
    }

    /// DEVIATE (owner greedy-anyisa, exit 2026-11-15): the pin has no byte
    /// trial. At -Os the trial that keeps `tmul`, `clamp`, `extend` inline
    /// (smaller in bytes, the unit model refuses them) is bounded by the
    /// caller's size, and jpeg_fixed's `bench_jpeg_fixed` is 600+
    /// instructions. Cost: 16 trials of about 15 ms, jpeg_fixed -Os
    /// compiles in 0.38 s, not 0.14 s. A callee rejected in one round is
    /// not tried in the next (`inline_trial_skips_rejected`).
    fn inline_trial_caller_limit(&self) -> usize {
        20_000
    }

    /// DEVIATE (owner greedy-anyisa, exit 2026-11-30): the trial's copy goes
    /// through the cleanup list, not the whole one: Ir of jpeg_fixed -Os
    /// 1.39 G, not 2.95 G (pre-trial 1.38 G). The cleanup list misses a few
    /// simplifications, so some trials decide otherwise (jpeg_single 3592
    /// -> 3608 B, jpeg_fixed 3516 -> 3484 B; the sum is the same). Re-running
    /// near misses with the whole list swaps the +16 B between the cells at
    /// 2.05 G Ir (parked, #8278): `extend` and `receive` read alike in both and
    /// pay in one only. Exit: a trial estimate that predicts the final
    /// bytes of those two. Exception (w15): at speed a trial of a callee
    /// with a loop uses the whole list, as the cleanup list has no loop
    /// passes (vla -O2: the 8-site copy priced +644 B, the unrolled and
    /// folded one is -40 B); at size the cleanup list stays (jpeg -Os +32 B
    /// with the whole one).
    fn inline_trial_light_simplify(&self) -> bool {
        true
    }

    fn inline_trial_skips_rejected(&self) -> bool {
        true
    }

    /// `FSQRT` is legal on `f32` and `f64`.
    fn have_fast_sqrt(
        &self,
        ty: llrm_mir::tti::Ty,
    ) -> bool {
        matches!(ty, llrm_mir::tti::Ty::Float | llrm_mir::tti::Ty::Double)
    }

    fn name(&self) -> &'static str {
        "AArch64"
    }

    /// -Os unrotate: a header no bigger than the branches rotation removes
    /// (w15-unrotate-os-2026-11-03: limit 2 saved 404 B on jpeg_fixed,
    /// jpeg_single, nbody and recchop, none larger).
    fn rotation_header_priced_at_size(&self) -> bool {
        true
    }

    /// DEVIATE LICM-FRAME (owner w15, exit 2026-11-30): rectwo -Os (+25%
    /// instructions against clang): `n - 1`, hoisted into the unguarded entry
    /// because rotation was refused, lived in x19 across the recursive call and
    /// kept the frame ahead of the base case.
    fn licm_speculation_keeps_frame_late(&self) -> bool {
        true
    }

    /// A loop is rotated at -Os only if the copied exit test folds in the
    /// preheader (gcc's `copy_headers`): an unfolded guard also leaves a second
    /// exit block (recsum, recfloat -Os: `mov x0, xzr; ret` beside the loop's).
    fn rotation_guard_must_fold_at_size(&self) -> bool {
        true
    }

    /// DEVIATE ROTATION-TRIAL and ROTATION-HOT (owner w11, exit 2026-12-05):
    /// see `crate::trial`.
    fn rotation_trial_at_size(&self) -> bool {
        true
    }

    fn rotation_hot_at_size(&self) -> bool {
        true
    }

    /// The inliner counts a callee's growth in cost units, one per instruction,
    /// and prices it as `units * inline_o1_unit_bytes_milli / 1000` bytes: an
    /// arm64 instruction is 4 bytes. (Left at 1000, a unit priced as a byte
    /// and the rate below was fitted to that; x86-m32 sets 3500.)
    fn inline_o1_unit_bytes_milli(&self) -> i64 {
        BYTES_PER_COST_UNIT_MILLI
    }

    /// 0.004 B/clock, provisional: a unit of growth is 250 frequency-weighted
    /// clocks, the median of the -O2/-O3 cells where inlining grew the code
    /// (w15-arm64-rate-proposal-11-03.md; widened to 30 clean cells by 11-12),
    /// fitted with one byte a unit. Stated per byte of the 4-byte unit, so the
    /// decisions are those of the fit.
    ///
    /// Refit scan (provisional: net = executed instructions + 0.3 x bytes, an
    /// instruction proxy, not clocks; 2026-11-08, 34 programs at -O2/-O3,
    /// release/next 8d22165d6d, rerun from the knob and scripts in draft PR
    /// 8402): 30 and 10 clk/B change no cell against 62.5; 3 changes
    /// jpeg_fixed and jpeg_single only (+84 B, -1.6k instructions); the
    /// scoring rate 0.3 makes both worse in bytes and instructions (+540 B,
    /// +688k instructions on jpeg_single -O2). Kept at the fitted rate.
    fn speed_inline_milliclocks_per_byte(&self) -> i64 {
        // Scan knob (w15/rate-scan only, never merged): milliclocks per byte.
        if let Some(rate) = std::env::var("LLRM_INL_RATE").ok().and_then(|value| value.parse().ok()) {
            return rate;
        }
        INLINE_MILLICLOCKS_PER_UNIT * 1000 / BYTES_PER_COST_UNIT_MILLI
    }

    /// DEVIATE UP1-RUNS (owner w15, exit 2026-11-20): an unknown-count loop
    /// around an unroll counts once, not 32 times (jpeg_fixed -O2 unrolled a
    /// 16-trip loop for 49,152 assumed clocks; its instructions did not move).
    fn unroll_assumes_unknown_loops_run(&self) -> bool {
        false
    }

    /// DEVIATE ARM64-IPRA (owner w15, exit 2026-11-30): the pin has
    /// `-enable-ipra` off; GCC's `-fipa-ra` is on from -O1 and its recpow -Os
    /// is 32 B smaller for it.
    ///
    /// DEVIATE ARM64-IPRA-OS (owner w15, exit 2026-11-15): jpeg_single -Os is
    /// 20 B larger (3,544 -> 3,564 kernel, +4 B object) with 2% fewer
    /// executed instructions: the greedy re-allocates the whole function and
    /// places 3 more moves and 2 more constant rematerializations. No single
    /// cause; the Os corpus is 40 B smaller in all. Linked to the Os jpeg
    /// register-allocation card (anyisa); re-measure when it lands.
    fn interprocedural_register_use(&self) -> bool {
        true
    }

    /// 250 clocks a byte (the bar's price, as UP1 and the speed inline rate).
    fn speed_milliclocks_per_byte(&self) -> i64 {
        250_000
    }

    /// DEVIATE REC-1 (arm64, owner w15, exit 2026-11-30): a tree-recursive
    /// function of at most 40 `gcc_insns` is copied into itself to depth 2, as
    /// gcc's recursive inlining does (the call, return and argument setup of a
    /// small body are most of its clocks). rectwo -O2: 777,916 instructions to
    /// 165,196 (gcc 266,373), fib 201,208 to 8,824. Depth 3 or a larger limit
    /// copies recchop and quicksort (+134k, +126k), a clock-priced depth never
    /// pays at 250 clk/B for the static trip guess. Open: bintree +97k (its
    /// body is loads and a multiply, not calls). Pin: none.
    fn recursive_inlining(&self) -> Option<(u32, i64)> {
        Some((2, 40))
    }

    /// The calls of a recursion are most of its clocks (rectwo `paths` 12 of
    /// 15, fib 8 of 11; bintree `walk` 16 of 33, quicksort 16 of 51, queens
    /// 8 of 37, which copies grew +97k, +126k and more).
    fn recursive_inlining_call_share(&self) -> Option<(i64, i64)> {
        Some((3, 5))
    }

    /// DEVIATE REC-9 (arm64, owner w15, exit 2026-11-30 with REC-1): a linear
    /// recursion called from another function is peeled into the caller to
    /// gcc's caps. recpow -O2: 67,475 instructions / 260 B to 42,666 / 920 B
    /// (gcc 44,084 / 1,244); the other 71 cells of the corpus are
    /// identical. Pin: none.
    fn peels_linear_recursion(&self) -> bool {
        true
    }

    /// DEVIATE UP1 (arm64): a full unroll at -O2/-O3 must save
    /// [`UNROLL_MILLICLOCKS_PER_BYTE`] for each byte it adds. Exit: the
    /// per-target clock model (2026-11-20).
    fn unroll_milliclocks_per_byte(&self) -> i64 {
        self.unroll_rate
    }

    /// DEVIATE IND-MASK (arm64, owner w15, exit 2026-11-30): see the trait.
    fn indvars_widens_narrow_counter_through_masks(&self) -> bool {
        true
    }

    /// DEVIATE ARM64-UNROLL-PRICE (owner w15, exit 2026-11-30): see the trait.
    fn unroll_credits_immediate_operands(&self) -> bool {
        false
    }

    /// DEVIATE ARM64-UNROLL-PRICE (owner w15, exit 2026-11-30): see the
    /// trait. crc -O2: the bit loop of a byte loop, 152 B against 88 B rolled.
    fn unroll_prices_nest_loops_without_outer_counter(&self) -> bool {
        true
    }

    /// DEVIATE ARM64-NEST-O3-INNER (owner arm64, exit 2026-11-24): see the
    /// trait. fpbench -O3: 2,300 B, 1,412 B with the inner loops unrolled by
    /// the pin's threshold.
    fn unroll_pin_decides_inner_of_known_trips(&self) -> bool {
        true
    }

    fn merges_unaligned_stores(&self) -> bool {
        true
    }

    /// `AArch64TTIImpl::enableMemCmpExpansion`
    /// ([L5499](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L5499)).
    /// llrm's arm64 targets are never `+strict-align`, whose hook returns
    /// none. The load cap is `getMaxExpandSizeMemcmp`
    /// ([L1266](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L1266)).
    fn enable_mem_cmp_expansion(
        &self,
        opt_size: bool,
        is_zero_cmp: bool,
    ) -> Option<MemCmpExpansionOptions> {
        let max_num_loads = if opt_size { 4 } else { 8 };
        Some(MemCmpExpansionOptions {
            load_sizes: vec![8, 4, 2, 1],
            max_num_loads,
            num_loads_per_block: if is_zero_cmp { max_num_loads } else { 1 },
            allow_overlapping_loads: true,
            allowed_tail_expansions: vec![3, 5, 6],
        })
    }

    /// `AArch64TargetLowering::allowsMisalignedMemoryAccesses`
    /// ([L3199](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L3199)):
    /// any scalar access, since the targets are not `+strict-align`.
    fn allows_misaligned_memory_accesses(
        &self,
        _bits: u32,
        _address_space: u32,
        _align: u64,
    ) -> bool {
        true
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

    fn vector_register_bits(&self) -> u32 {
        128
    }

    /// `AArch64TTIImpl::getNumberOfRegisters`: 32 NEON registers, 31 general.
    fn number_of_registers(
        &self,
        class: RegisterClass,
    ) -> u32 {
        match class {
            RegisterClass::Vector => 32,
            RegisterClass::Scalar => 31,
        }
    }

    /// `AArch64TTIImpl::getMaxInterleaveFactor`:
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L5951>
    fn max_interleave_factor(
        &self,
        vf: u32,
        has_unordered_reductions: bool,
    ) -> u32 {
        if vf == 1 || (has_unordered_reductions && vf <= 4) {
            return 4;
        }
        self.cpu.max_interleave_factor()
    }

    /// `AArch64TTIImpl::shouldMaximizeVectorBandwidth`: true for fixed-width
    /// NEON (L375).
    fn should_maximize_vector_bandwidth(&self) -> bool {
        true
    }

    fn vector_memory_op_cost(
        &self,
        op: MemoryOp,
        ty: VectorTy,
        align: u64,
        kind: CostKind,
    ) -> Option<Cost> {
        vector_memory_op_cost(self.cpu, op, ty, align, kind)
    }

    fn vector_arithmetic_cost(
        &self,
        op: ArithOp,
        ty: VectorTy,
        kind: CostKind,
    ) -> Option<Cost> {
        vector_arithmetic_cost(op, ty, kind)
    }

    fn vector_cast_cost(
        &self,
        kind: CastKind,
        dst: VectorTy,
        src: VectorTy,
        cost_kind: CostKind,
    ) -> Option<Cost> {
        crate::tti_vector::cast_cost(kind, dst, src, cost_kind)
    }

    fn vector_cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: VectorTy,
        predicate: Option<Predicate>,
        cost_kind: CostKind,
    ) -> Option<Cost> {
        crate::tti_vector::cmp_sel_cost(op, ty, predicate, cost_kind)
    }

    fn scalarization_overhead(
        &self,
        ty: VectorTy,
        insert: bool,
        extract: bool,
        cost_kind: CostKind,
    ) -> Option<Cost> {
        crate::tti_vector::scalarization_overhead(ty, insert, extract, cost_kind)
    }

    /// `AArch64TargetLowering::getJumpConditionMergingParams`: a base of
    /// `aarch64-br-merging-base-cost` 2 plus the `ccmp` bias 6, less the
    /// cbz/tbnz bias 6 where one side is already a single compare-and-branch
    /// and split where both are (or both compare a loaded value, or a side
    /// is a float compare). Likely bias 0, unlikely bias -1.
    fn jump_condition_merging_params(
        &self,
        query: &llrm_mir::jumpmerge::JumpMergeQuery,
    ) -> llrm_mir::jumpmerge::CondMerging {
        use llrm_mir::context::{ConstantKind, mask};
        use llrm_mir::jumpmerge::CondMerging;
        use llrm_mir::module::{Operand, ValueDef};
        use llrm_mir::opcode::{CastOp, IntPredicate, Opcode};
        const BIAS: i64 = 6;
        let (context, function) = (query.context, query.function);
        let def = |operand: Operand| match operand {
            Operand::Value(value) => match function.value(value).def {
                ValueDef::Instruction(inst) => Some(inst),
                _ => None,
            },
            _ => None,
        };
        let opcode = |operand: Operand| def(operand).map(|inst| function.instruction(inst).opcode.clone());
        if [query.lhs, query.rhs].iter().any(|&one| matches!(opcode(one), Some(Opcode::FCmp(_)))) {
            return CondMerging::SPLIT;
        }
        // An integer compare whose operand, under casts, is a load.
        let compares_loaded = |operand: Operand| {
            let Some(inst) = def(operand).filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::ICmp(_)))
            else {
                return false;
            };
            function
                .instruction(inst)
                .operands
                .iter()
                .any(
                    |&one| {
                        let mut at = one;
                        while let Some(inst) = def(at) {
                            match function.instruction(inst).opcode {
                                Opcode::Cast(_) => at = function.instruction(inst).operands[0],
                                Opcode::Load { .. } => return true,
                                _ => break,
                            }
                        }
                        false
                    },
                )
        };
        if compares_loaded(query.lhs) && compares_loaded(query.rhs) {
            return CondMerging::SPLIT;
        }
        let constant = |operand: Operand| match operand {
            Operand::Constant(id) => {
                let one = context.get(id);
                let bits = match one.kind {
                    ConstantKind::Int(bits) => Some(bits),
                    ConstantKind::Zero | ConstantKind::Null => Some(0),
                    _ => None,
                };
                bits.map(|bits| (bits, context.types.int_bits(one.ty).unwrap_or(0)))
            }
            _ => None,
        };
        // A condition that lowers to cbz/cbnz or tbz/tbnz alone.
        let fused = |operand: Operand| {
            let Some(inst) = def(operand) else { return false };
            let one = function.instruction(inst);
            match one.opcode {
                Opcode::Cast(CastOp::Trunc) => true,
                Opcode::ICmp(predicate) => match (predicate, constant(one.operands[1])) {
                    (IntPredicate::Eq | IntPredicate::Ne | IntPredicate::Slt, Some((0, _))) => true,
                    (IntPredicate::Sgt, Some((bits, width))) => width != 0 && bits == mask(width),
                    _ => false,
                },
                _ => false,
            }
        };
        let (left, right) = (fused(query.lhs), fused(query.rhs));
        if left && right {
            return CondMerging::SPLIT;
        }
        let base = 2 + BIAS - if left || right { BIAS } else { 0 };
        CondMerging { base_cost: base, likely_bias: 0, unlikely_bias: -1 }
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
        match intrinsic_price(self.cpu.name(), kind_name(kind), name) {
            Some(Some(cost)) => Cost::of(cost),
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

    /// `adrp` + `add` for each symbol base (see the trait).
    fn lsr_symbol_base_setup_insns(&self) -> u32 {
        2
    }

    fn is_legal_addressing_mode_at(
        &self,
        _addr_space: u32,
        access_bytes: u64,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        AArch64::is_legal_addressing_mode_for(access_bytes, base_gv, base_offset, has_base_reg, scale)
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

    /// `AArch64TTIImpl::getRegisterBitWidth(RGK_Scalar)`: 64.
    fn scalar_register_bits(&self) -> u32 {
        64
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
        self.scaling_factor_cost_at(ACCESS_BYTES, base_gv, base_offset, has_base_reg, scale)
    }

    fn scaling_factor_cost_at(
        &self,
        access_bytes: u64,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> Cost {
        if AArch64::is_legal_addressing_mode_for(access_bytes, base_gv, base_offset, has_base_reg, scale) {
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

impl Lowering for AArch64Lowering {
    // The register classes: GPR32 and GPR64 (AArch64ISelLowering.cpp,
    // addRegisterClass).
    fn is_type_legal(
        &self,
        bits: u32,
    ) -> bool {
        matches!(bits, 32 | 64)
    }

    // AArch64TargetMachine.cpp, `addCodeGenPrepare`: `TypePromotion` at -O1 and
    // up.
    fn runs_type_promotion(&self) -> bool {
        true
    }

    // `isOperationLegalOrCustom`, read from the table the DAG legalizer uses
    // (`TargetLowering::aarch64`, AArch64ISelLowering.cpp#L481): one table, so
    // the IR-level answer cannot drift from the legalizer's.
    fn is_operation_legal_or_custom(
        &self,
        op: Isd,
        bits: u32,
    ) -> bool {
        use llrm_sdag::actions::{Aarch64Features, TargetLowering};
        use llrm_sdag::isd::ISD;
        use llrm_sdag::mvt::EVT;
        let node = match op {
            Isd::Add => ISD::Add,
            Isd::Sub => ISD::Sub,
            Isd::Mul => ISD::Mul,
            Isd::UDiv => ISD::UDiv,
            Isd::SDiv => ISD::SDiv,
            Isd::URem => ISD::URem,
            Isd::SRem => ISD::SRem,
            Isd::Shl => ISD::Shl,
            Isd::Srl => ISD::Srl,
            Isd::Sra => ISD::Sra,
            Isd::And => ISD::And,
            Isd::Or => ISD::Or,
            Isd::Xor => ISD::Xor,
            Isd::Select => ISD::Select,
            Isd::SetCc => ISD::SetCC,
            Isd::Truncate => ISD::Truncate,
            Isd::ZeroExtend => ISD::ZeroExtend,
            Isd::SignExtend => ISD::SignExtend,
        };
        thread_local! {
            static TABLE: TargetLowering = TargetLowering::aarch64(Aarch64Features::default());
        }
        TABLE.with(|table| table.is_operation_legal_or_custom(node, EVT::integer(bits)))
    }

    // AArch64ISelLowering.cpp#L1274, `EnableExtLdPromotion = true`:
    // CodeGenPrepare promotes the operands of an extension towards the load
    // that feeds them (CodeGenPrepare.cpp#L6747):
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L1274
    fn enable_ext_ld_promotion(&self) -> bool {
        true
    }

    // AArch64ISelLowering.cpp#L1141, `setLoadExtAction(SEXTLOAD, VT, i1,
    // Expand)` for every integer type; the rest is the base class's
    // `Legal`:
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L1141
    fn is_load_ext_legal(
        &self,
        ext: LoadExt,
        value_bits: u32,
        memory_bits: u32,
    ) -> bool {
        !(ext == LoadExt::Sign && memory_bits == 1)
            && llrm_mir::lowering::simple_int(value_bits)
            && llrm_mir::lowering::simple_int(memory_bits)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L18942
    fn is_truncate_free(
        &self,
        from_bits: u32,
        to_bits: u32,
    ) -> bool {
        from_bits > to_bits
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L18984
    fn is_zext_free(
        &self,
        from_bits: u32,
        to_bits: u32,
    ) -> bool {
        from_bits == 32 && to_bits == 64
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L19014
    fn is_ext_free_impl(
        &self,
        users: &[ExtUser],
    ) -> bool {
        users
            .iter()
            .all(
                |user| match *user {
                    ExtUser::ConstShl | ExtUser::SameTypeTrunc => true,
                    // A shift of 1..=4 folds into the addressing mode.
                    ExtUser::Gep { store_bits } => (4..=7).contains(&store_bits.trailing_zeros()),
                    ExtUser::Other => false,
                },
            )
    }

    // The base class (TargetLoweringBase.cpp#L1813): the register type, i32 for
    // a narrower integer.
    fn preferred_switch_condition_bits(
        &self,
        bits: u32,
    ) -> u32 {
        bits.max(32)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.h#L339
    // Overflow ops for a check alone are cheap here: the base class's answer
    // with the math taken as used.
    fn should_form_overflow_op(
        &self,
        op: llrm_mir::lowering::OverflowOp,
        bits: u32,
        _math_used: bool,
    ) -> bool {
        op == llrm_mir::lowering::OverflowOp::UAdd && llrm_mir::lowering::simple_int(bits)
    }
}
