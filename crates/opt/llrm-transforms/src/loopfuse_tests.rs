//! Fusing on the interpreter: the answers before and after, and the loops.

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_mir::module::Module;

use super::Fuse;
use crate::testing::{f, managed, parsed, printed, results};

const INPUTS: &[&[i128]] = &[&[0], &[1], &[3], &[8], &[-2]];

fn loop_count(module: &mut Module) -> usize {
    let function = f(module);
    loops::loops(&cfg::graph(function), function.entry().map(cfg::id)).len()
}

/// `text` through the pass: the module, and how many loops went.
fn fused(text: &str) -> (Module, usize) {
    let mut module = parsed(text);
    let before = loop_count(&mut module);
    managed(&mut module, Fuse { passes: Vec::new() });
    let gone = before - loop_count(&mut module);
    (module, gone)
}

/// The answers of `text` and what the pass makes of it, over `INPUTS`.
fn same_answers(
    module: &Module,
    text: &str,
) {
    assert_eq!(results(module, INPUTS), results(&parsed(text), INPUTS), "{}", printed(module));
}

/// Two loops tested before each trip: `a[i + 8] = i * 3` for `%i` below `%n`,
/// then `second`, over `%j` below `%bound`, which reads `%src`, `a` at
/// `src_at` bytes, and writes `%dst`, `b[j + 8]`.
fn pair(
    src_at: i16,
    bound: &str,
    second: &str,
) -> String {
    // A bound that is not `%n` is made between the loops.
    let (bound, between) = match bound {
        "%n" => ("%n", ""),
        other => ("%bound", other),
    };
    format!(
        "define i16 @f(i16 %n) {{
b0:
  %a = alloca [64 x i16]
  %b = alloca [64 x i16]
  store i16 5, ptr %a
  br label %h0

h0:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %body0 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %body0, label %mid

body0:
  %o = mul i16 %i, 2
  %p = getelementptr i8, ptr %a, i16 %o
  %p8 = getelementptr i8, ptr %p, i16 16
  %v = mul i16 %i, 3
  store i16 %v, ptr %p8
  %i.next = add i16 %i, 1
  br label %h0

mid:
{between}
  br label %h1

h1:
  %j = phi i16 [ 0, %mid ], [ %j.next, %body1 ]
  %more1 = icmp slt i16 %j, {bound}
  br i1 %more1, label %body1, label %done

body1:
  %q = mul i16 %j, 2
  %at = getelementptr i8, ptr %a, i16 %q
  %src = getelementptr i8, ptr %at, i16 {src_at}
  %bt = getelementptr i8, ptr %b, i16 %q
  %dst = getelementptr i8, ptr %bt, i16 16
{second}
  %j.next = add i16 %j, 1
  br label %h1

done:
  %bx = getelementptr i8, ptr %b, i16 16
  %x = load i16, ptr %bx
  %ax = getelementptr i8, ptr %a, i16 16
  %y = load i16, ptr %ax
  %z = add i16 %x, %y
  ret i16 %z
}}
"
    )
}

const COPY_PLUS_ONE: &str = "  %w = load i16, ptr %src
  %w1 = add i16 %w, 1
  store i16 %w1, ptr %dst";

/// `b[j] = a[j] + 1` reads what the first loop wrote in the same trip: one
/// loop does both, as the interpreter's answers say.
#[test]
fn two_loops_over_one_range_become_one() {
    let text = pair(16, "%n", COPY_PLUS_ONE);
    let (module, gone) = fused(&text);
    assert_eq!(gone, 1, "{}", printed(&module));
    same_answers(&module, &text);
}

/// The second loop reading what the first wrote one trip before is fused; one
/// trip after is not: the first loop's next trip, run later, would write it.
#[test]
fn a_read_of_the_trip_before_is_fused_and_of_the_trip_after_is_not() {
    for (src_at, wanted) in [(14, 1), (16, 1), (18, 0)] {
        let text = pair(src_at, "%n", COPY_PLUS_ONE);
        let (module, gone) = fused(&text);
        assert_eq!(gone, wanted, "src at {src_at}: {}", printed(&module));
        same_answers(&module, &text);
    }
}

