//! The pass over whole functions, run against the interpreter.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::Outer;
use llrm_x86::tti::X86;

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
    let mut module = parsed(&format!("{prefix}{text}"));
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let tti = X86::of_cpu(cpu).unwrap();
    let changed = crate::lsr_pass::reduced(context, &layout, function, &mut declared, &outer, &tti);
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
}
