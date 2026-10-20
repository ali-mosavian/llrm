//! `unroll_function` on loops the interpreter runs, before and after.

use llrm_mir::module::Module;
use llrm_x86::tti::X86;

use super::{Options, unroll_function};
use crate::testing::{parsed, printed, results};

/// @f's loops, the way the loop pass manager hands them over: simplified, LCSSA
/// form; whether any was unrolled (priced for an i486, `-O2`).
fn unrolled(module: &mut Module) -> bool {
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::lcssa::closed(function).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let options =
        Options { opt_level: 2, opt_size: false, min_size: false, only_when_forced: false, prepare_for_lto: false };
    unroll_function(module, "f", &tti, options).expect("unrolls")
}

const INPUTS: &[&[i128]] = &[&[1, 2], &[0, 5], &[-3, 7], &[100, -1], &[0, 0]];

/// `for (i = 0; i < trips; i++) acc += i * n`, the test at the bottom.
fn counted(trips: u32) -> String {
    format!(
        "define i16 @f(i16 %x, i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b1 ]
  %m = mul i16 %i, %n
  %s = add i16 %acc, %m
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, {trips}
  br i1 %go, label %b1, label %b2

b2:
  %r = phi i16 [ %s, %b1 ]
  ret i16 %r
}}
"
    )
}

#[test]
fn test_a_loop_of_few_trips_is_unrolled_and_computes_the_same() {
    let mut module = parsed(&counted(4));
    let before = results(&module, INPUTS);
    assert!(unrolled(&mut module));
    let text = printed(&module);
    assert_eq!(results(&module, INPUTS), before, "{text}");
    assert!(!text.contains("phi i16 [ 0"), "no loop phi is left: {text}");
}

/// The unrolled copies are simplified: the induction variable's `add 0, 1` and
/// the exit test on a constant do not stay behind (opt's
/// `simplifyLoopAfterUnroll`).
#[test]
fn test_the_unrolled_copies_hold_no_arithmetic_on_constants() {
    let mut module = parsed(&counted(4));
    assert!(unrolled(&mut module));
    let text = printed(&module);
    assert!(!text.contains("add i16 0, 1") && !text.contains("icmp slt i16 1, 4"), "{text}");
}

#[test]
fn test_a_loop_of_many_trips_is_kept() {
    let mut module = parsed(&counted(500));
    assert!(!unrolled(&mut module));
}

/// `for (o < 8) for (j < 8) a[o * 8 + j] = 0`: the inner loop is unrolled, and
/// the outer one after it, because its cost does not count the branches whose
/// way is known. Counted as side effects they made the unrolled cost 225 and
/// the loop was kept, where opt's is 209 and unrolls it.
const NESTED_FILL: &str = "define i16 @f(i16 %x, i16 %n) {
b0:
  %a = alloca [256 x i8], align 1
  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 256, i1 false)
  br label %b2

b2:
  %o = phi i16 [ 0, %b0 ], [ %o1, %b9 ]
  %c = icmp slt i16 %o, 8
  br i1 %c, label %b3, label %b5

b3:
  %s1 = shl i16 %o, 3
  %s2 = shl i16 %s1, 2
  br label %b6

b5:
  %p = getelementptr inbounds i32, ptr %a, i16 26
  %v = load i32, ptr %p, align 2
  %r = trunc i32 %v to i16
  ret i16 %r

b6:
  %at = phi i16 [ %s2, %b3 ], [ %at1, %b7 ]
  %j = phi i16 [ 0, %b3 ], [ %j1, %b7 ]
  %d = icmp slt i16 %j, 8
  br i1 %d, label %b7, label %b9

b7:
  %q = getelementptr inbounds i8, ptr %a, i16 %at
  store i32 %n_ext, ptr %q, align 2
  %j1 = add i16 %j, 1
  %at1 = add i16 %at, 4
  br label %b6

b9:
  %o1 = add i16 %o, 1
  br label %b2
}

declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
";

#[test]
fn test_branches_of_a_known_way_do_not_keep_an_outer_loop_from_unrolling() {
    let text = NESTED_FILL.replace("%n_ext", "0");
    let mut module = parsed(&text);
    assert!(unrolled(&mut module));
    let text = printed(&module);
    assert!(!text.contains("phi i16 [ 0"), "both loops are unrolled: {text}");
}

