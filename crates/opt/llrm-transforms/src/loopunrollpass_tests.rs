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

/// GORILLA.BAS (qb45 -O2 m32, -O3): an `ON ERROR` pad inside a counted loop
/// was copied with each unrolled iteration, and `ehprepare` refuses a second
/// landing pad in a function ("@PLACEGORILLAS: more than one landing pad").
#[test]
fn test_a_loop_with_a_landing_pad_is_not_unrolled() {
    let mut module = parsed(
        "declare void @g()
declare i32 @__gxx_personality_v0(...)

define i16 @f(i16 %x, i16 %n) personality ptr @__gxx_personality_v0 {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  invoke void @g() to label %ok unwind label %pad

ok:
  br label %b3

pad:
  %lp = landingpad { ptr, i32 } cleanup
  br label %b3

b3:
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 4
  br i1 %go, label %b1, label %b2

b2:
  ret i16 %x
}
",
    );
    let unrolled = unrolled(&mut module);
    let text = printed(&module);
    assert!(!unrolled, "{text}");
    assert_eq!(text.matches("landingpad").count(), 1, "{text}");
}

/// An outer loop of two trips round an inner loop of unknown count: unrolling
/// the outer copies the inner loop. The loop pass manager hands the copies to
/// the passes after the unroll.
const OUTER_TWICE: &str = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %o

o:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %ol ]
  %acc = phi i16 [ %x, %b0 ], [ %r, %ol ]
  br label %h

h:
  %j = phi i16 [ 0, %o ], [ %j.next, %h ]
  %a = phi i16 [ %acc, %o ], [ %a.next, %h ]
  %a.next = add i16 %a, %j
  %j.next = add i16 %j, 1
  %go = icmp slt i16 %j.next, %n
  br i1 %go, label %h, label %ol

ol:
  %r = phi i16 [ %a.next, %h ]
  %i.next = add i16 %i, 1
  %again = icmp slt i16 %i.next, 2
  br i1 %again, label %o, label %out

out:
  %done = phi i16 [ %r, %ol ]
  ret i16 %done
}
";

#[test]
fn a_full_unroll_hands_over_the_loops_it_made() {
    let mut module = parsed(OUTER_TWICE);
    let created = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let text = crate::testing::managed(
        &mut module,
        super::LoopFullUnroll {
            opt_level: 2,
            opt_size: false,
            only_when_forced: false,
            created: std::rc::Rc::clone(&created),
        },
    );
    assert!(!text.contains("%i = phi"), "the outer loop is gone: {text}");
    assert!(!created.borrow().is_empty(), "the copy of the inner loop is a loop the unroll made: {text}");
    assert_eq!(created.borrow().len(), 2, "the copy and the first trip's own loop: {:?}: {text}", created.borrow());
}

/// The first trip of an unrolled loop keeps its inner loop's blocks, and
/// `addSiblingLoops` hands that loop on with the copies. Left out, shellsort's
/// `while (at >= 32 && ...)` after the `gap = 32` trip kept its one-trip loop
/// (28 blocks to the pin's 25).
#[test]
fn a_full_unroll_hands_over_the_inner_loop_of_its_first_trip() {
    let mut module = parsed(OUTER_TWICE);
    let created = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    crate::testing::managed(
        &mut module,
        super::LoopFullUnroll {
            opt_level: 2,
            opt_size: false,
            only_when_forced: false,
            created: std::rc::Rc::clone(&created),
        },
    );
    // The text has five blocks, b0 to out: a loop headed by one of them is the
    // first trip's own.
    assert!(created.borrow().iter().any(|header| header.0 < 5), "{:?}", created.borrow());
}

/// `while (phase == DYING && now >= next_frame) { frame++; next_frame += 0.1f;
/// if (frame == 13) ...}` of QCport's `_pl_death_selftest`: one exit tests a
/// float that no trip count comes from, the other counts 14 iterations. The old
/// full unroller (unroll.rs) admitted only a loop that leaves from its header,
/// and a loop with two exiting blocks stayed rolled; the pin takes the smallest
/// per-exit trip count (`getSmallConstantTripCount` over the exiting blocks,
/// `computeUnrollCount`). Passes today: it guards a shape the old pass refused.
#[test]
fn a_loop_with_a_float_exit_and_an_integer_exit_is_unrolled_by_the_integer_count_and_the_float_chain_folds() {
    let text = "define i16 @f(float %limit) {
b0:
  br label %b1

b1:
  %frame = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %t = phi float [ 0.000000e+00, %b0 ], [ %tn, %b2 ]
  %more = fcmp olt float %t, %limit
  br i1 %more, label %b2, label %done

b2:
  %next = add nuw nsw i16 %frame, 1
  %tn = fadd float %t, 0x3FB99999A0000000
  %again = icmp ne i16 %next, 14
  br i1 %again, label %b1, label %done

done:
  %r = phi i16 [ %frame, %b1 ], [ %next, %b2 ]
  ret i16 %r
}
";
    let mut module = parsed(text);
    assert!(unrolled(&mut module));
    let after = printed(&module);
    assert!(!after.contains("%frame") && !after.contains("%t ="), "no loop phi is left: {after}");
    assert!(!after.contains("fadd"), "the float chain folded into constants: {after}");
    // 14 copies of the float test remain, against constants.
    assert_eq!(after.matches("fcmp olt float").count(), 14, "{after}");
}

