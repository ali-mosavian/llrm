//! Each heuristic pinned on MIR: which one decides the branch (the premise),
//! then the probabilities and frequencies it gives.

use super::*;
use crate::testing::{DOS, function, parsed};

/// `@f`'s odds, and the id of each named block.
fn estimate(text: &str) -> (Odds, impl Fn(&str) -> i64 + use<>) {
    let module = parsed(&format!("{DOS}{text}"));
    let function = function(&module, "f");
    let odds = estimated(&module.context, &module.metadata, &module.globals, function, &Shape::of(function), &BTreeMap::new());
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
  ret i16 %x
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

/// GCC's `PRED_NEGATIVE_RETURN`, `PRED_NULL_RETURN` and `PRED_CONST_RETURN`: a path that only returns a
/// constant is the exception, all but never when the constant is negative (an error code).
#[test]
fn test_a_path_that_returns_a_constant_is_unlikely_by_what_it_returns() {
    let shape = |returned: &str, ty: &str| {
        format!(
            "define {ty} @f(i16 %x, i16 %y) {{
entry:
  %c = icmp ugt i16 %x, %y
  br i1 %c, label %early, label %more
early:
  ret {ty} {returned}
more:
  %z = mul i16 %x, %y
  br label %done
done:
  ret {ty} {}
}}
",
            if ty == "ptr" { "null" } else { "%z" }
        )
    };
    for (returned, ty, want) in [("-1", "i16", 2.0 / 100.0), ("7", "i16", 35.0 / 100.0), ("null", "ptr", 29.0 / 100.0)] {
        let text = if ty == "ptr" { shape(returned, ty).replace("  %z = mul i16 %x, %y\n", "  %z = mul i16 %x, %y\n  %w = inttoptr i16 %z to ptr\n").replace("ret ptr null\n}", "ret ptr %w\n}") } else { shape(returned, ty) };
        let (odds, at) = estimate(&text);
        assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Return), "{text}");
        assert!(close(odds.probability(at("entry"), at("early")), want), "{returned}: {:?}", odds.probability(at("entry"), at("early")));
    }
}

/// Nib's bool is a byte: a call proven to return 0 or 1 is a truth value, as an `i1` is, and `!= 0` of it says
/// nothing of how often it holds (queens' `if safe(..)` was given 62.5%).
#[test]
fn test_a_call_ranged_to_a_truth_value_is_no_zero_compare() {
    let text = |range: &str| {
        format!(
            "declare i8 @safe(i16) {range}
define i16 @f(i16 %x) {{
entry:
  %c = call i8 @safe(i16 %x)
  %t = icmp ne i8 %c, 0
  br i1 %t, label %yes, label %no
yes:
  %a = mul i16 %x, 3
  br label %join
no:
  %b = mul i16 %x, 5
  br label %join
join:
  %r = phi i16 [ %a, %yes ], [ %b, %no ]
  ret i16 %r
}}
"
        )
    };
    let (odds, at) = estimate(&text(""));
    assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Zero));
    let (odds, at) = estimate(&text("").replace("declare i8 @safe(i16) ", "declare range(i8 0, 2) i8 @safe(i16)"));
    assert_ne!(odds.by.get(&at("entry")), Some(&Heuristic::Zero));
    assert!(odds.probability(at("entry"), at("yes")).is_some_and(|yes| (yes - 0.5).abs() < 1e-9));
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
    let odds = estimated(&module.context, &module.metadata, &module.globals, function, &Shape::of(function), &BTreeMap::from([(at(header), trips)]));
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

/// `@f` branching on `compare` of `@callee`'s result, `declared` its declaration.
fn three_way_branch(declared: &str, callee: &str, compare: &str) -> (Odds, i64, i64) {
    let (odds, at) = estimate(&format!(
        "{declared}
define i16 @f(ptr %a, ptr %b) {{
entry:
  %r = call i16 @{callee}(ptr %a, ptr %b)
  %c = {compare}
  br i1 %c, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 0
}}
"
    ));
    (odds, at("entry"), at("yes"))
}