/// A function of `n` loops in sequence, each starting from the last one's
/// result (a value of `n` uses is `n^2` to remove one use at a time): unrolling
/// them looks over entries in proportion to `n`, not to its square (a count,
/// not seconds: time fails under load).
fn sequential(n: usize) -> String {
    let mut text = String::from("define i16 @f(i16 %x, i16 %y) {\nb0:\n  br label %l0\n\n");
    for k in 0..n {
        let next = if k + 1 == n { "end".to_owned() } else { format!("l{}", k + 1) };
        text += &format!(
            "l{k}:\n  %i{k} = phi i16 [ 0, %{pre} ], [ %n{k}, %l{k} ]\n  %a{k} = phi i16 [ %{init}, %{pre} ], [ %s{k}, %l{k} ]\n  %s{k} = add i16 %a{k}, %i{k}\n  %n{k} = add i16 %i{k}, 1\n  %g{k} = icmp slt i16 %n{k}, 4\n  br i1 %g{k}, label %l{k}, label %x{k}\n\nx{k}:\n  %r{k} = phi i16 [ %s{k}, %l{k} ]\n  br label %{next}\n\n",
            init = if k == 0 { "x".to_owned() } else { format!("r{}", k - 1) },
            pre = if k == 0 { "b0".to_owned() } else { format!("x{}", k - 1) },
        );
    }
    text += &format!("end:\n  ret i16 %r{}\n}}\n", n - 1);
    text
}

#[test]
fn test_unrolling_loops_in_sequence_scans_in_proportion_to_their_number() {
    use llrm_mir::edit::layout_scanned;
    let scanned = |n: usize| {
        let mut module = parsed(&sequential(n));
        let before = layout_scanned();
        assert!(unrolled(&mut module));
        printed(&module);
        layout_scanned() - before
    };
    let (a, b, c) = (scanned(64), scanned(128), scanned(256));
    assert!(b * 100 <= a * 250 && c * 100 <= b * 250, "64: {a}, 128: {b}, 256: {c}");
}

/// `LoopUnrollPass::run` puts every loop in simplified form first (L1762); a
/// loop with no preheader and a shared exit was skipped as "not in
/// loop-simplify form", so unroll@753 left what `opt` unrolled (bench's
/// oracle at i486).
#[test]
fn test_a_loop_not_in_simplified_form_is_simplified_then_unrolled() {
    let text = "define i16 @f(i16 %x, i16 %g) {
b0:
  %c = icmp eq i16 %g, 0
  br i1 %c, label %b2, label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b1 ]
  %s = add i16 %acc, %i
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 4
  br i1 %go, label %b1, label %b2

b2:
  %r = phi i16 [ %x, %b0 ], [ %s, %b1 ]
  ret i16 %r
}
";
    let mut module = parsed(text);
    let before = parsed(text);
    let function = module.function_mut("f").expect("@f").1;
    assert!(crate::loopsimplify::simplified(&mut function.clone()), "the input is in simplified form already");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let options =
        Options { opt_level: 2, opt_size: false, min_size: false, only_when_forced: false, prepare_for_lto: false };
    assert!(unroll_function(&mut module, "f", &tti, options).expect("unrolls"), "{}", printed(&module));
    assert_eq!(results(&module, &[&[1, 0], &[1, 1], &[-3, 7]]), results(&before, &[&[1, 0], &[1, 1], &[-3, 7]]));
}

/// `counted(trips)` with `hint` on the latch's loop ID.
fn hinted(
    trips: u32,
    hint: &str,
) -> String {
    let text =
        counted(trips).replace("br i1 %go, label %b1, label %b2", "br i1 %go, label %b1, label %b2, !llvm.loop !0");
    format!("{text}\n!0 = distinct !{{!0, !1}}\n!1 = !{{{hint}}}\n")
}

/// The language says never (`llvm.loop.unroll.disable`): a loop the budget
/// would copy stays rolled, and the same loop with a count that is its trips is
/// copied.
#[test]
fn a_loop_the_language_says_not_to_unroll_stays_rolled() {
    let mut disabled = parsed(&hinted(4, "!\"llvm.loop.unroll.disable\""));
    assert!(!unrolled(&mut disabled), "{}", printed(&disabled));
    let mut counted_four = parsed(&hinted(4, "!\"llvm.loop.unroll.count\", i32 4"));
    assert!(unrolled(&mut counted_four));
}

/// The language says as many as the trip count (`llvm.loop.unroll.full`): 200
/// trips are past the threshold of 150, which a pragma overrides.
#[test]
fn a_loop_the_language_permits_is_copied_past_the_budget_and_the_cap() {
    let mut plain = parsed(&counted(200));
    assert!(!unrolled(&mut plain), "premise: 200 trips are over the budget");
    let mut full = parsed(&hinted(200, "!\"llvm.loop.unroll.full\""));
    assert!(unrolled(&mut full));
}
