//! `unroll_function` on loops the interpreter runs, before and after.

use llrm_mir::module::Module;
use llrm_x86::tti::X86;

use super::{Options, unroll_function, unroll_function_on};
use llrm_mir::target::OperationCosts;
use crate::testing::{parsed, printed, results};

/// @f's loops, the way the loop pass manager hands them over: simplified, LCSSA
/// form; whether any was unrolled (priced for an i486, `-O2`).
fn unrolled(module: &mut Module) -> bool {
    unrolled_at(module, 2, None)
}

/// `unrolled` at `-O{opt_level}`, on an x87 stack of `slots` where given (the
/// target description's field: `X86::of_cpu` alone states none).
fn unrolled_at(
    module: &mut Module,
    opt_level: u32,
    slots: Option<u32>,
) -> bool {
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(module).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let tti = match slots {
        Some(slots) => {
            tti.with_tuning(llrm_mir::target::Tuning { x87_stack_slots: Some(slots), ..llrm_mir::target::Tuning::PIN })
        }
        None => tti,
    };
    let options = Options {
        opt_level,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: false,
        inliner_follows: true,
    };
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

/// A loop holding `llvm.fshl` was "not computed" to the unroller (no price for the call), so it
/// stayed rolled: 6502 `rotate_small` -O2 5861 B against 912 B once it folds.
#[test]
fn test_a_loop_holding_a_funnel_shift_is_sized_and_unrolled() {
    let mut module = parsed(
        "declare i16 @llvm.fshl.i16(i16, i16, i16)
define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b1 ]
  %r = call i16 @llvm.fshl.i16(i16 %x, i16 %n, i16 %i)
  %s = add i16 %acc, %r
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 4
  br i1 %go, label %b1, label %b2

b2:
  %out = phi i16 [ %s, %b1 ]
  ret i16 %out
}
",
    );
    // a target with no price of its own for the intrinsic: the generic profile
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let options = Options {
        opt_level: 2,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: false,
        inliner_follows: true,
    };
    let tti = crate::testing::Flagged::default();
    assert!(unroll_function(&mut module, "f", &tti, options).expect("unrolls"));
    let text = printed(&module);
    assert!(!text.contains("phi i16 [ 0"), "no loop phi is left: {text}");
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
    let options = Options {
        opt_level: 2,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: false,
        inliner_follows: true,
    };
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
            target_unrolling: false,
            inliner_follows: true,
            created: std::rc::Rc::clone(&created),
            decided: Default::default(),
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
            target_unrolling: false,
            inliner_follows: true,
            created: std::rc::Rc::clone(&created),
            decided: Default::default(),
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
    crate::testing::closed(&mut module).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let options = Options {
        opt_level: 3,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: false,
        inliner_follows: true,
    };
    assert!(unroll_function(&mut module, "f", &tti, options).expect("unrolls"));
    let after = printed(&module);
    assert_eq!(results(&module, inputs), before, "{after}");
    assert!(after.contains("%b = phi") || after.matches("phi i32 [ 0").count() > 0, "the outer loop is gone: {after}");
}

/// mandel -O2 ran its first trip (x = y = 0: two multiplies and a compare that
/// fold) 768 times at 25k of 660k clocks. The peel is made where the clocks
/// the fold saves repay the bytes of the copy at the 486's rate (312).
#[test]
fn a_first_iteration_is_peeled_only_where_its_folded_clocks_repay_the_copy() {
    // 15 units are 45 bytes: 14040 milliclocks at 312, 14 clocks.
    assert!(super::folded_entry_pays(39, 15, 312));
    assert!(super::folded_entry_pays(15, 15, 312));
    assert!(!super::folded_entry_pays(14, 15, 312));
    assert!(!super::folded_entry_pays(0, 15, 312));
}

/// A loop declaring a scope for its store and `!noalias` it on its load,
/// unrolled four times, kept one scope for every copy: the first copy's store
/// to `%p` and the second's load through `%q` were called apart by
/// `ScopedNoAliasAA`, though `%p` and `%q` may be one address at each trip
/// (`cloneAndAdaptNoAliasScopes`).
#[test]
fn test_each_unrolled_copy_of_a_loop_declaring_a_scope_has_its_own() {
    use llrm_analysis::aa::{AliasResult, BatchAAResults, MemoryLocation};
    use llrm_analysis::aggregate::AggregateAA;
    use llrm_analysis::memory::Unit;
    use llrm_analysis::scopednoaliasaa::ScopedNoAliasAA;

    let mut module = parsed(
        "declare void @llvm.experimental.noalias.scope.decl()

define void @f(ptr %p, ptr %q) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b1 ]
  call void @llvm.experimental.noalias.scope.decl(), !noalias.scope.decl !1
  store i32 1, ptr %p, !alias.scope !1
  %v = load i32, ptr %q, !noalias !1
  store i32 %v, ptr %q
  %next = add i32 %i, 1
  %go = icmp slt i32 %next, 4
  br i1 %go, label %b1, label %b2

b2:
  ret void
}

!0 = distinct !{!0, !\"domain\"}
!2 = distinct !{!2, !0, !\"scope\"}
!1 = !{!2}
",
    );
    assert!(unrolled(&mut module), "{}", printed(&module));
    printed(&module); // it verifies
    let layout = llrm_mir::datalayout::DataLayout::default();
    let function = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f").2;
    let unit = Unit::of(&module, &layout, function);
    let stores: Vec<MemoryLocation> = function
        .walk()
        .filter(|&(_, inst)| matches!(
            function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Store { .. }
        ))
        .filter_map(|(_, inst)| MemoryLocation::get(&unit, inst))
        .collect();
    let loads: Vec<MemoryLocation> = function
        .walk()
        .filter(|&(_, inst)| matches!(
            function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Load { .. }
        ))
        .filter_map(|(_, inst)| MemoryLocation::get(&unit, inst))
        .collect();
    // Per copy: the scoped store to %p, the load, the store to %q.
    assert_eq!((stores.len(), loads.len()), (8, 4), "{}", printed(&module));
    let mut chain = AggregateAA::new();
    chain.add_aa_result(ScopedNoAliasAA::new(&unit));
    let mut aa = BatchAAResults::new(&mut chain);
    assert_eq!(aa.alias(&stores[0], &loads[0]), AliasResult::NoAlias, "premise: one copy's own pair");
    assert_eq!(aa.alias(&stores[0], &loads[1]), AliasResult::MayAlias, "{}", printed(&module));
}

