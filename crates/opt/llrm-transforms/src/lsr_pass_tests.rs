//! The pass over whole functions, run against the interpreter.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;
use llrm_x86::tti::{RealMode, X86};

use crate::testing::{parsed, printed, results};

/// `text`'s @f with LSR run on it (the x86 profile `cpu`), printed.
fn reduced(
    text: &str,
    cpu: &str,
    inputs: &[&[i128]],
) -> (bool, String) {
    reduced_on(DOS, text, cpu, inputs)
}

/// 32-bit pointers, as the flat targets have.
const FLAT: &str = "target datalayout = \"e-p:32:32-i64:32-f64:32-f80:32-n8:16:32-S32\"\n\n";

/// [`reduced`] on the datalayout `prefix`.
fn reduced_on(
    prefix: &str,
    text: &str,
    cpu: &str,
    inputs: &[&[i128]],
) -> (bool, String) {
    reduced_stressed(prefix, text, cpu, inputs, false)
}

/// [`reduced_on`], with every IV chain made if `stress`.
fn reduced_stressed(
    prefix: &str,
    text: &str,
    cpu: &str,
    inputs: &[&[i128]],
    stress: bool,
) -> (bool, String) {
    let mut module = parsed(&format!("{prefix}{text}"));
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu(cpu).unwrap();
    let changed = crate::lsr_pass::reduced_with(context, &layout, function, &mut declared, &outer, &tti, stress);
    let after = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (changed, after)
}

/// A local array filled and summed by one loop, the exit `i == n`.
const ARRAY: &str = "define i32 @f(i32 %n) {
entry:
  %buf = alloca [64 x i32]
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s2, %loop ]
  %a = getelementptr i32, ptr %buf, i32 %i
  store i32 7, ptr %a
  %v = load i32, ptr %a
  %s2 = add i32 %s, %v
  %next = add i32 %i, 1
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret i32 %s2
}
";

/// `opt -passes=loop-reduce` (LLVM 20, i386) turns this into a pointer that
/// advances by 4 and a counter from `%n` down to zero compared with zero; the
/// index, the `getelementptr` on it and the `n` it was compared with go. The
/// interpreter agrees on every input.
#[test]
fn an_array_walk_is_a_pointer_and_a_counter_to_zero() {
    let (changed, after) = reduced(ARRAY, "i486", &[&[1], &[5], &[17], &[40]]);
    assert!(changed, "{after}");
    assert!(after.contains("getelementptr i8, ptr"), "{after}");
    assert!(!after.contains("getelementptr i32"), "{after}");
    assert!(after.contains(", 0\n"), "{after}");
    let phis = after.matches(" = phi ").count();
    assert_eq!(phis, 3, "the sum, the pointer and the counter: {after}");
}

/// A loop with no induction variable (the xor of two unknowns is no
/// recurrence) is left as it is.
#[test]
fn a_loop_with_nothing_to_reduce_is_left() {
    let (changed, after) = reduced(
        "define i32 @f(i32 %n) {
entry:
  br label %loop
loop:
  %s = phi i32 [ 0, %entry ], [ %s2, %loop ]
  %s2 = xor i32 %s, %n
  %c = icmp eq i32 %s2, 100
  br i1 %c, label %exit, label %loop
exit:
  ret i32 %s2
}
",
        "i486",
        &[&[100]],
    );
    assert!(!changed, "{after}");
}

/// An index that is both stored as a value and the subscript of a store (the
/// merge loop of `x_mergesort` keeps `k` and `k*4` live together with the old
/// pass): the address takes `k` scaled by 4, so the one counter serves both,
/// and no second induction variable is made.
#[test]
fn an_index_used_as_a_value_and_a_subscript_stays_one_counter() {
    let (changed, after) = reduced_on(
        FLAT,
        "define i32 @f(i32 %n) {
entry:
  %buf = alloca [64 x i32]
  br label %loop
loop:
  %k = phi i32 [ 0, %entry ], [ %next, %loop ]
  %a = getelementptr i32, ptr %buf, i32 %k
  store i32 %k, ptr %a
  %next = add i32 %k, 1
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  %p = getelementptr i32, ptr %buf, i32 3
  %v = load i32, ptr %p
  ret i32 %v
}
",
        "i486",
        &[&[5], &[17]],
    );
    let phis = after.matches(" = phi ").count();
    assert!(changed, "{after}");
    assert_eq!(phis, 1, "one induction variable: {after}");
    assert_eq!(after.matches("add i32 %k, 1").count(), 1, "the compare reuses the increment: {after}");
}

/// `(double) i` of a counter that cannot overflow is a second variable that
/// counts in doubles, and the conversion in the loop goes (`OptimizeShadowIV`).
#[test]
fn a_counter_converted_to_double_gets_a_double_counter() {
    let (changed, after) = reduced_on(
        FLAT,
        "define i32 @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %s = phi double [ 0.0, %entry ], [ %s2, %loop ]
  %d = sitofp i32 %i to double
  %s2 = fadd double %s, %d
  %next = add nsw i32 %i, 1
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  %r = fptosi double %s2 to i32
  ret i32 %r
}
",
        "i486",
        &[&[1], &[5], &[17]],
    );
    assert!(changed, "{after}");
    assert!(!after.contains("sitofp"), "{after}");
    assert!(after.contains("IV.S."), "{after}");
}

