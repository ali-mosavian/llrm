//! Ports of `tests/test_ranges.py`, `tests/test_edge_ranges.py` and
//! `tests/test_unsigned_edge_ranges.py`. `test_range_alias_checks_cover_width_and_wrap`
//! is `regions`' `overlapping_range_covering_respects_width_wrap_and_each_fact_map`.

use std::collections::BTreeMap;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Module, ValueId};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use super::{_computed, _recurrence_span, Interval, covering, dominated_edges, on_edge, singletons};
use crate::cfg;
use crate::memory::{MemRef, Unit};
use crate::testing::{DOS, block, function, layout, parsed, value};

fn interval(low: i64, high: i64, width: u32) -> Interval {
    Interval { low: low.into(), high: high.into(), width }
}

struct Parsed {
    module: Module,
    layout: DataLayout,
}

impl Parsed {
    fn new(body: &str) -> Self {
        let module = parsed(&format!("{DOS}{body}"));
        let layout = layout(&module);
        Self { module, layout }
    }

    fn unit(&self) -> Unit<'_> {
        Unit::of(&self.module, &self.layout, function(&self.module, "f"))
    }

    fn value(&self, name: &str) -> ValueId {
        value(function(&self.module, "f"), name)
    }

    fn block(&self, name: &str) -> llrm_mir::module::BlockId {
        block(function(&self.module, "f"), name)
    }

    /// The instruction defining `%name`.
    fn made(&self, name: &str) -> InstId {
        let f = function(&self.module, "f");
        let wanted = self.value(name);
        f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(wanted)).expect("defined")
    }
}

/// `%c = icmp <predicate> i<width> %x, <bound>` branching to `%yes` or `%no`.
fn compare(predicate: &str, width: u32, bound: i64) -> Parsed {
    Parsed::new(&format!(
        "define void @f(i{width} %x) {{
b0:
  %c = icmp {predicate} i{width} %x, {bound}
  br i1 %c, label %yes, label %no

yes:
  ret void

no:
  ret void
}}
"
    ))
}

#[test]
fn test_signed_comparison_edges() {
    for (predicate, taken, (low, high)) in [
        ("slt", true, (0, 3)),
        ("slt", false, (4, 9)),
        ("sle", true, (0, 4)),
        ("sgt", true, (5, 9)),
        ("sge", true, (4, 9)),
        ("eq", true, (4, 4)),
        ("ne", false, (4, 4)),
        ("ne", true, (0, 9)),
    ] {
        let parsed = compare(predicate, 16, 4);
        let x = parsed.value("x");
        let known = IndexMap::from_iter([(x, interval(0, 9, 16))]);
        let successor = parsed.block(if taken { "yes" } else { "no" });

        let result = on_edge(&parsed.unit(), parsed.block("b0"), successor, &known, None).unwrap().unwrap();

        assert_eq!(result[&x], interval(low, high, 16), "{predicate} {taken}");
    }
}

#[test]
fn an_edge_the_facts_rule_out_is_impossible() {
    let parsed = compare("slt", 16, 4);
    let known = IndexMap::from_iter([(parsed.value("x"), interval(5, 9, 16))]);
    assert_eq!(on_edge(&parsed.unit(), parsed.block("b0"), parsed.block("yes"), &known, None).unwrap(), None);
    assert!(on_edge(&parsed.unit(), parsed.block("b0"), parsed.block("no"), &known, None).unwrap().is_some());
}

#[test]
fn an_edge_to_a_block_that_is_no_successor_is_refused() {
    let parsed = compare("slt", 16, 4);
    assert!(on_edge(&parsed.unit(), parsed.block("yes"), parsed.block("no"), &IndexMap::default(), None).is_err());
}

#[test]
fn an_unconditional_edge_says_nothing_new() {
    let parsed = Parsed::new(
        "define void @f(i16 %x) {
b0:
  br label %b1

b1:
  ret void
}
",
    );
    let known = IndexMap::from_iter([(parsed.value("x"), interval(0, 9, 16))]);
    assert_eq!(on_edge(&parsed.unit(), parsed.block("b0"), parsed.block("b1"), &known, None).unwrap(), Some(known));
}