/// `for (i = 0; i < 8; i++) acc += (i * k + n) ...`: a body big enough that
/// unrolling it is judged by `analyzeLoopUnrollCost`.
fn heavy() -> String {
    let mut body = String::new();
    for k in 0..24 {
        body.push_str(&format!("  %t{k} = mul i16 %i, {}\n  %u{k} = add i16 %t{k}, %n\n", k + 3));
    }
    let sum = (0..24).fold("%acc".to_string(), |sum, k| {
        body.push_str(&format!("  %s{k} = add i16 {sum}, %u{k}\n"));
        format!("%s{k}")
    });
    format!(
        "define i16 @f(i16 %x, i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %acc = phi i16 [ %x, %b0 ], [ {sum}, %b1 ]
{body}  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 8
  br i1 %go, label %b1, label %b2

b2:
  %r = phi i16 [ {sum}, %b1 ]
  ret i16 %r
}}
"
    )
}

/// Old: `analyze_loop_unroll_cost` began a `ScalarEvolution` of its own for
/// each loop, so the loop's phis and steps were made again after the trip
/// count had made them (the unroll analyzer's `get_scev` was 18.0 M Ir on
/// jpeg_fixed -O2 m32, the pin's 5.5 M on its cached SE). The pass's own
/// session answers it.
#[test]
fn test_the_unroll_cost_analysis_reads_the_loops_expressions_the_pass_already_made() {
    use llrm_mir::counts::{Event, get};
    let mut module = parsed(&heavy());
    let before = get(Event::ScevNodes);
    unrolled(&mut module);
    let made = get(Event::ScevNodes) - before;
    assert!(made <= 29, "{made} expressions were made for one loop");
}

/// Old: every unrolled loop dropped the function's whole table of expressions
/// (`run.scev = ScalarEvolution::new()` after each of them, and a stand-in made
/// for each trip-count query), so the next loop's trip count and its IVs were
/// worked out again: 3,744 `will_not_overflow` calls against the pin's 2,001 on
/// memory_expansion. The pin keeps one `ScalarEvolution` and forgets the loop.
#[test]
fn test_unrolling_loops_in_sequence_keeps_one_table_of_expressions() {
    let mut module = parsed(&sequential(8));
    let before = llrm_analysis::scev::made();
    assert!(unrolled(&mut module));
    let made = llrm_analysis::scev::made() - before;
    assert!(made <= 2, "{made} tables of expressions made for 8 loops");
}

/// nbody -O2 at m32 priced its 4-trip loop over four float arrays at "saves
/// 2176 clk, adds 0 B" and unrolled it, +18.5k clocks: the line said nothing of
/// the arrays or the float file a reader needs to see why. It carries both now.
#[test]
fn a_priced_unroll_line_names_the_arrays_trips_and_float_file() {
    let line = super::price::Priced {
        trips: 4,
        saved: 2176,
        added: 0,
        body_bytes: 54,
        units: 40,
        rolled: 13,
        promoted: 6,
        ints: 0,
        floats: 4,
        rmw: 2,
        spilled: 3,
        float_file: 8,
        rate: 312,
        saved_units: 90,
        unit_clocks: 3,
        constant_clocks: 0,
        runs: 1,
        resource: 0,
        chain: 0,
    }
    .line();
    for want in ["4 trips", "ints 0", "floats 4", "rmw 2", "spilled 3", "float file 8", "rate 312"] {
        assert!(line.contains(want), "{want} missing from: {line}");
    }
}

/// An unnamed loop header printed as `unroll: %:` in `LLRM_DEBUG=unroll`, so
/// the six eight-trip decisions of x_jpeg_single -O3 could not be told apart
/// or found in the IR dumps.
#[test]
fn test_a_debug_line_names_an_unnamed_loop_header_by_its_printed_label() {
    let text = concat!(
        "define i16 @f(i16 %x) {\n  br label %1\n1:\n",
        "  %i = phi i16 [ 0, %0 ], [ %n, %1 ]\n  %n = add i16 %i, 1\n",
        "  %c = icmp slt i16 %n, 4\n  br i1 %c, label %1, label %2\n",
        "2:\n  ret i16 %n\n}\n",
    );
    let mut module = parsed(text);
    let function = &*module.function_mut("f").expect("@f").1;
    let header = function.layout()[1];
    assert_eq!(super::loop_label(function, header, "f"), "@f %1");
}

/// Eight trips, sixteen floats live at once (a body whose unrolled cost sits
/// between the default bar 150 and the aggressive 300): `kept` stores
/// into a local array it also reads, `independent` into a global one.
fn sixteen_floats(
    kept: bool,
    global: bool,
) -> String {
    let loads: String = (0..16)
        .map(|k| {
            format!("  %p{k} = getelementptr inbounds float, ptr %t, i16 %j{k}\n  %x{k} = load float, ptr %p{k}\n")
        })
        .collect();
    let offsets: String = (0..16).map(|k| format!("  %j{k} = add i16 %i, {k}\n")).collect();
    let chain: String = (0..15)
        .map(|k| {
            let left = if k == 0 { "%x0".to_string() } else { format!("%s{}", k - 1) };
            format!("  %s{k} = fadd float {left}, %x{}\n", k + 1)
        })
        .collect();
    let (decl, target, after) = if kept && global {
        ("", "@o", "  %q3 = getelementptr inbounds float, ptr @o, i16 3\n  %r = load float, ptr %q3\n")
    } else if kept {
        (
            "  %acc = alloca [8 x float], align 4\n  call void @llvm.memset.p0.i16(ptr %acc, i8 0, i16 32, i1 false)\n",
            "%acc",
            "  %q3 = getelementptr inbounds float, ptr %acc, i16 3\n  %r = load float, ptr %q3\n",
        )
    } else {
        ("", "@o", "  %q3 = getelementptr inbounds float, ptr @o, i16 3\n  %r = load float, ptr %q3\n")
    };
    let old = if kept { "  %old = load float, ptr %q\n" } else { "  %old = fadd float 0.000000e+00, 0.000000e+00\n" };
    format!(
        "@o = internal global [8 x float] zeroinitializer

define float @f(ptr %t) {{
b0:
{decl}  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
{offsets}{loads}{chain}  %q = getelementptr inbounds float, ptr {target}, i16 %i
{old}  %new = fadd float %old, %s14
  store float %new, ptr %q
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 8
  br i1 %go, label %b1, label %b2

b2:
{after}  ret float %r
}}
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
"
    )
}

/// x_jpeg_single -O3 (m32): three 8-trip loops of nine live floats, trips that
/// keep nothing across them, were held to the default bar 150 by the
/// float-stack guard and stayed rolled where the pin unrolls them: +212k FMUL,
/// 2.3M clocks (3.6%) over gcc.
#[test]
fn test_a_float_body_whose_trips_keep_nothing_across_them_is_unrolled_at_o3() {
    let mut module = parsed(&sixteen_floats(false, false));
    assert!(unrolled_at(&mut module, 3, Some(8)), "{}", printed(&module));
}