/// Stores at `i`, `i + x` and `i + 2x`: `opt-20` shares the one counter `i`
/// and gives each address its own base, with no chain; so do we, and the
/// interpreter agrees.
#[test]
fn stores_a_variable_stride_apart_keep_one_counter() {
    let (changed, after) = reduced_on(
        FLAT,
        "define i32 @f(i32 %n, i32 %x) {
entry:
  %buf = alloca [256 x i32]
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %p0 = getelementptr i32, ptr %buf, i32 %i
  store i32 %i, ptr %p0
  %j = add i32 %i, %x
  %p1 = getelementptr i32, ptr %buf, i32 %j
  store i32 %i, ptr %p1
  %k = add i32 %j, %x
  %p2 = getelementptr i32, ptr %buf, i32 %k
  store i32 %i, ptr %p2
  %next = add i32 %i, 1
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  %q = getelementptr i32, ptr %buf, i32 1
  %v = load i32, ptr %q
  ret i32 %v
}
",
        "i486",
        &[&[3, 4], &[5, 7], &[9, 2]],
    );
    assert!(changed, "{after}");
    assert_eq!(after.matches(" = phi ").count(), 1, "{after}");
}

const STRIDES: &str = "define i32 @f(i32 %n, i32 %x) {
entry:
  %buf = alloca [256 x i32]
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %p0 = getelementptr i32, ptr %buf, i32 %i
  store i32 %i, ptr %p0
  %j = add i32 %i, %x
  %p1 = getelementptr i32, ptr %buf, i32 %j
  store i32 %i, ptr %p1
  %k = add i32 %j, %x
  %p2 = getelementptr i32, ptr %buf, i32 %k
  store i32 %i, ptr %p2
  %next = add i32 %i, 1
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  %q = getelementptr i32, ptr %buf, i32 1
  %v = load i32, ptr %q
  ret i32 %v
}
";

/// With every chain made (`-lsr-stress-ivchain`), the three stores are one
/// chain: each address is made from the one before by the increment `x`, and
/// the interpreter agrees with the original on every input.
#[test]
fn a_chain_of_stores_a_variable_stride_apart_computes_the_same() {
    let (_, plain) = reduced_on(FLAT, STRIDES, "i486", &[&[3, 4], &[5, 7], &[9, 2]]);
    let (changed, chained) = reduced_stressed(FLAT, STRIDES, "i486", &[&[3, 4], &[5, 7], &[9, 2]], true);
    assert!(changed, "{chained}");
    assert_ne!(plain, chained, "the chain changes the code");
    // `p1 = p0 + 4x` and `p2 = p1 + 4x`: the increment is made once.
    assert_eq!(chained.matches("shl i32 %x, 2").count(), 1, "{chained}");
    assert_eq!(chained.matches("getelementptr i8, ptr %").count(), 3, "{chained}");
}

/// The loop of qb's `SEVEN_TERMS`: seven `sp(yy(k) + f(x - xp(k)))` of one
/// counter `x` on 16-bit pointers. `opt -passes=loop-reduce` (LLVM 20, i386)
/// keeps the one counter and writes each `f(x - xp(k))` as a base plus `x * 2`:
/// one phi.
#[test]
fn seven_affine_terms_of_one_counter_keep_one_phi() {
    let text = include_str!("lsr_seven_terms.ll");
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu("i486").unwrap();
    let changed = crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
    let after = printed(&module);
    assert!(changed, "{after}");
    assert_eq!(after.matches(" = phi ").count(), 1, "{after}");
    // As `opt` has them, one `x * 2` per term; a later CSE makes it one.
    assert_eq!(after.matches("shl nuw nsw i16 %").count(), 7, "{after}");
}

/// The dominator tree, loops and assumptions are made once for a function, not
/// once per loop and attempt: the old `lsr` asked each analysis again per solve
/// and doubled in cost with the body (W2's callgrind of d_faces).
#[test]
fn the_analyses_are_made_once_for_the_loops_of_a_function() {
    let one = |k: usize| {
        format!(
            "  br label %h{k}\nh{k}:\n  %i{k} = phi i32 [ 0, %e{k} ], [ %n{k}, %h{k} ]\n  %a{k} = getelementptr i32, ptr %buf, i32 %i{k}\n  store i32 7, ptr %a{k}\n  %n{k} = add i32 %i{k}, 1\n  %c{k} = icmp eq i32 %n{k}, %n\n  br i1 %c{k}, label %e{}, label %h{k}\ne{}:\n",
            k + 1,
            k + 1
        )
    };
    let mut text = String::from("define i32 @f(i32 %n) {\ne0:\n  %buf = alloca [64 x i32]\n");
    for k in 0..4 {
        text += &one(k);
    }
    text += "  ret i32 0\n}\n";
    crate::lsr_pass::SESSIONS.with(|made| made.set(0));
    let (changed, after) = reduced_on(FLAT, &text, "i486", &[&[5]]);
    assert!(changed, "{after}");
    assert_eq!(crate::lsr_pass::SESSIONS.with(|made| made.get()), 1, "{after}");
}

/// nib's `zip(a, b)` dot product as the strength stage gets it: three exit
/// tests of the element index (the bounds checks, not yet merged). `opt
/// -passes=loop-reduce` (LLVM 20, i386) keeps the element index and writes each
/// address as a base plus `x << 1`, two phis with the sum; so does this pass,
/// with `add`s and `icmp`s of that index as before. The zip pin's byte counter
/// (`add bx, 2`) needs the exits merged upstream first.
#[test]
fn a_zip_loop_with_three_element_exits_keeps_the_element_counter_as_opt_does() {
    let text = include_str!("lsr_zip.ll");
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("dot").expect("@dot");
    let tti = X86::of_cpu("i486").unwrap();
    let changed = crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
    let after = printed(&module);
    assert!(changed, "{after}");
    assert_eq!(after.matches(" = phi i").count(), 4, "the element index, the sum and the two joins: {after}");
    assert_eq!(after.matches("shl i16 %8, 1").count(), 2, "{after}");
}

