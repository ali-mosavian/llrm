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

/// The pin vectorizes `trip 1000` of `i16` with a scalar epilogue; this port
/// plans only the loops a whole number of vector iterations covers (VP3), and
/// says why for the others.
#[test]
fn a_trip_count_the_factors_do_not_divide_is_not_planned() {
    let tti = AArch64::of_cpu("generic").unwrap();
    assert!(plan(&tti, &facts(16, 1920, false)).is_ok());
    assert_eq!(plan(&tti, &facts(16, 1000, false)).unwrap_err().0, "A scalar epilogue would remain");
    assert_eq!(plan(&tti, &facts(32, 100, true)).unwrap_err().0, "A scalar epilogue would remain");
}

#[test]
fn a_loop_without_a_constant_trip_count_is_not_planned() {
    let tti = AArch64::of_cpu("generic").unwrap();
    assert!(plan(&tti, &facts(16, 0, false)).is_err());
}
