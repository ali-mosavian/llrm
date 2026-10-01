//! Each heuristic pinned on MIR: which one decides the branch (the premise),
//! then the probabilities and frequencies it gives.

use super::*;
use crate::testing::{DOS, function, parsed};

/// `@f`'s odds, and the id of each named block.
fn estimate(text: &str) -> (Odds, impl Fn(&str) -> i64 + use<>) {
    let module = parsed(&format!("{DOS}{text}"));
    let function = function(&module, "f");
    let odds = estimated(&module.context, &module.globals, function, &Shape::of(function), &BTreeMap::new());
    let names: Vec<(String, i64)> = function.layout().iter().map(|&one| (function.block(one).name.clone().unwrap_or_default(), id(one))).collect();
    (odds, move |name: &str| names.iter().find(|(one, _)| one == name).unwrap_or_else(|| panic!("no %{name}")).1)
}

fn close(got: Option<f64>, want: f64) -> bool {
    got.is_some_and(|got| (got - want).abs() < 1e-9)
}

/// A branch to a block that ends in `unreachable` is all but never taken.
#[test]
fn test_a_branch_to_unreachable_is_all_but_never_taken() {
    let (odds, at) = estimate(
        "declare void @fail() noreturn
define i16 @f(i16 %x) {
entry:
  %c = icmp ugt i16 %x, 9
  br i1 %c, label %bad, label %good
bad:
  call void @fail()
  unreachable
good:
  ret i16 %x
}
",
    );
    assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Unreachable));
    assert!(close(odds.probability(at("entry"), at("bad")), 1.0 / (1 << 20) as f64));
}

/// Staying in a loop is taken 124 times to every 4 exits, and the header
/// runs 1 / (1 - 31/32) = 32 times per entry.
#[test]
fn test_a_loop_stays_in_31_of_32_and_its_header_runs_32_times() {
    let (odds, at) = estimate(
        "define i16 @f(i16 %n) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %head ]
  %j = add i16 %i, 1
  %c = icmp slt i16 %j, %n
  br i1 %c, label %head, label %out
out:
  ret i16 %j
}
",
    );
    assert_eq!(odds.by.get(&at("head")), Some(&Heuristic::Loop));
    assert!(close(odds.probability(at("head"), at("head")), 124.0 / 128.0));
    assert!(close(odds.frequency.get(&at("head")).copied(), 32.0));
    assert!(close(odds.frequency.get(&at("out")).copied(), 1.0));
}

/// `p == q` on pointers fails 20 times in 32.
#[test]
fn test_pointers_are_unlikely_equal() {
    let (odds, at) = estimate(
        "define i16 @f(ptr %p) {
entry:
  %c = icmp eq ptr %p, null
  br i1 %c, label %none, label %some
none:
  ret i16 0
some:
  ret i16 1
}
",
    );
    assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Pointer));
    assert!(close(odds.probability(at("entry"), at("none")), 12.0 / 32.0));
}

/// `x == 0` fails, `x < 0` fails, `x > -1` holds: 20 in 32.
#[test]
fn test_zero_and_negative_compares() {
    for (compare, likely) in [("icmp eq i16 %x, 0", false), ("icmp slt i16 %x, 0", false), ("icmp sgt i16 %x, -1", true), ("icmp ne i16 0, %x", true)] {
        let (odds, at) = estimate(&format!(
            "define i16 @f(i16 %x) {{
entry:
  %c = {compare}
  br i1 %c, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 0
}}
"
        ));
        assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Zero), "{compare}");
        assert!(close(odds.probability(at("entry"), at("yes")), if likely { 20.0 / 32.0 } else { 12.0 / 32.0 }), "{compare}");
    }
}

/// Floats are unlikely equal (20:12), and all but never NaN.
#[test]
fn test_float_equality_and_nan_compares() {
    for (compare, want) in [("fcmp oeq float %x, %y", 12.0 / 32.0), ("fcmp une float %x, %y", 20.0 / 32.0), ("fcmp uno float %x, %y", 1.0 / (1 << 20) as f64)] {
        let (odds, at) = estimate(&format!(
            "define i16 @f(float %x, float %y) {{
entry:
  %c = {compare}
  br i1 %c, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 0
}}
"
        ));
        assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Float), "{compare}");
        assert!(close(odds.probability(at("entry"), at("yes")), want), "{compare}");
    }
}