/// The guard's reason stays: a body that reads and writes a float array
/// forwards its elements from copy to copy (nbody_single and fpbench -O3
/// spilled 57 stores and 90 reloads against 30 and 30 rolled).
#[test]
fn test_a_float_body_that_forwards_an_array_across_its_trips_stays_a_loop_at_o3() {
    let mut module = parsed(&sixteen_floats(true, false));
    assert!(!unrolled_at(&mut module, 3, Some(8)), "{}", printed(&module));
}

/// The same body over a static global array (nbody_single -O3: six-trip loops
/// over `pos_x`, `vel_x`, each read and written): the gate counted local arrays
/// only, released the hold and the kernel grew +2.5% clocks, +825 B.
#[test]
fn test_a_float_body_that_forwards_a_global_array_across_its_trips_stays_a_loop_at_o3() {
    let mut module = parsed(&sixteen_floats(true, true));
    assert!(!unrolled_at(&mut module, 3, Some(8)), "{}", printed(&module));
}

/// `unrolled_at` at -O1 with `unroll_never_grows_at_o1` as stated.
fn unrolled_o1(
    module: &mut Module,
    never_grows: bool,
) -> bool {
    unrolled_o1_analysing(module, never_grows, None)
}

/// `unrolled_o1`, the target stating how many trips the no-growth rule
/// analyses.
fn unrolled_o1_analysing(
    module: &mut Module,
    never_grows: bool,
    analyzed_trips: Option<u32>,
) -> bool {
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(module).expect("closes");
    let tti = X86::of_cpu("i486")
        .expect("a cpu")
        .with_tuning(
            llrm_mir::target::Tuning {
                unroll_never_grows_at_o1: never_grows,
                unroll_o1_analyzed_trips: analyzed_trips,
                ..llrm_mir::target::Tuning::PIN
            },
        );
    let options = Options {
        opt_level: 1,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: false,
        inliner_follows: true,
    };
    unroll_function(module, "f", &tti, options).expect("unrolls")
}

/// x_life's neighbour sum: three loads at constant offsets from one base.
const THREE_LOADS: &str = "define i32 @f(ptr %p) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b1 ]
  %acc = phi i32 [ 0, %b0 ], [ %s, %b1 ]
  %q = getelementptr inbounds i8, ptr %p, i32 %i
  %v = load i8, ptr %q
  %z = zext i8 %v to i32
  %s = add i32 %acc, %z
  %next = add i32 %i, 1
  %go = icmp ult i32 %next, 3
  br i1 %go, label %b1, label %b2

b2:
  %r = phi i32 [ %s, %b1 ]
  ret i32 %r
}
";

/// m32 -O1 took gcc's `-funroll-loops` and every small loop unrolled whatever
/// the copies cost: matmul +2066 B, fpbench +1209 B (O1 is strict, no cell
/// larger). With the target's `unroll_never_grows_at_o1` a loop whose copies
/// cost more than the rolled loop stays rolled.
#[test]
fn test_at_o1_a_loop_whose_copies_cost_more_than_the_loop_stays_rolled() {
    assert!(unrolled_o1(&mut parsed(&counted(8)), false), "the pin's O1 bar unrolls it");
    assert!(!unrolled_o1(&mut parsed(&counted(8)), true), "eight copies of a multiply and an add grow");
}

/// float_in_memory_loop -O1 m32: sixteen trips of a load from a constant table
/// and an add fold to nothing when copied (the analysis prices the copies at 0
/// units against 4 rolled), but the -O1 guard analysed only the pin's first ten
/// trips (`-unroll-max-iteration-count-to-analyze`), found nothing for sixteen
/// and kept the loop: 64 -> 158 B. The pricing path analyses all the trips; so
/// does the guard now.
#[test]
fn test_at_o1_a_loop_of_more_than_ten_trips_that_folds_away_is_unrolled() {
    let table = (1..=16).map(|n| format!("i32 {n}")).collect::<Vec<_>>().join(", ");
    let text = format!(
        "@t = constant [16 x i32] [{table}]

define i32 @f() {{
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b1 ]
  %acc = phi i32 [ 0, %b0 ], [ %s, %b1 ]
  %q = getelementptr inbounds i32, ptr @t, i32 %i
  %v = load i32, ptr %q
  %s = add i32 %acc, %v
  %next = add i32 %i, 1
  %go = icmp ult i32 %next, 16
  br i1 %go, label %b1, label %b2

b2:
  ret i32 %s
}}
"
    );
    assert!(!unrolled_o1(&mut parsed(&text), true), "the pin's cap of ten analysed trips keeps it rolled");
    let mut module = parsed(&text);
    assert!(unrolled_o1_analysing(&mut module, true, Some(16)), "{}", printed(&module));
}

/// x_life O1 (gcc unrolls the dx loop and shrinks): three copies of a load
/// and an add cost what the rolled loop's body and its branch do, so the
/// size-neutral bar keeps this unroll.
#[test]
fn test_at_o1_a_loop_whose_copies_fit_the_loop_is_unrolled() {
    let mut module = parsed(THREE_LOADS);
    assert!(unrolled_o1(&mut module, true), "{}", printed(&module));
}

/// An outer loop of unknown trips holding `n` four-trip loops in sequence.
fn nested(n: usize) -> String {
    let mut text = String::from(
        "define i16 @f(i16 %x, i16 %m) {\nb0:\n  br label %o\n\no:\n  %j = phi i16 [ 0, %b0 ], [ %jn, %latch ]\n  %c0 = phi i16 [ %x, %b0 ], [ %r, %latch ]\n  br label %l0\n\n",
    );
    for k in 0..n {
        let next = if k + 1 == n { "latch".to_owned() } else { format!("l{}", k + 1) };
        let (pre, init) =
            if k == 0 { ("o".to_owned(), "c0".to_owned()) } else { (format!("x{}", k - 1), format!("r{}", k - 1)) };
        text += &format!(
            "l{k}:\n  %i{k} = phi i16 [ 0, %{pre} ], [ %n{k}, %l{k} ]\n  %a{k} = phi i16 [ %{init}, %{pre} ], [ %s{k}, %l{k} ]\n  %s{k} = add i16 %a{k}, %i{k}\n  %n{k} = add i16 %i{k}, 1\n  %g{k} = icmp slt i16 %n{k}, 4\n  br i1 %g{k}, label %l{k}, label %x{k}\n\nx{k}:\n  %r{k} = phi i16 [ %s{k}, %l{k} ]\n  br label %{next}\n\n",
        );
    }
    text += &format!(
        "latch:\n  %r = add i16 %r{last}, 0\n  %jn = add i16 %j, 1\n  %e = icmp slt i16 %jn, %m\n  br i1 %e, label %o, label %end\n\nend:\n  ret i16 %r\n}}\n",
        last = n - 1
    );
    text
}