/// A second loop that writes what the first will read is no different.
#[test]
fn a_write_over_what_the_first_loop_reads_later_is_not_fused() {
    // The first loop reads `a[i + 8]` and the second writes `a[j + 9]`.
    let text = pair(18, "%n", "  store i16 7, ptr %src").replace(
        "  store i16 %v, ptr %p8",
        "  %old = load i16, ptr %p8\n  %new = add i16 %old, %v\n  store i16 %new, ptr %p8",
    );
    let (module, gone) = fused(&text);
    assert_eq!(gone, 0, "{}", printed(&module));
    same_answers(&module, &text);
}

/// Another bound is another count: one trip more in the second loop.
#[test]
fn loops_of_different_trips_are_not_fused() {
    let text = pair(16, "  %bound = add i16 %n, 1", COPY_PLUS_ONE);
    let (module, gone) = fused(&text);
    assert_eq!(gone, 0, "{}", printed(&module));
    same_answers(&module, &text);
}

/// A sum the first loop leaves, stored by every trip of the second, is the
/// final sum: fused, the second would store a part of it.
#[test]
fn a_value_the_first_loop_leaves_is_not_read_by_the_second() {
    let text = pair(16, "%n", "  store i16 %total, ptr %dst")
        .replace(
            "%i = phi i16 [ 0, %b0 ], [ %i.next, %body0 ]",
            "%i = phi i16 [ 0, %b0 ], [ %i.next, %body0 ]\n  %total = phi i16 [ 0, %b0 ], [ %sum, %body0 ]",
        )
        .replace(
            "  %i.next = add i16 %i, 1\n  br label %h0",
            "  %sum = add i16 %total, %i\n  %i.next = add i16 %i, 1\n  br label %h0",
        );
    let (module, gone) = fused(&text);
    assert_eq!(gone, 0, "{}", printed(&module));
    same_answers(&module, &text);
}

/// What the first loop leaves for the code after both is not for the second
/// either; here the loops after it only read it.
#[test]
fn a_value_the_first_loop_leaves_for_the_code_after_is_not_fused() {
    let text = pair(16, "%n", COPY_PLUS_ONE)
        .replace(
            "%i = phi i16 [ 0, %b0 ], [ %i.next, %body0 ]",
            "%i = phi i16 [ 0, %b0 ], [ %i.next, %body0 ]\n  %total = phi i16 [ 0, %b0 ], [ %sum, %body0 ]",
        )
        .replace(
            "  %i.next = add i16 %i, 1\n  br label %h0",
            "  %sum = add i16 %total, %i\n  %i.next = add i16 %i, 1\n  br label %h0",
        )
        .replace(
            "  %z = add i16 %x, %y\n  ret i16 %z",
            "  %z = add i16 %x, %y\n  %r = add i16 %z, %total\n  ret i16 %r",
        );
    let (module, gone) = fused(&text);
    assert_eq!(gone, 0, "{}", printed(&module));
    same_answers(&module, &text);
}

/// A call may touch anything the loops do.
#[test]
fn a_call_in_the_second_loop_is_not_fused() {
    let text = format!("declare void @g()\n{}", pair(16, "%n", "  call void @g()"));
    let (module, gone) = fused(&text);
    assert_eq!(gone, 0, "{}", printed(&module));
}

/// A second loop's header work that is read after it was computed on the way
/// out: fused, the test no longer computes it there.
#[test]
fn a_value_of_the_second_header_read_after_it_is_not_fused() {
    let text = pair(16, "%n", COPY_PLUS_ONE)
        .replace("  %x = load i16, ptr %bx", "  %x = add i16 %more1.copy, 0")
        .replace("  %more1 = icmp", "  %more1.copy = mul i16 %j, 3\n  %more1 = icmp");
    let (module, gone) = fused(&text);
    assert_eq!(gone, 0, "{}", printed(&module));
}

