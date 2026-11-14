//! The loop vectorizer's planner: which vectorization factor (VF) and
//! interleave count (IC) a legal loop gets, and the plan the transform runs.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopVectorizationPlanner.cpp` and
//! `LoopVectorize.cpp`, in the pin's order. The plan is the loop's widenable
//! instructions ([`Recipe`]). The widest VF comes from the registers
//! (`getMaximizedVFForTarget`, L235: the register width over the widest type,
//! or over the smallest where the target maximizes bandwidth, clamped to the
//! safe dependence distance and the trip count). Each power of two up to it is
//! priced with the target's throughput costs, per lane; the cheapest wins and
//! a tie goes to the wider (`isMoreProfitable`, L709). The interleave count
//! (`selectInterleaveCount`, LoopVectorize.cpp#L3705) divides the registers by
//! the loop's register pressure, caps at the target's maximum and the trip
//! count, then interleaves small loops until the loop overhead is about 5 %.
//!
//! # Example
//! A copy of `i16`s over 1920 trips on a 128-bit target: widest VF `128 / 16`
//! = 8; VF 8 costs `(load 1 + store 1 + induction + branch) / 8` per lane
//! against 6 for the scalar loop; IC = 2 (`getMaxInterleaveFactor` 2, the
//! trip count allows 120 trips of 16).
//!
//! # DEVIATE rows
//! - VP1 the plan is one flat list of recipes of the loop's single block: no
//!   VPlan CFG or regions, so no VPlan transforms (`optimizeForVFAndUF`, tail
//!   folding, epilogue vectorization). Exit: the VPlan data structure.
//! - VP2 recipes are the memory accesses, integer binary operations, integer
//!   casts, and the loop-control instructions; every other instruction makes
//!   the loop unplannable (the pin widens or replicates it). Exit: one recipe
//!   kind per row of the card (reductions, floating point, select, compare).
//! - VP3 the remainder is a scalar loop only (`CM_EpilogueAllowed` with a
//!   scalar epilogue): the pin also vectorizes it (`vec.epilog.*`, the epilogue
//!   VF of `EpilogueVectorizationMinVF`), which the plan does not price. Exit:
//!   epilogue vectorization (VEC3).
//! - VP4 the scalar cost is the recipe prices at VF 1, not the legacy
//!   `CM->expectedCost`; the two agree for this recipe set (the loop-control
//!   instructions are priced alike).
//! - VP5 register pressure (`calculateRegisterUsageForPlan`) is the live vector
//!   values of the one block, not the interval walk over a VPlan;
//!   loop-invariant registers are the splatted operands.
//! - VP6 `-Os`/`-Oz` and a constant trip count under 16
//!   (`TinyTripCountVectorThreshold`): no epilogue is allowed there, so only a
//!   trip count VF x IC divides is planned (`CM_EpilogueNotAllowedOptSize`,
//!   `CM_EpilogueNotAllowedLowTripLoop`); IC stays 1 there (L3705). The pin's
//!   other low-trip branches (`MaxVFForTC`, trip = a power of two + 1) and the
//!   divisibility test against the largest VF before any smaller one are not
//!   translated: the divisibility is tested on the chosen VF x IC. Exit: VEC3
//!   (LV-X3, 2026-11-20).
//! - VP8 closed: the loop-control prices are the pin's. The scalar loop pays
//!   its `add` and exit `icmp` (the phis, the branch and the address
//!   computations are free at throughput), the vector loop its
//!   `branch-on-count` compare, its canonical `add` and, where an access is
//!   scalarized, one `SCALAR-STEPS` add (`LV: Found an estimated cost of N for
//!   VF 1` and `Cost of N for VF k` of the pin's `-debug-only=loop-vectorize`).
//! - VP7 no hints (LV-D7 of the legality translation), no scalable VFs (NEON
//!   only). No tail folding: `preferTailFoldingOverEpilogue` is false without
//!   SVE
//!   ([L7378](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L7378)),
//!   so the pin's NEON loops end in an epilogue, as here.

use llrm_mir::tti::{
    ArithOp, CastKind, CmpSel, Cost, CostKind, ExtOperand, ExtUser, MemoryOp, OperandInfo, Predicate, RegisterClass,
    Tti, Ty, VectorTy,
};

