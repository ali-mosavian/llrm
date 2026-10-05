//! Difference bounds over the compares of quicksort's slice loop.

use std::collections::BTreeMap;

use num_bigint::BigInt;

use super::proves;
use crate::guards::Guard;
use crate::induction::{Monomial, Scev};
use llrm_mir::module::ValueId;
use llrm_mir::opcode::IntPredicate::{self, *};

const W: u32 = 16;

fn unknown(n: u32) -> Scev {
    Scev::unknown(ValueId(n), W)
}

fn guard(predicate: IntPredicate, left: &Scev, right: &Scev) -> Guard {
    Guard { predicate, left: left.clone(), right: right.clone() }
}

/// `lo`, `hi` and `len` as the slice loop has them: `hi` below `len`,
/// `lo` below `hi` signed, and `lo` below `len`, the start's own test.
fn slice(len_range: Option<i64>) -> (Vec<Guard>, BTreeMap<Monomial, (BigInt, BigInt)>, [Scev; 3]) {
    let (lo, hi, len) = (unknown(0), unknown(1), unknown(2));
    let facts = vec![guard(Ult, &hi, &len), guard(Slt, &lo, &hi), guard(Ult, &lo, &len)];
    let mut ranges = BTreeMap::new();
    if let Some(most) = len_range {
        ranges.insert(Monomial::of(ValueId(2)), (BigInt::from(0), BigInt::from(most)));
    }
    (facts, ranges, [lo, hi, len])
}

/// Every `j` from `lo` below `hi` is below `len`: `hi - 1 <u len` and
/// `lo <=u hi - 1`, from a length of at most 32767.
#[test]
fn test_a_slice_loops_last_index_is_below_the_length() {
    let (facts, ranges, [lo, hi, len]) = slice(Some(32767));
    let last = hi.minus(&Scev::constant(1, W));
    assert!(proves(W, &facts, &ranges, Ult, &last, &len));
    assert!(proves(W, &facts, &ranges, Ule, &lo, &last));
}

/// Without a bound on the length, `lo <u len` leaves `lo` possibly
/// negative: len 0xFFF0, lo 0x8008 passes, and `j` reaches 0xFFFF.
#[test]
fn test_an_unbounded_length_proves_nothing_of_the_last_index() {
    let (facts, ranges, [lo, hi, len]) = slice(None);
    let last = hi.minus(&Scev::constant(1, W));
    assert!(!proves(W, &facts, &ranges, Ult, &last, &len));
    assert!(!proves(W, &facts, &ranges, Ule, &lo, &last));
}

/// A guard that holds of the sides in the other order, or is weaker, proves nothing more.
#[test]
fn test_a_weaker_guard_proves_nothing_stronger() {
    let (_, ranges, [_, hi, len]) = slice(Some(32767));
    let facts = vec![guard(Ule, &hi, &len)];
    assert!(!proves(W, &facts, &ranges, Ult, &hi, &len));
    assert!(proves(W, &facts, &ranges, Ule, &hi, &len));
}

/// An offset that may wrap is not trusted: `hi + 1 <u len` from `hi <u len`.
#[test]
fn test_an_offset_that_may_wrap_proves_nothing() {
    let (facts, ranges, [_, hi, len]) = slice(None);
    let next = hi.plus(&Scev::constant(1, W));
    assert!(!proves(W, &facts, &ranges, Ult, &next, &len));
}
