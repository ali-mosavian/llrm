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
    let mut module = parsed(&format!("{DOS}{text}"));
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