/// One widenable instruction of the loop, by what its cost and widening need.
#[derive(Clone, Debug, PartialEq)]
pub enum Recipe {
    /// A load of `lane` bits with consecutive addresses (stride 1), aligned to
    /// `align` bytes.
    Load { lane: Ty, align: u64 },
    /// A store, as above.
    Store { lane: Ty, align: u64 },
    /// A binary operation of `lane`, integer or floating point; `ext`: which
    /// operands are a `sext` or `zext` (the pin's `Args`: the widening
    /// forms price on them).
    Binary { op: ArithOp, lane: Ty, ext: [Option<ExtOperand>; 2], second: OperandInfo },
    /// `llvm.fmuladd` of `lane`s (`VPWidenIntrinsicRecipe`): AArch64 prices it
    /// as one `fmul` (`getIntrinsicInstrCost`, L797).
    FMulAdd { lane: Ty },
    /// An access whose addresses are not consecutive and not a pattern the
    /// pin widens (an interleave group, a reversal, a uniform address): one
    /// scalar access per lane (`CM_Scalarize`), a load's lanes put together
    /// by inserts, a store's taken apart by extracts.
    Scalarized { op: MemoryOp, lane: Ty },
    /// A cast; `folded`: the scalar one is part of the load it extends (LVI1);
    /// `users`: what reads an extension, which may absorb it
    /// (`GetUserAbsorbedCastCost`).
    Cast { kind: CastKind, dst: Ty, src: Ty, folded: bool, users: Vec<ExtUser> },
    /// A compare of `lane` operands.
    Compare { op: CmpSel, lane: Ty, predicate: Predicate },
    /// A select of `lane` values by the compare of `predicate`, if the loop
    /// makes the condition.
    Select { lane: Ty, predicate: Option<Predicate> },
    /// A widened induction (`VPWidenIntOrFpInductionRecipe`): a vector phi of
    /// `lane`s that steps by VF each iteration, priced as the vector `add`.
    /// `truncated_from`: the scalar loop's `trunc` of the integer induction
    /// (the recipe replaces it).
    Induction { lane: Ty, truncated_from: Option<Ty> },
    /// `VPDerivedIVRecipe`: the address induction `start + index * step` of a
    /// loop whose induction does not start at zero or counts down; one `add`
    /// of `lane` (the scalar loop has none: its phi is free).
    DerivedIv { lane: Ty },
    /// An instruction the pin prices as a vector and this port does not (a
    /// uniform, reversed or grouped access, a cast without a table row): its
    /// scalar price is known, a vector factor cannot be weighed.
    Unpriced { scalar_cost: i64 },
}

/// The scalar loop's control: the `add` that steps the induction and the
/// `icmp` that tests it; the phis and the branch are free at throughput.
pub(crate) fn scalar_control_cost(tti: &dyn Tti) -> i64 {
    let kind = CostKind::RecipThroughput;
    tti.arithmetic_cost(ArithOp::Add, Ty::Int(64), kind, Default::default(), Default::default()).value
        + tti.cmp_sel_cost(CmpSel::ICmp, Ty::Int(64), kind).value
        + tti.cf_instr_cost(true, kind).value
        + tti.cf_instr_cost(false, kind).value
}

/// The vector loop's control per iteration: `branch-on-count` (a compare and a
/// branch), the canonical induction's `add`, and, when an access is
/// scalarized, the `SCALAR-STEPS` add that numbers its lanes.
fn vector_control_cost(
    tti: &dyn Tti,
    scalarized: bool,
) -> i64 {
    let kind = CostKind::RecipThroughput;
    let add = tti.arithmetic_cost(ArithOp::Add, Ty::Int(64), kind, Default::default(), Default::default()).value;
    let steps = if scalarized { add } else { 0 };
    tti.cmp_sel_cost(CmpSel::ICmp, Ty::Int(64), kind).value + tti.cf_instr_cost(false, kind).value + add + steps
}

/// `LoopVectorizationCostModel::SmallLoopCost`: interleave until the loop
/// overhead is about 5 %.
const SMALL_LOOP_COST: i64 = 20;

