//! Exits their counts decide, over loops the interpreter runs before and after.

use llrm_analysis::testing::DOS;
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::testing::{managed, parsed, results};

/// `exitfold::folded` as a pass of its own.
struct Folded;

impl FunctionPass for Folded {
    fn name(&self) -> &'static str {
        "exitfold"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if super::folded(unit.context, unit.layout, unit.function, &outer) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// `text` folded, its results on `inputs` checked against the original's.
fn folded(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut after = before.clone();
    let printed = managed(&mut after, Folded);
    assert_eq!(results(&after, inputs), results(&before, inputs), "{printed}");
    printed
}

/// `i` below `n`, then below `bound`, a count each: `i` from 0 by one.
fn checked(bound: &str) -> String {
    format!(
        "define i16 @f(i16 %n, i16 %len) {{
entry:
  %fits = icmp ult i16 %n, %len
  br i1 %fits, label %head, label %done
head:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %s = phi i16 [ 0, %entry ], [ %t, %body ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %check, label %done
check:
  %inside = icmp ult i16 %i, {bound}
  br i1 %inside, label %body, label %done
body:
  %t = add i16 %s, %i
  %j = add nuw i16 %i, 1
  br label %head
done:
  %r = phi i16 [ 0, %entry ], [ %s, %head ], [ 99, %check ]
  ret i16 %r
}}
"
    )
}

const INPUTS: &[&[i128]] = &[&[0, 5], &[3, 5], &[5, 9], &[4, 4], &[9, 3]];

/// The check below `len` never leaves where entry proved `n < len`: the
/// header's test leaves on that trip first.
#[test]
fn test_an_exit_the_guard_proves_later_never_leaves() {
    let printed = folded(&checked("%len"), INPUTS);
    assert!(printed.contains("br i1 true, label %body"), "{printed}");
}

/// A check with the header's own count never leaves: the header leaves
/// on that trip before it runs.
#[test]
fn test_an_exit_sharing_an_earlier_count_never_leaves() {
    let printed = folded(&checked("%n"), INPUTS);
    assert!(printed.contains("br i1 true, label %body"), "{printed}");
}

/// Constants decide it without a guard: `i < 7` after `i < 3` never leaves.
#[test]
fn test_an_exit_whose_constant_count_is_higher_never_leaves() {
    let text = checked("7").replace("icmp ult i16 %i, %n", "icmp ult i16 %i, 3");
    let printed = folded(&text, INPUTS);
    assert!(printed.contains("br i1 true, label %body"), "{printed}");
}

/// A test that fails at once leaves on the first trip.
#[test]
fn test_an_exit_counted_zero_leaves_at_once() {
    let text = checked("0").replace("icmp ult i16 %i, %n", "icmp ult i16 %i, 3");
    let printed = folded(&text, INPUTS);
    assert!(printed.contains("br i1 false, label %body"), "{printed}");
}

/// Nothing proves which of two lengths is shorter: both stay.
#[test]
fn test_exits_nothing_orders_keep_their_tests() {
    let text = checked("%len").replace(
        "  %fits = icmp ult i16 %n, %len\n  br i1 %fits, label %head, label %done",
        "  %fits = icmp ult i16 %n, 1000\n  br i1 %fits, label %head, label %done",
    );
    let printed = folded(&text, INPUTS);
    assert!(printed.contains("br i1 %inside"), "{printed}");
}

/// `i` below `n`, then at most `len` as signed numbers, where entry proved
/// `0 <= n <= len`. An inclusive signed test counts nothing: `len` may be
/// the largest number.
const UNCOUNTED: &str = "define i16 @f(i16 %n, i16 %len) {
entry:
  %fits = icmp sle i16 %n, %len
  %whole = icmp sge i16 %n, 0
  %both = and i1 %fits, %whole
  br i1 %both, label %head, label %done
head:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %s = phi i16 [ 0, %entry ], [ %t, %body ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %check, label %done
check:
  %inside = icmp sle i16 %i, %len
  br i1 %inside, label %body, label %done
body:
  %t = add i16 %s, %i
  %j = add i16 %i, 1
  br label %head
done:
  %r = phi i16 [ 0, %entry ], [ %s, %head ], [ 99, %check ]
  ret i16 %r
}
";

/// An exit no count decides is tested once, on the counter's start, where
/// the guards prove its test holds up to the loop's most trips: `i <= len`
/// on every trip below `n <= len` once `0 <= len`.
#[test]
fn test_an_uncounted_exit_is_tested_once_on_its_start() {
    let printed = folded(UNCOUNTED, &[&[0, 5], &[3, 5], &[5, 5], &[4, 9], &[-2, 3], &[9, 3]]);
    let check =
        printed.split("check:").nth(1).expect("the check block").split("\n\n").next().unwrap_or_default().to_owned();
    assert!(!check.contains("%i"), "{printed}");
}