/// `LLRM_DEBUG=counts` could not say whether LSR still rewrote anything on a
/// bench (d_faces read 11 runs and a fraction of the old cost): the loop it
/// looked at, the solution it chose and the rewrite it made are counted.
#[test]
fn lsr_counts_the_loops_it_looks_at_solves_and_rewrites() {
    use llrm_mir::counts::{Event, get};
    let before = [Event::LsrLoops, Event::LsrSolutions, Event::LsrRewritten].map(get);
    let (changed, after) = reduced(ARRAY, "i486", &[&[5]]);
    assert!(changed, "{after}");
    let now = [Event::LsrLoops, Event::LsrSolutions, Event::LsrRewritten].map(get);
    assert_eq!(now.iter().zip(&before).map(|(now, before)| now - before).collect::<Vec<_>>(), [1, 1, 1], "{after}");
}

/// `for i < 33: p[i] = (7 * i) & 31`, stored through `p + 2 * i`.
const FILL: &str = "define i16 @f(i16 %k) {
b1:
  %p = alloca [70 x i8]
  br label %b2
b2:
  %v = phi i16 [ 0, %b1 ], [ %v2, %b5 ]
  %i = phi i16 [ 0, %b1 ], [ %i2, %b5 ]
  %c = icmp samesign ult i16 %i, 33
  br i1 %c, label %b5, label %b6
b5:
  %o = shl nuw nsw i16 %i, 1
  %a = getelementptr i8, ptr %p, i16 %o
  %m = and i16 %v, 31
  store i16 %m, ptr %a
  %i2 = add nuw nsw i16 %i, 1
  %v2 = add nuw nsw i16 %v, 7
  br label %b2
b6:
  %o2 = shl i16 %k, 1
  %q = getelementptr i8, ptr %p, i16 %o2
  %r = load i16, ptr %q
  ret i16 %r
}
";

/// #3075: real mode priced like the flat model kept the `shl` and the element
/// counter, listed as `lea` and 32-bit addresses in 16-bit code. A priced
/// prefixed form was only a late tie-break (the scale key sits behind the
/// registers), so the 16-bit forms alone are legal and the fill steps a
/// pointer by 2 as before the flip.
#[test]
fn a_scaled_address_is_not_formed_in_real_mode_and_the_fill_steps_a_pointer() {
    let real = RealMode {
        scale_costs: vec![(1, 0), (2, 2), (4, 2), (8, 2)],
        registers: 6,
        scaled_index_insns: 2,
        huge_space: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
    };
    let tti = X86::of_cpu("i486").unwrap().in_real_mode(real);
    let mut module = parsed(&format!("{DOS}{FILL}"));
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
    let after = printed(&module);
    assert_eq!(results(&module, &[&[0], &[3]]), results(&before, &[&[0], &[3]]), "{after}");
    assert!(after.contains("getelementptr i8, ptr %0, i16 2"), "{after}");
    assert!(!after.contains(" shl nuw nsw i16 %i, 1"), "{after}");
}

/// planets.nib and flat_containers.nib (m16 and m32) panicked at
/// `intrinsic_name`: a call to a function declared after the globals LSR was
/// handed were read indexed past their end.
#[test]
fn a_call_to_a_global_the_outer_snapshot_lacks_is_no_intrinsic() {
    let text = "declare void @h(ptr)\n\ndefine i32 @f(i32 %n) {
entry:
  %buf = alloca [64 x i32]
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %a = getelementptr i32, ptr %buf, i32 %i
  store i32 7, ptr %a
  call void @h(ptr %a)
  %next = add i32 %i, 1
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret i32 %next
}
";
    let mut module = parsed(&format!("{FLAT}{text}"));
    let (layout, mut outer) = (layout(&module), Outer::of(&module, None));
    let known = module.globals.iter().position(|global| global.name.as_deref() == Some("h")).unwrap();
    outer.globals = std::rc::Rc::new(outer.globals[..known].to_vec());
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu("i486").unwrap();
    crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
}

/// An 8x8 walk: the inner loop's base and addend step with the outer loop. The
/// outer loop is entered by a conditional branch, so it has no preheader.
const NEST: &str = "define i32 @f(i32 %k) {
entry:
  %p = alloca [256 x i8]
  %g = icmp sgt i32 %k, 0
  br i1 %g, label %outer, label %exit
outer:
  %o = phi i32 [ 0, %entry ], [ %o2, %latch ]
  %acc = phi i32 [ 0, %entry ], [ %r, %latch ]
  %q = getelementptr inbounds nuw [32 x i8], ptr %p, i32 %o
  %s = shl nuw nsw i32 %o, 3
  %t = or disjoint i32 %s, 1
  br label %inner
inner:
  %acc1 = phi i32 [ %acc, %outer ], [ %r, %inner ]
  %i = phi i32 [ 0, %outer ], [ %i2, %inner ]
  %a = getelementptr inbounds nuw [4 x i8], ptr %q, i32 %i
  store i32 %i, ptr %a
  %v = load i32, ptr %a
  %w = add nuw nsw i32 %t, %i
  %m = mul i32 %v, %w
  %r = add i32 %m, %acc1
  %i2 = add nuw nsw i32 %i, 1
  %c = icmp samesign ult i32 %i2, 8
  br i1 %c, label %inner, label %latch