/// The loop to plan.
#[derive(Clone, Debug)]
pub struct LoopFacts {
    pub recipes: Vec<Recipe>,
    /// The exact trip count (`getSmallConstantTripCount`), 0 when it is not
    /// constant.
    pub trip_count: u64,
    /// `getMaxSafeVectorWidthInBits` of the dependence checker.
    pub max_safe_width_in_bits: u64,
    /// `isSafeForAnyVectorWidth`: no dependence bounds the width; otherwise IC
    /// stays 1 (`selectInterleaveCount` uses the distance for the VF
    /// alone).
    pub safe_for_any_vector_width: bool,
    /// `getMaxStoreLoadForwardSafeDistanceInBits`, unless
    /// `isSafeForAnyStoreLoadForwardDistances`.
    pub store_load_forward_bits: Option<u64>,
    /// The widest induction type's width, `getWidestInductionType`.
    pub widest_induction_bits: u32,
    /// The function is `optsize` or `minsize` (VP6).
    pub opt_size: bool,
    /// The loop carries reductions (`!Legal->getReductionVars().empty()`).
    pub has_reductions: bool,
}

/// The chosen factors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Plan {
    pub vf: u32,
    pub ic: u32,
}

/// Why a loop is not planned (`LV: Not vectorizing: ...`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NotPlanned(pub &'static str);

fn lane_bits(ty: Ty) -> u32 {
    match ty {
        Ty::Int(bits) => bits,
        Ty::Float => 32,
        Ty::Double | Ty::Pointer => 64,
        Ty::X87 => 80,
    }
}

/// The lane types of a recipe's vectors: its result's, and a conversion's
/// source.
fn recipe_lanes(recipe: &Recipe) -> impl Iterator<Item = Ty> {
    let source = match *recipe {
        Recipe::Cast { src, .. } => Some(src),
        _ => None,
    };
    recipe_lane(recipe).into_iter().chain(source)
}

fn recipe_lane(recipe: &Recipe) -> Option<Ty> {
    match *recipe {
        Recipe::Load { lane, .. }
        | Recipe::Store { lane, .. }
        | Recipe::Binary { lane, .. }
        | Recipe::FMulAdd { lane }
        | Recipe::Scalarized { lane, .. }
        | Recipe::Compare { lane, .. }
        | Recipe::Select { lane, .. }
        | Recipe::Induction { lane, .. } => Some(lane),
        Recipe::Cast { dst, .. } => Some(dst),
        Recipe::DerivedIv { .. } | Recipe::Unpriced { .. } => None,
    }
}

/// `getSmallestAndWidestTypes` (L513): the loads' and stores' lane widths; the
/// widest starts at 8.
fn smallest_and_widest(recipes: &[Recipe]) -> (u32, u32) {
    let (mut smallest, mut widest) = (u32::MAX, 8);
    for recipe in recipes {
        if matches!(
            recipe,
            Recipe::Load { .. } | Recipe::Store { .. } | Recipe::Scalarized { .. }
        ) {
            let bits = lane_bits(recipe_lane(recipe).expect("an access has a lane"));
            smallest = smallest.min(bits);
            widest = widest.max(bits);
        }
    }
    (if smallest == u32::MAX { widest } else { smallest }, widest)
}

/// `bit_floor`.
fn bit_floor(value: u64) -> u64 {
    if value == 0 { 0 } else { 1 << (63 - value.leading_zeros()) }
}

/// `clampVFByMaxTripCount` (L360 region): no VF past the largest power of two
/// within the trip count / IC.
fn clamp_vf_by_max_trip_count(
    vf: u64,
    max_trip_count: u64,
) -> u64 {
    if max_trip_count != 0 && max_trip_count <= vf {
        return bit_floor(max_trip_count).max(1);
    }
    vf
}

/// `getMaximizedVFForTarget` (L235), fixed width.
fn maximized_vf_for_target(
    tti: &dyn Tti,
    max_trip_count: u64,
    smallest: u32,
    widest: u32,
    max_safe_vf: u64,
) -> u64 {
    let register_bits = u64::from(tti.vector_register_bits());
    let max_vector = bit_floor(register_bits / u64::from(widest)).min(max_safe_vf);
    if max_vector == 0 {
        return 1;
    }
    let max_vf = clamp_vf_by_max_trip_count(max_vector, max_trip_count);
    // If the MaxVF was already clamped, there is no point in trying a larger
    // one.
    if max_vf != max_vector {
        return max_vf;
    }
    if tti.should_maximize_vector_bandwidth() {
        let max_bandwidth = bit_floor(register_bits / u64::from(smallest)).min(max_safe_vf);
        return clamp_vf_by_max_trip_count(max_bandwidth, max_trip_count);
    }
    max_vf
}