/// Every full unroll of a loop inside another made the dominator tree and loop
/// info again 3-4 times (1,211 calls on memory_expansion, the pin updates both
/// in place and makes none): after the CFG-neutral `simplify_loop_after_unroll`
/// the tree and nest were still right.
#[test]
fn test_unrolling_inner_loops_makes_the_loop_analyses_at_most_twice_per_loop() {
    use llrm_mir::counts::{Event, get};
    let mut module = parsed(&nested(8));
    let before = get(Event::LoopAnalyses);
    assert!(unrolled(&mut module));
    let made = get(Event::LoopAnalyses) - before;
    assert!(made <= 2 * 8 + 3, "{made} analyses made for 8 inner loops");
}

/// A loop of unknown trips that nothing unrolls: the pass made the dominator
/// tree and loop info again (449 of memory_expansion's 983 calls were this
/// entry rebuild, 0.8 M Ir each) where the pin takes both from the analysis
/// manager.
#[test]
fn test_a_function_the_unroll_leaves_alone_makes_no_loop_analyses() {
    use llrm_mir::counts::{Event, get};
    let mut module = parsed(
        "define i16 @f(i16 %n) {\nb0:\n  br label %l\n\nl:\n  %i = phi i16 [ 0, %b0 ], [ %j, %l ]\n  %j = add i16 %i, 1\n  %g = icmp slt i16 %j, %n\n  br i1 %g, label %l, label %x\n\nx:\n  %r = phi i16 [ %j, %l ]\n  ret i16 %r\n}\n",
    );
    let before = get(Event::LoopAnalyses);
    crate::testing::managed(
        &mut module,
        super::LoopFullUnroll {
            opt_level: 2,
            opt_size: false,
            only_when_forced: false,
            target_unrolling: false,
            inliner_follows: true,
            created: Default::default(),
            decided: Default::default(),
        },
    );
    let made = get(Event::LoopAnalyses) - before;
    assert_eq!(made, 0, "{made} rebuilds of the tree and nest for a function nothing was unrolled in");
}

/// Every partial unroll of a top-level loop built the function's dominator tree
/// and loop info again (467 builds of each, 517 M and 277 M Ir of
/// memory_expansion -O2); the pin updates both in place (`UnrollLoop`'s `DTU`
/// and `LI`).
#[test]
fn test_partial_unrolls_in_sequence_build_one_dominator_tree_and_nest() {
    use llrm_mir::counts::{Event, get};
    let text = (0..8).fold(sequential(8), |text, k| {
        text.replace(&format!("icmp slt i16 %n{k}, 4"), &format!("icmp slt i16 %n{k}, 1024"))
    });
    let mut module = parsed(&text);
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let tti = X86::of_cpu("x86-64").expect("a cpu");
    let options = Options {
        opt_level: 2,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: true,
        inliner_follows: true,
    };
    let (trees, nests) = (get(Event::Dominators), get(Event::LoopAnalyses));
    assert!(unroll_function(&mut module, "f", &tti, options).expect("unrolls"));
    assert_eq!(printed(&module).matches("phi i16 [ 0,").count(), 8, "partial: the eight loops remain");
    let (trees, nests) = (get(Event::Dominators) - trees, get(Event::LoopAnalyses) - nests);
    assert!(trees <= 2 && nests <= 2, "{trees} dominator trees and {nests} nests built for 8 partial unrolls");
}

/// Compile time: the unroll pass looked at its function's loops through a
/// shape made again (`Shape::of`: a dominator tree and loops, 484 K Ir a call
/// on memexp -O2) while the manager held both. A function whose loops are in
/// canonical form and stay rolled derives no shape of its own.
#[test]
fn test_the_unroll_pass_asks_the_managers_loops_not_a_new_shape() {
    let mut module = parsed(
        "define i16 @f(i16 %n) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %head ]
  %next = add i16 %i, 1
  %more = icmp ult i16 %next, %n
  br i1 %more, label %head, label %out

out:
  ret i16 %next
}
",
    );
    let before = llrm_analysis::cfg::shapes_derived();
    crate::testing::managed(
        &mut module,
        super::LoopFullUnroll {
            opt_level: 2,
            opt_size: false,
            only_when_forced: false,
            target_unrolling: false,
            inliner_follows: true,
            created: Default::default(),
            decided: Default::default(),
        },
    );
    assert_eq!(llrm_analysis::cfg::shapes_derived(), before, "a shape was made again");
}

/// Compile time: the unroll pass closed its function's SSA form through a
/// dominator tree and loops of its own (755 calls, 748 M Ir on memexp -O2,
/// every one of them a no-op) while the manager held both. A function the
/// pass left as it found it makes none.
#[test]
fn test_the_unroll_pass_closes_over_the_managers_tree_and_loops() {
    let mut module = parsed(
        "define i16 @f(i16 %n) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %head ]
  %next = add i16 %i, 1
  %more = icmp ult i16 %next, %n
  br i1 %more, label %head, label %out

out:
  ret i16 %next
}
",
    );
    crate::lcssa::TREES_MADE.with(|count| count.set(0));
    crate::testing::managed(
        &mut module,
        super::LoopFullUnroll {
            opt_level: 2,
            opt_size: false,
            only_when_forced: false,
            target_unrolling: false,
            inliner_follows: true,
            created: Default::default(),
            decided: Default::default(),
        },
    );
    assert_eq!(crate::lcssa::TREES_MADE.with(std::cell::Cell::get), 0, "a tree was made again");
}

/// Compile time: a full unroll run for one loop of the loop pass manager
/// (`loopfilter::only`) still put the whole function in simplified and closed
/// form first, 755 runs, 403 of them over one function of 403 loops (a
/// quadratic 1.1 B Ir on memexp -O2). The pin's `LoopFullUnrollPass` does
/// neither: no loop is asked, no shape made.
#[test]
fn test_a_full_unroll_for_one_loop_does_not_re_form_the_function() {
    let mut module = parsed(
        "define i16 @f(i16 %n) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %head ]
  %next = add i16 %i, 1
  %more = icmp ult i16 %next, %n
  br i1 %more, label %head, label %out

out:
  ret i16 %next
}
",
    );
    let _scope = crate::loopfilter::only(&[llrm_mir::module::BlockId(1)]);
    crate::lcssa::LOOPS_FORMED.with(|count| count.set(0));
    let shapes = llrm_analysis::cfg::shapes_derived();
    crate::testing::managed(
        &mut module,
        super::LoopFullUnroll {
            opt_level: 2,
            opt_size: false,
            only_when_forced: false,
            target_unrolling: false,
            inliner_follows: true,
            created: Default::default(),
            decided: Default::default(),
        },
    );
    assert_eq!(crate::lcssa::LOOPS_FORMED.with(std::cell::Cell::get), 0, "the function was closed again");
    assert_eq!(llrm_analysis::cfg::shapes_derived(), shapes, "a shape was made again");
}