latch:
  %o2 = add nuw nsw i32 %o, 1
  %c2 = icmp samesign ult i32 %o2, 8
  br i1 %c2, label %outer, label %exit
exit:
  %res = phi i32 [ 0, %entry ], [ %r, %latch ]
  ret i32 %res
}
";

/// matmul -Os (465 bytes against 326 for llc): the outer loop of its last nest
/// had no preheader, so LSR left the inner loop's `q + 32 * o` and `8 * o + 1`
/// as per-trip values of the outer loop; `opt -passes=loop-reduce`
/// (LoopSimplify required) steps one pointer and one counter by the outer loop.
#[test]
fn an_outer_loop_without_a_preheader_gets_its_invariants_stepped() {
    let (changed, after) = reduced_on(FLAT, NEST, "i486", &[&[1], &[3]]);
    assert!(changed, "{after}");
    assert!(after.contains(" = phi ptr"), "{after}");
    assert!(!after.contains(" = shl nuw nsw i32 %o, 3"), "{after}");
}

/// `loops` counted loops one after another, each with an array walk to reduce.
fn walks(loops: usize) -> String {
    let mut text = String::from("define i32 @f(i32 %n) {\ne0:\n  %buf = alloca [64 x i32]\n");
    for k in 0..loops {
        text += &format!(
            "  br label %h{k}\nh{k}:\n  %i{k} = phi i32 [ 0, %e{k} ], [ %n{k}, %h{k} ]\n  %a{k} = getelementptr i32, ptr %buf, i32 %i{k}\n  store i32 7, ptr %a{k}\n  %n{k} = add i32 %i{k}, 1\n  %c{k} = icmp eq i32 %n{k}, %n\n  br i1 %c{k}, label %e{}, label %h{k}\ne{}:\n",
            k + 1,
            k + 1
        );
    }
    text + "  ret i32 0\n}\n"
}

/// The trip counts LSR works out over `loops` loops it rewrites.
fn lsr_trip_counts(loops: usize) -> usize {
    let before = llrm_analysis::scev::trip::backedge_taken_computations();
    let (changed, after) = reduced_on(FLAT, &walks(loops), "i486", &[&[5]]);
    assert!(changed, "{after}");
    llrm_analysis::scev::trip::backedge_taken_computations() - before
}

/// LSR dropped every expression after each edit, so each loop it came to worked
/// out the trip counts of the loops before it again: 4 loops cost 26 counts and
/// 16 cost 392 (quadratic; 40 M of d_faces -O2's LSR against the pin's 18 M).
/// Only what an edit touched is dropped now: 4x the loops, about 4x the counts.
#[test]
fn the_trip_counts_lsr_works_out_grow_with_the_loops_not_their_square() {
    let (small, large) = (lsr_trip_counts(4), lsr_trip_counts(16));
    assert!(large < 6 * small, "4 loops: {small} counts, 16 loops: {large}");
}

/// recsum after tail recursion elimination: `n` only ends the loop, and `3n`
/// (the factor 3 is odd, so `n == 0` and `3n == 0` are the same compare even
/// where `3n` wraps) is read in it. LLVM's `getExactSDiv` wants the product to
/// not overflow and keeps `n` as a second counter: main's loop exits on the
/// scaled one (`add ebx, -3; jne`), this kept `dec ebx` beside it, 79296 clocks
/// against 77291.
#[test]
fn a_counter_that_only_ends_the_loop_is_dropped_for_its_odd_multiple() {
    let text = "define i32 @f(i32 %n) {
e:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %x, label %p
p:
  %m = mul i32 %n, 3
  br label %h
h:
  %t = phi i32 [ %m, %p ], [ %t1, %h ]
  %c = phi i32 [ %n, %p ], [ %c1, %h ]
  %acc = phi i32 [ 0, %p ], [ %acc1, %h ]
  %c1 = add nsw i32 %c, -1
  %r = srem i32 %t, 11
  %acc1 = add nsw i32 %r, %acc
  %t1 = add i32 %t, -3
  %done = icmp eq i32 %c1, 0
  br i1 %done, label %q, label %h
q:
  br label %x
x:
  %res = phi i32 [ 0, %e ], [ %acc1, %q ]
  ret i32 %res
}
";
    let (changed, after) = reduced_on(FLAT, text, "i486", &[&[0], &[1], &[7], &[2000]]);
    assert!(changed, "{after}");
    // The loop's phis are the scaled counter and the sum; the exit phi is the
    // third.
    assert_eq!(after.matches(" = phi i32").count(), 3, "{after}");
    assert!(after.contains("icmp eq i32 %t1, 0"), "{after}");
}

/// particle's ADVANCE: six arrays read and written at `base + 2 * i`. With
/// the prefixed index priced once for each scaled register the six share one
/// counter; priced as illegal they were six pointer recurrences, 44 executed
/// instructions a trip in place of 13.
#[test]
fn six_arrays_indexed_by_one_counter_share_it_in_real_mode() {
    let arrays = ["a", "b", "c", "d", "e", "g"];
    let params: Vec<String> = arrays.iter().map(|name| format!("ptr %{name}")).collect();
    let mut body = String::new();
    for name in arrays {
        body += &format!(
            "  %p{name} = getelementptr i8, ptr %{name}, i16 %o\n  %v{name} = load i16, ptr %p{name}\n  %w{name} = add i16 %v{name}, 1\n  store i16 %w{name}, ptr %p{name}\n"
        );
    }
    let text = format!(
        "{DOS}define void @f({}, i16 %n) {{
b1:
  br label %b2
b2:
  %i = phi i16 [ 0, %b1 ], [ %i2, %b2 ]
  %o = shl nuw nsw i16 %i, 1
{body}  %i2 = add nuw nsw i16 %i, 1
  %cond = icmp slt i16 %i2, %n
  br i1 %cond, label %b2, label %b3
b3:
  ret void
}}
",
        params.join(", ")
    );
    let real = RealMode {
        scale_costs: vec![(1, 0), (2, 2), (4, 2), (8, 2)],
        registers: 6,
        scaled_index_insns: 2,
        huge_space: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
    };
    let tti = X86::of_cpu("i486").unwrap().in_real_mode(real);
    let mut module = parsed(&text);
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
    let after = printed(&module);
    assert!(!after.contains(" = phi ptr"), "no pointer recurrence: {after}");
    assert_eq!(after.matches(" = phi i16").count(), 1, "the one counter: {after}");
}

