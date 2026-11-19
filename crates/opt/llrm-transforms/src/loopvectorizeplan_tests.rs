//! `choose` against the pinned LLVM's `opt -passes=loop-vectorize` remark
//! ("vectorization width: W, interleaved count: I") on aarch64, for copy and
//! add loops of each element width and several trip counts (the rows below are
//! that remark).

use llrm_arm64::tti::AArch64;
use llrm_mir::opcode::FloatPredicate;
use llrm_mir::tti::{ArithOp, CastKind, CmpSel, MemoryOp, Predicate, Ty};

use super::{LoopFacts, Plan, Recipe, choose, epilogue_vf, iteration_cost, pin_may_vectorize, plan};

fn facts(
    lane_bits: u32,
    trip_count: u64,
    add: bool,
) -> LoopFacts {
    let lane = Ty::Int(lane_bits);
    let align = u64::from(lane_bits / 8);
    let mut recipes = vec![Recipe::Load { lane, align }];
    if add {
        recipes.push(Recipe::Binary { op: ArithOp::Add, lane, ext: [None, None], second: Default::default() });
    }
    recipes.push(Recipe::Store { lane, align });
    LoopFacts {
        recipes,
        trip_count,
        max_safe_width_in_bits: 1 << 40,
        safe_for_any_vector_width: true,
        store_load_forward_bits: None,
        widest_induction_bits: 64,
        opt_size: false,
        very_cheap: false,
        has_reductions: false,
        size_priced: false,
        reduction_bits: Vec::new(),
        pressure: None,
    }
}

/// (element bits, trip count, VF, IC) of the pin, for the copy and for the add
/// form alike.
const PIN: &[(u32, u64, u32, u32)] = &[
    (8, 1920, 16, 2),
    (16, 1920, 8, 2),
    (32, 1920, 4, 2),
    (64, 1920, 2, 2),
    (16, 64, 8, 2),
    (16, 48, 8, 2),
    (16, 16, 8, 2),
    (16, 2000, 8, 2),
    (32, 100, 4, 2),
    (16, 1000, 8, 2),
    // A remainder (a scalar epilogue) or no known trip count (0): pin rows of
    // the same copy and add loops.
    (8, 0, 16, 2),
    (8, 16, 16, 1),
    (8, 17, 16, 1),
    (8, 33, 16, 2),
    (8, 100, 16, 2),
    (8, 1921, 16, 2),
    (16, 0, 8, 2),
    (16, 17, 8, 2),
    (16, 33, 8, 2),
    (16, 100, 8, 2),
    (16, 1921, 8, 2),
    (32, 0, 4, 2),
    (32, 17, 4, 2),
    (32, 33, 4, 2),
    (32, 1921, 4, 2),
    (64, 0, 2, 2),
    (64, 17, 2, 2),
    (64, 33, 2, 2),
    (64, 100, 2, 2),
    (64, 1921, 2, 2),
];

#[test]
fn the_factors_are_the_pins() {
    let tti = AArch64::of_cpu("generic").unwrap();
    for &(bits, trips, vf, ic) in PIN {
        for add in [false, true] {
            assert_eq!(choose(&tti, &facts(bits, trips, add)), Ok(Plan { vf, ic }), "i{bits} x{trips} add={add}");
        }
    }
}

/// The pin vectorizes a loop whose trip count VF x IC does not divide, or that
/// is not constant, with a scalar epilogue; this planner refused both ("A
/// scalar epilogue would remain", "The trip count is not a known constant").
#[test]
fn a_remainder_or_an_unknown_trip_count_is_planned() {
    let tti = AArch64::of_cpu("generic").unwrap();
    for trips in [0, 1000, 1921] {
        assert_eq!(plan(&tti, &facts(16, trips, false)), Ok(Plan { vf: 8, ic: 2 }), "trips {trips}");
    }
    assert_eq!(plan(&tti, &facts(32, 100, true)), Ok(Plan { vf: 4, ic: 2 }));
}

/// No epilogue under `-Os` (VP6) or for a constant trip count under 16: the
/// pin does not vectorize i16 x 15 (remark-free), nor an `optsize` loop it
/// would leave a remainder on.
#[test]
fn no_epilogue_is_planned_where_the_pin_allows_none() {
    let tti = AArch64::of_cpu("generic").unwrap();
    for bits in [8, 16, 32, 64] {
        for trips in [3, 15] {
            // The clamped VF (trip count 3 or 15) fills no register: LV-SPLIT
            // refuses it before the epilogue rule can.
            assert!(plan(&tti, &facts(bits, trips, false)).is_err(), "i{bits} x{trips}");
        }
    }
    let mut size = facts(16, 1001, false);
    size.opt_size = true;
    assert_eq!(plan(&tti, &size).unwrap_err().0, "A scalar epilogue would remain");
    size.trip_count = 0;
    assert!(plan(&tti, &size).is_err());
    // A count the factors divide (VF 8, IC 1 at `-Os`) needs none.
    size.trip_count = 1000;
    assert_eq!(plan(&tti, &size), Ok(Plan { vf: 8, ic: 1 }));
}