/// fib(20) at m64 -O3 with its copies priced panicked in ScalarEvolution ("the
/// header of a loop"): ten outer trips round an inner loop started from the
/// outer counter (`%start = %i - 1`). The full unroll of the outer loop kept
/// `%start` with a constant, and its cached expression, an add recurrence of
/// the gone outer loop, was read by the next inner loop's trip count.
#[test]
fn a_full_unroll_of_a_nest_forgets_the_expressions_of_its_kept_values() {
    let text = "declare i32 @g(i32)

define i32 @f(i32 %x) {
b0:
  br label %o

o:
  %i = phi i32 [ 20, %b0 ], [ %i.next, %ol ]
  %acc = phi i32 [ 0, %b0 ], [ %r, %ol ]
  %start = add nsw i32 %i, -1
  %skip = icmp ult i32 %i, 3
  br i1 %skip, label %ol, label %pre

pre:
  br label %h

h:
  %j = phi i32 [ %start, %pre ], [ %j.next, %h ]
  %a = phi i32 [ 0, %pre ], [ %a.next, %h ]
  %j1 = add nsw i32 %j, -1
  %c = tail call i32 @g(i32 %j1)
  %j.next = add nsw i32 %j, -2
  %a.next = add i32 %c, %a
  %low = icmp ult i32 %j, 4
  br i1 %low, label %hx, label %h

hx:
  %jl = phi i32 [ %j.next, %h ]
  %al = phi i32 [ %a.next, %h ]
  br label %ol

ol:
  %m = phi i32 [ %start, %o ], [ %jl, %hx ]
  %t = phi i32 [ 0, %o ], [ %al, %hx ]
  %i.next = add nsw i32 %i, -2
  %r1 = add i32 %m, %acc
  %r = add i32 %r1, %t
  %done = icmp eq i32 %i.next, 0
  br i1 %done, label %out, label %o

out:
  ret i32 %r
}
";
    let mut module = parsed(text);
    let created = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let out = crate::testing::managed(
        &mut module,
        super::LoopFullUnroll {
            opt_level: 3,
            opt_size: false,
            only_when_forced: false,
            target_unrolling: false,
            inliner_follows: true,
            created,
            decided: Default::default(),
        },
    );
    assert!(!out.contains("%i = phi"), "the outer loop is gone: {out}");
}

/// A counted loop over `ty` elements of `@a`, `n` run-time trips, `@f`'s body
/// given by `body` (defining `%y`, the next `%x`; `%v` is the element), as the
/// loop pass manager hands it over; whether the x86-64 loop unroller
/// (`target_unrolling`) changed `@f`, with the recurrence veto as asked.
fn runtime_unrolled(
    ty: &str,
    init: &str,
    body: &str,
    veto: bool,
) -> bool {
    runtime_unrolled_text(
        &format!(
            "define {ty} @f(ptr %a, i64 %n) {{
entry:
  %skip = icmp sle i64 %n, 0
  br i1 %skip, label %out, label %loop

loop:
  %i = phi i64 [ 0, %entry ], [ %next, %latch ]
  %x = phi {ty} [ {init}, %entry ], [ %y, %latch ]
  %p = getelementptr {ty}, ptr %a, i64 %i
  %v = load {ty}, ptr %p
{body}
  %next = add i64 %i, 1
  %more = icmp ne i64 %next, %n
  br i1 %more, label %loop, label %done

done:
  ret {ty} %y

out:
  ret {ty} {init}
}}
"
        )
        .replace("%latch", "%loop"),
        veto,
    )
}

fn runtime_unrolled_text(
    text: &str,
    veto: bool,
) -> bool {
    let mut module = parsed(text);
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let tuning = llrm_mir::target::Tuning { unroll_recurrence_veto: veto, ..llrm_mir::target::Tuning::PIN };
    let tti = X86::of_cpu("x86-64").expect("a cpu").with_tuning(tuning);
    let options = Options {
        opt_level: 2,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: true,
        inliner_follows: true,
    };
    unroll_function(&mut module, "f", &tti, options).expect("unrolls")
}

const CHAIN: &str = "  %m = mul i64 %x, 3\n  %y = add i64 %m, %v";
const WIDE: &str = "  %m = mul i64 %v, 5\n  %w = add i64 %m, 7\n  %y = add i64 %x, %w";
const FDIV: &str = "  %q = fdiv double %x, %v\n  %y = fadd double %q, 1.0";

/// quicksort -O3 (`x = x * 25173 + 13849; values[i] = x & 0x7FFF`): an integer
/// chain of constants is not a bound, the unrolled copies compose into one
/// multiply and add per pair (instcombine), halving the chain. Its unroll was
/// vetoed: 1731 -> 1708 B and +7.3% clocks (spread 4%).
#[test]
fn test_a_runtime_loop_of_integer_arithmetic_is_left_to_the_pin() {
    assert!(runtime_unrolled("i64", "1", CHAIN, false), "premise: the pin's rule unrolls this loop");
    assert!(runtime_unrolled("i64", "1", CHAIN, true), "an integer chain composes when unrolled");
}

/// The same for a divide chain, `s = s / a[i] + 1.0` (floats -O2, recfloat).
#[test]
fn test_a_runtime_loop_bound_by_a_divide_chain_is_not_unrolled() {
    assert!(runtime_unrolled("double", "1.0", FDIV, false), "premise: the pin's rule unrolls this loop");
    assert!(!runtime_unrolled("double", "1.0", FDIV, true));
}

/// floats -O2: `volatile double value` keeps the chain in memory, a load of
/// what the last trip stored.
#[test]
fn test_a_runtime_loop_bound_by_a_chain_through_memory_is_not_unrolled() {
    let text = "define double @f(i64 %n) {
entry:
  %slot = alloca double
  store volatile double 1.0, ptr %slot
  %skip = icmp sle i64 %n, 0
  br i1 %skip, label %out, label %loop

loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %x = load volatile double, ptr %slot
  %m = fmul double %x, 1.5
  %q = fdiv double %m, 1.25
  store volatile double %q, ptr %slot
  %next = add i64 %i, 1
  %more = icmp ne i64 %next, %n
  br i1 %more, label %loop, label %out

out:
  %r = load volatile double, ptr %slot
  ret double %r
}
";
    assert!(runtime_unrolled_text(text, false), "premise: the pin's rule unrolls this loop");
    assert!(!runtime_unrolled_text(text, true));
}