/// bench/huge (`-O2 --huge-arrays`): a row's elements read at `row + 2 * c`.
/// The pin hands `isLegalAddressingMode` the access's address space; ours did
/// not, so a huge pointer plus a counter looked like a free `[base + index]`
/// and the inner loop kept a counter, no huge recurrence for `window` to
/// split: the selector:offset carry was recomputed on every element, 1549119
/// executed instructions against 1052305.
#[test]
fn a_huge_array_row_is_walked_by_a_huge_pointer_in_real_mode() {
    let text = format!(
        "{DOS}define i32 @f(ptr addrspace(3) %h) {{
b1:
  br label %b2
b2:
  %r = phi i16 [ 0, %b1 ], [ %r2, %b4 ]
  %t = phi i32 [ 0, %b1 ], [ %t2, %b4 ]
  %rz = zext nneg i16 %r to i32
  %rb = mul nuw nsw i32 %rz, 201
  br label %b3
b3:
  %c = phi i16 [ 0, %b2 ], [ %c2, %b3 ]
  %s = phi i32 [ %t, %b2 ], [ %s2, %b3 ]
  %cz = zext nneg i16 %c to i32
  %e = add nuw nsw i32 %rb, %cz
  %o = shl nuw nsw i32 %e, 1
  %p = getelementptr i8, ptr addrspace(3) %h, i32 %o
  %v = load i16, ptr addrspace(3) %p
  %vz = sext i16 %v to i32
  %s2 = add i32 %s, %vz
  %c2 = add nuw nsw i16 %c, 1
  %ce = icmp ne i16 %c2, 201
  br i1 %ce, label %b3, label %b4
b4:
  %t2 = phi i32 [ %s2, %b3 ]
  %r2 = add nuw nsw i16 %r, 1
  %re = icmp ne i16 %r2, 200
  br i1 %re, label %b2, label %b5
b5:
  ret i32 %t2
}}
"
    );
    let real = RealMode {
        scale_costs: vec![(1, 0), (2, 2), (4, 2), (8, 2)],
        registers: 6,
        scaled_index_insns: 2,
        huge_space: Some(3),
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
    };
    let tti = X86::of_cpu("i486").unwrap().in_real_mode(real);
    let mut module = parsed(&text);
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
    let after = printed(&module);
    let inner = after.split("b3:").nth(1).and_then(|rest| rest.split("\nb4:").next()).expect("b3");
    assert!(inner.contains(" = phi ptr addrspace(3)"), "the inner loop walks a huge pointer: {after}");
}