/// Why a vector factor has no price: the target prices no vector for a recipe
/// the pin would scalarize, so the factor is not taken.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NotTaken;

/// The price of one iteration of the loop at `vf`, the recipes plus the loop
/// control. A recipe the pin prices as a vector and this port does not counts
/// nothing when `optimistic`: the sum is then a lower bound of the pin's, so a
/// loop whose bound is not beneficial is one the pin does not vectorize.
fn iteration_cost(
    tti: &dyn Tti,
    recipes: &[Recipe],
    vf: u32,
    backend_limits: bool,
    optimistic: bool,
) -> Result<i64, NotTaken> {
    let kind = CostKind::RecipThroughput;
    let scalarized = recipes.iter().any(|recipe| matches!(recipe, Recipe::Scalarized { .. }));
    let mut total = if vf == 1 { scalar_control_cost(tti) } else { vector_control_cost(tti, scalarized) };
    // DEVIATE LV-SPLIT: the pin prices a vector narrower than one register as
    // its widened register (`WidenVectorResult`, the D-register types), which
    // the backends here do not do yet, so a VF is taken only when every
    // recipe's vectors (a conversion's source too) are one register or two
    // (`SplitVectorResult` of a 256-bit type). Integer vectors of half a
    // register (the D registers) and of four registers are taken too: those
    // MVTs exist. Owner llrm-arm64-vec, exit LV-X4.
    let register_bits = u64::from(tti.vector_register_bits());
    let taken = |lane: Ty| {
        let bits = u64::from(lane_bits(lane)) * u64::from(vf);
        [register_bits, 2 * register_bits].contains(&bits)
            || (matches!(lane, Ty::Int(_)) && [register_bits / 2, 4 * register_bits].contains(&bits))
    };
    if backend_limits && vf > 1 && !recipes.iter().flat_map(recipe_lanes).all(taken) {
        return Err(NotTaken);
    }
    let vector = |ty: Ty| VectorTy { lane: ty, count: vf };
    let address = tti.address_computation_cost(kind).value;
    // A vector price this port lacks: nothing in a lower bound, else the factor
    // has no price.
    let unknown = |priced: Option<Cost>| priced.or(optimistic.then_some(Cost::FREE)).ok_or(NotTaken);
    for recipe in recipes {
        let cost = match *recipe {
            Recipe::Load { lane, align } | Recipe::Store { lane, align } => {
                let op = if matches!(recipe, Recipe::Load { .. }) { MemoryOp::Load } else { MemoryOp::Store };
                if vf == 1 {
                    let access = tti.memory_op_cost(op, lane, kind, false);
                    Cost { value: access.value + address, valid: access.valid }
                } else {
                    tti.vector_memory_op_cost(op, vector(lane), align, kind).ok_or(NotTaken)?
                }
            }
            Recipe::FMulAdd { lane } => {
                if vf == 1 {
                    tti.arithmetic_cost(ArithOp::FMul, lane, kind, Default::default(), Default::default())
                } else {
                    unknown(tti.vector_arithmetic_cost(ArithOp::FMul, vector(lane), kind))?
                }
            }
            Recipe::Binary { op, lane, ext, second } => {
                if vf == 1 {
                    tti.arithmetic_cost(op, lane, kind, Default::default(), second)
                } else if ext.iter().any(Option::is_some) {
                    // The widening forms; `Err`: a price that needs known bits.
                    match tti.vector_arithmetic_cost_ext(op, vector(lane), ext, kind) {
                        Ok(priced) => unknown(priced)?,
                        Err(()) => unknown(None)?,
                    }
                } else if lane.is_float() {
                    // The pin may price a floating-point operation some other
                    // way.
                    unknown(tti.vector_arithmetic_cost(op, vector(lane), kind))?
                } else {
                    // An integer operation the target does not price is the
                    // pin's scalarized one (a multiply of
                    // `<2 x i64>`).
                    tti.vector_arithmetic_cost_op2(op, vector(lane), second, kind).ok_or(NotTaken)?
                }
            }
            Recipe::Scalarized { op, lane } => {
                let access = tti.memory_op_cost(op, lane, kind, false);
                if vf == 1 {
                    Cost { value: access.value + address, valid: access.valid }
                } else {
                    let overhead = unknown(tti.scalarization_overhead(
                        vector(lane),
                        op == MemoryOp::Load,
                        op == MemoryOp::Store,
                        kind,
                    ))?;
                    Cost { value: i64::from(vf) * (access.value + address) + overhead.value, valid: access.valid }
                }
            }
            Recipe::Cast { kind: cast, dst, src, folded, ref users } => {
                if vf == 1 {
                    if folded { Cost::FREE } else { tti.cast_cost(cast, dst, src, kind) }
                } else if matches!(cast, CastKind::SExt | CastKind::ZExt) {
                    unknown(tti.vector_ext_cost(cast, vector(dst), vector(src), users, kind))?
                } else {
                    unknown(tti.vector_cast_cost(cast, vector(dst), vector(src), kind))?
                }
            }
            Recipe::Compare { op, lane, predicate } => {
                if vf == 1 {
                    tti.cmp_sel_cost_for(op, lane, kind, Some(predicate))
                } else {
                    unknown(tti.vector_cmp_sel_cost(op, vector(lane), Some(predicate), kind))?
                }
            }
            Recipe::Select { lane, predicate } => {
                if vf == 1 {
                    tti.cmp_sel_cost(CmpSel::Select, lane, kind)
                } else {
                    unknown(tti.vector_cmp_sel_cost(CmpSel::Select, vector(lane), predicate, kind))?
                }
            }
            // `VPWidenIntOrFpInductionRecipe::computeCost`: the phi is free, the
            // step an `add` of the vector.
            Recipe::Induction { lane, truncated_from } => {
                if vf == 1 {
                    truncated_from.map_or(Cost::FREE, |src| tti.cast_cost(CastKind::Trunc, lane, src, kind))
                } else {
                    tti.vector_arithmetic_cost(ArithOp::Add, vector(lane), kind).ok_or(NotTaken)?
                }
            }
            Recipe::DerivedIv { lane } => {
                if vf == 1 {
                    Cost::FREE
                } else {
                    tti.arithmetic_cost(ArithOp::Add, lane, kind, Default::default(), Default::default())
                }
            }
            Recipe::Unpriced { scalar_cost } => {
                if vf == 1 {
                    Cost::of(scalar_cost)
                } else {
                    unknown(None)?
                }
            }
        };
        if !cost.valid {
            return Err(NotTaken);
        }
        total += cost.value;
    }
    Ok(total)
}