/// Of two successors, the one that calls is not taken, 67%; with no call,
/// the one that returns is not taken, 66%.
#[test]
fn test_calls_and_returns_are_unlikely() {
    let (odds, at) = estimate(
        "declare void @log(i16)
define i16 @f(i16 %x, i16 %y) {
entry:
  %c = icmp ugt i16 %x, %y
  br i1 %c, label %noisy, label %quiet
noisy:
  call void @log(i16 %x)
  br label %join
quiet:
  br label %join
join:
  ret i16 %x
}
",
    );
    assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Call));
    assert!(close(odds.probability(at("entry"), at("noisy")), 33.0 / 100.0));

    let (odds, at) = estimate(
        "define i16 @f(i16 %x, i16 %y) {
entry:
  %c = icmp ugt i16 %x, %y
  br i1 %c, label %early, label %more
early:
  ret i16 0
more:
  %z = mul i16 %x, %y
  br label %done
done:
  ret i16 %z
}
",
    );
    assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Return));
    assert!(close(odds.probability(at("entry"), at("early")), 34.0 / 100.0));
}

/// No heuristic: both edges even, and a diamond's join runs as its entry.
#[test]
fn test_no_heuristic_is_even() {
    let (odds, at) = estimate(
        "define i16 @f(i16 %x, i16 %y) {
entry:
  %c = icmp ugt i16 %x, %y
  br i1 %c, label %a, label %b
a:
  br label %join
b:
  br label %join
join:
  %r = phi i16 [ %x, %a ], [ %y, %b ]
  ret i16 %r
}
",
    );
    assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Even));
    assert!(close(odds.probability(at("entry"), at("a")), 0.5));
    assert!(close(odds.frequency.get(&at("join")).copied(), 1.0));
}

/// BASIC's `IF x = 159 AND y = 99`: truth values are 0 and -1, so the AND
/// is an i16 and the branch tests it against 0. The zero heuristic read
/// that as "nonzero, likely": a rare AND of equalities taken 20 times in 32.
/// A truth value is no quantity; LLVM, whose branches read the i1, never
/// asks.
#[test]
fn test_a_basic_truth_value_is_not_a_zero_compare() {
    let (odds, at) = estimate(
        "define i16 @f(i16 %x, i16 %y) {
entry:
  %a = icmp eq i16 %x, 159
  %sa = sext i1 %a to i16
  %b = icmp eq i16 %y, 99
  %sb = sext i1 %b to i16
  %both = and i16 %sa, %sb
  %c = icmp ne i16 %both, 0
  br i1 %c, label %rare, label %common
rare:
  br label %join
common:
  br label %join
join:
  %r = phi i16 [ 255, %rare ], [ %x, %common ]
  ret i16 %r
}
",
    );
    assert_ne!(odds.by.get(&at("entry")), Some(&Heuristic::Zero));
    assert!(odds.probability(at("entry"), at("rare")).is_some_and(|rare| rare <= 0.5));
}

/// Nested loops multiply: the inner header runs 32 times per pass of an
/// outer one that runs 32 times, and the inner exit lands in the outer loop.
#[test]
fn test_a_nested_loop_runs_32_times_32() {
    let (odds, at) = estimate(
        "define i16 @f(i16 %n) {
entry:
  br label %outer
outer:
  %i = phi i16 [ 0, %entry ], [ %i2, %latch ]
  br label %inner
inner:
  %j = phi i16 [ 0, %outer ], [ %j2, %inner ]
  %j2 = add i16 %j, 1
  %c = icmp slt i16 %j2, %n
  br i1 %c, label %inner, label %latch
latch:
  %i2 = add i16 %i, 1
  %d = icmp slt i16 %i2, %n
  br i1 %d, label %outer, label %out
out:
  ret i16 %i2
}
",
    );
    assert_eq!(odds.by.get(&at("inner")), Some(&Heuristic::Loop));
    assert_eq!(odds.by.get(&at("latch")), Some(&Heuristic::Loop));
    assert!(close(odds.frequency.get(&at("outer")).copied(), 32.0));
    assert!(close(odds.frequency.get(&at("latch")).copied(), 32.0));
    assert!(close(odds.frequency.get(&at("inner")).copied(), 1024.0));
    assert!(close(odds.frequency.get(&at("out")).copied(), 1.0));
}

