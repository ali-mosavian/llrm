//! `processLoad` and `processNonLocalLoad` of `GVNPass` (`gvn::pass::loads`):
//! a load served by a store or a load before it, locally and across blocks,
//! with the value coerced where the sizes differ. Each case runs the
//! interpreter before and after.

use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use crate::gvn::pass::run_impl;
use crate::testing::{parsed, printed, results};

const XY: &[&[i128]] = &[&[0, 0], &[3, 5], &[-7, 2], &[0x7fff, 1]];
const XYC: &[&[i128]] = &[&[0, 0, 0], &[3, 5, 1], &[-7, 2, 0], &[0x7fff, 1, 1]];

/// `body`'s @f after the pass, with `@g` a global of 16 bytes; whether it
/// changed.
fn loaded(
    body: &str,
    inputs: &[&[i128]],
) -> (String, bool) {
    let text = format!("@g = global [8 x i16] zeroinitializer\n@h = global [8 x i16] zeroinitializer\n\n{body}");
    let before = parsed(&text);
    let mut module: Module = before.clone();
    let layout = llrm_analysis::testing::layout(&module);
    let outer = Outer::of(&module, None);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = run_impl(context, &layout, function, &outer);
    let printed = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{printed}");
    (printed, changed)
}

#[test]
fn test_a_load_after_a_store_of_the_same_bytes_is_the_stored_value() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  store i16 %x, ptr @g
  %r = load i16, ptr @g
  ret i16 %r
}
",
        XY,
    );
    assert!(changed && text.contains("ret i16 %x") && !text.contains("load"), "{text}");
}

#[test]
fn test_two_loads_of_the_same_bytes_are_one() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = load i16, ptr @g
  %b = load i16, ptr @g
  %r = add i16 %a, %b
  ret i16 %r
}
",
        XY,
    );
    assert!(changed && text.matches("load").count() == 1, "{text}");
}

#[test]
fn test_a_store_that_may_alias_keeps_the_second_load() {
    let (text, _) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %p = getelementptr i8, ptr @g, i16 %y
  %a = load i16, ptr @g
  store i16 %x, ptr %p
  %b = load i16, ptr @g
  %r = add i16 %a, %b
  ret i16 %r
}
",
        &[&[1, 2]],
    );
    assert_eq!(text.matches("load").count(), 2, "{text}");
}

#[test]
fn test_a_volatile_load_is_kept() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  store i16 %x, ptr @g
  %r = load volatile i16, ptr @g
  ret i16 %r
}
",
        XY,
    );
    assert!(!changed && text.contains("load volatile"), "{text}");
}

#[test]
fn test_a_load_nothing_uses_is_removed() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %unused = load i16, ptr @g
  ret i16 %x
}
",
        XY,
    );
    assert!(changed && !text.contains("load"), "{text}");
}

#[test]
fn test_a_load_below_a_join_takes_the_value_each_arm_stored() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 %x, ptr @g
  br label %b3

b2:
  store i16 %y, ptr @g
  br label %b3

b3:
  %r = load i16, ptr @g
  ret i16 %r
}
",
        XYC,
    );
    assert!(changed && !text.contains("load") && text.contains("phi i16 [ %y, %b2 ], [ %x, %b1 ]"), "{text}");
}

#[test]
fn test_a_load_partly_available_is_left_for_load_pre() {
    // One arm stores, the other does not: stage 5 makes it full (OWED).
    let (text, _) = loaded(
        "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 %x, ptr @g
  br label %b3

b2:
  br label %b3

b3:
  %r = load i16, ptr @g
  ret i16 %r
}
",
        XYC,
    );
    assert!(text.contains("%r = load i16, ptr @g"), "{text}");
}

#[test]
fn test_a_load_of_a_part_of_a_wider_store_is_cut_from_it() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %w = zext i16 %x to i32
  store i32 %w, ptr @g
  %r = load i16, ptr @g
  ret i16 %r
}
",
        XY,
    );
    assert!(changed && !text.contains("load"), "{text}");
}

#[test]
fn test_a_load_of_stack_memory_nothing_wrote_is_poison() {
    let (text, changed) = loaded(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %slot = alloca i16
  %r = load i16, ptr %slot
  ret i16 %x
}
",
        XY,
    );
    assert!(changed && !text.contains("load"), "{text}");
}

#[test]
fn test_a_load_after_a_memset_is_the_byte_repeated() {
    let (text, changed) = loaded(
        "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)

define i16 @f(i16 %x, i16 %y) {
b0:
  call void @llvm.memset.p0.i16(ptr @g, i8 7, i16 16, i1 false)
  %r = load i16, ptr @g
  ret i16 %r
}
",
        XY,
    );
    assert!(changed && !text.contains("load i16"), "{text}");
}
