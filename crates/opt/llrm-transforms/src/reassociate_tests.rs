//! Each body is a test of LLVM's `test/Transforms/Reassociate` at d1106deb71cc,
//! run with `opt -passes=reassociate`: the expected body has the structure of
//! the file's CHECK lines (new instructions are unnamed here, so the printer
//! numbers them).

use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use super::reassociate;
use crate::testing::{parsed, printed, results};

/// @f after Reassociate; whether anything changed.
fn reassociated(module: &mut Module) -> bool {
    let outer = Outer::of(module, None);
    let layout = llrm_analysis::testing::layout(module);
    let (context, function) = module.function_mut("f").expect("@f");
    reassociate(context, &layout, &outer, function)
}

/// `text` after Reassociate is `expected`.
fn check_text(
    text: &str,
    expected: &str,
) {
    let mut module = parsed(text);
    let changed = reassociated(&mut module);
    assert_eq!(printed(&module), expected);
    assert_eq!(changed, text != expected);
}

/// `check_text`, and the body computes what it did on `inputs`.
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let before = results(&parsed(text), inputs);
    check_text(text, expected);
    assert_eq!(results(&parsed(expected), inputs), before);
}

/// `basictest.ll` @test2: the constants of `(x + -30) + y + 30` meet and go.
#[test]
fn test_constants_of_an_add_chain_meet_and_cancel() {
    check(
        "define i32 @f(i32 %reg109, i32 %reg1111) {
b0:
  %reg115 = add i32 %reg109, -30
  %reg116 = add i32 %reg115, %reg1111
  %reg117 = add i32 %reg116, 30
  ret i32 %reg117
}
",
        "define i32 @f(i32 %reg109, i32 %reg1111) {
b0:
  %reg117 = add i32 %reg1111, %reg109
  ret i32 %reg117
}
",
        &[&[1, 2], &[-5, 7]],
    );
}

/// `absorption.ll` @or_all_ones: constants that combine to the absorbing value
/// are the value.
#[test]
fn test_constants_that_or_to_all_ones_are_the_result() {
    check(
        "define i8 @f(i8 %x) {
b0:
  %tmp1 = or i8 %x, 127
  %tmp2 = or i8 %tmp1, -128
  ret i8 %tmp2
}
",
        "define i8 @f(i8 %x) {
b0:
  ret i8 -1
}
",
        &[&[0], &[5]],
    );
}

/// `binop-identity.ll` @add_0 .. @xor_0: constants that combine to the
/// identity add nothing, so no instruction is made.
#[test]
fn test_constants_that_combine_to_the_identity_leave_the_operand() {
    for (op, c1, c2) in [("add", "-30", "30"), ("mul", "-1", "-1"), ("and", "-1", "-1"), ("or", "0", "0"), ("xor", "42", "42")] {
        check(
            &format!(
                "define i8 @f(i8 %x) {{
b0:
  %a1 = {op} i8 %x, {c1}
  %a2 = {op} i8 %a1, {c2}
  ret i8 %a2
}}
"
            ),
            "define i8 @f(i8 %x) {
b0:
  ret i8 %x
}
",
            &[&[0], &[7], &[-3]],
        );
    }
}

/// `binop-identity.ll` @fmul_1: `x * 4.0 * 0.25` is `x`.
#[test]
fn test_float_constants_that_multiply_to_one_leave_the_operand() {
    check_text(
        "define float @f(float %x) {
b0:
  %a1 = fmul fast float %x, 4.0
  %a2 = fmul fast float %a1, 0.25
  ret float %a2
}
",
        "define float @f(float %x) {
b0:
  ret float %x
}
",
    );
}

/// `commute.ll` @test1: `x*y` and `y*x` get one operand order.
#[test]
fn test_commuted_products_get_one_operand_order() {
    check_text(
        "declare void @use(i32)

define void @f(i32 %x, i32 %y) {
b0:
  %0 = mul i32 %x, %y
  %1 = mul i32 %y, %x
  %2 = sub i32 %0, %1
  call void @use(i32 %0)
  call void @use(i32 %2)
  ret void
}
",
        "declare void @use(i32)

define void @f(i32 %x, i32 %y) {
b0:
  %0 = mul i32 %y, %x
  %1 = mul i32 %y, %x
  %2 = sub i32 %0, %1
  call void @use(i32 %0)
  call void @use(i32 %2)
  ret void
}
",
    );
}