/// A three-way compare's sign says which string is greater, not how often:
/// `B$SCMP(a, b) > 0`, BASIC's `a$ > b$`, read as `x > 0`, likely.
#[test]
fn test_a_three_way_compares_sign_is_no_zero_compare() {
    let declared = "declare i16 @scmp(ptr, ptr) threeway";
    // The premise: the routine is stated a three-way compare.
    let module = crate::testing::parsed(&format!("{}{declared}\n", crate::testing::DOS));
    assert!(llrm_mir::facts::Facts::of(&crate::testing::function(&module, "scmp").attrs).three_way_compare());
    for compare in ["icmp sgt i16 %r, 0", "icmp slt i16 %r, 0", "icmp sle i16 %r, 0"] {
        let (odds, entry, _) = three_way_branch(declared, "scmp", compare);
        assert_ne!(odds.by.get(&entry), Some(&Heuristic::Zero), "{compare}");
    }
}

/// Of its result only equality with 0 counts: the data unlikely equal.
#[test]
fn test_a_three_way_compares_equality_is_unlikely() {
    let declared = "declare i16 @strcmp(ptr, ptr) threeway";
    let (odds, entry, yes) = three_way_branch(declared, "strcmp", "icmp eq i16 %r, 0");
    assert_eq!(odds.by.get(&entry), Some(&Heuristic::Zero));
    assert!(close(odds.probability(entry, yes), 12.0 / 32.0));
    let (odds, entry, yes) = three_way_branch(declared, "strcmp", "icmp ne i16 %r, 0");
    assert!(close(odds.probability(entry, yes), 20.0 / 32.0) && odds.by.get(&entry) == Some(&Heuristic::Zero));
}

/// The fact, not the name: a routine called strcmp the language does not
/// state a three-way compare is an ordinary call, its `> 0` likely.
#[test]
fn test_a_name_alone_states_no_three_way_compare() {
    let (odds, entry, yes) = three_way_branch("declare i16 @strcmp(ptr, ptr)", "strcmp", "icmp sgt i16 %r, 0");
    assert_eq!(odds.by.get(&entry), Some(&Heuristic::Zero));
    assert!(close(odds.probability(entry, yes), 20.0 / 32.0));
}

/// A one-bit mask tested against 0 is a flag, not a quantity: LLVM's zero
/// heuristic skips `(x & pow2) ==/!= 0`. deedlines tests `x AND 1`.
#[test]
fn test_a_single_bit_test_is_no_zero_compare() {
    for mask in ["1", "128"] {
        let (odds, at) = estimate(&format!(
            "define i16 @f(i16 %x) {{
entry:
  %b = and i16 %x, {mask}
  %c = icmp ne i16 %b, 0
  br i1 %c, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 0
}}
"
        ));
        assert_ne!(odds.by.get(&at("entry")), Some(&Heuristic::Zero), "and {mask}");
    }
    // A mask of several bits is a quantity: `x & 6 != 0` keeps the heuristic.
    let (odds, at) = estimate(
        "define i16 @f(i16 %x) {
entry:
  %b = and i16 %x, 6
  %c = icmp ne i16 %b, 0
  br i1 %c, label %yes, label %no
yes:
  ret i16 1
no:
  ret i16 0
}
",
    );
    assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Zero));
}

/// A loop's proven trips say how often it is left, not which way: tail
/// duplication gave SPHEREMAPLASMA's counted loop two exits, 3 to 5, and
/// each took half of the leaving mass, which sent more into a nest below
/// and read the procedure 2.7% hotter (#217).
#[test]
fn test_a_counted_loops_exits_share_by_their_odds() {
    let (latches, body) = (BTreeSet::from([2]), BTreeSet::from([2]));
    let cycles = [Cycle { header: 2, latches: &latches, body: &body, trips: Some(4) }];
    let successors = |at: i64| match at {
        1 => vec![2],
        2 => vec![2, 3, 4],
        _ => vec![],
    };
    let predecessors = |at: i64| match at {
        2 => vec![1, 2],
        3 | 4 => vec![2],
        _ => vec![],
    };
    let given = |from: i64, to: i64| match (from, to) {
        (1, 2) => 1.0,
        (2, 2) => 31.0 / 32.0,
        (2, 3) => 0.375 / 32.0,
        (2, 4) => 0.625 / 32.0,
        _ => 0.0,
    };
    let frequency = propagated(&[1, 2, 3, 4], &predecessors, &successors, &cycles, &given);
    assert!(close(frequency.get(&2).copied(), 4.0), "premise: the trips decide the loop: {:?}", frequency.get(&2));
    assert!(close(frequency.get(&3).copied(), 0.375), "{frequency:?}");
    assert!(close(frequency.get(&4).copied(), 0.625), "{frequency:?}");
}

