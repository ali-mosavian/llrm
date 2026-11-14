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

/// [`reduced_on`] under the flat cost model, where every cost is 1.
fn reduced_flat(
    text: &str,
    inputs: &[&[i128]],
) -> (bool, String) {
    let mut module = parsed(&format!("{FLAT}{text}"));
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed =
        crate::lsr_pass::reduced_with(context, &layout, function, &mut declared, &outer, &llrm_mir::tti::Basic, false);
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

const TWO_CONVERSIONS: &str = "define double @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %s = phi double [ 0.0, %entry ], [ %s3, %loop ]
  %d = sitofp i32 %i to double
  %e = uitofp nneg i32 %i to double
  %s2 = fadd double %s, %d
  %s3 = fmul double %s2, %e
  %next = add nuw nsw i32 %i, 1
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret double %s3
}
";

/// Two conversions of one counter got two floating counters off the x87
/// stack: the pin's `OptimizeShadowIV` makes one per loop and breaks. wasm
/// x_matmul_f -O2 grew 29 B (1584 -> 1613) once both conversions were `uitofp
/// nneg` and GVN had no pass left to merge them.
#[test]
fn a_loop_gets_one_floating_counter_for_two_conversions_off_the_x87_stack() {
    let (changed, after) = reduced_flat(TWO_CONVERSIONS, &[&[1], &[5], &[17]]);
    assert!(changed, "{after}");
    assert_eq!(after.matches("= phi double [ 0.000000e+00").count() - 1, 1, "{after}");
}