/// Three loops of one range are one loop.
#[test]
fn three_loops_over_one_range_become_one() {
    let text = pair(16, "%n", COPY_PLUS_ONE);
    let third = "
done3:
  br label %h2

h2:
  %k = phi i16 [ 0, %done3 ], [ %k.next, %body2 ]
  %more2 = icmp slt i16 %k, %n
  br i1 %more2, label %body2, label %done

body2:
  %kq = mul i16 %k, 2
  %kb = getelementptr i8, ptr %b, i16 %kq
  %kb8 = getelementptr i8, ptr %kb, i16 16
  %kv = load i16, ptr %kb8
  %kw = add i16 %kv, 2
  store i16 %kw, ptr %kb8
  %k.next = add i16 %k, 1
  br label %h2
";
    let text = text.replace("br i1 %more1, label %body1, label %done", "br i1 %more1, label %body1, label %done3") + "";
    let text = text.replace("\ndone:\n", &format!("{third}\ndone:\n"));
    let (module, gone) = fused(&text);
    assert_eq!(gone, 2, "{}", printed(&module));
    same_answers(&module, &text);
}

/// Two loops tested after each trip, both entered behind `%first` and
/// `%second`, over `%n` trips (or eight where `bound` is `8`, which needs no
/// guard): `a[i + 8] = i * 3`, then `b[j + 8] = a[j + 8] + 1`.
fn rotated(
    first: &str,
    second: &str,
) -> String {
    format!(
        "define i16 @f(i16 %n) {{
b0:
  %a = alloca [64 x i16]
  %b = alloca [64 x i16]
  store i16 5, ptr %a
  %g0 = icmp {first}
  br i1 %g0, label %pre0, label %g1

pre0:
  br label %l0

l0:
  %i = phi i16 [ 0, %pre0 ], [ %i.next, %l0 ]
  %o = mul i16 %i, 2
  %p = getelementptr i8, ptr %a, i16 %o
  %p8 = getelementptr i8, ptr %p, i16 16
  %v = mul i16 %i, 3
  store i16 %v, ptr %p8
  %i.next = add i16 %i, 1
  %c0 = icmp slt i16 %i.next, %n
  br i1 %c0, label %l0, label %e0

e0:
  br label %g1

g1:
  %g = icmp {second}
  br i1 %g, label %pre1, label %done

pre1:
  br label %l1

l1:
  %j = phi i16 [ 0, %pre1 ], [ %j.next, %l1 ]
  %q = mul i16 %j, 2
  %at = getelementptr i8, ptr %a, i16 %q
  %src = getelementptr i8, ptr %at, i16 16
  %bt = getelementptr i8, ptr %b, i16 %q
  %dst = getelementptr i8, ptr %bt, i16 16
  %w = load i16, ptr %src
  %w1 = add i16 %w, 1
  store i16 %w1, ptr %dst
  %j.next = add i16 %j, 1
  %c1 = icmp slt i16 %j.next, %n
  br i1 %c1, label %l1, label %e1

e1:
  br label %done

done:
  %bx = getelementptr i8, ptr %b, i16 16
  %x = load i16, ptr %bx
  %ax = getelementptr i8, ptr %a, i16 16
  %y = load i16, ptr %ax
  %z = add i16 %x, %y
  ret i16 %z
}}
"
    )
}

/// Two loops entered behind one test, each tested after its trips, are one
/// loop behind that test.
#[test]
fn two_loops_behind_one_test_become_one() {
    let text = rotated("sgt i16 %n, 0", "sgt i16 %n, 0");
    let (module, gone) = fused(&text);
    assert_eq!(gone, 1, "{}", printed(&module));
    same_answers(&module, &text);
}