/// What `-debug-only=loop-vectorize` prints of the factors tried: the scalar
/// loop's price and each vector factor's lower bound (`-` for one not taken).
pub fn describe_costs(
    tti: &dyn Tti,
    facts: &LoopFacts,
) -> String {
    let mut text = format!("scalar {}", iteration_cost(tti, &facts.recipes, 1, false, false).unwrap_or(-1));
    let mut vf = 2u64;
    while vf <= max_vf_of(tti, facts) {
        match iteration_cost(tti, &facts.recipes, vf as u32, false, true) {
            Ok(cost) => text += &format!(", VF {vf} >= {cost}"),
            Err(NotTaken) => text += &format!(", VF {vf} -"),
        }
        vf *= 2;
    }
    text
}

/// `expectedCost(VF=1)`: the scalar loop's price.
pub(crate) fn scalar_loop_cost(
    tti: &dyn Tti,
    recipes: &[Recipe],
) -> i64 {
    iteration_cost(tti, recipes, 1, false, false).expect("the scalar loop is priced")
}

/// `isMoreProfitable` (L709) for fixed VFs, the throughput kind: `a` against
/// `b` by the cost per lane, then, for a known trip count, by the cost of the
/// whole loop; a tie is not better.
fn is_more_profitable(
    (cost_a, vf_a, scalar_a): (i64, u64, i64),
    (cost_b, vf_b, scalar_b): (i64, u64, i64),
    trip_count: u64,
    has_tail: bool,
) -> bool {
    // (CostA / WidthA) < (CostB / WidthB), without the division.
    let lower_without_trip_count = cost_a * (vf_b as i64) < cost_b * (vf_a as i64);
    if trip_count == 0 {
        return lower_without_trip_count;
    }
    let for_trip_count = |vf: u64, vector: i64, scalar: i64| -> i64 {
        if has_tail {
            vector * (trip_count / vf) as i64 + scalar * (trip_count % vf) as i64
        } else {
            vector * trip_count.div_ceil(vf) as i64
        }
    };
    for_trip_count(vf_a, cost_a, scalar_a) < for_trip_count(vf_b, cost_b, scalar_b)
}

