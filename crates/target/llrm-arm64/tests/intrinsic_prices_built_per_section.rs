//! The intrinsic price table is indexed for the rows asked for, not all 1081.

use llrm_arm64::tti::{AArch64, intrinsic_rows_indexed};
use llrm_mir::tti::{Cost, CostKind, Tti};

/// The first price asked for built a hash map of every row of the file (eight
/// cpu/kind sections of 135): 1.09 M of queens' 57 M instructions, once a
/// compile, for the few rows of one section a compile reads.
#[test]
fn the_first_intrinsic_price_indexes_one_section_of_the_table() {
    let generic = AArch64::of_cpu("generic").unwrap();
    let before = intrinsic_rows_indexed();
    let cost = generic.intrinsic_cost("llvm.smax.i8", None, &[], CostKind::RecipThroughput);
    assert_eq!(cost, Cost::of(2), "the premise: the row `generic throughput llvm.smax.i8 2`");
    let indexed = intrinsic_rows_indexed() - before;
    assert_eq!(indexed, 135, "{indexed} rows indexed for one price");
    // Asking again, or for the same section, indexes nothing more.
    generic.intrinsic_cost("llvm.smin.i8", None, &[], CostKind::RecipThroughput);
    assert_eq!(intrinsic_rows_indexed() - before, 135);
    // Another section is indexed when asked for, and answers from its own rows.
    let m1 = AArch64::of_cpu("apple-m1").unwrap();
    assert_eq!(m1.intrinsic_cost("llvm.smax.i8", None, &[], CostKind::CodeSize), Cost::of(2));
    assert_eq!(intrinsic_rows_indexed() - before, 270);
}