/// gcc's `vect-cost-model=very-cheap` (VP9): a loop of an unknown trip count
/// is not planned at -O2 (no epilogue); a known count keeps the pin's plan,
/// remainder and all (particle -O2: 101 trips, 8x the clocks left scalar).
#[test]
fn a_very_cheap_cost_model_plans_no_loop_of_an_unknown_trip_count() {
    let tti = AArch64::of_cpu("generic").unwrap();
    let mut cheap = facts(16, 0, false);
    cheap.very_cheap = true;
    assert_eq!(plan(&tti, &cheap).unwrap_err().0, "A scalar epilogue would remain");
    for trips in [101, 1001, 1920] {
        cheap.trip_count = trips;
        assert_eq!(plan(&tti, &cheap), plan(&tti, &facts(16, trips, false)), "trips {trips}");
    }
}

/// Bandwidth maximization (`shouldMaximizeVectorBandwidth`) takes VF 8 for a
/// loop storing `i16` and `i64`. It was held scalar (LV-SPLIT) while the
/// backend could not split a `<8 x i64>`; it splits it into four registers
/// now, as the pin does.
#[test]
fn a_loop_whose_types_need_split_registers_is_vectorized_as_the_pin_does() {
    let tti = AArch64::of_cpu("generic").unwrap();
    let store = |bits: u32| Recipe::Store { lane: Ty::Int(bits), align: u64::from(bits / 8) };
    let mixed = LoopFacts { recipes: vec![store(16), store(64)], ..facts(16, 64, false) };
    let plan = plan(&tti, &mixed).expect("the backend splits a <8 x i64> into four registers");
    assert_eq!(plan.vf, 8, "{plan:?}");
}

fn loop_of(recipes: Vec<Recipe>) -> LoopFacts {
    LoopFacts { recipes, ..facts(32, 400, false) }
}

/// `opt -passes=loop-vectorize -debug-only=loop-vectorize` on `fp_canon_sum.ll`
/// (sfm16_float_add): scalar loop costs 11, `Cost for VF 2: 26`, `VF 4: 48`,
/// "not beneficial", interleave count 4. The scalar cost was the count of a
/// loop's phi, add, compare and branch plus its recipes (4 + 1 + ...: 14 here,
/// a load 1 not 2), and no vector cost of a float, a compare, a select or a
/// cast was priced: the loop could not be weighed, so `sfm16_float_add` ran
/// 3,609 instructions against clang's 2,721.
#[test]
fn a_loop_of_scattered_floats_is_priced_as_the_pin_prices_it() {
    let tti = AArch64::of_cpu("apple-m1").unwrap();
    let uno = FloatPredicate::Uno;
    let recipes = vec![
        Recipe::Scalarized { op: MemoryOp::Load, lane: Ty::Float, stride_bytes: Some(8) },
        Recipe::Scalarized { op: MemoryOp::Load, lane: Ty::Float, stride_bytes: Some(8) },
        Recipe::Binary { op: ArithOp::FAdd, lane: Ty::Float, ext: [None, None], second: Default::default() },
        Recipe::Compare { op: CmpSel::FCmp, lane: Ty::Float, predicate: Predicate::Float(uno) },
        Recipe::Cast { kind: CastKind::BitCast, dst: Ty::Int(32), src: Ty::Float, folded: false, users: Vec::new() },
        Recipe::Select { lane: Ty::Int(32), predicate: Some(Predicate::Float(uno)) },
        Recipe::Binary { op: ArithOp::Add, lane: Ty::Int(32), ext: [None, None], second: Default::default() },
    ];
    let costs = |vf| iteration_cost(&tti, &recipes, vf, false, false).unwrap();
    assert_eq!((costs(1), costs(2), costs(4)), (11, 26, 48));
    let looped = loop_of(recipes);
    // 12 a lane at VF 4 against the scalar 11: the pin keeps the loop scalar.
    assert!(!pin_may_vectorize(&tti, &looped));
}

/// `fp_scale.ll` (`b[i] = a[i] * 2.0f`): scalar 8, `VF 2: 6`, `VF 4: 6`,
/// vectorized at VF 4.
#[test]
fn a_loop_of_consecutive_floats_is_priced_as_the_pin_prices_it() {
    let tti = AArch64::of_cpu("apple-m1").unwrap();
    let recipes = vec![
        Recipe::Load { lane: Ty::Float, align: 4 },
        Recipe::Binary { op: ArithOp::FMul, lane: Ty::Float, ext: [None, None], second: Default::default() },
        Recipe::Store { lane: Ty::Float, align: 4 },
    ];
    let costs = |vf| iteration_cost(&tti, &recipes, vf, false, false).unwrap();
    assert_eq!((costs(1), costs(2), costs(4)), (8, 6, 6));
    assert!(pin_may_vectorize(&tti, &loop_of(recipes)));
}