/// QCport's `pl_death_tick` loop with its count read from a constant table
/// (`death_counts[set]`, `set` known): the integer exit's bound is a load.
/// Fold's `_numbers` table folded it; with that switched off the loop kept
/// "trips 0, max 32766" because MIR had no `simplifyLoadInst`. SCEV folds
/// the load at scope (`getSCEVAtScope`'s constant fold) and the loop unrolls
/// by the table's value.
#[test]
fn a_loop_bounded_by_a_constant_table_unrolls_as_scev_folds_the_load() {
    let text = "@counts = internal constant [12 x i8] c\"\\0B\\00\\09\\00\\0F\\00\\09\\00\\09\\00\\09\\00\"

define i16 @f(float %limit) {
b0:
  %n = load i16, ptr getelementptr inbounds nuw (i8, ptr @counts, i16 4)
  %bound = add nsw i16 %n, -1
  br label %b1

b1:
  %frame = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %t = phi float [ 0.000000e+00, %b0 ], [ %tn, %b2 ]
  %more = fcmp olt float %t, %limit
  br i1 %more, label %b2, label %done

b2:
  %next = add nuw nsw i16 %frame, 1
  %tn = fadd float %t, 0x3FB99999A0000000
  %again = icmp ne i16 %frame, %bound
  br i1 %again, label %b1, label %done

done:
  %r = phi i16 [ %frame, %b1 ], [ %next, %b2 ]
  ret i16 %r
}
";
    let mut module = parsed(text);
    assert!(unrolled(&mut module));
    let after = printed(&module);
    assert!(!after.contains("fadd"), "the float chain folded into constants: {after}");
}

/// A nest whose inner loop compares with the outer counter was peeled four
/// times in one run and then unrolled completely (nbody_single -O3: 114 -> 826
/// IR insns, the pin's 424): `LoopFullUnrollPass` does not revisit a loop it
/// peeled, so one iteration comes off and the outer loop stays.
#[test]
fn a_peeled_loop_is_not_visited_again_by_the_full_unroll() {
    let mut text = String::from(
        "define i32 @f(i32 %x) {
b0:
  br label %outer
outer:
  %b = phi i32 [ 0, %b0 ], [ %b2, %latch ]
  %acc = phi i32 [ %x, %b0 ], [ %s, %latch ]
  br label %inner
inner:
  %o = phi i32 [ 0, %outer ], [ %o2, %inner ]
  %a = phi i32 [ %acc, %outer ], [ %a2, %inner ]
  %ne = icmp ne i32 %o, %b
",
    );
    for k in 0..14 {
        let previous = if k == 0 { "%a".to_owned() } else { format!("%t{}", k - 1) };
        text += &format!("  %t{k} = mul i32 {previous}, {}\n", k + 3);
    }
    text += "  %pick = select i1 %ne, i32 %t12, i32 %t13
  %a2 = add i32 %a, %pick
  %o2 = add i32 %o, 1
  %done = icmp eq i32 %o2, 6
  br i1 %done, label %latch, label %inner
latch:
  %s = phi i32 [ %a2, %inner ]
  %b2 = add i32 %b, 1
  %end = icmp eq i32 %b2, 6
  br i1 %end, label %exit, label %outer
exit:
  %r = phi i32 [ %s, %latch ]
  ret i32 %r
}
";
    let mut module = parsed(&text);
    let inputs: &[&[i128]] = &[&[1], &[0], &[-7], &[1000]];
    let before = results(&module, inputs);
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::lcssa::closed(function).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let options =
        Options { opt_level: 3, opt_size: false, min_size: false, only_when_forced: false, prepare_for_lto: false };
    assert!(unroll_function(&mut module, "f", &tti, options).expect("unrolls"));
    let after = printed(&module);
    assert_eq!(results(&module, inputs), before, "{after}");
    assert!(after.contains("%b = phi") || after.matches("phi i32 [ 0").count() > 0, "the outer loop is gone: {after}");
}

/// Two double arrays updated in a four-trip loop: eight cells once unrolled,
/// which SROA would promote to the eight values the x87 stack (seven) cannot
/// hold (nbody -O2 at m32: 3808 dynamic `fxch` against gcc's 1200). With
/// SSE2 the registers are there and the loop unrolls as before.
fn two_arrays() -> String {
    "@a = internal global [4 x double] zeroinitializer
@b = internal global [4 x double] zeroinitializer

define void @f() {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %pa = getelementptr [4 x double], ptr @a, i16 0, i16 %i
  %pb = getelementptr [4 x double], ptr @b, i16 0, i16 %i
  %x = load double, ptr %pa
  %y = load double, ptr %pb
  %nx = fadd double %x, %y
  %ny = fsub double %y, %x
  store double %nx, ptr %pa
  store double %ny, ptr %pb
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 4
  br i1 %go, label %b1, label %b2

b2:
  ret void
}
"
    .to_string()
}

#[test]
fn a_loop_of_floats_that_would_outgrow_the_x87_stack_stays_rolled_but_not_with_sse2() {
    let run = |cpu: &str| {
        let mut module = parsed(&two_arrays());
        let function = module.function_mut("f").expect("@f").1;
        crate::loopsimplify::simplified(function);
        crate::lcssa::closed(function).expect("closes");
        let tti = X86::of_cpu(cpu).expect("a cpu");
        let options =
            Options { opt_level: 2, opt_size: false, min_size: false, only_when_forced: false, prepare_for_lto: false };
        unroll_function(&mut module, "f", &tti, options).expect("unrolls")
    };
    assert!(run("core2"), "control: with SSE2 the loop is unrolled");
    assert!(!run("i486"), "on the x87 stack the eight promoted cells stay in memory");
}