/// `computeBestVF` (L5640): the cheapest VF in `1, 2, 4 .. max_vf`.
fn best_vf(
    tti: &dyn Tti,
    facts: &LoopFacts,
    max_vf: u64,
    backend_limits: bool,
    optimistic: bool,
) -> (u32, i64) {
    let scalar = iteration_cost(tti, &facts.recipes, 1, backend_limits, false).expect("the scalar loop is priced");
    let mut best = (scalar, 1u64, scalar);
    let mut vf = 2u64;
    while vf <= max_vf {
        if let Ok(cost) = iteration_cost(tti, &facts.recipes, vf as u32, backend_limits, optimistic) {
            let candidate = (cost, vf, scalar);
            if is_more_profitable(candidate, best, facts.trip_count, true) {
                best = candidate;
            }
        }
        vf *= 2;
    }
    (best.1 as u32, best.0)
}

/// The loop's register pressure at `vf`, for `selectInterleaveCount` (VP5): the
/// vector values live at once in the block, one per widened load and binary
/// operation, and the splatted operands as the loop-invariant ones.
fn max_local_users(recipes: &[Recipe]) -> u32 {
    let values = recipes.iter().filter(|recipe| !matches!(recipe, Recipe::Store { .. })).count() as u32;
    values.max(1)
}

/// What `selectInterleaveCount` reads of the loop and its plan.
#[derive(Clone, Copy, Debug)]
struct IcInputs {
    epilogue_allowed: bool,
    safe_for_any_vector_width: bool,
    trip_count: u64,
    /// `MaxLocalUsers` of the register class the loop's values live in.
    users: u32,
    /// `LoopInvariantRegs`: values defined outside the loop and read in it.
    invariants: u32,
    loads: u32,
    stores: u32,
    has_reductions: bool,
    /// The loop is inside another (`OrigLoop->getLoopDepth() > 1`).
    nested: bool,
}

/// `selectInterleaveCount` (LoopVectorize.cpp#L3705) of the vector plan, which
/// (VP5 prices its register pressure by the values).
fn select_interleave_count(
    tti: &dyn Tti,
    facts: &LoopFacts,
    vf: u32,
    loop_cost: i64,
) -> u32 {
    let values = max_local_users(&facts.recipes);
    let inputs = IcInputs {
        epilogue_allowed: epilogue_allowed(facts),
        safe_for_any_vector_width: facts.safe_for_any_vector_width,
        trip_count: facts.trip_count,
        users: values,
        invariants: 0,
        loads: facts.recipes.iter().filter(|recipe| matches!(recipe, Recipe::Load { .. })).count() as u32,
        stores: facts.recipes.iter().filter(|recipe| matches!(recipe, Recipe::Store { .. })).count() as u32,
        has_reductions: facts.has_reductions,
        nested: false,
    };
    interleave_count(tti, &inputs, vf, loop_cost)
}