/// Only counted loops were scoped, so `x < 8` under `x < 8` in
/// straight-line code kept both branches.
#[test]
fn test_a_compare_under_the_same_compare_is_decided_outside_any_loop() {
    let parsed = Parsed::new(
        "define void @f(i16 %x) {
b0:
  %c = icmp slt i16 %x, 8
  br i1 %c, label %b1, label %b3

b1:
  %d = icmp slt i16 %x, 8
  br i1 %d, label %b2, label %b4

b2:
  ret void

b3:
  ret void

b4:
  ret void
}
",
    );
    let unit = parsed.unit();
    let scoped = dominated_edges(&unit).unwrap();
    let inside = &scoped[&cfg::id(parsed.block("b1"))];

    assert_eq!(on_edge(&unit, parsed.block("b1"), parsed.block("b4"), inside, None).unwrap(), None);
}

#[test]
fn a_guard_does_not_bound_a_join_another_path_reaches() {
    let parsed = Parsed::new(
        "define void @f(i16 %x, i1 %p) {
b0:
  br i1 %p, label %check, label %join

check:
  %c = icmp slt i16 %x, 8
  br i1 %c, label %join, label %out

join:
  %y = add i16 %x, 1
  ret void

out:
  ret void
}
",
    );
    let scoped = dominated_edges(&parsed.unit()).unwrap();
    assert!(scoped.get(&cfg::id(parsed.block("join"))).is_none_or(|facts| !facts.contains_key(&parsed.value("x"))));
}

/// `tests/test_edge_ranges.py`'s guard outside a loop: `0 <= i < 4`
/// bounds a word-array offset to 0..6.
#[test]
fn a_guarded_block_bounds_what_it_computes_from_the_guarded_value() {
    let parsed = Parsed::new(
        "define void @f(i16 %x) {
b0:
  %c = icmp slt i16 %x, 4
  br i1 %c, label %b1, label %b2

b1:
  %lo = icmp sge i16 %x, 0
  br i1 %lo, label %b3, label %b2

b3:
  %y = mul i16 %x, 2
  ret void

b2:
  ret void
}
",
    );
    let scoped = dominated_edges(&parsed.unit()).unwrap();
    let facts = &scoped[&cfg::id(parsed.block("b3"))];
    assert_eq!(facts[&parsed.value("x")], interval(0, 3, 16));
    assert_eq!(facts[&parsed.value("y")], interval(0, 6, 16));
    assert!(!scoped.contains_key(&cfg::id(parsed.block("b2"))));
}

fn computed(body: &str, name: &str, known: &[(&str, Interval)]) -> Option<Interval> {
    let parsed = Parsed::new(body);
    let known = known.iter().map(|(name, one)| (parsed.value(name), one.clone())).collect::<IndexMap<_, _>>();
    _computed(&parsed.unit(), parsed.made(name), &known, &IndexMap::default())
}

fn unary(operation: &str, width: u32) -> String {
    format!(
        "define void @f(i16 %x) {{
b0:
  %y = {operation}
  ret void
}}
"
    )
    .replace("WIDTH", &width.to_string())
}

#[test]
fn test_unit_steps_require_nonwrapping_intervals() {
    for (operation, low, high, expected) in [
        ("sub i16 %x, 1", 1, 3, Some(interval(0, 2, 16))),
        ("add i16 %x, 1", -3, -1, Some(interval(-2, 0, 16))),
        ("sub i16 %x, 1", -32768, 0, None),
        ("add i16 %x, 1", 0, 32767, None),
    ] {
        assert_eq!(computed(&unary(operation, 16), "y", &[("x", interval(low, high, 16))]), expected, "{operation}");
    }
}

/// ADDRM's bounded 1..20 counter lost its interval when converted to a long.
#[test]
fn test_signed_widening_keeps_the_numeric_range() {
    for (low, high) in [(1, 20), (-32768, -1), (-10, 10)] {
        assert_eq!(computed(&unary("sext i16 %x to i32", 32), "y", &[("x", interval(low, high, 16))]), Some(interval(low, high, 32)));
    }
}

#[test]
fn a_zero_extension_is_not_taken_for_a_sign_extension() {
    assert_eq!(computed(&unary("zext i16 %x to i32", 32), "y", &[("x", interval(-1, 1, 16))]), None);
}

/// `x AND 511` had no interval unless `x` had one, so a masked subscript was
/// never known to fit.
#[test]
fn test_a_nonnegative_mask_bounds_an_unknown_operand() {
    assert_eq!(computed(&unary("and i16 %x, 511", 16), "y", &[]), Some(interval(0, 511, 16)));
    assert_eq!(computed(&unary("and i16 %x, -2", 16), "y", &[]), None);
}