/// The second loop's own test is read the same way round or not. Both guards
/// weigh 1:1: `BranchProbabilityInfo`'s zero heuristic reads `sgt %n, 0` and
/// not `slt 0, %n` (InstCombine makes the one the other before LLVM's loop
/// passes see it), and the price would favour the unfused pair.
#[test]
fn the_second_test_may_be_the_first_written_another_way() {
    let text = rotated("sgt i16 %n, 0", "slt i16 0, %n")
        .replace("label %pre0, label %g1\n", "label %pre0, label %g1, !prof !0\n")
        .replace("label %pre1, label %done\n", "label %pre1, label %done, !prof !0\n")
        + "\n!0 = !{!\"branch_weights\", i32 1, i32 1}\n";
    assert_eq!(text.matches("!prof !0").count(), 2, "the premise: both guards weighed");
    let (module, gone) = fused(&text);
    assert_eq!(gone, 1, "{}", printed(&module));
    same_answers(&module, &text);
}

/// A second test the first does not settle: for `%n` of 1 the second loop
/// would be skipped, which fused it is not. The answers differ if it fuses.
#[test]
fn a_second_test_the_first_does_not_prove_is_not_fused() {
    for second in ["sgt i16 %n, 1", "ne i16 %n, 5", "sgt i16 %n, 2"] {
        let text = rotated("sgt i16 %n, 0", second);
        let (module, gone) = fused(&text);
        assert_eq!(gone, 0, "{second}: {}", printed(&module));
        same_answers(&module, &text);
    }
}

/// Setup that may trap is not run earlier than it was: a division by `%n`
/// between the loops would fault before the first loop's work.
#[test]
fn setup_that_may_trap_is_not_moved_before_the_first_loop() {
    let text = pair(16, "%n", COPY_PLUS_ONE)
        .replace("mid:\n\n", "mid:\n  %t = udiv i16 100, %n\n")
        .replace("  store i16 %w1, ptr %dst", "  %w2 = add i16 %w1, %t\n  store i16 %w2, ptr %dst");
    let (module, gone) = fused(&text);
    assert_eq!(gone, 0, "{}", printed(&module));
}

/// `a[i] = i`, then `b[j] = a[j]` over `%n` trips, through two pointer
/// parameters of the given kind.
fn through_parameters(kind: &str) -> String {
    format!(
        "define void @f(ptr {kind} %a, ptr {kind} %b, i16 %n) {{
b0:
  br label %h0

h0:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %body0 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %body0, label %mid

body0:
  %o = mul i16 %i, 2
  %p = getelementptr i8, ptr %a, i16 %o
  store i16 %i, ptr %p
  %i.next = add i16 %i, 1
  br label %h0

mid:
  br label %h1

h1:
  %j = phi i16 [ 0, %mid ], [ %j.next, %body1 ]
  %more1 = icmp slt i16 %j, %n
  br i1 %more1, label %body1, label %done

body1:
  %q = mul i16 %j, 2
  %src = getelementptr i8, ptr %a, i16 %q
  %dst = getelementptr i8, ptr %b, i16 %q
  %w = load i16, ptr %src
  store i16 %w, ptr %dst
  %j.next = add i16 %j, 1
  br label %h1

done:
  ret void
}}
"
    )
}

/// Two `noalias` parameters are different objects; two plain ones may be one,
/// and fusing might then read what the first loop has yet to write.
#[test]
fn pointers_fuse_where_they_cannot_alias() {
    let (module, gone) = fused(&through_parameters("noalias"));
    assert_eq!(gone, 1, "{}", printed(&module));
    let (module, gone) = fused(&through_parameters(""));
    assert_eq!(gone, 0, "{}", printed(&module));
}