/// The pass began with no expressions of its own function, so the loop passes
/// before it (IndVars) had made them for nothing: a second run on a function it
/// leaves as it is made as many expressions as the first (4.6 M Ir of
/// `get_scev` inside LSR on d_faces -O2, against the pin's cached SE).
#[test]
fn test_lsr_continues_the_expressions_the_passes_before_it_made() {
    use llrm_mir::counts::{Event, get};
    let mut module = parsed(&format!("{DOS}{ARRAY}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu("i486").unwrap();
    let (_, kept) =
        crate::lsr_pass::reduced_held(context, &layout, function, &mut declared, &outer, &tti, false, false, None);
    let kept = kept.expect("no edge was split");
    let mut nodes = |held| {
        let before = get(Event::ScevNodes);
        crate::lsr_pass::reduced_held(context, &layout, function, &mut declared, &outer, &tti, false, false, held);
        get(Event::ScevNodes) - before
    };
    let (cold, warm) = (nodes(None), nodes(Some(kept)));
    assert!(cold > 0);
    assert!(warm < cold, "the held expressions were not read: {warm} new nodes, {cold} cold");
}

/// x_reverse (AM-BT-26): the outer loop's `k & 15` was made its own `phi i4`
/// stepped beside the i32 counter `k`, one live value too many, which spilled
/// the counter at m32 (50 instructions against the pin's 42). The pin's final
/// loop reads `trunc i32 %k to i4`: no second counter.
#[test]
fn a_masked_counter_is_a_truncation_of_the_counter_not_a_second_induction_variable() {
    let (_changed, after) = reduced_on(FLAT, REVERSE, "i486", &[]);
    assert!(!after.contains("phi i4"), "a second induction variable of the masked counter:\n{after}");
}

const REVERSE: &str = "@s = global [512 x i8] zeroinitializer
define i32 @f(i32 %n) {
entry:
  %c0 = icmp sgt i32 %n, 0
  br i1 %c0, label %outer, label %done
outer:
  %sum = phi i32 [ %sum2, %latch ], [ 0, %entry ]
  %k = phi i32 [ %k2, %latch ], [ 0, %entry ]
  %m = and i32 %k, 15
  %len = sub nuw nsw i32 400, %m
  %p = getelementptr i8, ptr @s, i32 %len
  br label %inner
inner:
  %j = phi i32 [ 0, %outer ], [ %j2, %inner ]
  %a = getelementptr i8, ptr @s, i32 %j
  %x = load i8, ptr %a
  %q = getelementptr i8, ptr %p, i32 %j
  %y = load i8, ptr %q
  store i8 %y, ptr %a
  store i8 %x, ptr %q
  %j2 = add nuw nsw i32 %j, 1
  %ic = icmp ne i32 %j2, %len
  br i1 %ic, label %inner, label %latch
latch:
  %b = and i32 %k, 255
  %ga = getelementptr inbounds nuw i8, ptr @s, i32 %b
  %v = load i8, ptr %ga
  %vs = sext i8 %v to i32
  %sum2 = add nsw i32 %sum, %vs
  %k2 = add nuw nsw i32 %k, 1
  %ec = icmp ne i32 %k2, %n
  br i1 %ec, label %outer, label %exit
exit:
  ret i32 %sum2
done:
  ret i32 0
}
";

/// The exit value of `s += (i * 97) % 211` was expanded at the end of the loop
/// body, a second division in every trip (`-Os` m32: 35 instructions, two
/// `div`, a spill, where one in the loop and one after it is 23).
#[test]
fn an_exit_value_is_expanded_in_the_exit_block_not_the_loop() {
    let text = "define i32 @f(i32 %n) {
b1:
  %c = icmp sgt i32 %n, 0
  br i1 %c, label %pre, label %b3
pre:
  br label %b4
exit:
  %last = phi i32 [ %s2, %b4 ]
  br label %b3
b3:
  %r = phi i32 [ 0, %b1 ], [ %last, %exit ]
  ret i32 %r
b4:
  %s = phi i32 [ %s2, %b4 ], [ 0, %pre ]
  %i = phi i32 [ %i2, %b4 ], [ 0, %pre ]
  %m = mul nuw nsw i32 %i, 97
  %q = urem i32 %m, 211
  %s2 = add nuw nsw i32 %q, %s
  %i2 = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %i2, %n
  br i1 %done, label %exit, label %b4
}
";
    let (_, after) = reduced_on(FLAT, text, "i486", &[&[0], &[1], &[5], &[300]]);
    let body = after.split("b4:").nth(1).unwrap().split("\n}").next().unwrap();
    assert_eq!(body.matches("urem").count() + body.matches("udiv").count(), 1, "{after}");
}

/// The exit value was expanded at the end of the exit block, after the store
/// that reads it: addrm.bas -Os m16 died in live ranges ("a unique value").
#[test]
fn an_exit_value_is_defined_before_the_readers_in_the_exit_block() {
    let text = "@g = global i32 0

define void @f(i32 %n) {
b1:
  %c = icmp sgt i32 %n, 0
  br i1 %c, label %pre, label %b3
pre:
  br label %b4
exit:
  %last = phi i32 [ %s2, %b4 ]
  store i32 %last, ptr @g
  br label %b3
b3:
  ret void
b4:
  %s = phi i32 [ %s2, %b4 ], [ 0, %pre ]
  %i = phi i32 [ %i2, %b4 ], [ 0, %pre ]
  %m = mul nuw nsw i32 %i, 97
  %q = urem i32 %m, 211
  %s2 = add nuw nsw i32 %q, %s
  %i2 = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %i2, %n
  br i1 %done, label %exit, label %b4
}
";
    let (_, after) = reduced_on(FLAT, text, "i486", &[&[1], &[7]]);
    let exit = after.split("exit:").nth(1).unwrap().split("b3:").next().unwrap();
    let (stored, defined) = (exit.find("store").unwrap(), exit.find("urem").or(exit.find("udiv")).unwrap());
    assert!(defined < stored, "{after}");
}

/// Two sums of a loop-invariant step over a loop of seven trips were left as
/// two phis and two adds in the loop (udtacc.bas, udtrng.bas m16 -Os); the pin
/// reads the exit values as `step * 7` and leaves only the counter.
#[test]
fn a_sum_of_an_invariant_step_read_after_a_counted_loop_is_a_product() {
    let text = "define i32 @f(i32 %x, i32 %y) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 1, %entry ], [ %next, %loop ]
  %a = phi i32 [ 0, %entry ], [ %a2, %loop ]
  %b = phi i32 [ 0, %entry ], [ %b2, %loop ]
  %a2 = add i32 %x, %a
  %b2 = add i32 %y, %b
  %next = add nuw nsw i16 %i, 1
  %c = icmp samesign ugt i16 %i, 6
  br i1 %c, label %exit, label %loop
exit:
  %ra = phi i32 [ %a2, %loop ]
  %rb = phi i32 [ %b2, %loop ]
  %r = xor i32 %ra, %rb
  ret i32 %r
}
";
    let (_, after) = reduced_on(FLAT, text, "i486", &[&[3, 4], &[0, 9], &[-5, 6]]);
    let body = after.split("loop:").nth(1).unwrap().split("exit:").next().unwrap();
    assert_eq!(body.matches("phi").count(), 1, "{after}");
}

