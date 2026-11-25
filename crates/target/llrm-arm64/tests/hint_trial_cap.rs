//! INLINEHINT-GROWTH-TRIAL keeps the compile with the calls of `inlinehint`
//! callees when the object is smaller and the work grows by at most the cap or
//! a quarter of the percent of bytes saved.

use llrm_arm64::trial::{Price, keeps_unhinted};

fn price(
    bytes: i64,
    work: f64,
) -> Price {
    Price { bytes, work, laid_out: work, divides: 0.0, weighted: work }
}

/// sfm16_dbg -Os (bench): `sf_clz32` inlined at six sites made the object 400 B
/// larger than gcc's; the trial that keeps the calls saves 400 B of 2472 for
/// 1.5% more work, and a flat 1% cap put it back (20,104 B against 19,704 B).
#[test]
fn a_sixteen_percent_smaller_object_is_kept_for_one_and_a_half_percent_more_work() {
    let (inlined, called) = (price(2472, 1009.97), price(2072, 1025.46));
    assert!(keeps_unhinted(inlined, called, Some(1)));
}

/// QuickrBasic flow.c -Os: keeping the negative-cost calls saves 20 B of 6032
/// (0.3%) for 2.1% more work: not kept.
#[test]
fn a_tiny_saving_does_not_buy_two_percent_more_work() {
    let (inlined, called) = (price(6032, 3456.03), price(6012, 3528.90));
    assert!(!keeps_unhinted(inlined, called, Some(1)));
}

#[test]
fn a_larger_object_is_never_kept() {
    assert!(!keeps_unhinted(price(2000, 100.0), price(2004, 90.0), Some(1)));
}
