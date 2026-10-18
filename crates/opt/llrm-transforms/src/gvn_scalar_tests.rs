//! The scalar part of `GVNPass` (`gvn::pass`, `gvn::table`): value numbering of
//! arithmetic, comparisons, GEPs and calls that touch no memory, equality
//! propagated from branches, switches and assumes, and dead blocks. Each case
//! runs the interpreter before and after.

use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use crate::gvn::pass::run_impl;
use crate::testing::{parsed, printed, results};

const XY: &[&[i128]] = &[&[0, 0], &[3, 5], &[-7, 2], &[0x7fff, 1]];

/// `text`'s @f after the scalar pass: printed, and whether it changed. The pass
/// again changes nothing; what @f returns for `inputs` stays.
fn numbered(
    text: &str,
    inputs: &[&[i128]],
) -> (String, bool) {
    let before = parsed(text);
    let mut module: Module = before.clone();
    let layout = llrm_analysis::testing::layout(&module);
    let outer = Outer::of(&module, None);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = run_impl(context, &layout, function, &outer);
    let printed = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{printed}");
    let (context, function) = module.function_mut("f").expect("@f");
    assert!(!run_impl(context, &layout, function, &outer), "a second run changes nothing:\n{printed}");
    (printed, changed)
}

fn kept(text: &str) {
    let (printed_text, changed) = numbered(text, XY);
    assert!(!changed, "{printed_text}");
    assert_eq!(printed_text, printed(&parsed(text)));
}

#[test]
fn test_a_repeated_expression_is_computed_once() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = mul i16 %x, %y
  %b = mul i16 %x, %y
  %r = sub i16 %a, %b
  ret i16 %r
}
",
        XY,
    );
    assert!(changed);
    // The difference of a value and itself is simplified on the way.
    assert!(text.contains("ret i16 0"), "{text}");
}

#[test]
fn test_operands_in_either_order_are_one_commutative_expression() {
    for op in ["add", "xor", "mul", "and", "or"] {
        let (text, changed) = numbered(
            &format!(
                "define i32 @f(i16 %x, i16 %y) {{
b0:
  %a = {op} i16 %x, %y
  %b = {op} i16 %y, %x
  %s = zext i16 %a to i32
  %t = zext i16 %b to i32
  %r = sub i32 %s, %t
  ret i32 %r
}}
"
            ),
            XY,
        );
        assert!(changed, "{op}");
        assert!(text.contains("ret i32 0"), "{op}: {text}");
    }
}

/// `icmp slt x, y` and `icmp sgt y, x` are one comparison: LLVM sorts the
/// operand numbers and swaps the predicate with them.
#[test]
fn test_a_compare_with_its_operands_and_predicate_swapped_is_one() {
    let (text, changed) = numbered(
        "define i32 @f(i16 %x, i16 %y) {
b0:
  %a = icmp slt i16 %x, %y
  %b = icmp sgt i16 %y, %x
  %s = zext i1 %a to i32
  %t = zext i1 %b to i32
  %r = sub i32 %s, %t
  ret i32 %r
}
",
        XY,
    );
    assert!(changed);
    assert!(text.contains("ret i32 0"), "{text}");
}

#[test]
fn test_an_ordered_expression_with_its_operands_swapped_stays() {
    for op in ["sub", "shl", "udiv"] {
        kept(&format!(
            "define i32 @f(i16 %x, i16 %y) {{
b0:
  %a = {op} i16 %x, %y
  %b = {op} i16 %y, %x
  %s = zext i16 %a to i32
  %t = zext i16 %b to i32
  %r = add i32 %s, %t
  ret i32 %r
}}
"
        ));
    }
}

/// The key has no IR flags: two adds that differ in `nsw` are one, and the
/// survivor keeps only the flags both had (`patchReplacementInstruction`). The
/// old table kept them apart.
#[test]
fn test_expressions_differing_in_flags_are_one_with_the_flags_both_have() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x) {
b0:
  %a = add nsw i16 %x, 1
  %b = add i16 %x, 1
  %r = xor i16 %a, %b
  ret i16 %r
}
",
        &[&[0], &[5], &[0x7fff]],
    );
    assert!(changed);
    assert!(text.contains("ret i16 0") && !text.contains("nsw"), "{text}");
}

#[test]
fn test_expressions_differing_in_an_operand_or_type_stay() {
    for (first, second) in [
        ("%a = add i16 %x, 1", "%b = add i16 %x, 2"),
        ("%a = sext i8 %n to i16", "%b = zext i8 %n to i16"),
    ] {
        kept(&format!(
            "define i16 @f(i16 %x, i8 %n) {{
b0:
  {first}
  {second}
  %r = add i16 %a, %b
  ret i16 %r
}}
"
        ));
    }
    kept(
        "define i32 @f(i8 %n) {
b0:
  %a = zext i8 %n to i16
  %b = zext i8 %n to i32
  %w = zext i16 %a to i32
  %r = add i32 %w, %b
  ret i32 %r
}
",
    );
}

/// Addresses that differ in their type encoding but add the same bytes are one
/// (`createGEPExpr`).
#[test]
fn test_geps_adding_the_same_bytes_are_one() {
    let (text, changed) = numbered(
        "define i16 @f(ptr %p) {
b0:
  %a = getelementptr i16, ptr %p, i16 2
  %b = getelementptr i8, ptr %p, i16 4
  %x = ptrtoint ptr %a to i16
  %y = ptrtoint ptr %b to i16
  %r = sub i16 %x, %y
  ret i16 %r
}
",
        &[],
    );
    assert!(changed);
    assert!(text.contains("ret i16 0"), "{text}");
}