/// Two loops of eight trips tested after each, with no test before either:
/// one loop of eight.
#[test]
fn two_loops_of_a_known_count_tested_after_their_trips_become_one() {
    let text = rotated("sgt i16 %n, 0", "sgt i16 %n, 0")
        .replace("  %g0 = icmp sgt i16 %n, 0\n  br i1 %g0, label %pre0, label %g1\n", "  br label %pre0\n")
        .replace("  %g = icmp sgt i16 %n, 0\n  br i1 %g, label %pre1, label %done\n", "  br label %pre1\n")
        .replace("%c0 = icmp slt i16 %i.next, %n", "%c0 = icmp slt i16 %i.next, 8")
        .replace("%c1 = icmp slt i16 %j.next, %n", "%c1 = icmp slt i16 %j.next, 8");
    let (module, gone) = fused(&text);
    assert_eq!(gone, 1, "{}", printed(&module));
    same_answers(&module, &text);
}

/// A body with a branch in it is fused whole.
#[test]
fn a_first_loop_with_a_branch_in_its_body_is_fused_whole() {
    let text = pair(16, "%n", COPY_PLUS_ONE)
        .replace("[ %i.next, %body0 ]", "[ %i.next, %join ]")
        .replace(
            "  store i16 %v, ptr %p8\n",
            "  %odd = and i16 %i, 1\n  %is = icmp eq i16 %odd, 0\n  br i1 %is, label %even, label %other\n\neven:\n  store i16 %v, ptr %p8\n  br label %join\n\nother:\n  %v2 = add i16 %v, 100\n  store i16 %v2, ptr %p8\n  br label %join\n\njoin:\n",
        );
    let (module, gone) = fused(&text);
    assert_eq!(gone, 1, "{}", printed(&module));
    same_answers(&module, &text);
}

/// Two loops of one outer loop's body: the outer loop stays, its inner ones
/// are one.
#[test]
fn two_loops_inside_a_third_are_fused_and_the_third_stays() {
    let text = "define i16 @f(i16 %n) {
b0:
  %a = alloca [64 x i16]
  %b = alloca [64 x i16]
  store i16 5, ptr %a
  br label %outer

outer:
  %t = phi i16 [ 0, %b0 ], [ %t.next, %latch ]
  %sum = phi i16 [ 0, %b0 ], [ %sum.next, %latch ]
  %again = icmp slt i16 %t, 3
  br i1 %again, label %pre, label %exit

pre:
  br label %h0

h0:
  %i = phi i16 [ 0, %pre ], [ %i.next, %body0 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %body0, label %mid

body0:
  %o = mul i16 %i, 2
  %p = getelementptr i8, ptr %a, i16 %o
  %v = add i16 %i, %t
  store i16 %v, ptr %p
  %i.next = add i16 %i, 1
  br label %h0

mid:
  br label %h1

h1:
  %j = phi i16 [ 0, %mid ], [ %j.next, %body1 ]
  %more1 = icmp slt i16 %j, %n
  br i1 %more1, label %body1, label %latch

body1:
  %q = mul i16 %j, 2
  %src = getelementptr i8, ptr %a, i16 %q
  %dst = getelementptr i8, ptr %b, i16 %q
  %w = load i16, ptr %src
  %w1 = add i16 %w, 1
  store i16 %w1, ptr %dst
  %j.next = add i16 %j, 1
  br label %h1

latch:
  %x = load i16, ptr %b
  %sum.next = add i16 %sum, %x
  %t.next = add i16 %t, 1
  br label %outer

exit:
  ret i16 %sum
}
";
    let (module, gone) = fused(text);
    assert_eq!(gone, 1, "{}", printed(&module));
    same_answers(&module, text);
}