/// The veto asks the loop, not the pass: a trip that is many independent
/// operations around a one-cycle `add` chain is bound by the core's issue
/// width and is unrolled as the pin does.
#[test]
fn test_a_runtime_loop_bound_by_issue_is_still_unrolled() {
    assert!(runtime_unrolled("i64", "0", WIDE, true));
}

/// Known limit: only a single-block loop is priced (its operations all run each
/// trip). A chain across a branch inside the loop keeps the pin's unroll.
#[test]
fn test_a_runtime_loop_of_several_blocks_is_left_to_the_pin() {
    let text = "define i64 @f(ptr %a, i64 %n) {
entry:
  %skip = icmp sle i64 %n, 0
  br i1 %skip, label %out, label %loop

loop:
  %i = phi i64 [ 0, %entry ], [ %next, %latch ]
  %x = phi i64 [ 1, %entry ], [ %y, %latch ]
  %p = getelementptr i64, ptr %a, i64 %i
  %v = load i64, ptr %p
  %odd = icmp slt i64 %v, 0
  br i1 %odd, label %neg, label %latch

neg:
  br label %latch

latch:
  %m = mul i64 %x, 3
  %y = add i64 %m, %v
  %next = add i64 %i, 1
  %more = icmp ne i64 %next, %n
  br i1 %more, label %loop, label %done

done:
  ret i64 %y

out:
  ret i64 1
}
";
    assert!(runtime_unrolled_text(text, false), "premise: the pin's rule unrolls this loop");
    assert!(runtime_unrolled_text(text, true));
}

/// recfloat -O2: `bench_recfloat` inlined into `main` with a trip count of
/// 1000 was unrolled by 5 (`decay`'s chain is `mulsd`, `addsd`), 209 bytes
/// against gcc's 153 for the same clocks. A known trip count is no reason to
/// copy a loop its own chain bounds.
#[test]
fn test_a_known_trip_loop_bound_by_its_chain_is_not_unrolled() {
    let text = "define double @f(ptr %a) {
entry:
  br label %loop

loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %x = phi double [ 1.0, %entry ], [ %y, %loop ]
  %m = fmul double %x, 0.5
  %y = fadd double %m, 1.0
  %next = add i64 %i, 1
  %more = icmp ne i64 %next, 1000
  br i1 %more, label %loop, label %done

done:
  ret double %y
}
";
    assert!(runtime_unrolled_text(text, false), "premise: the pin's rule unrolls this loop");
    assert!(!runtime_unrolled_text(text, true));
}

/// histo -O2/-O3 (`++counts[(data[i] * 7 + seed) & 255]`): the store of one trip
/// was read as feeding the next trip's load because both name the same SSA
/// address, a chain of 7 that the loop does not have. Its partial unroll was
/// vetoed, +7% instructions. The address changes every trip.
#[test]
fn test_a_counter_in_memory_at_a_new_address_each_trip_is_still_unrolled() {
    let text = "define i64 @f(ptr %counts, ptr %data) {
entry:
  br label %loop

loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %d = getelementptr i8, ptr %data, i64 %i
  %b = load i8, ptr %d
  %k = zext i8 %b to i64
  %p = getelementptr i64, ptr %counts, i64 %k
  %c = load i64, ptr %p
  %c1 = add i64 %c, 1
  store i64 %c1, ptr %p
  %next = add i64 %i, 1
  %more = icmp ne i64 %next, 4096
  br i1 %more, label %loop, label %done

done:
  ret i64 0
}
";
    assert!(runtime_unrolled_text(text, false), "premise: the pin's rule unrolls this loop");
    assert!(runtime_unrolled_text(text, true), "vetoed for a chain through an address that moves");
}