#[test]
fn test_geps_adding_different_bytes_stay() {
    kept(
        "define i16 @f(ptr %p) {
b0:
  %a = getelementptr i16, ptr %p, i16 2
  %b = getelementptr i8, ptr %p, i16 5
  %x = ptrtoint ptr %a to i16
  %y = ptrtoint ptr %b to i16
  %r = sub i16 %x, %y
  ret i16 %r
}
",
    );
}

#[test]
fn test_a_dominated_repeat_reads_the_dominating_value() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = and i16 %x, %y
  %c = icmp eq i16 %x, 0
  br i1 %c, label %b1, label %b2

b1:
  %b = and i16 %y, %x
  ret i16 %b

b2:
  ret i16 %a
}
",
        XY,
    );
    assert!(changed);
    assert!(text.contains("b1:\n  ret i16 0\n") || text.contains("b1:\n  ret i16 %a\n"), "{text}");
}

/// Neither arm runs before the other.
#[test]
fn test_a_repeat_in_a_sibling_block_is_kept() {
    kept(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %c = icmp eq i16 %x, 0
  br i1 %c, label %b1, label %b2

b1:
  %a = or i16 %x, %y
  ret i16 %a

b2:
  %b = or i16 %x, %y
  ret i16 %b
}
",
    );
}

#[test]
fn test_a_branch_condition_is_known_in_each_arm() {
    let (text, changed) = numbered(
        "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %t = zext i1 %c to i16
  ret i16 %t

b2:
  %u = zext i1 %c to i16
  ret i16 %u
}
",
        &[&[0, 0], &[1, 0]],
    );
    assert!(changed);
    assert!(text.contains("zext i1 true") || text.contains("ret i16 1"), "{text}");
}

/// `x == 5` taken: `x` is 5 in the arm.
#[test]
fn test_an_equality_taken_replaces_the_variable_by_the_constant() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 5
  br i1 %c, label %b1, label %b2

b1:
  %y = add i16 %x, %x
  ret i16 %y

b2:
  ret i16 0
}
",
        &[&[5], &[6]],
    );
    assert!(changed);
    assert!(text.contains("ret i16 10"), "{text}");
}

/// The case value is the switch value in the case's block, when one edge
/// reaches it.
#[test]
fn test_a_switch_case_value_replaces_the_variable_in_its_block() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 3, label %c3
    i16 4, label %c4
  ]

c3:
  %a = add i16 %x, 1
  ret i16 %a

c4:
  %b = add i16 %x, 1
  ret i16 %b

d:
  ret i16 0
}
",
        &[&[3], &[4], &[9]],
    );
    assert!(changed);
    assert!(text.contains("ret i16 4") && text.contains("ret i16 5"), "{text}");
}

#[test]
fn test_an_assume_makes_its_condition_true_below_it() {
    let (text, changed) = numbered(
        "declare void @llvm.assume(i1)

define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 7
  call void @llvm.assume(i1 %c)
  %y = add i16 %x, 1
  ret i16 %y
}
",
        &[&[7]],
    );
    assert!(changed);
    assert!(text.contains("ret i16 8"), "{text}");
}

/// A branch on a constant makes its other arm dead; the phi it feeds takes
/// poison from it.
#[test]
fn test_a_branch_on_a_constant_poisons_the_dead_arms_phi_input() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x) {
b0:
  br i1 true, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ 1, %b1 ], [ %x, %b2 ]
  ret i16 %p
}
",
        &[&[3]],
    );
    assert!(changed);
    assert!(text.contains("poison") || text.contains("ret i16 1"), "{text}");
}

#[test]
fn test_two_identical_phis_are_one() {
    let (text, changed) = numbered(
        "define i16 @f(i1 %c, i16 %x, i16 %y) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  %q = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  %r = sub i16 %p, %q
  ret i16 %r
}
",
        &[&[0, 1, 2], &[1, 1, 2]],
    );
    assert!(changed);
    assert!(text.contains("ret i16 0"), "{text}");
}

#[test]
fn test_two_calls_that_touch_no_memory_are_one() {
    let (text, changed) = numbered(
        "declare i16 @pure(i16) readnone

define i16 @f(i16 %x) {
b0:
  %a = call i16 @pure(i16 %x)
  %b = call i16 @pure(i16 %x)
  %r = sub i16 %a, %b
  ret i16 %r
}
",
        &[],
    );
    assert!(changed);
    assert!(text.contains("ret i16 0"), "{text}");
}

#[test]
fn test_two_calls_that_read_memory_stay() {
    kept(
        "declare i16 @peek(i16) readonly

define i16 @f(i16 %x) {
b0:
  %a = call i16 @peek(i16 %x)
  %b = call i16 @peek(i16 %x)
  %r = sub i16 %a, %b
  ret i16 %r
}
",
    );
}

/// The wrapped result of `add.with.overflow` is the add.
#[test]
fn test_the_sum_of_an_overflow_intrinsic_is_the_add() {
    let (text, changed) = numbered(
        "declare { i16, i1 } @llvm.sadd.with.overflow.i16(i16, i16)

define i16 @f(i16 %x, i16 %y) {
b0:
  %o = call { i16, i1 } @llvm.sadd.with.overflow.i16(i16 %x, i16 %y)
  %s = extractvalue { i16, i1 } %o, 0
  %a = add i16 %x, %y
  %r = sub i16 %s, %a
  ret i16 %r
}
",
        XY,
    );
    assert!(changed);
    assert!(text.contains("ret i16 0"), "{text}");
}