fn weighted_branch(weights: &str, then_cold: bool) -> (Odds, i64, i64) {
    let body = if then_cold { "  call void @abort()\n  unreachable\n" } else { "  ret i16 1\n" };
    let (odds, at) = estimate(&format!(
        "declare void @abort()

define i16 @f(i16 %x) {{
entry:
  %c = icmp eq i16 %x, 7
  br i1 %c, label %then, label %else, !prof !0

then:
{body}
else:
  ret i16 2
}}

!0 = !{{!\"branch_weights\", {weights}}}
"
    ));
    (odds, at("then"), at("else"))
}

/// A program's `!prof` weights come before every heuristic: 1 to 2 is a third
/// and two thirds where the heuristics call the branch even, so a frontend
/// that knows a path is rare could not say so.
#[test]
fn test_branch_weights_set_the_odds_before_any_heuristic() {
    let (odds, then, other) = weighted_branch("i32 1, i32 2", false);
    let entry = odds.by.keys().next().copied().expect("a branch");
    assert_eq!(odds.by.get(&entry), Some(&Heuristic::Declared));
    assert!(close(odds.taken.get(&(entry, then)).copied(), 1.0 / 3.0), "{odds:?}");
    assert!(close(odds.taken.get(&(entry, other)).copied(), 2.0 / 3.0), "{odds:?}");
}

/// Weights that do not name every successor say nothing: the heuristics decide.
#[test]
fn test_branch_weights_that_do_not_fit_the_successors_are_ignored() {
    let (odds, ..) = weighted_branch("i32 1, i32 2, i32 3", false);
    let entry = odds.by.keys().next().copied().expect("a branch");
    assert_ne!(odds.by.get(&entry), Some(&Heuristic::Declared));
    let (odds, ..) = weighted_branch("i32 0, i32 0", false);
    assert_ne!(odds.by.get(&entry), Some(&Heuristic::Declared), "weights of nothing");
}

/// An invoke's unwind edge was weighed as any branch to a block that does not
/// return: two thirds. QB's `ON ERROR` makes every runtime call in its region
/// an invoke of its handler, so the handler ran twice for every three calls.
#[test]
fn test_an_invokes_unwind_edge_is_all_but_never_taken() {
    let (odds, at) = estimate(
        "declare i16 @g(i16)
declare i32 @__gxx_personality_v0(...)

define i16 @f(i16 %x) personality ptr @__gxx_personality_v0 {
entry:
  %r = invoke i16 @g(i16 %x) to label %ok unwind label %lp

ok:
  ret i16 %r

lp:
  %e = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %e
}
",
    );
    assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Invoke));
    assert!(odds.probability(at("entry"), at("lp")).is_some_and(|one| one < 1e-5), "{:?}", odds.taken);
}

/// A switch whose cases share a body (QB `CASE 1, 2`, a C fallthrough) names
/// that block twice: its weights add. The check that every target be
/// distinct dropped the weights of exactly these switches.
#[test]
fn test_switch_cases_to_one_block_weigh_together() {
    let (odds, at) = estimate(
        "define i16 @f(i16 %x) {
entry:
  switch i16 %x, label %other [
    i16 1, label %shared
    i16 2, label %shared
    i16 3, label %rare
  ], !prof !0

shared:
  ret i16 1

rare:
  ret i16 2

other:
  ret i16 3
}

!0 = !{!\"branch_weights\", i32 4, i32 30, i32 50, i32 16}
",
    );
    assert_eq!(odds.by.get(&at("entry")), Some(&Heuristic::Declared));
    assert!(close(odds.probability(at("entry"), at("shared")), 0.8), "{:?}", odds.taken);
    assert!(close(odds.probability(at("entry"), at("rare")), 0.16), "{:?}", odds.taken);
    assert!(close(odds.probability(at("entry"), at("other")), 0.04), "{:?}", odds.taken);
}