/// `i` below `n` and below `len`, the loop reading `@g` and leaving by
/// either exit with nothing: `store` and `crash` fill the body and the
/// second exit's block.
fn scanned(
    store: &str,
    crash: &str,
) -> String {
    format!(
        "@g = global [64 x i16] zeroinitializer

declare void @stop() memory(none)

define i16 @f(i16 %n, i16 %len) {{
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %check, label %done
check:
  %inside = icmp ult i16 %i, %len
  br i1 %inside, label %body, label %bad
body:
  %p = getelementptr inbounds i16, ptr @g, i16 %i
  %v = load i16, ptr %p
{store}  %j = add nuw i16 %i, 1
  br label %head
done:
  ret i16 1
bad:
{crash}}}
"
    )
}

/// Where it would leave, on the first trip, as `i` reaches `n` or `len`.
const SCANS: &[&[i128]] = &[&[0, 5], &[3, 5], &[5, 5], &[7, 9]];

/// A loop that writes nothing and carries nothing out leaves on its first
/// trip by the exit it would have: each leaves where its count is the loop's.
#[test]
fn test_a_read_only_loop_leaves_on_its_first_trip() {
    let printed = folded(&scanned("", "  ret i16 2\n"), &[&[0, 5], &[3, 5], &[5, 5], &[7, 9], &[9, 4]]);
    for block in ["head:", "check:"] {
        let tested = printed.split(block).nth(1).expect(block).split("\n\n").next().unwrap_or_default().to_owned();
        assert!(!tested.contains("%i,"), "{printed}");
    }
}

/// A loop that stores, leaving by an exit that crashes at once touching no
/// memory, may leave there first: the stores are never seen. The bounds
/// check comes first; the ordinary exit after it keeps its test.
#[test]
fn test_a_storing_loop_leaves_first_where_its_exit_crashes() {
    let store = "  %q = getelementptr inbounds i16, ptr @g, i16 %n\n  store i16 %v, ptr %q\n";
    let text = scanned(store, "  call void @stop()\n  unreachable\n")
        .replace(
            "  %more = icmp ult i16 %i, %n\n  br i1 %more, label %check, label %done",
            "  %inside = icmp ult i16 %i, %len\n  br i1 %inside, label %check, label %bad",
        )
        .replace(
            "  %inside = icmp ult i16 %i, %len\n  br i1 %inside, label %body, label %bad",
            "  %more = icmp ult i16 %i, %n\n  br i1 %more, label %body, label %done",
        );
    let printed = folded(&text, &[&[0, 5], &[3, 5], &[4, 9]]);
    let tested = printed.split("head:").nth(1).expect("head").split("\n\n").next().unwrap_or_default().to_owned();
    assert!(!tested.contains("%i,"), "{printed}");
    assert!(printed.contains("icmp ult i16 %i, %n"), "{printed}");
    // The check is made once, ahead of the loop, and leaves for the crash.
    let ahead = printed.split("head:").next().unwrap_or_default();
    assert!(ahead.contains("br i1") && ahead.contains("label %bad"), "{printed}");
}

/// Stores seen after an ordinary exit keep every trip.
#[test]
fn test_a_storing_loop_with_an_ordinary_exit_keeps_its_trips() {
    let store = "  %q = getelementptr inbounds i16, ptr @g, i16 %n\n  store i16 %v, ptr %q\n";
    let printed = folded(&scanned(store, "  ret i16 2\n"), SCANS);
    assert!(printed.contains("icmp ult i16 %i, %len"), "{printed}");
}

/// `zip` over three lengths, as Nib lowers it: an exit per length, each
/// through its own LCSSA block into one join with the sum so far.
fn zipped(between: &str) -> String {
    format!(
        "define i16 @f(i16 %la, i16 %lb, i16 %lc) {{
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %s = phi i16 [ 0, %entry ], [ %t, %body ]
  %ina = icmp ult i16 %i, %la
  br i1 %ina, label %checkb, label %outa
checkb:
{between}  %inb = icmp ult i16 %i, %lb
  br i1 %inb, label %checkc, label %outb
checkc:
  %inc = icmp ult i16 %i, %lc
  br i1 %inc, label %body, label %outc
body:
  %t = add i16 %s, %i
  %j = add i16 %i, 1
  br label %head
outa:
  %sa = phi i16 [ %s, %head ]
  br label %done
outb:
  %sb = phi i16 [ %s, %checkb ]
  br label %done
outc:
  %sc = phi i16 [ %s, %checkc ]
  br label %done
done:
  %r = phi i16 [ %sa, %outa ], [ %sb, %outb ], [ %sc, %outc ]
  ret i16 %r
}}
"
    )
}

const LENGTHS: &[&[i128]] = &[&[0, 5, 5], &[3, 5, 9], &[5, 2, 9], &[9, 9, 4], &[7, 7, 7], &[4, 0, 3]];