/// A header tested before each trip runs once more than the body: a store in
/// the second loop's header is made `n + 1` times, and fused it would be made
/// `n`. The `b[n]` it writes last is read after the loops.
#[test]
fn a_store_in_the_second_header_is_not_fused() {
    let text = pair(16, "%n", COPY_PLUS_ONE).replace(
        "  %more1 = icmp",
        "  %jb = mul i16 %j, 2\n  %jh = getelementptr i8, ptr %b, i16 %jb\n  %jh8 = getelementptr i8, ptr %jh, i16 16\n  store i16 77, ptr %jh8\n  %more1 = icmp",
    ).replace("  %x = load i16, ptr %bx", "  %nb = mul i16 %n, 2\n  %nbp = getelementptr i8, ptr %b, i16 %nb\n  %nbq = getelementptr i8, ptr %nbp, i16 16\n  %x = load i16, ptr %nbq");
    let (module, gone) = fused(&text);
    assert_eq!(gone, 0, "{}", printed(&module));
    same_answers(&module, &text);
}

/// The first loop's header, run once more after its last trip, comes before
/// the second loop; fused it would come after it.
#[test]
fn a_store_in_the_first_header_is_not_fused() {
    let text = pair(16, "%n", COPY_PLUS_ONE).replace(
        "  %more = icmp",
        "  %ho = mul i16 %i, 2\n  %hp = getelementptr i8, ptr %a, i16 %ho\n  %hq = getelementptr i8, ptr %hp, i16 16\n  store i16 55, ptr %hq\n  %more = icmp",
    );
    let (module, gone) = fused(&text);
    assert_eq!(gone, 0, "{}", printed(&module));
    same_answers(&module, &text);
}

/// A division in either body may trap, and where it traps decides what the
/// other loop has done by then.
#[test]
fn a_loop_that_may_trap_is_not_fused() {
    let text = pair(16, "%n", COPY_PLUS_ONE).replace("  %w1 = add i16 %w, 1", "  %w1 = sdiv i16 %w, %n");
    let (module, gone) = fused(&text);
    assert_eq!(gone, 0, "{}", printed(&module));
}

/// Another way into the blocks between the loops reaches the second guard,
/// which must still be asked: `%n` of 3 enters the second loop alone.
#[test]
fn a_join_between_the_loops_keeps_the_second_test() {
    let text = rotated("sgt i16 %n, 0", "sgt i16 %n, 0")
        .replace(
            "  %g0 = icmp sgt i16 %n, 0\n  br i1 %g0, label %pre0, label %g1\n",
            "  %three = icmp eq i16 %n, 3\n  br i1 %three, label %y, label %fg\n\ny:\n  br label %bb\n\nfg:\n  %g0 = icmp sgt i16 %n, 0\n  br i1 %g0, label %pre0, label %g1\n",
        )
        .replace("e0:\n  br label %g1", "e0:\n  br label %bb\n\nbb:\n  br label %g1");
    let (module, gone) = fused(&text);
    assert_eq!(gone, 0, "{}", printed(&module));
    same_answers(&module, &text);
}

/// A guarded pair tested before each trip is left alone.
#[test]
fn a_guarded_pair_tested_before_each_trip_is_not_fused() {
    let text = pair(16, "%n", COPY_PLUS_ONE);
    let guarded = text
        .replace("  br label %h0\n\nh0:", "  %g0 = icmp sgt i16 %n, 0\n  br i1 %g0, label %pre0, label %g1\n\npre0:\n  br label %h0\n\nh0:")
        .replace("mid:\n\n  br label %h1", "mid:\n  br label %g1\n\ng1:\n  %g = icmp sgt i16 %n, 0\n  br i1 %g, label %pre1, label %done\n\npre1:\n  br label %h1")
        .replace("[ 0, %b0 ], [ %i.next", "[ 0, %pre0 ], [ %i.next")
        .replace("[ 0, %mid ], [ %j.next", "[ 0, %pre1 ], [ %j.next")
        .replace("br i1 %more1, label %body1, label %done", "br i1 %more1, label %body1, label %exit1")
        .replace("\ndone:\n", "\nexit1:\n  br label %done\n\ndone:\n");
    let (module, gone) = fused(&guarded);
    assert_eq!(gone, 0, "{}", printed(&module));
}