/// An inner loop's guard compares the outer counter (`4 <= i`, i from 1 for 4
/// trips) and holds on one trip in four. Given no odds by any heuristic, it
/// split the trips even, so lsr weighed the arm that enters the inner loop at
/// half its share and dropped nbody's stride-8 counter (#386).
#[test]
fn test_a_guard_on_an_enclosing_counter_is_taken_as_often_as_it_holds() {
    let text = "define i16 @f(i16 %n) {
entry:
  br label %outer
outer:
  %i = phi i16 [ 1, %entry ], [ %next, %latch ]
  %done = icmp ne i16 %i, 5
  br i1 %done, label %guard, label %out
guard:
  %skip = icmp sle i16 4, %i
  br i1 %skip, label %latch, label %enter
enter:
  %x = add i16 %i, %n
  br label %latch
latch:
  %next = add i16 %i, 1
  br label %outer
out:
  ret i16 %n
}
";
    let module = parsed(&format!("{DOS}{text}"));
    let function = function(&module, "f");
    let names: Vec<(String, i64)> = function.layout().iter().map(|&one| (function.block(one).name.clone().unwrap_or_default(), id(one))).collect();
    let at = move |name: &str| names.iter().find(|(one, _)| one == name).unwrap_or_else(|| panic!("no %{name}")).1;
    let odds = estimated(&module.context, &module.metadata, &module.globals, function, &Shape::of(function), &BTreeMap::from([(at("outer"), 4)]));
    assert_eq!(odds.by.get(&at("guard")), Some(&Heuristic::Counted), "{:?}", odds.by);
    assert!(close(odds.probability(at("guard"), at("enter")), 0.75), "{:?}", odds.taken);
    let (guessed, _) = estimate(text);
    assert!(close(guessed.probability(at("guard"), at("enter")), 0.5), "premise: with no counted loop the guard is even");
}

/// Every branch in a counted loop was decided by running the loop's counters through all its trips again: k branches in a loop of
/// n trips ran it k times (`nbody_single -Omax`: `peel` 22% of the compile, 17 points in `counted`). The counters' values at each
/// trip are the loop's, so the loop is run once and each branch reads them.
#[test]
fn test_a_counted_loop_is_run_once_for_all_the_branches_in_it() {
    let guards = 6;
    let mut text = String::from("define i16 @f(i16 %n) {\nentry:\n  br label %outer\nouter:\n  %i = phi i16 [ 1, %entry ], [ %next, %latch ]\n  %done = icmp ne i16 %i, 5\n  br i1 %done, label %g0, label %out\n");
    for k in 0..guards {
        let after = if k + 1 == guards { "latch".to_owned() } else { format!("g{}", k + 1) };
        text += &format!("g{k}:\n  %skip{k} = icmp sle i16 {k}, %i\n  br i1 %skip{k}, label %{after}, label %e{k}\ne{k}:\n  %x{k} = add i16 %i, %n\n  br label %{after}\n");
    }
    text += "latch:\n  %next = add i16 %i, 1\n  br label %outer\nout:\n  ret i16 %n\n}\n";
    let module = parsed(&format!("{DOS}{text}"));
    let function = function(&module, "f");
    let names: Vec<(String, i64)> = function.layout().iter().map(|&one| (function.block(one).name.clone().unwrap_or_default(), id(one))).collect();
    let at = move |name: &str| names.iter().find(|(one, _)| one == name).unwrap_or_else(|| panic!("no %{name}")).1;
    let before = loops_run();
    let odds = estimated(&module.context, &module.metadata, &module.globals, function, &Shape::of(function), &BTreeMap::from([(at("outer"), 4)]));
    assert_eq!(loops_run() - before, 1, "the loop is run once for {guards} branches");
    for k in 0..guards {
        assert_eq!(odds.by.get(&at(&format!("g{k}"))), Some(&Heuristic::Counted), "g{k}");
    }
    // `3 <= i` holds on 2 of the 4 trips.
    assert!(close(odds.probability(at("g3"), at("e3")), 0.5), "{:?}", odds.taken);
}