/// `selectInterleaveCount` (LoopVectorize.cpp#L3705).
fn interleave_count(
    tti: &dyn Tti,
    facts: &IcInputs,
    vf: u32,
    loop_cost: i64,
) -> u32 {
    // Do not interleave where no epilogue is allowed (optsize or a tiny trip
    // count, VP6), where the distance already bounded the VF ("We used the
    // distance for the interleave count"), and a loop that is free needs
    // none.
    if !facts.epilogue_allowed || !facts.safe_for_any_vector_width || loop_cost == 0 {
        return 1;
    }
    let registers = tti.number_of_registers(if vf == 1 { RegisterClass::Scalar } else { RegisterClass::Vector });
    let users = facts.users.max(1);
    let mut ic = bit_floor(u64::from(registers.saturating_sub(facts.invariants) / users)).max(1) as u32;

    let mut max_interleave = tti.max_interleave_factor(vf, facts.has_reductions);
    // The exact trip count bounds the interleave count (the pin's
    // `InterleaveCountLB` and `UB`).
    if facts.trip_count != 0 {
        let available = facts.trip_count;
        let lower = bit_floor((available / (u64::from(vf) * 2)).min(u64::from(max_interleave)).max(1)) as u32;
        let upper = bit_floor((available / u64::from(vf)).min(u64::from(max_interleave)).max(1)) as u32;
        max_interleave = lower;
        if upper != lower {
            let tail_upper = available % (u64::from(vf) * u64::from(upper));
            let tail_lower = available % (u64::from(vf) * u64::from(lower));
            if tail_upper == tail_lower {
                max_interleave = upper;
            }
        }
    }
    ic = ic.min(max_interleave).max(1);

    // Interleave if we vectorized this loop and there is a reduction that
    // could benefit from interleaving.
    if vf > 1 && facts.has_reductions {
        return ic;
    }

    // We want to interleave small loops in order to reduce the loop overhead
    // and expose ILP.
    if loop_cost < SMALL_LOOP_COST {
        // Interleave until the loop overhead is about 5 % of the cost of the
        // loop.
        let mut small_ic = ic.min(bit_floor((SMALL_LOOP_COST / loop_cost) as u64) as u32);
        // Interleave until the store and load ports (estimated by the max
        // interleave count) are saturated.
        let mut stores_ic = ic / facts.stores.max(1);
        let mut loads_ic = ic / facts.loads.max(1);
        // A scalar reduction inside another loop lengthens the critical path:
        // tree-wise reductions are capped (`MaxNestedScalarReductionIC`).
        if facts.has_reductions && facts.nested {
            small_ic = small_ic.min(MAX_NESTED_SCALAR_REDUCTION_IC);
            stores_ic = stores_ic.min(MAX_NESTED_SCALAR_REDUCTION_IC);
            loads_ic = loads_ic.min(MAX_NESTED_SCALAR_REDUCTION_IC);
        }
        // `EnableLoadStoreRuntimeInterleave` is on by default.
        if stores_ic.max(loads_ic) > small_ic {
            return stores_ic.max(loads_ic);
        }
        // A scalar VF with aggressive interleaving: "to expose ILP".
        if vf == 1 && tti.enable_aggressive_interleaving() {
            return (ic / 2).max(small_ic);
        }
        // "Interleaving to reduce branch cost."
        return small_ic;
    }
    // A large loop that could benefit from interleaving.
    if tti.enable_aggressive_interleaving() {
        return ic;
    }
    1
}

/// `MaxNestedScalarReductionIC`.
const MAX_NESTED_SCALAR_REDUCTION_IC: u32 = 2;

/// A loop that is interleaved and not vectorized: `computeBestVF` chose VF 1.
#[derive(Clone, Copy, Debug)]
pub struct ScalarFacts {
    /// `expectedCost(VF=1)`: the body's instructions and the loop control.
    pub cost: i64,
    pub loads: u32,
    pub stores: u32,
    /// `MaxLocalUsers` and `LoopInvariantRegs` of `calculateRegisterUsage`.
    pub users: u32,
    pub invariants: u32,
    pub has_reductions: bool,
    pub nested: bool,
    pub trip_count: u64,
    pub safe_for_any_vector_width: bool,
    pub opt_size: bool,
}

/// The interleave count of a loop kept scalar (`InterleaveLoop` without
/// `VectorizeLoop` in `processLoop`, L7882), or why not. Where no epilogue is
/// allowed the count must divide the trip count.
pub fn choose_interleave(
    tti: &dyn Tti,
    facts: &ScalarFacts,
) -> Result<u32, NotPlanned> {
    if facts.trip_count == 1 {
        return Err(NotPlanned("Single iteration (non) loop"));
    }
    let allowed = !facts.opt_size && !(facts.trip_count != 0 && facts.trip_count < TINY_TRIP_COUNT);
    let inputs = IcInputs {
        epilogue_allowed: allowed,
        safe_for_any_vector_width: facts.safe_for_any_vector_width,
        trip_count: facts.trip_count,
        users: facts.users,
        invariants: facts.invariants,
        loads: facts.loads,
        stores: facts.stores,
        has_reductions: facts.has_reductions,
        nested: facts.nested,
    };
    let ic = interleave_count(tti, &inputs, 1, facts.cost);
    if ic <= 1 {
        return Err(NotPlanned("The cost-model indicates that interleaving is not beneficial"));
    }
    if !allowed && (facts.trip_count == 0 || facts.trip_count % u64::from(ic) != 0) {
        return Err(NotPlanned("A scalar epilogue would remain"));
    }
    Ok(ic)
}