/// `add-like-or.ll` @shl_add: an `or` of shifted copies of one value has no
/// common bit, so it is an add.
#[test]
fn test_an_or_of_disjoint_shifts_is_an_add() {
    check(
        "define i32 @f(i8 %x) {
b0:
  %conv = zext i8 %x to i32
  %shl = shl nuw nsw i32 %conv, 8
  %shl2 = shl nuw nsw i32 %conv, 16
  %add = or i32 %shl, %shl2
  ret i32 %add
}
",
        "define i32 @f(i8 %x) {
b0:
  %conv = zext i8 %x to i32
  %shl = shl nuw nsw i32 %conv, 8
  %shl2 = shl nuw nsw i32 %conv, 16
  %add = add nuw nsw i32 %shl, %shl2
  ret i32 %add
}
",
        &[&[1], &[255]],
    );
}

/// `add-like-or.ll` @test1: an `or` whose operands may share a bit stays.
#[test]
fn test_an_or_that_may_share_bits_is_not_an_add() {
    check(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %c = or i32 %a, %b
  %c.plus.one = add i32 %c, 1
  ret i32 %c.plus.one
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %c = or i32 %b, %a
  %c.plus.one = add i32 %c, 1
  ret i32 %c.plus.one
}
",
        &[&[1, 2], &[3, 3]],
    );
}

/// `add-like-or.ll` @test4: a `disjoint` `or` reassociates as an add.
#[test]
fn test_a_disjoint_or_reassociates_as_an_add() {
    check(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %c = or disjoint i32 %a, %b
  %c.plus.one = add i32 %c, 1
  ret i32 %c.plus.one
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %c = add i32 %a, 1
  %c.plus.one = add i32 %c, %b
  ret i32 %c.plus.one
}
",
        &[&[1, 2], &[4, 8]],
    );
}

#[test]
fn test_mulfactor_test1() {
    check(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %t2 = mul i32 %a, %a
  %t5 = shl i32 %a, 1
  %t6 = mul i32 %t5, %b
  %t8 = mul i32 %b, %b
  %t7 = add i32 %t6, %t2
  %t11 = add i32 %t7, %t8
  ret i32 %t11
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %t2 = mul i32 %a, %a
  %t6 = mul i32 %a, 2
  %0 = add i32 %t6, %b
  %1 = mul i32 %0, %b
  %t11 = add i32 %1, %t2
  ret i32 %t11
}
",
        &[&[1, 2], &[3, -4], &[5, 5]],
    );
}

#[test]
fn test_mulfactor_test2() {
    check(
        "define i32 @f(i32 %t) {
b0:
  %a = mul i32 %t, 6
  %b = mul i32 %t, 36
  %c = add i32 %b, 15
  %d = add i32 %c, %a
  ret i32 %d
}
",
        "define i32 @f(i32 %t) {
b0:
  %0 = mul i32 %t, 42
  %d = add i32 %0, 15
  ret i32 %d
}
",
        &[&[1], &[2], &[-7]],
    );
}

#[test]
fn test_mulfactor_test3() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %a = mul i32 %x, %x
  %b = mul i32 %a, %x
  %c = mul i32 %b, %x
  %d = mul i32 %c, %x
  %e = mul i32 %d, %x
  %f = mul i32 %e, %x
  %g = mul i32 %f, %x
  ret i32 %g
}
",
        "define i32 @f(i32 %x) {
b0:
  %0 = mul i32 %x, %x
  %1 = mul i32 %0, %0
  %2 = mul i32 %1, %1
  ret i32 %2
}
",
        &[&[2], &[3]],
    );
}