/// SandyBridge's resources as the x86-64 `[schedule]` states them, for the operations a loop of integer
/// arithmetic, loads and stores is made of: micro-ops over the issue width, the three ALU ports, the
/// multiplier port, the two load/store address ports, the store-data port and the branch port.
fn sandy_bridge_throughput() -> llrm_mir::target::Throughput {
    use llrm_mir::target::{Resource, Throughput};
    let costs = |add, multiply, load, store, branch, mv| OperationCosts { add, multiply, load, store, branch, r#move: mv, ..OperationCosts::default() };
    Throughput {
        latency: costs(1000, 4000, 5000, 1000, 1000, 1000),
        resources: vec![
            Resource { units: 4, costs: costs(1000, 3000, 1000, 1000, 1000, 1000) },
            Resource { units: 3, costs: costs(1000, 1000, 0, 0, 0, 1000) },
            Resource { units: 1, costs: costs(0, 1000, 0, 0, 0, 0) },
            Resource { units: 2, costs: costs(0, 0, 1000, 1000, 0, 0) },
            Resource { units: 1, costs: costs(0, 0, 0, 1000, 0, 0) },
            Resource { units: 1, costs: costs(0, 0, 0, 0, 1000, 0) },
        ],
    }
}

/// A 16-trip fill of a global byte array whose value folds in every copy
/// (`(i + 1) * 179 + 85 + zero`, the multiply and add of the counter), unrolled whole at `-O2` with the unroll
/// priced at 600 milliclocks a byte (the test's knob: 55 B added; 87 clocks saved at a clock an instruction,
/// about 20 on a core with the SandyBridge resources); with that core's throughput model, or without any;
/// whether the loop is gone.
fn fill_unrolled(throughput: bool, call: bool) -> bool {
    fill_unrolled_at(throughput, call, 600)
}

/// `fill_unrolled` at `rate` milliclocks a byte.
fn fill_unrolled_at(throughput: bool, call: bool, rate: i64) -> bool {
    let (callee, first, folded, stored) = if call {
        // Called twice, so not an inline candidate of the loop's own metrics (one live use is): the inliner decides it
        // once the copies make the argument a constant.
        let steps: String = (1..=120).map(|step| format!("  %y{step} = mul i32 %y{}, 7\n", step - 1)).collect();
        (
            format!("define internal i32 @g(i32 %y0) {{\n{steps}  ret i32 %y120\n}}\n"),
            "%pre = call i32 @g(i32 5)".to_string(),
            "%g = call i32 @g(i32 %s)".to_string(),
            "%g".to_string(),
        )
    } else {
        (String::new(), String::new(), String::new(), "%s".to_string())
    };
    // The copies of a loop around a call stay under the pin's threshold of 150.
    let trips = if call { 8 } else { 16 };
    let mut module = parsed(&format!(
        "@stream = global [{trips} x i8] zeroinitializer
@zero = global i32 0

{callee}
define void @f() {{
entry:
  {first}
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %n = add i32 %i, 1
  %m = mul i32 %n, 179
  %c = add i32 %m, 85
  %z = load i32, ptr @zero
  %s = add i32 %c, %z
  {folded}
  %t = trunc i32 {stored} to i8
  %p = getelementptr [{trips} x i8], ptr @stream, i32 0, i32 %i
  store i8 %t, ptr %p
  %next = add i32 %i, 1
  %more = icmp ne i32 %next, {trips}
  br i1 %more, label %loop, label %done

done:
  ret void
}}
"
    ));
    unroll_on_core(&mut module, throughput, !call, 2, 0, rate);
    !printed(&module).contains("%i = phi")
}

/// The control: a rate that no saving repays keeps the loop rolled where 600 unrolls it, so the rate is read at all.
#[test]
fn test_an_absurd_rate_keeps_a_loop_rolled_that_a_real_one_unrolls() {
    assert!(fill_unrolled_at(false, false, 600), "premise: unrolled at 600");
    assert!(!fill_unrolled_at(false, false, 1_000_000_000), "a billion milliclocks a byte repays nothing");
}

/// The unroll pass on `@f` of `module` for x86-64 at `rate` milliclocks a byte (the test's knob), with or without
/// the SandyBridge throughput model.
fn unroll_on_core(module: &mut Module, throughput: bool, target_unrolling: bool, opt_level: u32, immediate_bits: u32, rate: i64) {
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(module).expect("closes");
    let tuning = llrm_mir::target::Tuning { unroll_milliclocks_per_byte: rate, unroll_promotion_fits_registers: true, unroll_store_immediate_bits: immediate_bits, ..llrm_mir::target::Tuning::PIN };
    let tti = X86::of_cpu("x86-64").expect("a cpu").with_tuning(tuning);
    let options = Options {
        opt_level,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling,
        inliner_follows: true,
    };
    let target = throughput.then(|| {
        use llrm_target::Target;
        let prices = llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts::default(),
            spaces: llrm_x86_m32::layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: llrm_x86_m32::M32.flat_ports(),
        };
        llrm_target::described_throughput(&prices, None, None, None, Some(sandy_bridge_throughput()))
    });
    unroll_function_on(module, "f", &tti, options, target).expect("unrolls");
}

/// huffman_decode -O2 at m64: a 16-trip fill of folded constants was priced "saves 97 clk" at one clock a
/// folded instruction and unrolled, +208 B for 23 clocks run (paired, 13 blocks, ratio spread 1.5%). Its
/// trip is bound by the issue width and the ALU ports, not by a chain: the folded instructions free issue
/// slots, about a clock for the three removed, so the 55 B stay rolled at 600 milliclocks a byte.
#[test]
fn test_a_fill_of_folded_constants_is_unrolled_only_where_the_core_is_priced_serially() {
    assert!(fill_unrolled(false, false), "premise: priced at a clock an instruction, the unroll pays");
    assert!(!fill_unrolled(true, false), "the SandyBridge resources: the folded instructions free issue slots, not clocks");
}

/// dot -O3 m64: the 40-trip loop around `dot(a, b)` (a function the inliner takes next) was priced by the core's
/// resources at "saves 0 clk", kept rolled, and `dot` stayed unfolded: +306 B and 87 -> 170 clocks a call. A
/// trip that calls it is outside the model; the serial price decides, as before.
#[test]
fn test_a_loop_calling_a_function_the_inliner_takes_is_priced_serially_on_a_core_with_resources() {
    assert!(fill_unrolled(false, true), "premise: priced serially, the unroll pays");
    assert!(fill_unrolled(true, true), "the callee folds on the copies' constants: not a trip the resources show");
}

/// A local array of `ty` filled by one loop from its counter (`step` times it) and read by the next: the fill loop
/// (`%i`) after the unroll pass at -O3, on a core with the SandyBridge throughput model and a store immediate of
/// `immediate_bits` signed bits (0: none). The reader multiplies by a loop-invariant argument.
fn array_fill_unrolled(ty: &str, step: u64, immediate_bits: u32) -> bool {
    let conv = match ty {
        "i64" => "zext i32 %i to i64",
        "i16" => "trunc i32 %i to i16",
        _ => "add i32 %i, 0",
    };
    let mut module = parsed(&format!(
        "define {ty} @f({ty} %k) {{
entry:
  %b = alloca [48 x {ty}]
  br label %i.loop

i.loop:
  %i = phi i32 [ 0, %entry ], [ %in, %i.loop ]
  %iw = {conv}
  %v = mul {ty} %iw, {step}
  %w = sub {ty} 100, %v
  %p = getelementptr [48 x {ty}], ptr %b, i32 0, i32 %i
  store {ty} %w, ptr %p
  %in = add i32 %i, 1
  %c = icmp ne i32 %in, 48
  br i1 %c, label %i.loop, label %j.loop

j.loop:
  %j = phi i32 [ 0, %i.loop ], [ %jn, %j.loop ]
  %s = phi {ty} [ 0, %i.loop ], [ %sn, %j.loop ]
  %q = getelementptr [48 x {ty}], ptr %b, i32 0, i32 %j
  %x = load {ty}, ptr %q
  %m = mul {ty} %x, %k
  %sn = add {ty} %s, %m
  %jn = add i32 %j, 1
  %d = icmp ne i32 %jn, 48
  br i1 %d, label %j.loop, label %done

done:
  ret {ty} %sn
}}
"
    ));
    unroll_on_core(&mut module, true, true, 3, immediate_bits, 1000);
    !printed(&module).contains("%i = phi")
}

/// dot -O3 m64: `b[i] = 70 - 2 * i` over 48 trips, read by the 40-trip loop that follows, was priced "saves 24 clk,
/// adds 330 B" and kept rolled: the 48 constants were counted as 48 live values for 16 registers, though a stored
/// constant is an immediate of the reader, not a register. The array stayed in memory, the reader's loop stayed
/// rolled, and the function grew 817 -> 1123 B (87 -> 170 clk).
#[test]
fn test_a_local_array_filled_with_constants_is_unrolled_for_the_reader_that_folds_it() {
    assert!(array_fill_unrolled("i32", 7, 32), "constants stored to a local array are immediates in the reader, not registers");
    assert!(array_fill_unrolled("i16", 7, 32), "a narrower constant is an immediate as well");
    assert!(array_fill_unrolled("i64", 7, 32), "an i64 constant in the sign-extended imm32 range is an immediate");
}