/// x_base64 m16 -Os grew 352 -> 397 B: the exit value of a counter in a loop
/// that only counts, `4 * ((smax(n, 5) - 5) / 3 + ...)`, was written out
/// after LSR under `-Os`, where the loop is kept (`exitvalues` DEVIATE 15).
#[test]
fn at_size_the_exit_value_of_a_counting_loop_is_not_written_out_by_lsr() {
    let text = "define i16 @f(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 2, %entry ], [ %i2, %loop ]
  %q = phi i16 [ -4, %entry ], [ %q2, %loop ]
  %i2 = add nuw nsw i16 %i, 3
  %q2 = add i16 %q, 4
  %c = icmp slt i16 %i2, %n
  br i1 %c, label %loop, label %exit
exit:
  %last = phi i16 [ %q2, %loop ]
  ret i16 %last
}
";
    let phis = |size: bool| {
        let mut module = parsed(&format!("{DOS}{text}"));
        let (layout, outer) = (layout(&module), Outer::of(&module, None));
        let mut declared = llrm_mir::passes::Declared::of(&module);
        let (context, function) = module.function_mut("f").expect("@f");
        let tti = X86::of_cpu("i486").unwrap();
        crate::lsr_pass::reduced_held(context, &layout, function, &mut declared, &outer, &tti, false, size, None);
        declared.place(&mut module).expect("the declarations");
        printed(&module).split("loop:").nth(1).unwrap().split("exit:").next().unwrap().matches("phi").count()
    };
    assert_eq!(phis(false), 1, "for speed the counter is written out after the loop");
    assert_eq!(phis(true), 2, "the loop at size lost its second counter");
}

/// The inner loop of an insertion sort, `for (j = i - 1; j >= 0 && a[j] > v;
/// --j)` as the strength stage gets it: the countdown's `add nsw %j, -1` sits
/// in the header beside the loads LSR rewrites. The pin moves the increment to
/// the latch once those uses are gone (`ImplementSolution`: delete the dead
/// instructions, then move the simple recurrences' increments), so the
/// decrement and the compare share a block and isel emits `dec; cmp; jg`. Here
/// the move ran before the deletion, saw the header gep as a user the latch
/// does not dominate, and left `lea edi, [ebx-1]; cmp ebx, 1; mov ebx, edi`.
#[test]
fn a_countdown_loop_decrements_in_the_block_that_tests_it() {
    let mut module = parsed(include_str!("lsr_insertion.ll"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu("i486").unwrap();
    let changed = crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
    let after = printed(&module);
    assert!(changed, "{after}");
    let block = after
        .split("\n\n")
        .find(|block| block.contains("add nsw i32") && block.contains(", -1"))
        .unwrap_or_else(|| panic!("no decrement: {after}"));
    assert!(
        block.contains("icmp sgt i32") && block.contains(", 1\n"),
        "the compare is in another block than the decrement: {after}"
    );
}

/// particle.bas ADVANCE at m16 -Os with `foldICmpBinOp`: six far arrays read at
/// one index. The rewrite to six pointer recurrences costs two instructions
/// more than the loop as written and needs more registers than exist: six
/// pointers reloaded and stored back every trip, +79 B and 344,761 -> 530,461
/// clocks. The loop is kept.
#[test]
fn six_far_arrays_at_one_index_are_not_six_recurrences_in_real_mode() {
    let arrays = ["a", "b", "c", "d", "e", "g"];
    let mut globals = String::new();
    let mut loads = String::new();
    let mut body = String::new();
    for name in arrays {
        globals += &format!("@{name} = internal global [2 x i8] zeroinitializer\n");
        loads += &format!(
            "  %w{name} = load i16, ptr @{name}\n  %i{name} = inttoptr i16 %w{name} to ptr addrspace(2)\n  %f{name} = addrspacecast ptr addrspace(2) %i{name} to ptr addrspace(1)\n"
        );
        body += &format!(
            "  %p{name} = getelementptr i8, ptr addrspace(1) %f{name}, i16 %o\n  %v{name} = load i16, ptr addrspace(1) %p{name}\n  %x{name} = add i16 %v{name}, 1\n  store i16 %x{name}, ptr addrspace(1) %p{name}\n"
        );
    }
    let text = format!(
        "{DOS}{globals}define void @f(i16 %n) {{
b1:
  %neg = sub i16 0, %n
{loads}  br label %b5
b5:
  %i = phi i16 [ %neg, %b1 ], [ %i2, %b5 ]
  %s = shl i16 %i, 1
  %o = add i16 %s, 100
{body}  %i2 = add i16 %i, 1
  %done = icmp eq i16 %i, %n
  br i1 %done, label %b6, label %b5
b6:
  ret void
}}
"
    );
    let real = RealMode {
        scale_costs: vec![(1, 0)],
        registers: 6,
        scaled_index_insns: 0,
        huge_space: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
    };
    let tti = X86::of_cpu("i486").unwrap().in_real_mode(real);
    let mut module = parsed(&text);
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
    let after = printed(&module);
    let body = after.split("b5:").nth(1).unwrap().split("b6:").next().unwrap();
    assert!(body.matches(" = phi ptr").count() == 0, "the loop is kept: {after}");
}

