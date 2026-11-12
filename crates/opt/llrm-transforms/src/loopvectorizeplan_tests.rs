//! `choose` against the pinned LLVM's `opt -passes=loop-vectorize` remark
//! ("vectorization width: W, interleaved count: I") on aarch64, for copy and
//! add loops of each element width and several trip counts (the rows below are
//! that remark).

use llrm_arm64::tti::AArch64;
use llrm_mir::tti::{ArithOp, Ty};

use super::{LoopFacts, Plan, Recipe, choose, plan};

fn facts(
    lane_bits: u32,
    trip_count: u64,
    add: bool,
) -> LoopFacts {
    let lane = Ty::Int(lane_bits);
    let align = u64::from(lane_bits / 8);
    let mut recipes = vec![Recipe::Load { lane, align }];
    if add {
        recipes.push(Recipe::Binary { op: ArithOp::Add, lane });
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
/// is not constant, with a scalar epilogue; this planner refused both ("A scalar
/// epilogue would remain", "The trip count is not a known constant").
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

/// Bandwidth maximization (`shouldMaximizeVectorBandwidth`) took VF 8 for a loop
/// storing `i16` and `i64`: a `<8 x i64>` the backend cannot split, so arm64
/// refused the whole function ("vector with no register class"); at VF 2 the
/// `<2 x i16>` has no register either. With no VF filling a register for every
/// type, the loop stays scalar (LV-SPLIT).
#[test]
fn a_loop_whose_types_fill_no_common_register_is_not_vectorized() {
    let tti = AArch64::of_cpu("generic").unwrap();
    let store = |bits: u32| Recipe::Store { lane: Ty::Int(bits), align: u64::from(bits / 8) };
    let mixed = LoopFacts { recipes: vec![store(16), store(64)], ..facts(16, 64, false) };
    assert!(plan(&tti, &mixed).is_err(), "{:?}", plan(&tti, &mixed));
}