/// Where floats live on the x87 stack each conversion gets its own counter,
/// as before: a conversion is a store and a `fild` every trip. m16 x_ldconv
/// -O2 was 250 B and one `fild` fewer a trip with two (259 B with one).
#[test]
fn a_loop_gets_a_counter_per_conversion_where_floats_live_on_the_x87_stack() {
    let tuning = llrm_mir::target::Tuning { x87_stack_slots: Some(8), ..llrm_mir::target::Tuning::PIN };
    let mut module = parsed(&format!("{FLAT}{TWO_CONVERSIONS}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu("i486").unwrap().with_tuning(tuning);
    let changed = crate::lsr_pass::reduced_with(context, &layout, function, &mut declared, &outer, &tti, false);
    let after = printed(&module);
    assert!(changed, "{after}");
    assert_eq!(after.matches("= phi double [ 0.000000e+00").count() - 1, 2, "{after}");
}

/// `OptimizeShadowIV` on the outer loop of a nest: `(double) r` is taken in
/// the outer loop's own body and read by the inner loop. x_ld_div at -Os kept
/// `fiadd [r]` in every inner trip (19+ clocks on a 486) where the pin's loop
/// has `fadd st(3)` of a variable counting in floating point: 29,293 clocks
/// against clang's 26,528.
#[test]
fn an_outer_counter_converted_in_its_loop_gets_a_floating_counter() {
    let (changed, after) = reduced_on(
        FLAT,
        "define i32 @f(i32 %n) {
entry:
  br label %outer
outer:
  %r = phi i32 [ 0, %entry ], [ %rnext, %latch ]
  %t = phi double [ 0.0, %entry ], [ %t2, %latch ]
  %d = sitofp i32 %r to double
  br label %inner
inner:
  %i = phi i32 [ 0, %outer ], [ %inext, %inner ]
  %s = phi double [ %t, %outer ], [ %s2, %inner ]
  %s2 = fadd double %s, %d
  %inext = add nsw i32 %i, 1
  %ic = icmp eq i32 %inext, %n
  br i1 %ic, label %latch, label %inner
latch:
  %t2 = phi double [ %s2, %inner ]
  %rnext = add nuw nsw i32 %r, 1
  %rc = icmp eq i32 %rnext, 4
  br i1 %rc, label %exit, label %outer
exit:
  %q = fptosi double %t2 to i32
  ret i32 %q
}
",
        "i486",
        &[&[1], &[5], &[17]],
    );
    assert!(changed, "{after}");
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
        address_registers: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
        recursive_inlining: None,
        priced_self_inlining: None,
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
    // A byte offset stepped by 2 off the frame slot (`[bp+si+disp]`) or a
    // pointer stepped by 2: both are 16-bit addresses; the `shl` is not
    // formed.
    assert!(
        after.contains("getelementptr i8, ptr %0, i16 2") || after.contains("getelementptr i8, ptr %p, i16 %0"),
        "{after}"
    );
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
        address_registers: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
        recursive_inlining: None,
        priced_self_inlining: None,
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
        address_registers: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
        recursive_inlining: None,
        priced_self_inlining: None,
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

/// bench/huge bq16 -O1 (`h(c, r) = (r * 201& + c) MOD 251`): the inner loop
/// steps a huge pointer, the 32-bit index and, for the remainder's `trunc`,
/// a 16-bit copy of the index. `replaceCongruentIVs` keys the truncation of the
/// wide counter by the header's last phi, the pointer, so the copy stayed: a
/// fifth value live across the inner loop (an extra slot store and reload per
/// outer trip, 18 B). CIV1 takes the narrowest integer phi.
#[test]
fn a_narrow_copy_of_the_wide_index_is_its_truncation_beside_a_huge_pointer() {
    let text = format!(
        "{DOS}define i32 @f(ptr addrspace(3) %h) {{
entry:
  br label %outer
outer:
  %r = phi i16 [ 0, %entry ], [ %r1, %latch ]
  %m = mul nuw i16 %r, 201
  %base = zext i16 %m to i32
  br label %inner
inner:
  %c = phi i16 [ 0, %outer ], [ %c1, %inner ]
  %cz = zext nneg i16 %c to i32
  %x = add nuw nsw i32 %cz, %base
  %off = shl nuw nsw i32 %x, 1
  %p = getelementptr i8, ptr addrspace(3) %h, i32 %off
  %q = urem i32 %x, 251
  %qt = trunc nuw nsw i32 %q to i16
  store i16 %qt, ptr addrspace(3) %p
  %c1 = add nuw nsw i16 %c, 1
  %ec = icmp eq i16 %c1, 201
  br i1 %ec, label %latch, label %inner
latch:
  %r1 = add nuw nsw i16 %r, 1
  %er = icmp eq i16 %r1, 200
  br i1 %er, label %exit, label %outer
exit:
  ret i32 0
}}
"
    );
    let real = RealMode {
        scale_costs: vec![(1, 0), (2, 2), (4, 2), (8, 2)],
        registers: 6,
        scaled_index_insns: 2,
        huge_space: Some(3),
        address_registers: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
        recursive_inlining: None,
        priced_self_inlining: None,
    };
    let tti = X86::of_cpu("i486").unwrap().in_real_mode(real);
    let mut module = parsed(&text);
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
    let after = printed(&module);
    let inner = after.split("inner:").nth(1).and_then(|rest| rest.split("\nlatch:").next()).expect("inner");
    assert!(inner.contains(" = phi ptr addrspace(3)"), "the inner loop walks a huge pointer: {after}");
    assert_eq!(inner.matches(" = phi i16").count(), 1, "the exit counter only, no 16-bit copy of the index: {after}");
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
    let (_, kept) = crate::lsr_pass::reduced_held(
        context,
        &layout,
        function,
        &mut declared,
        &outer,
        &tti,
        false,
        false,
        false,
        None,
    );
    let kept = kept.expect("no edge was split");
    let mut nodes = |held| {
        let before = get(Event::ScevNodes);
        crate::lsr_pass::reduced_held(
            context,
            &layout,
            function,
            &mut declared,
            &outer,
            &tti,
            false,
            false,
            false,
            held,
        );
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

/// `s += (i * 97) % 211` stays a remainder where the pass expands it: the
/// pin's `SCEVExpander::visitAddExpr` recognizes the canonical form of an
/// unsimplified `urem` (`matchURem`) and emits `urem`. Ours expanded `x - (x /
/// c) * c`: a `div_u`, a `mul` and an `add` where one `rem_u` is (wasm recsum,
/// 42032 instructions where the pin's llc runs 30030).
#[test]
fn a_urem_the_pass_expands_is_a_urem() {
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
    assert!(after.contains("urem") && !after.contains("udiv"), "{after}");
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
        crate::lsr_pass::reduced_held(
            context,
            &layout,
            function,
            &mut declared,
            &outer,
            &tti,
            false,
            size,
            false,
            None,
        );
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
        address_registers: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
        recursive_inlining: None,
        priced_self_inlining: None,
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
/// the second look keeps the first's users. The module's outer loop is looked
/// at once, for its shadow variables and exit tests as the pin does: 2 + 1.
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
    assert_eq!(built, 3, "{}", printed(&module));
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
/// values a loop against 20 (branches1024: `mir lsr` 640 -> 320 Minstr). The
/// end-biased formulae (LG3) make two more for the exit counter's bias: 22.
#[test]
fn the_expressions_of_a_value_an_edit_touched_are_kept_through_lsr() {
    let loops = 16;
    assert!(lsr_scevs_made(loops) <= loops * 22, "{} values made", lsr_scevs_made(loops));
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
        address_registers: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
        recursive_inlining: None,
        priced_self_inlining: None,
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

/// A byte index stepping by 2 against a doubled limit kept a second counter
/// (`{n,+,-1}`, `dec cx`) beside the index, and a spill: examples/dot m16.
#[test]
fn a_byte_stepped_index_with_a_doubled_limit_keeps_one_recurrence() {
    let mut module = parsed(&format!("{DOS}{DOT}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::lsr_pass::LoopStrengthReduce { size: false, bytes_strict: false });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned())).unwrap();
    let after = printed(&module);
    assert_eq!(after.matches("= phi i16").count(), 1, "{after}");
}

/// An `i32` chain xored with a loaded value and an `i32` sum, beside two
/// strength-reduced `i32` recurrences, left a register short at -Os m16: the
/// solver spent a third register on a formula to save an instruction and the
/// allocator spilled the chain (shellsort bas -Os, +6 B, +129 memory operands
/// against main). The temporaries now count against the registers.
#[test]
fn the_temporaries_of_the_body_count_against_the_registers() {
    let mut module = parsed(&format!("{DOS}{SHELLSORT_INIT}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::lsr_pass::LoopStrengthReduce { size: true, bytes_strict: false });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned())).unwrap();
    let after = printed(&module);
    // At LU3c the live count is exact at m16, so the offset that ends at zero
    // takes the exit: bench/shellsort -Os keeps its bytes and memory operands
    // (927 B, 28261) and runs 64 fewer instructions.
    assert!(!after.contains("7013") && after.contains("icmp eq i16"), "the exit stayed on the i32 recurrence: {after}");
}

const SHELLSORT_INIT: &str = "
define i32 @f(ptr %v0, ptr %v5) {
b1:
  br label %b5
b5:
  %v10 = phi i16 [ 0, %b1 ], [ %v21, %b5 ]
  %v11 = shl nuw nsw i16 %v10, 2
  %v12 = getelementptr i8, ptr %v5, i16 %v11
  %v13 = zext nneg i16 %v10 to i32
  %v14 = mul nuw nsw i32 %v13, 109
  %v15 = add nuw nsw i32 %v14, 37
  %v16 = shl nuw nsw i32 %v13, 7
  %v17 = xor i32 %v15, %v16
  %v18 = load i32, ptr %v0
  %v19 = and i32 %v18, 65535
  %v20 = xor i32 %v17, %v19
  store i32 %v20, ptr %v12
  %v21 = add nuw nsw i16 %v10, 1
  %exitcond2 = icmp eq i16 %v21, 64
  br i1 %exitcond2, label %b6, label %b5
b6:
  ret i32 0
}
";

/// At size the rule is off: the doubled limit made `copy` -Os m16 3 B bigger
/// (`add ax,ax` and `cmp` for `dec`), idioms.c +51 B over the m16 corpus.
#[test]
fn at_size_a_word_copy_keeps_the_counter_and_does_not_double_the_limit() {
    let mut module = parsed(&format!("{DOS}{COPY}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::lsr_pass::LoopStrengthReduce { size: true, bytes_strict: false });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned())).unwrap();
    let after = printed(&module);
    assert!(!after.contains("shl i16"), "{after}");
}

const COPY: &str = "
@a = global [100 x i16] zeroinitializer
@b = global [100 x i16] zeroinitializer
define void @copy(i16 %0) {
  %2 = icmp sgt i16 %0, 0
  br i1 %2, label %3, label %11
3:
  br label %4
4:
  %5 = phi i16 [ %9, %4 ], [ 0, %3 ]
  %6 = getelementptr inbounds nuw [2 x i8], ptr @b, i16 %5
  %7 = load i16, ptr %6, align 2
  %8 = getelementptr inbounds nuw [2 x i8], ptr @a, i16 %5
  store i16 %7, ptr %8, align 2
  %9 = add nuw nsw i16 %5, 1
  %10 = icmp eq i16 %9, %0
  br i1 %10, label %11, label %4
11:
  ret void
}
";

const DOT: &str = "
define i32 @dot(ptr %0, ptr %1, i16 %2) {
  %4 = icmp sgt i16 %2, 0
  br i1 %4, label %5, label %19
5:
  br label %6
6:
  %7 = phi i16 [ %17, %6 ], [ 0, %5 ]
  %8 = phi i32 [ %16, %6 ], [ 0, %5 ]
  %9 = getelementptr inbounds nuw [2 x i8], ptr %0, i16 %7
  %10 = load i16, ptr %9, align 2
  %11 = sext i16 %10 to i32
  %12 = getelementptr inbounds nuw [2 x i8], ptr %1, i16 %7
  %13 = load i16, ptr %12, align 2
  %14 = sext i16 %13 to i32
  %15 = mul nsw i32 %14, %11
  %16 = add nsw i32 %15, %8
  %17 = add nuw nsw i16 %7, 1
  %exitcond = icmp eq i16 %17, %2
  br i1 %exitcond, label %18, label %6
18:
  br label %19
19:
  %20 = phi i32 [ 0, %3 ], [ %16, %18 ]
  ret i32 %20
}
";

/// The even-factor rule must agree with the wrapped product at every width
/// it is allowed to claim: a nonzero `x` within reach never multiplies to 0.
#[test]
fn the_even_factor_zero_rule_matches_the_wrapped_product_exhaustively() {
    for bits in [8u32, 16] {
        let modulus = 1i128 << bits;
        let span = modulus.min(512) / 2;
        for factor in (-span..span).chain([1024, 16384]).filter(|&f| f != 0) {
            let reach = (0..modulus / 2)
                .take_while(|&r| crate::lsr_generate::zero_test_survives(bits, factor as i64, r as u128))
                .count() as i128;
            for x in -reach.max(1) + 1..reach.max(1) {
                let zero = (x * factor).rem_euclid(modulus) == 0;
                let claimed = reach > 0 && x.abs() < reach;
                assert!(!claimed || zero == (x == 0), "bits {bits} factor {factor} x {x}");
            }
        }
    }
}

/// A counter that is both an address index and a value ended on a compare
/// against 257 next to a pointer that stepped by 2 (bench/histo's sum loop,
/// +256 instructions at m16): the first use's formulae hid the counter from
/// the rest. The byte index ending at zero is one fewer instruction.
#[test]
fn a_counter_serving_two_uses_lets_the_byte_index_end_at_zero() {
    let mut module = parsed(&format!("{DOS}{SUM_OF_COUNTS}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::lsr_pass::LoopStrengthReduce { size: false, bytes_strict: false });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned())).unwrap();
    let after = printed(&module);
    assert!(after.contains(", 0\n") && !after.contains("257"), "{after}");
}

const SUM_OF_COUNTS: &str = "
@counts = internal global [256 x i16] zeroinitializer
define i32 @f() {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %sum = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %p = getelementptr i16, ptr @counts, i16 %i
  %v = load i16, ptr %p
  %w = sext i16 %v to i32
  %k = add i16 %i, 1
  %kw = sext i16 %k to i32
  %m = mul i32 %w, %kw
  %s1 = add i32 %sum, %m
  %i1 = add nuw nsw i16 %i, 1
  %c = icmp eq i16 %i1, 256
  br i1 %c, label %exit, label %loop
exit:
  ret i32 %s1
}
";

/// The loop that reads a block (bench/stft_fixed: `_re[_rev[i]] = _window[i] *
/// _block[i] / 4`) took the fewest registers at -Os m16 once the loop's other
/// values were counted, and the allocator was 16 B larger for it (+11 B against
/// main). No solution fits beside those values, so the count told them apart
/// by registers alone; the pointer over `_rev` is the choice without it.
#[test]
fn a_cut_nothing_fits_keeps_the_pointer_over_the_table() {
    let mut module = parsed(&format!("{DOS}{BLOCK_LOOP}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::lsr_pass::LoopStrengthReduce { size: true, bytes_strict: false });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned())).unwrap();
    let after = printed(&module);
    assert!(after.contains("phi ptr"), "{after}");
}

/// bench/stft_fixed -Os m16: the same loop inside `main`, beside the outer
/// loop's values. The solution that fits the three address registers cost one
/// more instruction than the pointer over `_rev`, and the function came out
/// 18 B larger (7302 against 7284): at size a reload folds into a memory
/// operand while the extra instruction is paid in full.
#[test]
fn at_size_a_fitting_solution_does_not_cost_an_instruction_more() {
    let mut module = parsed(include_str!("lsr_stft_rev.ll"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::lsr_pass::LoopStrengthReduce { size: true, bytes_strict: false });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned())).unwrap();
    let after = printed(&module);
    assert!(after.contains("phi ptr [ %") && after.contains("@$var_rev, i16 128"), "{after}");
}

const BLOCK_LOOP: &str = "
@_window = internal global [256 x i32] zeroinitializer, align 2
@_rev = internal global [256 x i8] zeroinitializer, align 1
@_re = internal global [256 x i32] zeroinitializer, align 2
@_im = internal global [256 x i32] zeroinitializer, align 2
@_block = internal global [2 x [128 x i16]] zeroinitializer, align 2
define void @f(i16 %v60) {
entry:
  %v68 = getelementptr inbounds nuw [256 x i8], ptr @_block, i16 %v60
  %v62 = getelementptr inbounds nuw [256 x i8], ptr @_block, i16 1
  br label %v69
v69:
  %v70 = phi i16 [ 0, %entry ], [ %v96, %v69 ]
  %v71 = getelementptr inbounds nuw [2 x i8], ptr %v68, i16 %v70
  %v72 = load i16, ptr %v71, align 2
  %v73 = sdiv i16 %v72, 4
  %v74 = sext i16 %v73 to i32
  %v75 = getelementptr inbounds nuw [4 x i8], ptr @_window, i16 %v70
  %v76 = load i32, ptr %v75, align 2
  %v77 = mul nsw i32 %v76, %v74
  %v78 = getelementptr inbounds nuw i8, ptr @_rev, i16 %v70
  %v79 = load i8, ptr %v78, align 1
  %v80 = zext i8 %v79 to i16
  %v81 = getelementptr inbounds nuw [4 x i8], ptr @_re, i16 %v80
  store i32 %v77, ptr %v81, align 2
  %v82 = getelementptr inbounds nuw [2 x i8], ptr %v62, i16 %v70
  %v83 = load i16, ptr %v82, align 2
  %v84 = sdiv i16 %v83, 4
  %v85 = sext i16 %v84 to i32
  %v86 = or disjoint i16 %v70, 128
  %v87 = getelementptr inbounds nuw [4 x i8], ptr @_window, i16 %v86
  %v88 = load i32, ptr %v87, align 2
  %v89 = mul nsw i32 %v88, %v85
  %v90 = getelementptr inbounds nuw i8, ptr @_rev, i16 %v86
  %v91 = load i8, ptr %v90, align 1
  %v92 = zext i8 %v91 to i16
  %v93 = getelementptr inbounds nuw [4 x i8], ptr @_re, i16 %v92
  store i32 %v89, ptr %v93, align 2
  %v94 = getelementptr inbounds nuw [4 x i8], ptr @_im, i16 %v80
  store i32 0, ptr %v94, align 2
  %v95 = getelementptr inbounds nuw [4 x i8], ptr @_im, i16 %v92
  store i32 0, ptr %v95, align 2
  %v96 = add nuw nsw i16 %v70, 1
  %exitcond2 = icmp eq i16 %v96, 128
  br i1 %exitcond2, label %exit, label %v69
exit:
  ret void
}
";

/// fpbench nib: two arrays on the frame under three more live values made LSR
/// price the frame bases as registers and walk two pointers (`lea bx`,
/// `lea si`, `add bx, 4`) where main addressed `[bp+si-96]` and `[bp+si-72]`
/// off one index (+4 B at -O2).
#[test]
fn frame_arrays_share_one_index_under_pressure() {
    let mut module = parsed(&format!("{DOS}{FRAME_LOOP}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::lsr_pass::LoopStrengthReduce { size: false, bytes_strict: false });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned())).unwrap();
    let after = printed(&module);
    assert!(!after.contains("phi ptr"), "{after}");
}

const FRAME_LOOP: &str = "
declare void @bad()
define i32 @f() {
b1:
  %a = alloca [24 x i8]
  %b = alloca [24 x i8]
  br label %b73
b73:
  %v70 = phi i16 [ %v85, %b79 ], [ 0, %b1 ]
  %v71 = phi i32 [ %v93, %b79 ], [ 0, %b1 ]
  %v72 = getelementptr inbounds nuw [4 x i8], ptr %b, i16 %v70
  %v73 = load float, ptr %v72
  %v74 = fmul float %v73, 6.400000e+01
  %v75 = fcmp oge float %v74, 0xC1E0000000000000
  %v76 = fcmp olt float %v74, 0x41E0000000000000
  %v77 = and i1 %v75, %v76
  br i1 %v77, label %b75, label %b76
b75:
  %v78 = getelementptr inbounds nuw [4 x i8], ptr %a, i16 %v70
  %v79 = load float, ptr %v78
  %v80 = fmul float %v79, 6.400000e+01
  %v81 = fcmp oge float %v80, 0xC1E0000000000000
  %v82 = fcmp olt float %v80, 0x41E0000000000000
  %v83 = and i1 %v81, %v82
  br i1 %v83, label %b79, label %b80
b76:
  call void @bad()
  unreachable
b79:
  %v84 = fptosi float %v74 to i32
  %v85 = add nuw nsw i16 %v70, 1
  %v86 = zext nneg i16 %v85 to i32
  %v87 = mul i32 %v84, %v86
  %v88 = fptosi float %v80 to i32
  %v89 = add nuw nsw i16 %v70, 7
  %v90 = zext nneg i16 %v89 to i32
  %v91 = mul i32 %v88, %v90
  %v92 = add i32 %v87, %v71
  %v93 = add i32 %v92, %v91
  %exitcond = icmp eq i16 %v85, 6
  br i1 %exitcond, label %b72, label %b73
b72:
  ret i32 %v93
b80:
  call void @bad()
  unreachable
}
";

/// shellsort bq32 -Os: the m32 model budgeted the pin's 8 registers though ebp
/// and esp are never allocated, so the zero-test index off the array (4 regs,
/// one instruction fewer) looked affordable beside two other values and the
/// loop spilled (+13 B, +125 instructions). With the 6 the target allocates the
/// pointer and the exit compare on the value counter stay.
#[test]
fn the_registers_the_m32_target_allocates_bound_the_budget() {
    let text = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"
define void @f(ptr %p0, ptr %p3, i32 %k) {
b1:
  %h = load i32, ptr %p0
  br label %b5
b5:
  %v6 = phi i32 [ 0, %b1 ], [ %v16, %b5 ]
  %v7 = shl nuw nsw i32 %v6, 2
  %v8 = getelementptr i8, ptr %p3, i32 %v7
  %v9 = mul nuw nsw i32 %v6, 109
  %v10 = add nuw nsw i32 %v9, 37
  %v11 = shl nuw nsw i32 %v6, 7
  %v12 = xor i32 %v10, %v11
  %v13 = load i32, ptr %p0
  %v14 = and i32 %v13, 65535
  %v15 = xor i32 %v12, %v14
  store i32 %v15, ptr %v8
  %v16 = add nuw nsw i32 %v6, 1
  %c = icmp eq i32 %v16, 64
  br i1 %c, label %e, label %b5
e:
  ret void
}
";
    let mut module = parsed(text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::lsr_pass::LoopStrengthReduce { size: true, bytes_strict: false });
    manager.run_module(&mut module, crate::indvars::tests::m32()).unwrap();
    let after = printed(&module);
    assert!(after.contains("phi ptr") && after.contains(", 7013"), "{after}");
}

/// bench/ring -O1 (m32): `a[i] = 3 * i - 4` took an end-biased counter and a
/// base made by two geps (`ptr + i - 4096`, then `+ 4096`), 4 B over the pin's
/// pointer walk. Formulae of the first use hid the pointer's counter from the
/// store (the pin's `VisitedRegs` rule was off for every target).
#[test]
fn a_store_through_the_counter_keeps_the_pointer_walk() {
    let text = "@a = internal global [1024 x i32] zeroinitializer
define void @f() {
entry:
  br label %l
l:
  %i = phi i32 [ 0, %entry ], [ %n, %l ]
  %m = mul nuw nsw i32 %i, 3
  %v = add nsw i32 %m, -4
  %p = getelementptr inbounds nuw [4 x i8], ptr @a, i32 %i
  store i32 %v, ptr %p, align 4
  %n = add nuw nsw i32 %i, 1
  %c = icmp eq i32 %n, 1024
  br i1 %c, label %e, label %l
e:
  ret void
}
";
    let (changed, after) = reduced_on(FLAT, text, "i486", &[&[0]]);
    assert!(changed, "{after}");
    assert!(after.contains("phi ptr") && !after.contains("4096"), "{after}");
}

/// switch_table_holes (m32 -O2): `buf[t[i - 7]] += (i8)(i + 1)` made the byte
/// add one `lea` with the offset in the wide formula, so the i32 counter
/// plus 25 won and the store paid a zext, an add and a trunc. The pin keeps
/// the byte counter `{1,+,1}` and adds it as a byte.
#[test]
fn a_byte_sum_with_the_counter_keeps_a_byte_counter() {
    let text = "@buf = internal global [256 x i8] zeroinitializer
@t = internal constant [14 x i8] [i8 1, i8 2, i8 3, i8 4, i8 5, i8 6, i8 7, i8 8, i8 9, i8 10, i8 11, i8 12, i8 13, i8 14]

define i32 @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %latch ]
  %acc = phi i32 [ 0, %entry ], [ %acc2, %latch ]
  %next = add nuw nsw i32 %i, 1
  %k = add i32 %i, -7
  %in = icmp ult i32 %k, 14
  %k16 = trunc i32 %k to i16
  %bit = lshr i16 8707, %k16
  %hit = trunc i16 %bit to i1
  %go = select i1 %in, i1 %hit, i1 false
  br i1 %go, label %body, label %latch
body:
  %slot = getelementptr inbounds i8, ptr @t, i32 %k
  %idx8 = load i8, ptr %slot
  %idx = zext i8 %idx8 to i32
  %p = getelementptr inbounds i8, ptr @buf, i32 %idx
  %old = load i8, ptr %p
  %step = trunc nuw nsw i32 %next to i8
  %sum = add i8 %old, %step
  store i8 %sum, ptr %p
  br label %latch
latch:
  %one = phi i32 [ 1, %body ], [ 0, %loop ]
  %acc2 = add i32 %acc, %one
  %c = icmp eq i32 %next, 24
  br i1 %c, label %exit, label %loop
exit:
  ret i32 %acc2
}
";
    let (changed, after) = reduced_on(FLAT, text, "i486", &[&[0]]);
    assert!(changed, "{after}");
    assert!(after.contains("phi i8"), "{after}");
    assert!(!after.contains(" to i8"), "{after}");
}

fn three_address_registers() -> llrm_x86::tti::X86 {
    let real = RealMode {
        scale_costs: vec![(1, 0), (2, 2), (4, 2), (8, 2)],
        registers: 6,
        scaled_index_insns: 2,
        huge_space: None,
        address_registers: Some(3),
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
        recursive_inlining: None,
        priced_self_inlining: None,
    };
    X86::of_cpu("i486").unwrap().in_real_mode(real)
}

/// qb-metadata hir_part3 `sum_three` at m16: three far rows summed by one
/// counter. The cheapest formula by the general-register count scales the
/// counter and holds three bases, more address registers (bx, si, di) than the
/// target has: the loop reloaded two of them every trip (8 insns, 5 memory
/// operands against 6 and 3).
#[test]
fn three_rows_summed_by_one_counter_do_not_need_more_address_registers_than_the_target_has() {
    let text = "define i16 @f(ptr addrspace(1) %a, ptr addrspace(1) %b, ptr addrspace(1) %c, i16 %lo, i16 %end) {
entry:
  br label %loop
loop:
  %t = phi i16 [ 0, %entry ], [ %t3, %loop ]
  %i = phi i16 [ %lo, %entry ], [ %next, %loop ]
  %o = shl i16 %i, 1
  %pa = getelementptr i8, ptr addrspace(1) %a, i16 %o
  %va = load i16, ptr addrspace(1) %pa
  %t1 = add i16 %va, %t
  %pb = getelementptr i8, ptr addrspace(1) %b, i16 %o
  %vb = load i16, ptr addrspace(1) %pb
  %t2 = add i16 %t1, %vb
  %pc = getelementptr i8, ptr addrspace(1) %c, i16 %o
  %vc = load i16, ptr addrspace(1) %pc
  %t3 = add i16 %t2, %vc
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %end
  br i1 %done, label %exit, label %loop
exit:
  ret i16 %t3
}
";
    let after = reduced_real(text, &three_address_registers());
    let in_loop = &after[after.find("loop:").expect("the loop")..];
    assert!(!in_loop.contains("shl i16"), "{after}");
}

/// bench/queens at m16 -O2: the inner `safe` loop reads `q[r]` and compares
/// with `row - r` and `r - row`. The pin's formulae include a pointer walked
/// by two bytes; DEVIATE LG3's end-biased formulae brought a register of their
/// own, the solver's reuse rule then cut the pointer solution out of the
/// search, and the solution left cost an instruction more than the loop as it
/// stood, so LSR was dropped: 236848 clocks against 170823 at -Os, which
/// finds the pointer. LG3 now joins after the pin's narrowing and is kept
/// only where it costs less.
const QUEENS_INNER: &str = "declare i16 @g(ptr, i16, i16)

define i16 @f(ptr %q, i16 %row, i16 %n) {
b0:
  %e = icmp eq i16 %row, %n
  br i1 %e, label %ret, label %b5
b5:
  %pos = icmp sgt i16 %n, 0
  br i1 %pos, label %b7, label %ret
b7:
  %rp = icmp sgt i16 %row, 0
  %slot = getelementptr inbounds [2 x i8], ptr %q, i16 %row
  %row1 = add nsw i16 %row, 1
  br label %b11
b11:
  %found = phi i16 [ 0, %b7 ], [ %found2, %b32 ]
  %col = phi i16 [ 0, %b7 ], [ %col2, %b32 ]
  br i1 %rp, label %b14, label %b28
b14:
  br label %b17
b15:
  %r2 = add nuw nsw i16 %r, 1
  %exit = icmp eq i16 %r2, %row
  br i1 %exit, label %b27, label %b17
b17:
  %r = phi i16 [ %r2, %b15 ], [ 0, %b14 ]
  %p = getelementptr inbounds nuw [2 x i8], ptr %q, i16 %r
  %v = load i16, ptr %p, align 2
  %d = sub nsw i16 %v, %col
  %z = icmp eq i16 %v, %col
  %a = sub nuw nsw i16 %row, %r
  %c1 = icmp eq i16 %d, %a
  %o1 = select i1 %z, i1 true, i1 %c1
  %b = sub nsw i16 %r, %row
  %c2 = icmp eq i16 %d, %b
  %o2 = select i1 %o1, i1 true, i1 %c2
  br i1 %o2, label %b31, label %b15
b27:
  br label %b28
b28:
  store i16 %col, ptr %slot, align 2
  %x = call i16 @g(ptr %q, i16 %row1, i16 %n)
  %f1 = add nsw i16 %x, %found
  br label %b32
b31:
  br label %b32
b32:
  %found2 = phi i16 [ %f1, %b28 ], [ %found, %b31 ]
  %col2 = add nuw nsw i16 %col, 1
  %exit2 = icmp eq i16 %col2, %n
  br i1 %exit2, label %b35, label %b11
b35:
  br label %ret
ret:
  %res = phi i16 [ 1, %b0 ], [ 0, %b5 ], [ %found2, %b35 ]
  ret i16 %res
}
";

/// The pin's space is solved beside the end-biased one, and each build counted
/// the loop's register units again (21% of LSR on jpeg_fixed): once is enough.
#[test]
fn a_loop_solved_twice_has_its_register_units_counted_once() {
    let before = crate::lsr_collect::register_units_computed();
    let after = reduced_real(QUEENS_INNER, &six_registers());
    let counted = crate::lsr_collect::register_units_computed() - before;
    assert_eq!(counted, 1, "{after}");
}

/// The pin's space was generated again beside the end-biased one: every loop
/// the end-biased formulae touch generated twice (LSR calls x2.0 the pin's on
/// memory_expansion). It is taken from the first generation.
#[test]
fn a_loop_solved_twice_has_its_formulae_generated_once() {
    let before = crate::lsr_generate::reuse_formulae_generated();
    let after = reduced_real(QUEENS_INNER, &six_registers());
    let generated = crate::lsr_generate::reuse_formulae_generated() - before;
    assert_eq!(generated, 1, "{after}");
}

#[test]
fn a_loop_the_pin_makes_a_pointer_of_stays_one_when_end_biased_formulae_exist_in_real_mode() {
    let after = reduced_real(QUEENS_INNER, &six_registers());
    assert!(after.contains("phi ptr"), "{after}");
}

/// An unroll's remainder loop (`n & 3` trips at most three) stored through
/// `@g`: counting from `-4*trips` up to zero saves a compare a trip and costs
/// a shift, a negate and an add before the loop. In `recchop` that was +14 B
/// and +4% cycles against the pin's count-up with a compare.
#[test]
fn a_remainder_loop_of_few_trips_keeps_its_count_up_compare() {
    let (changed, after) = reduced_on(
        FLAT,
        "@g = global [64 x i32] zeroinitializer
define void @f(i32 %n, i32 %init) {
entry:
  %t = and i32 %n, 3
  %go = icmp ne i32 %t, 0
  br i1 %go, label %loop, label %exit
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %k = add nuw nsw i32 %init, %i
  %a = getelementptr i32, ptr @g, i32 %k
  %v = load i32, ptr %a
  store i32 %v, ptr %a
  %next = add nuw nsw i32 %i, 1
  %c = icmp eq i32 %next, %t
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
",
        "core2",
        &[&[1, 0], &[2, 3], &[3, 5]],
    );
    let _ = changed;
    assert!(!after.contains("-4"), "{after}");
}

/// A nested loop whose outer loop feeds the inner one `i * 300 - 30000`.
const OUTER_MULTIPLY: &str = "define i32 @f(i32 %n) {
entry:
  br label %outer
outer:
  %i = phi i32 [ 0, %entry ], [ %i1, %latch ]
  %acc = phi i32 [ 0, %entry ], [ %acc1, %latch ]
  %m = mul nuw nsw i32 %i, 300
  %z0 = add nsw i32 %m, -30000
  br label %inner
inner:
  %k = phi i32 [ 0, %outer ], [ %k1, %inner ]
  %z = phi i32 [ %z0, %outer ], [ %z1, %inner ]
  %t = xor i32 %z, %k
  %z1 = add i32 %t, 7
  %k1 = add nuw nsw i32 %k, 1
  %c = icmp eq i32 %k1, 4
  br i1 %c, label %latch, label %inner
latch:
  %acc1 = add i32 %acc, %z1
  %i1 = add nuw nsw i32 %i, 1
  %e = icmp eq i32 %i1, %n
  br i1 %e, label %exit, label %outer
exit:
  ret i32 %acc1
}
";

/// DEVIATE LSR-OUTER. x_cordic -O1 kept `imul ebx, ecx, 300` in its outer
/// loop (200 executions, 19 clocks each: 3,800 clocks, +7.5% over gcc) because
/// the pin's LSR skips a loop with a subloop. Where the target prices a
/// multiply in clocks the outer loop is reduced, the multiply gone; the pin's
/// target keeps it.
#[test]
fn an_outer_loops_multiply_is_a_counter_where_the_target_prices_multiplies() {
    let run = |tuning: llrm_mir::target::Tuning| {
        let mut module = parsed(&format!("{FLAT}{OUTER_MULTIPLY}"));
        let before = module.clone();
        let (layout, outer) = (layout(&module), Outer::of(&module, None));
        let mut declared = llrm_mir::passes::Declared::of(&module);
        let (context, function) = module.function_mut("f").expect("@f");
        let mut tti = X86::of_cpu("i486").unwrap();
        tti.tuning = tuning;
        crate::lsr_pass::reduced_with(context, &layout, function, &mut declared, &outer, &tti, false);
        let after = printed(&module);
        let inputs: &[&[i128]] = &[&[1], &[2], &[5]];
        assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
        after
    };
    // a multiply left in the preheader runs once; one in the loop, every trip
    let in_loop = |text: &str| text[text.find("outer:").expect("the outer loop")..].contains(" mul ");
    let priced =
        run(llrm_mir::target::Tuning { multiply_clocks: 19, loop_carried_clocks: 2, ..llrm_mir::target::Tuning::PIN });
    assert!(!in_loop(&priced), "{priced}");
    let pin = run(llrm_mir::target::Tuning::PIN);
    assert!(in_loop(&pin), "{pin}");
}

/// An outer loop whose two arms compute `i * 300 + 21472` and `i * 300 - 81472`
/// for one phi that feeds the inner loop.
const OUTER_PHI_OF_AFFINES: &str = "define i32 @f(i32 %n) {
entry:
  br label %outer
outer:
  %i = phi i32 [ 0, %entry ], [ %i1, %latch ]
  %acc = phi i32 [ 0, %entry ], [ %acc1, %latch ]
  %m = mul nuw nsw i32 %i, 300
  %c = icmp ult i32 %i, 100
  br i1 %c, label %a, label %b
a:
  %pa = add nuw nsw i32 %m, 21472
  br label %join
b:
  %pb = add nsw i32 %m, -81472
  br label %join
join:
  %z0 = phi i32 [ %pa, %a ], [ %pb, %b ]
  br label %inner
inner:
  %k = phi i32 [ 0, %join ], [ %k1, %inner ]
  %z = phi i32 [ %z0, %join ], [ %z1, %inner ]
  %t = xor i32 %z, %k
  %z1 = add i32 %t, 7
  %k1 = add nuw nsw i32 %k, 1
  %e4 = icmp eq i32 %k1, 4
  br i1 %e4, label %latch, label %inner
latch:
  %acc1 = add i32 %acc, %z1
  %i1 = add nuw nsw i32 %i, 1
  %e = icmp eq i32 %i1, %n
  br i1 %e, label %exit, label %outer
exit:
  ret i32 %acc1
}
";

/// DEVIATE LSR-PHI. x_cordic m32 -O3: the pin made a counter for one operand
/// of the phi that merges two affine values and left the other a 5-instruction
/// multiply chain, 100 trips. Where the target asks, both are counters.
#[test]
fn both_affine_operands_of_a_phi_are_counters_where_the_target_asks() {
    let run = |phi_operand_uses: bool| {
        let mut module = parsed(&format!("{FLAT}{OUTER_PHI_OF_AFFINES}"));
        let before = module.clone();
        let (layout, outer) = (layout(&module), Outer::of(&module, None));
        let mut declared = llrm_mir::passes::Declared::of(&module);
        let (context, function) = module.function_mut("f").expect("@f");
        let mut tti = X86::of_cpu("i486").unwrap();
        tti.tuning = llrm_mir::target::Tuning {
            multiply_clocks: 19,
            loop_carried_clocks: 2,
            lsr_phi_operand_uses: phi_operand_uses,
            ..llrm_mir::target::Tuning::PIN
        };
        crate::lsr_pass::reduced_with(context, &layout, function, &mut declared, &outer, &tti, false);
        let after = printed(&module);
        let inputs: &[&[i128]] = &[&[1], &[2], &[150]];
        assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
        after
    };
    let muls = |text: &str| text.matches(" mul ").count();
    let asked = run(true);
    assert_eq!(muls(&asked), 0, "{asked}");
    let pin = run(false);
    assert!(muls(&pin) > 0, "{pin}");
}

/// A remainder by a constant that feeds addresses, at -Os on a target that
/// keeps the divide.
const REMAINDER_INDEX: &str = "@V = global [400 x [16 x i32]] zeroinitializer

define i32 @f(i32 %n) {
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %in, %loop ]
  %acc = phi i32 [ 0, %entry ], [ %an, %loop ]
  %r = urem i32 %i, 400
  %p0 = getelementptr [16 x i32], ptr @V, i32 %r, i32 0
  %p1 = getelementptr [16 x i32], ptr @V, i32 %r, i32 1
  %p2 = getelementptr [16 x i32], ptr @V, i32 %r, i32 2
  %a = load i32, ptr %p0
  %b = load i32, ptr %p1
  %c = load i32, ptr %p2
  %s = add i32 %a, %b
  %t = add i32 %s, %c
  %an = add i32 %acc, %t
  %in = add i32 %i, 1
  %done = icmp ult i32 %in, %n
  br i1 %done, label %loop, label %exit

exit:
  ret i32 %an
}
";

fn reduced_for_size(
    text: &str,
    size: bool,
) -> String {
    let mut module = parsed(&format!("{FLAT}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, Some(std::rc::Rc::new(llrm_x86_m16::Dos::default()))));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu("i486").unwrap();
    crate::lsr_pass::reduced_held(context, &layout, function, &mut declared, &outer, &tti, false, size, size, None);
    printed(&module)
}

/// x_crt_i2d at m32 -Os: `V[i % 400].f` was strength-reduced through
/// `i - (i / 400) * 400` into three field pointers plus a multiply by
/// -25600, 152 B of loop where gcc's `idiv` and one scaled index took 99.
#[test]
fn test_a_kept_divides_remainder_is_not_split_into_the_addresses() {
    let text = reduced_for_size(REMAINDER_INDEX, true);
    assert!(text.contains("urem i32"), "{text}");
    assert!(!text.contains("udiv"), "{text}");
}

const NEXT_CONVERTED: &str = "define double @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %s = phi double [ 0.0, %entry ], [ %s3, %loop ]
  %d = sitofp i32 %i to double
  %next = add nuw nsw i32 %i, 1
  %e = sitofp i32 %next to double
  %q = fdiv double 1.0, %e
  %s2 = fadd double %s, %d
  %s3 = fadd double %s2, %q
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret double %s3
}
";

fn reduced_next_converted(x87: bool) -> String {
    let tuning = llrm_mir::target::Tuning { x87_stack_slots: x87.then_some(8), ..llrm_mir::target::Tuning::PIN };
    let mut module = parsed(&format!("{FLAT}{NEXT_CONVERTED}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu("i486").unwrap().with_tuning(tuning);
    crate::lsr_pass::reduced_with(context, &layout, function, &mut declared, &outer, &tti, false);
    printed(&module)
}

/// `1.0 / (double)(i + 1)` beside `(double)i`: x_ldmath at O2 kept an `fadd`
/// counter and also converted `i + 1` (an `fidiv` of the integer) every trip,
/// +400 clk against gcc's one `fild` shared by both.
#[test]
fn test_a_conversion_of_the_increment_is_the_double_counters_increment() {
    let after = reduced_next_converted(true);
    assert!(!after.contains("sitofp"), "{after}");
}

/// Off the x87 stack the conversion stays: 6502 x_ldmath grew 32 B with the
/// counter's next value kept live across the soft-float calls.
#[test]
fn test_a_conversion_of_the_increment_stays_off_the_x87_stack() {
    let after = reduced_next_converted(false);
    assert!(after.contains("sitofp"), "{after}");
}

/// The trip counts the session works out for the headers `h0..h{loops}`.
fn session_counts(
    session: &mut llrm_analysis::scev::analysis::ScevSession,
    module: &mut llrm_mir::module::Module,
    loops: usize,
) -> usize {
    let (data, outer) = (layout(module), Outer::of(module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let (context, function) = (&*context, &*function);
    let before = llrm_analysis::scev::trip::backedge_taken_computations();
    let (se, env) = session.split(&data, &outer.globals, &outer.metadata);
    let cx = env.cx(context, function);
    for k in 0..loops {
        let header = llrm_analysis::testing::block(function, &format!("h{k}"));
        se.get_backedge_taken_count(&cx, header, llrm_analysis::scev::backedge::ExitCountKind::Exact);
    }
    llrm_analysis::scev::trip::backedge_taken_computations() - before
}

/// An edge LSR splits dropped every expression of the function (the session's
/// `forget_since` drops all where the CFG changed), so each loop after it
/// worked out the trip counts of the loops before it again: 27 G of a 400-loop
/// function's LSR. The pin edits the edge and keeps its one `ScalarEvolution`:
/// only the loop whose blocks were touched is counted again.
#[test]
fn an_edge_split_makes_lsr_count_only_the_loop_it_touched_again() {
    let mut module = parsed(&format!("{FLAT}{}", walks(3)));
    let outer = Outer::of(&module, None);
    let mut session = {
        let (context, function) = module.function_mut("f").expect("@f");
        llrm_analysis::scev::analysis::ScevSession::within(&*context, &outer.globals, &outer.metadata, &*function)
    };
    assert_eq!(session_counts(&mut session, &mut module, 3), 3, "premise: three loops, three counts");
    let mark = module.function_mut("f").unwrap().1.mark();
    {
        let function = module.function_mut("f").expect("@f").1;
        let (from, to) = (llrm_analysis::testing::block(function, "h0"), llrm_analysis::testing::block(function, "e1"));
        crate::edges::split_edge(function, from, to);
    }
    let (data, outer) = (layout(&module), Outer::of(&module, None));
    let kept = session.into_scev();
    let (context, function) = module.function_mut("f").expect("@f");
    let (context, function) = (&*context, &*function);
    let mut session =
        llrm_analysis::scev::analysis::ScevSession::within(context, &outer.globals, &outer.metadata, function);
    session.adopt(kept);
    session.forget_since_through_edges(context, &data, &outer.globals, &outer.metadata, function, mark);
    assert_eq!(session_counts(&mut session, &mut module, 3), 1, "only the loop whose edge was split is counted again");
}

/// x_sf_add -O3 m32 (`V[i % 100]` read twice, nine and more other values live in the body): the
/// pin's tie between the loop as written and its rewrite took the rewrite, which holds a pointer
/// walk per field and a counter down to zero beside `i`, across a body that already fills the
/// registers: 41,235 clocks against 37,986 with the loop left alone.
#[test]
fn a_rewrite_no_cheaper_than_the_loop_is_not_made_where_the_registers_are_full() {
    let text = "@V = internal global [1100 x i32] zeroinitializer\n\ndefine i32 @f(i32 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [ 0, %entry ], [ %next, %loop ]\n  %acc = phi i32 [ 0, %entry ], [ %s9, %loop ]\n  %r = urem i32 %i, 100\n  %p0 = getelementptr [11 x i32], ptr @V, i32 %r, i32 0\n  %a = load i32, ptr %p0\n  %p1 = getelementptr [11 x i32], ptr @V, i32 %r, i32 1\n  %b = load i32, ptr %p1\n  %m0 = mul i32 %a, 3\n  %x0 = xor i32 %m0, %b\n  %m1 = mul i32 %a, 4\n  %x1 = xor i32 %m1, %b\n  %m2 = mul i32 %a, 5\n  %x2 = xor i32 %m2, %b\n  %m3 = mul i32 %a, 6\n  %x3 = xor i32 %m3, %b\n  %m4 = mul i32 %a, 7\n  %x4 = xor i32 %m4, %b\n  %m5 = mul i32 %a, 8\n  %x5 = xor i32 %m5, %b\n  %m6 = mul i32 %a, 9\n  %x6 = xor i32 %m6, %b\n  %m7 = mul i32 %a, 10\n  %x7 = xor i32 %m7, %b\n  %m8 = mul i32 %a, 11\n  %x8 = xor i32 %m8, %b\n  %m9 = mul i32 %a, 12\n  %x9 = xor i32 %m9, %b\n  %s0 = add i32 %acc, %x0\n  %s1 = add i32 %s0, %x1\n  %s2 = add i32 %s1, %x2\n  %s3 = add i32 %s2, %x3\n  %s4 = add i32 %s3, %x4\n  %s5 = add i32 %s4, %x5\n  %s6 = add i32 %s5, %x6\n  %s7 = add i32 %s6, %x7\n  %s8 = add i32 %s7, %x8\n  %s9 = add i32 %s8, %x9\n  %next = add nuw nsw i32 %i, 1\n  %c = icmp eq i32 %next, %n\n  br i1 %c, label %exit, label %loop\nexit:\n  ret i32 %s9\n}\n";
    let (_, after) = reduced_on(FLAT, text, "i486", &[&[8]]);
    assert_eq!(after.matches("phi ").count(), 2, "the counter and the sum stay the loop's only recurrences: {after}");
}