/// The first look at a loop whose exit test is edited made the users, the
/// second made them again over the edited loop, the third (after the edge
/// split) once more: 3 constructions for 1 loop. The edit moves no compare, so
/// the second look keeps the first's users.
#[test]
fn the_users_of_a_loop_are_kept_through_the_edit_of_its_exit_test() {
    let mut module = parsed(include_str!("lsr_insertion.ll"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu("i486").unwrap();
    crate::ivusers::BUILDS.with(|made| made.set(0));
    crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
    let built = crate::ivusers::BUILDS.with(|made| made.get());
    assert_eq!(built, 2, "{}", printed(&module));
}

/// The values LSR has analysed over `loops` loops it rewrites.
fn lsr_scevs_made(loops: usize) -> usize {
    let before = llrm_analysis::scev::create::scevs_created();
    let (changed, after) = reduced_on(FLAT, &walks(loops), "i486", &[&[5]]);
    assert!(changed, "{after}");
    llrm_analysis::scev::create::scevs_created() - before
}

/// LSR forgot the expressions of every value an edit touched, which LLVM's
/// never does (it has no `forget` call: only a deleted or replaced value is
/// told to ScalarEvolution), and analysed them again for the next look: 30
/// values a loop against 20 (branches1024: `mir lsr` 640 -> 320 Minstr).
#[test]
fn the_expressions_of_a_value_an_edit_touched_are_kept_through_lsr() {
    let loops = 16;
    assert!(lsr_scevs_made(loops) <= loops * 21, "{} values made", lsr_scevs_made(loops));
}

/// bench/sieve at m16: the inner loop starts at `i * i`, a quadratic addrec of
/// the outer loop. The pin's LSR expands it into two outer-loop induction
/// variables (a pointer and its step, `opt -passes=loop-reduce` gives the same
/// on this IR); with six registers they spill every outer iteration (sieve
/// bq16 -O2 memory operands 6087 -> 12191). DEVIATE LU5.
const SIEVE: &str = "define i32 @f() {
b1:
  %a = alloca [1024 x i8]
  br label %b13
b8:
  %c2 = phi i16 [ %c1, %b13 ], [ %c3, %b12 ], [ %c3, %e ]
  %s2 = phi i32 [ %s1, %b13 ], [ %s3, %b12 ], [ %s3, %e ]
  %n = add nuw nsw i16 %i, 1
  %done = icmp eq i16 %n, 1024
  br i1 %done, label %b9, label %b13
b9:
  %z = zext i16 %c2 to i32
  %h = shl nuw i32 %z, 16
  %x = xor i32 %h, %s2
  ret i32 %x
b12:
  %c3 = add i16 %c1, 1
  %w = zext nneg i16 %i to i32
  %s3 = add i32 %s1, %w
  %small = icmp samesign ult i16 %i, 32
  br i1 %small, label %sq, label %b8
sq:
  %m = mul nuw nsw i16 %i, %i
  br label %b21
b13:
  %i = phi i16 [ %n, %b8 ], [ 2, %b1 ]
  %s1 = phi i32 [ %s2, %b8 ], [ 0, %b1 ]
  %c1 = phi i16 [ %c2, %b8 ], [ 0, %b1 ]
  %p = getelementptr inbounds nuw i8, ptr %a, i16 %i
  %v = load i8, ptr %p
  %z8 = icmp eq i8 %v, 0
  br i1 %z8, label %b12, label %b8
b21:
  %j = phi i16 [ %j2, %b21 ], [ %m, %sq ]
  %q = getelementptr inbounds nuw i8, ptr %a, i16 %j
  store i8 1, ptr %q
  %j2 = add nuw nsw i16 %j, %i
  %go = icmp samesign ult i16 %j2, 1024
  br i1 %go, label %b21, label %e
e:
  br label %b8
}
";

fn six_registers() -> llrm_x86::tti::X86 {
    let real = RealMode {
        scale_costs: vec![(1, 0), (2, 2), (4, 2), (8, 2)],
        registers: 6,
        scaled_index_insns: 2,
        huge_space: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
    };
    X86::of_cpu("i486").unwrap().in_real_mode(real)
}

fn reduced_real(
    text: &str,
    tti: &llrm_x86::tti::X86,
) -> String {
    let mut module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, tti);
    printed(&module)
}

#[test]
fn a_quadratic_start_is_not_expanded_into_the_outer_loop_in_real_mode() {
    let after = reduced_real(SIEVE, &six_registers());
    assert!(!after.contains("[ 5, %b1 ]"), "{after}");
}

/// The flat target keeps the pin's choice.
#[test]
fn a_quadratic_start_is_expanded_into_the_outer_loop_on_a_flat_target() {
    let mut module = parsed(&format!("{}{}", DOS, SIEVE));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu("i486").unwrap();
    crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
    let after = printed(&module);
    assert!(after.contains("[ 5, %b1 ]"), "{after}");
}

/// The pin's post-increment-insertion.ll `test_01`: the address is of an
/// atomic load. LSR took an atomic access for no address use at all (it is
/// `Opcode::Atomic`, not `Load`), priced the use `Basic` and made a second,
/// pointer phi; `opt` keeps the one index phi and scales it into the address.
#[test]
fn an_atomic_load_is_an_address_use_as_in_opt() {
    let text = "define i32 @f(ptr %p, i64 %len, i32 %x) {\nentry:\n  br label %loop\nloop:\n  %iv = phi i64 [ %iv.next, %backedge ], [ %len, %entry ]\n  %iv.next = add nsw i64 %iv, -1\n  %cond_1 = icmp eq i64 %iv, 0\n  br i1 %cond_1, label %exit, label %backedge\nbackedge:\n  %addr = getelementptr inbounds i32, ptr %p, i64 %iv.next\n  %loaded = load atomic i32, ptr %addr unordered, align 4\n  %cond_2 = icmp eq i32 %loaded, %x\n  br i1 %cond_2, label %failure, label %loop\nexit:\n  ret i32 -1\nfailure:\n  unreachable\n}\n";
    let mut module = parsed(&format!("target datalayout = \"e-m:e-i64:64-f80:128-n8:16:32:64-S128\"\n{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu("x86-64").unwrap();
    let changed = crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
    let after = printed(&module);
    assert!(changed, "{after}");
    assert_eq!(after.matches(" = phi ").count(), 1, "{after}");
    assert!(after.contains("shl i64 %"), "{after}");
}