/// LLVM's zero heuristic also takes `x > 0` as likely.
#[test]
fn test_a_positive_compare_is_likely() {
    let (odds, at) = estimate(
        "define i16 @f(i16 %x) {
entry:
  %c = icmp sgt i16 %x, 0
  br i1 %c, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 0
}
",
    );
    assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Zero));
    assert!(close(odds.probability(at("entry"), at("yes")), 20.0 / 32.0));
}

/// ... and `x < 1`, that is `x <= 0`, as unlikely.
#[test]
fn test_a_below_one_compare_is_unlikely() {
    let (odds, at) = estimate(
        "define i16 @f(i16 %x) {
entry:
  %c = icmp slt i16 %x, 1
  br i1 %c, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 0
}
",
    );
    assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Zero));
    assert!(close(odds.probability(at("entry"), at("yes")), 12.0 / 32.0));
}

/// `@f`'s odds with `header` known to run `trips` trips.
fn estimate_counted(text: &str, header: &str, trips: i64) -> (Odds, impl Fn(&str) -> i64 + use<>) {
    let module = parsed(&format!("{DOS}{text}"));
    let function = function(&module, "f");
    let names: Vec<(String, i64)> = function.layout().iter().map(|&one| (function.block(one).name.clone().unwrap_or_default(), id(one))).collect();
    let at = move |name: &str| names.iter().find(|(one, _)| one == name).unwrap_or_else(|| panic!("no %{name}")).1;
    let odds = estimated(&module.context, &module.globals, function, &Shape::of(function), &BTreeMap::from([(at(header), trips)]));
    (odds, at)
}

const COUNTED: &str = "define i16 @f(i16 %n) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %head ]
  %j = add i16 %i, 1
  %c = icmp slt i16 %j, 100
  br i1 %c, label %head, label %out
out:
  ret i16 %j
}
";

/// A loop whose trips induction proves runs them, not 32: the header 100
/// times per entry, and the block after it once. The heuristic's 31 in 32
/// made every loop 32 trips, whatever its bound.
#[test]
fn test_a_proven_trip_count_replaces_the_loop_heuristic() {
    let (counted, at) = estimate_counted(COUNTED, "head", 100);
    assert!(close(counted.frequency.get(&at("head")).copied(), 100.0), "{:?}", counted.frequency);
    assert!(close(counted.frequency.get(&at("out")).copied(), 1.0), "{:?}", counted.frequency);
    let (guessed, at) = estimate(COUNTED);
    assert!(close(guessed.frequency.get(&at("head")).copied(), 32.0), "premise: without a count it is 32: {:?}", guessed.frequency);
}

/// A loop tested at its header, before a trip, runs its header one more time
/// than it trips, as the rotated one's tested after.
#[test]
fn test_a_header_tested_loop_runs_its_header_once_more_than_it_trips() {
    let (odds, at) = estimate_counted(
        "define i16 @f(i16 %n) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %c = icmp slt i16 %i, 100
  br i1 %c, label %body, label %out
body:
  %j = add i16 %i, 1
  br label %head
out:
  ret i16 %i
}
",
        "head",
        100,
    );
    assert!(close(odds.frequency.get(&at("head")).copied(), 101.0), "{:?}", odds.frequency);
    assert!(close(odds.frequency.get(&at("body")).copied(), 100.0), "{:?}", odds.frequency);
    assert!(close(odds.frequency.get(&at("out")).copied(), 1.0), "{:?}", odds.frequency);
}