/// The same fill where the constant is not an immediate: 48 live values for the registers, the fill stays rolled.
#[test]
fn test_a_fill_of_constants_the_store_cannot_take_as_immediates_keeps_counting_registers() {
    assert!(!array_fill_unrolled("i64", 1 << 33, 32), "an i64 beyond imm32 is loaded into a register first");
    assert!(!array_fill_unrolled("i32", 7, 0), "a target with no store immediate (arm64) is unchanged");
}

/// memfuncs_runtime arm64 -O2/-O3: a call in a loop named a callee the pipeline had declared after the pass's snapshot
/// of the module (an id past `run.globals`); pricing the trip indexed it and the compiler panicked ("the len is 14
/// but the index is 14"), where the parent compiled it. The trip is priced, not a crash.
#[test]
fn test_a_loop_calling_a_callee_declared_after_the_snapshot_is_priced_not_a_crash() {
    let mut module = parsed(
        "define void @f(ptr %p) {
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %q = getelementptr i8, ptr %p, i32 %i
  call void @late(ptr %q)
  %next = add i32 %i, 1
  %more = icmp ne i32 %next, 4
  br i1 %more, label %loop, label %done

done:
  ret void
}

declare void @late(ptr)
",
    );
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let tuning = llrm_mir::target::Tuning { unroll_milliclocks_per_byte: 600, ..llrm_mir::target::Tuning::PIN };
    let tti = X86::of_cpu("x86-64").expect("a cpu").with_tuning(tuning);
    let options = Options {
        opt_level: 2,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: false,
        inliner_follows: true,
    };
    super::unroll_function_hiding(&mut module, "f", &tti, options, None, 1).expect("unrolls without a panic");
}

/// m68k float_in_memory_loop: sixteen trips of `acc += 0.5` fold to a constant,
/// but the target's unroll threshold (32) is under the unrolled size (64) and
/// the analysis stopped at the pin's 10 trips, so the loop stayed: 1600
/// soft-float adds, 3.54 M clocks against clang's 1798. The pin unrolls it
/// without analysis (its bar is 150); under a lower bar the analysis now runs
/// for every loop the pin's bar would have unrolled.
#[test]
fn test_a_constant_float_chain_under_a_low_unroll_threshold_is_unrolled() {
    let mut module = parsed(
        "define double @f() {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b1 ]
  %acc = phi double [ 0.0, %b0 ], [ %s, %b1 ]
  %s = fadd double %acc, 5.000000e-01
  %next = add i32 %i, 1
  %go = icmp ult i32 %next, 16
  br i1 %go, label %b1, label %b2

b2:
  ret double %s
}
",
    );
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu").with_tuning(llrm_mir::target::Tuning {
        unroll_threshold: Some(32),
        unroll_max_percent_threshold_boost: Some(400),
        ..llrm_mir::target::Tuning::PIN
    });
    let options = Options {
        opt_level: 2,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: false,
        inliner_follows: true,
    };
    assert!(unroll_function(&mut module, "f", &tti, options).expect("unrolls"), "{}", printed(&module));
}

/// `n` run-time loops in a row behind one guard, as the pipeline hands them to
/// the unroll (`for (i = 0; i < n; i++) { s += v[i] * 3; v[i] = s ^ 1; }`), a
/// sum carried from one to the next.
fn runtime_loops(n: usize) -> String {
    let mut text = String::from(
        "@v = global [64 x i32] zeroinitializer\n\ndefine i32 @f(i32 %n) {\nentry:\n  %g = icmp sgt i32 %n, 0\n  br i1 %g, label %ph0, label %end\n\n",
    );
    for k in 0..n {
        let before = if k == 0 { "0".to_owned() } else { format!("%u{}", k - 1) };
        let after = if k + 1 == n { "end".to_owned() } else { format!("ph{}", k + 1) };
        text += &format!(
            "ph{k}:\n  %t{k} = zext nneg i32 %n to i64\n  br label %b{k}\n\nb{k}:\n  %i{k} = phi i64 [ 0, %ph{k} ], [ %in{k}, %b{k} ]\n  %s{k} = phi i32 [ %u{k}, %b{k} ], [ {before}, %ph{k} ]\n  %p{k} = getelementptr inbounds nuw [4 x i8], ptr @v, i64 %i{k}\n  %v{k} = load i32, ptr %p{k}\n  %m{k} = mul nsw i32 %v{k}, {}\n  %u{k} = add nsw i32 %m{k}, %s{k}\n  %e{k} = xor i32 %u{k}, {}\n  store i32 %e{k}, ptr %p{k}\n  %in{k} = add nuw nsw i64 %i{k}, 1\n  %x{k} = icmp eq i64 %in{k}, %t{k}\n  br i1 %x{k}, label %{after}, label %b{k}\n\n",
            3 + k % 5,
            k + 1
        );
    }
    text + &format!("end:\n  %r = phi i32 [ 0, %entry ], [ %u{}, %b{} ]\n  ret i32 %r\n}}\n", n - 1, n - 1)
}

/// The trip counts the run-time unroll of `n` loops in a row works out.
fn runtime_unroll_counts(n: usize) -> usize {
    let before = llrm_analysis::scev::trip::backedge_taken_computations();
    let mut module = parsed(&runtime_loops(n));
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let tti = X86::of_cpu("x86-64").expect("a cpu");
    let options = Options {
        opt_level: 2,
        opt_size: false,
        min_size: false,
        only_when_forced: false,
        prepare_for_lto: false,
        target_unrolling: true,
        inliner_follows: true,
    };
    assert!(unroll_function(&mut module, "f", &tti, options).expect("unrolls"), "premise: the loops are unrolled");
    llrm_analysis::scev::trip::backedge_taken_computations() - before
}

/// The unroll threw its expressions away after each run-time unrolled loop, so
/// each loop after it worked out the trip counts of all those before it again:
/// 8 loops cost 51 counts, 16 cost 167, 32 cost 591. The pin's one
/// `ScalarEvolution` lives on through the pass, and `forgetTopmostLoop` drops
/// the loop it unrolled: 4x the loops, about 4x the counts.
#[test]
fn the_trip_counts_a_run_time_unroll_works_out_grow_with_the_loops_not_their_square() {
    let (small, large) = (runtime_unroll_counts(8), runtime_unroll_counts(32));
    assert!(large < 6 * small, "8 loops: {small} counts, 32 loops: {large}");
}