/// Every exit to the same place with the same values joins the first: one
/// test against the least of all their counts, the others never leaving.
#[test]
fn test_exits_to_one_place_become_one_on_the_least_count() {
    let printed = folded(&zipped(""), LENGTHS);
    assert!(printed.contains("br i1 true, label %checkc") && printed.contains("br i1 true, label %body"), "{printed}");
    assert_eq!(printed.matches("select").count(), 2, "{printed}");
}

/// A store between two tests keeps the later one: leaving earlier would
/// skip it.
#[test]
fn test_an_exit_after_a_store_keeps_its_test() {
    let text = format!("@g = global i16 0\n\n{}", zipped("  store i16 %i, ptr @g\n"));
    let printed = folded(&text, LENGTHS);
    assert!(printed.contains("icmp ult i16 %i, %lb"), "{printed}");
}

/// An exit carrying another value out keeps its test.
#[test]
fn test_an_exit_carrying_another_value_keeps_its_test() {
    let printed =
        folded(&zipped("").replace("%sb = phi i16 [ %s, %checkb ]", "%sb = phi i16 [ %i, %checkb ]"), LENGTHS);
    assert!(printed.contains("icmp ult i16 %i, %lb"), "{printed}");
}

/// Nib's `for j in lo..hi: a[j]` after `a[hi]` passed: `hi` below the
/// length, `lo` below `hi` as signed numbers and maybe negative, a length
/// the frontend stated at most 32768. Failing, 99 stands for the panic.
const SLICE_LOOP: &str = "@cell = global i16 10

define i16 @f(i16 %lo, i16 %hi) {
entry:
  %len = load i16, ptr @cell, !range !0
  %order = icmp slt i16 %lo, %hi
  br i1 %order, label %first, label %done
first:
  %fits = icmp ult i16 %hi, %len
  br i1 %fits, label %pre, label %crash
pre:
  br label %head
head:
  %j = phi i16 [ %lo, %pre ], [ %next, %body ]
  %s = phi i16 [ 0, %pre ], [ %t, %body ]
  %more = icmp slt i16 %j, %hi
  br i1 %more, label %check, label %done
check:
  %inside = icmp ult i16 %j, %len
  br i1 %inside, label %body, label %crash
body:
  %t = add i16 %s, %j
  %next = add nsw i16 %j, 1
  br label %head
done:
  %r = phi i16 [ 0, %entry ], [ %s, %head ]
  ret i16 %r
crash:
  ret i16 99
}

!0 = !{i16 0, i16 -32768}
";

/// quicksort's `j` loop tested `j <u len` on every trip: with the length at
/// most 32768, `lo <u len` once settles it for each `j` from `lo` below
/// `hi`, which entry proved below `len`. It cost 8 instructions a trip.
#[test]
fn test_a_slice_loop_checks_its_start_not_every_trip() {
    let before = parsed(&format!("{DOS}{SLICE_LOOP}"));
    assert!(llrm_mir::print::module(&before).contains("icmp ult i16 %j, %len"), "premise: the check is in the loop");
    let printed = folded(SLICE_LOOP, &[&[0, 5], &[2, 9], &[-1, 4], &[3, 3], &[5, 2]]);
    assert!(!printed.contains("icmp ult i16 %j, %len"), "{printed}");
    // Taken before the loop: the loop's branch on it is a constant.
    assert!(printed.contains("icmp ult i16 %lo, %len") && printed.contains("br i1 true, label %body"), "{printed}");
}

/// A store before the check keeps it in the loop: a crash taken before the
/// loop would skip that store on the first trip. Its test is still made once.
#[test]
fn test_a_slice_loop_that_stores_before_its_check_keeps_the_branch_in_the_loop() {
    let text =
        format!("@seen = global i16 0\n\n{}", SLICE_LOOP.replace("check:\n", "check:\n  store i16 %j, ptr @seen\n"));
    let printed = folded(&text, &[&[0, 5], &[2, 9], &[-1, 4], &[3, 3], &[5, 2]]);
    assert!(!printed.contains("icmp ult i16 %j, %len"), "{printed}");
    assert!(printed.contains("br i1 %0, label %body"), "{printed}");
}

/// A loop whose exit block reads a value the loop made, with no phi to carry it: the exit tested once in the preheader
/// left the loop on its first trip, where that value is not made (the torture program 20010224-1 at -Os stored poison).
#[test]
fn test_an_exit_whose_block_reads_the_loop_s_values_is_not_tested_before_it() {
    let text = "define i16 @f(i16 %start) {
b0:
  br label %head
head:
  %i = phi i16 [ %start, %b0 ], [ %j, %body ]
  %s = phi i16 [ 0, %b0 ], [ %t, %body ]
  %more = icmp slt i16 %i, 4
  br i1 %more, label %body, label %done
body:
  %t = add i16 %s, %i
  %j = add nsw i16 %i, 1
  br label %head
done:
  ret i16 %s
}
";
    let printed = folded(text, &[&[-3], &[0], &[3], &[4], &[7]]);
    assert!(printed.contains("%s = phi"), "{printed}");
}