#[test]
fn test_mulfactor_test4() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %a = mul i32 %x, %x
  %b = mul i32 %a, %x
  %c = mul i32 %b, %x
  %d = mul i32 %c, %x
  %e = mul i32 %d, %x
  %f = mul i32 %e, %x
  ret i32 %f
}
",
        "define i32 @f(i32 %x) {
b0:
  %0 = mul i32 %x, %x
  %1 = mul i32 %0, %x
  %2 = mul i32 %1, %x
  %f = mul i32 %2, %1
  ret i32 %f
}
",
        &[&[2], &[3]],
    );
}

#[test]
fn test_mulfactor_test5() {
    check(
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %a = mul i32 %x, %y
  %b = mul i32 %a, %y
  %c = mul i32 %b, %x
  %d = mul i32 %c, %x
  %e = mul i32 %d, %x
  ret i32 %e
}
",
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %0 = mul i32 %x, %x
  %1 = mul i32 %0, %y
  %2 = mul i32 %1, %1
  ret i32 %2
}
",
        &[&[2, 3], &[3, 2]],
    );
}

#[test]
fn test_mulfactor_test6() {
    check(
        "define i32 @f(i32 %x, i32 %y, i32 %z) {
b0:
  %a = mul i32 %x, %y
  %b = mul i32 %a, %x
  %c = mul i32 %b, %y
  %d = mul i32 %c, %x
  %e = mul i32 %d, %y
  %f = mul i32 %e, %x
  %g = mul i32 %f, %z
  %h = mul i32 %g, %x
  ret i32 %h
}
",
        "define i32 @f(i32 %x, i32 %y, i32 %z) {
b0:
  %0 = mul i32 %x, %x
  %1 = mul i32 %0, %y
  %f = mul i32 %y, %x
  %g = mul i32 %f, %1
  %2 = mul i32 %g, %1
  %h = mul i32 %2, %z
  ret i32 %h
}
",
        &[&[2, 3, 5]],
    );
}

#[test]
fn test_mulfactor_test7() {
    check(
        "define i32 @f(i32 %x, i32 %y, i32 %z) {
b0:
  %a = mul i32 %y, %x
  %b = mul i32 %a, %z
  %c = mul i32 %b, %z
  %d = mul i32 %c, %x
  %e = mul i32 %d, %y
  %f = mul i32 %e, %y
  %g = mul i32 %f, %x
  %h = mul i32 %g, %x
  ret i32 %h
}
",
        "define i32 @f(i32 %x, i32 %y, i32 %z) {
b0:
  %0 = mul i32 %x, %x
  %1 = mul i32 %0, %y
  %2 = mul i32 %1, %z
  %3 = mul i32 %2, %y
  %h = mul i32 %3, %2
  ret i32 %h
}
",
        &[&[2, 3, 5]],
    );
}

#[test]
fn test_multistep1() {
    check(
        "define i64 @f(i64 %a, i64 %b, i64 %c) {
b0:
  %t0 = mul i64 %a, %b
  %t1 = mul i64 %a, %t0
  %t2 = mul i64 %a, %c
  %t3 = mul i64 %a, %t2
  %t4 = add i64 %t1, %t3
  ret i64 %t4
}
",
        "define i64 @f(i64 %a, i64 %b, i64 %c) {
b0:
  %0 = add i64 %c, %b
  %1 = mul i64 %a, %a
  %2 = mul i64 %1, %0
  ret i64 %2
}
",
        &[&[2, 3, 5]],
    );
}

#[test]
fn test_multistep2() {
    check(
        "define i64 @f(i64 %a, i64 %b, i64 %c, i64 %d) {
b0:
  %t0 = mul i64 %a, %b
  %t1 = mul i64 %a, %c
  %t2 = add i64 %t1, %d
  %t3 = add i64 %t0, %t2
  ret i64 %t3
}
",
        "define i64 @f(i64 %a, i64 %b, i64 %c, i64 %d) {
b0:
  %0 = add i64 %c, %b
  %1 = mul i64 %0, %a
  %t3 = add i64 %1, %d
  ret i64 %t3
}
",
        &[&[2, 3, 5, 7]],
    );
}