/// The factors the pin chooses for `facts` on `tti` (`computeBestVF`,
/// `selectInterleaveCount`), or why not, before the epilogue is considered.
pub fn choose(
    tti: &dyn Tti,
    facts: &LoopFacts,
) -> Result<Plan, NotPlanned> {
    choose_with(tti, facts, true, false)
}

/// [`choose`] as the pin would, with no limit of the backend's vector types
/// (LV-SPLIT): whether the pin may vectorize a loop of these recipes. A
/// division the target does not price as a vector counts as a possible one
/// (LVI4): the recipe carries no divisor, and the pin takes a vector divided by
/// a constant at a few multiplies. A multiply the target does not price is the
/// pin's scalarized one, which keeps the loop scalar.
pub fn pin_may_vectorize(
    tti: &dyn Tti,
    facts: &LoopFacts,
) -> bool {
    if choose_with(tti, facts, false, true).is_ok() {
        return true;
    }
    if tti.vector_register_bits() == 0 || facts.trip_count == 1 {
        return false;
    }
    let register_bits = tti.vector_register_bits();
    facts
        .recipes
        .iter()
        .any(
            |recipe| match *recipe {
                Recipe::Binary { op, lane, .. } => {
                    let count = register_bits / lane_bits(lane).max(1);
                    matches!(
                        op,
                        ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem
                    )
                        && count >= 2
                        && tti.vector_arithmetic_cost(op, VectorTy { lane, count }, CostKind::RecipThroughput).is_none()
                }
                _ => false,
            },
        )
}

/// `getMaximizedVFForTarget` of the widest and smallest types and the safe
/// width.
fn max_vf_of(
    tti: &dyn Tti,
    facts: &LoopFacts,
) -> u64 {
    let (smallest, widest) = smallest_and_widest(&facts.recipes);
    // MaxSafeElementsPowerOf2 = bit_floor(MaxSafeVectorWidthInBits /
    // WidestType).
    let mut max_safe = bit_floor(facts.max_safe_width_in_bits / u64::from(widest));
    if let Some(distance) = facts.store_load_forward_bits {
        max_safe = max_safe.min(distance / u64::from(widest));
    }
    maximized_vf_for_target(tti, facts.trip_count, smallest, widest, max_safe)
}

fn choose_with(
    tti: &dyn Tti,
    facts: &LoopFacts,
    backend_limits: bool,
    optimistic: bool,
) -> Result<Plan, NotPlanned> {
    if tti.vector_register_bits() == 0 {
        return Err(NotPlanned("The target has no vector registers"));
    }
    if facts.trip_count == 1 {
        return Err(NotPlanned("Single iteration (non) loop"));
    }
    let max_vf = max_vf_of(tti, facts);
    let (vf, cost) = best_vf(tti, facts, max_vf, backend_limits, optimistic);
    if vf == 1 {
        return Err(NotPlanned("Vectorization is possible but not beneficial"));
    }
    Ok(Plan { vf, ic: select_interleave_count(tti, facts, vf, cost) })
}

/// The constant trip count under which the pin allows no epilogue
/// (`TinyTripCountVectorThreshold`, L7809).
const TINY_TRIP_COUNT: u64 = 16;

/// `getEpilogueLowering`'s `CM_EpilogueAllowed` against
/// `CM_EpilogueNotAllowedOptSize` (L7076) and
/// `CM_EpilogueNotAllowedLowTripLoop` (L7809).
fn epilogue_allowed(facts: &LoopFacts) -> bool {
    !facts.opt_size && !(facts.trip_count != 0 && facts.trip_count < TINY_TRIP_COUNT)
}

/// [`choose`], then the epilogue policy: where none is allowed the plan must
/// cover the trip count whole.
pub fn plan(
    tti: &dyn Tti,
    facts: &LoopFacts,
) -> Result<Plan, NotPlanned> {
    let chosen = choose(tti, facts)?;
    let covers = facts.trip_count != 0 && facts.trip_count % (u64::from(chosen.vf) * u64::from(chosen.ic)) == 0;
    if !covers && !epilogue_allowed(facts) {
        return Err(NotPlanned("A scalar epilogue would remain"));
    }
    Ok(chosen)
}

#[cfg(test)]
#[path = "loopvectorizeplan_tests.rs"]
mod tests;