/// A recipe the port cannot price as a vector counts nothing in a lower bound:
/// the loop the pin vectorizes stays the pin's (`quicksort`, `histo`,
/// `particle`, `scroll` kept their interleave in the corpus when casts gained
/// prices, and `sfm16_float` is left to a widening it has not got).
#[test]
fn an_unpriced_recipe_never_makes_a_loop_look_costlier_than_it_is() {
    let tti = AArch64::of_cpu("apple-m1").unwrap();
    let recipes = vec![
        Recipe::Load { lane: Ty::Int(32), align: 4 },
        Recipe::Unpriced { scalar_cost: 3 },
        Recipe::Store { lane: Ty::Int(32), align: 4 },
    ];
    assert!(iteration_cost(&tti, &recipes, 4, false, false).is_err());
    assert_eq!(iteration_cost(&tti, &recipes, 4, false, true), Ok(4));
    assert!(pin_may_vectorize(&tti, &loop_of(recipes)));
}

/// A main loop of VF 16 and IC 1 of unknown trip count leaves fewer than 16
/// iterations, so an epilogue of 16 lanes would never run (`SkipVF` against
/// `urem(tc, 16)`); the epilogue's factor is below the main loop's lanes. It
/// was 16, and the vectorized remainder was the main loop again.
#[test]
fn an_epilogue_is_narrower_than_the_lanes_of_the_main_loop() {
    let tti = AArch64::of_cpu("generic").unwrap();
    let facts = facts(8, 0, false);
    assert!(epilogue_vf(&tti, &facts, Plan { vf: 16, ic: 1 }).is_none_or(|vf| vf < 16));
    assert!(epilogue_vf(&tti, &facts, Plan { vf: 16, ic: 2 }).is_none_or(|vf| vf <= 16));
}

/// `opt -debug-only=loop-vectorize -force-vector-width=8` on a store of
/// bytes, `o[i * 8]`, `o[i * 100]` and of floats at stride 8: the scalarized
/// store costs 48, 120 and 44. Ours was 32 whatever the stride: the pointers'
/// extracts and the address of a step of 65 bytes or more (10 a lane) were
/// not counted, so jpeg_single's idct y-loop (VF 8 at stride 3 x 40 bytes)
/// priced 967 against the pin's 1668 and was widened.
#[test]
fn a_scalarized_store_is_priced_as_the_pin_prices_it() {
    let tti = AArch64::of_cpu("apple-m1").unwrap();
    // The loop's control is the same for each; only the store differs.
    let store = |lane, stride_bytes| {
        iteration_cost(&tti, &[Recipe::Scalarized { op: MemoryOp::Store, lane, stride_bytes }], 8, false, false)
            .unwrap()
    };
    let bytes = store(Ty::Int(8), Some(8));
    assert_eq!(store(Ty::Int(8), Some(100)) - bytes, 120 - 48);
    assert_eq!(store(Ty::Int(8), None) - bytes, 120 - 48);
    assert_eq!(store(Ty::Float, Some(32)) - bytes, 44 - 48);
    assert_eq!(store(Ty::Int(8), Some(-100)) - bytes, 0);
}

fn widening_loop(live_values: usize) -> LoopFacts {
    let wide = Ty::Int(64);
    let mut recipes = vec![
        Recipe::Load { lane: Ty::Int(8), align: 1 },
        Recipe::Cast { kind: CastKind::ZExt, dst: wide, src: Ty::Int(8), folded: false, users: Vec::new() },
    ];
    for _ in 0..live_values {
        recipes.push(Recipe::Binary { op: ArithOp::Add, lane: wide, ext: [None, None], second: Default::default() });
    }
    recipes.push(Recipe::Store { lane: wide, align: 8 });
    let mut facts = facts(8, 1920, false);
    facts.recipes = recipes;
    // `live_values` i64 values all open across the last one.
    let live = (0..live_values)
        .map(|at| super::Live { lane: wide, start: at, end: live_values + 1, accumulator: false })
        .collect();
    facts.pressure = Some(super::Pressure { live, invariants: Vec::new() });
    facts
}

/// A byte loop widened to six live i64 vectors: at the bandwidth VF 16 each is
/// eight registers, past AArch64's 32, so `spillCost` (L5612) prices VF 16
/// above VF 8. Without the term the planner chose VF 16 and spilled.
#[test]
fn a_bandwidth_vf_past_the_register_file_pays_its_spills() {
    let tti = AArch64::of_cpu("generic").unwrap();
    assert_eq!(choose(&tti, &widening_loop(6)), Ok(Plan { vf: 8, ic: 1 }));
    assert_eq!(choose(&tti, &widening_loop(3)), Ok(Plan { vf: 16, ic: 1 }));
}