#[test]
fn test_secondary_recurrence_bounds_reject_wrap() {
    for (start, step, advances, expected) in [
        (0, 4, 5, Some(interval(0, 20, 16))),
        (20, -4, 5, Some(interval(0, 20, 16))),
        (7, 0, 5, Some(interval(7, 7, 16))),
        (32760, 4, 1, None),
        (-32760, -4, 2, None),
        (0, 16384, 4, None),
        (0, 4, -1, None),
    ] {
        let found = _recurrence_span(&BigInt::from(start), &BigInt::from(step), &BigInt::from(advances), 16);
        assert_eq!(found, expected, "{start} {step} {advances}");
    }
}

#[test]
fn test_shift_ranges_refuse_wraparound() {
    for (low, high, count, expected) in [
        (0, 5, 2, Some(interval(0, 20, 16))),
        (-5, -1, 2, Some(interval(-20, -4, 16))),
        (0, 16384, 1, None),
        (-32768, -1, 1, None),
        (0, 5, 32, None),
    ] {
        let operation = format!("shl i16 %x, {count}");
        assert_eq!(computed(&unary(&operation, 16), "y", &[("x", interval(low, high, 16))]), expected, "{low} {high} {count}");
    }
}

#[test]
fn a_sum_of_intervals_that_could_wrap_has_none() {
    let body = "define void @f(i16 %x, i16 %z) {
b0:
  %y = add i16 %x, %z
  ret void
}
";
    assert_eq!(computed(body, "y", &[("x", interval(0, 20000, 16)), ("z", interval(0, 20000, 16))]), None);
    assert_eq!(computed(body, "y", &[("x", interval(0, 10, 16)), ("z", interval(-5, 5, 16))]), Some(interval(-5, 15, 16)));
    assert_eq!(computed(body, "y", &[("x", interval(0, 10, 16))]), None);
}

#[test]
fn test_unsigned_edge_never_removes_a_possible_selector() {
    for predicate in ["ugt", "uge", "ult", "ule"] {
        for span in [(1, 3), (255, 255), (256, 260), (-3, -1), (-2, 2)] {
            for taken in [true, false] {
                for width in [16, 32] {
                    let parsed = compare(predicate, width, 255);
                    let known = IndexMap::from_iter([(parsed.value("x"), interval(span.0, span.1, width))]);
                    let answer = |value: i64| match predicate {
                        "ugt" => value > 255,
                        "uge" => value >= 255,
                        "ult" => value < 255,
                        _ => value <= 255,
                    };
                    let possible = (span.0..=span.1).any(|number| answer(number & ((1_i64 << width) - 1)) == taken);
                    let successor = parsed.block(if taken { "yes" } else { "no" });
                    let result = on_edge(&parsed.unit(), parsed.block("b0"), successor, &known, None).unwrap();
                    assert_eq!(result.is_some(), possible, "{predicate} {span:?} {taken} {width}");
                }
            }
        }
    }
}

#[test]
fn a_phi_of_one_constant_is_that_singleton_and_of_two_is_none() {
    let parsed = Parsed::new(
        "define void @f(i1 %p) {
b0:
  br i1 %p, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %same = phi i16 [ 5, %b1 ], [ 5, %b2 ]
  %either = phi i16 [ 5, %b1 ], [ 6, %b2 ]
  %twice = shl i16 %same, 1
  ret void
}
",
    );
    let known = singletons(&parsed.unit());
    assert_eq!(known.get(&parsed.value("same")), Some(&interval(5, 5, 16)));
    assert_eq!(known.get(&parsed.value("twice")), Some(&interval(10, 10, 16)));
    assert_eq!(known.get(&parsed.value("either")), None);
}

#[test]
fn covering_turns_a_bounded_index_into_the_bytes_it_reaches_and_refuses_a_wrap() {
    let parsed = Parsed::new(
        "@g = global [64 x i16] zeroinitializer

define void @f(i16 %i) {
b0:
  %e = getelementptr i16, ptr @g, i16 %i
  %a = getelementptr i8, ptr %e, i16 4
  store i16 0, ptr %a
  ret void
}
",
    );
    let unit = parsed.unit();
    let f = function(&parsed.module, "f");
    let store = f.walk().map(|(_, inst)| inst).find_map(|inst| MemRef::of(&unit, inst)).unwrap();
    let i = parsed.value("i");

    let covered = covering(&store, &BTreeMap::from([(i, interval(0, 3, 16))]));
    assert_eq!((covered.base, covered.disp, covered.width), (None, 4, 8));
    for outside in [interval(-3, 3, 16), interval(0, 40000, 16), interval(0, 3, 32)] {
        assert_eq!(covering(&store, &BTreeMap::from([(i, outside)])).base, Some(i));
    }
}
