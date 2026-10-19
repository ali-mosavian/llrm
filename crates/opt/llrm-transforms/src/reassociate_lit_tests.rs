//! Generated from the CHECK lines of LLVM's `test/Transforms/Reassociate` at
//! d1106deb71cc, `opt -passes=reassociate`: the expected body is the file's,
//! compared up to the names of values and blocks.

use super::tests::{check_lit, check_lit_dce};

/// `2002-05-15-AgressiveSubMove.ll` @test1.
#[test]
fn test_2002_05_15_agressivesubmove_test1() {
    check_lit(
        "define i32 @f(i32 %A) {
b0:
  %X = add i32 %A, 1
  %Y = add i32 %A, 1
  %r = sub i32 %X, %Y
  ret i32 %r
}
",
        "define i32 @f(i32 %A) {
b0:
ret i32 0
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `2019-08-22-FNegAssert.ll` @test1.
#[test]
fn test_2019_08_22_fnegassert_test1() {
    check_lit(
        "define float @f(float %arg, float %arg1, float %arg2, float %arg3) {
b0:
  %tmp1 = fsub fast float %arg3, %arg2
  %tmp2 = fadd fast float %arg, -5.000000e+02
  %tmp3 = fsub fast float %tmp1, %tmp2
  %tmp4 = fmul fast float %tmp3, %tmp3
  %tmp5 = fneg fast float %arg
  %tmp6 = fdiv fast float %tmp5, %arg1
  %tmp7 = fadd fast float %tmp6, %tmp4
  ret float %tmp7
}
",
        "define float @f(float %arg, float %arg1, float %arg2, float %arg3) {
b0:
%tmp5 = fneg fast float %arg
%tmp1 = fsub fast float %arg3, %arg2
%tmp2_neg = fadd fast float %tmp5, 5.000000e+02
%tmp3 = fadd fast float %tmp2_neg, %tmp1
%tmp4 = fmul fast float %tmp3, %tmp3
%tmp6 = fdiv fast float %tmp5, %arg1
%tmp7 = fadd fast float %tmp4, %tmp6
ret float %tmp7
}
",
        &[],
    );
}

/// `absorption.ll` @or_all_ones.
#[test]
fn test_absorption_or_all_ones() {
    check_lit(
        "define i8 @f(i8 %x) {
b0:
  %tmp1 = or i8 %x, 127
  %tmp2 = or i8 %tmp1, 128
  ret i8 %tmp2
}
",
        "define i8 @f(i8 %x) {
b0:
ret i8 -1
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `absorption.ll` @fmul_zero.
#[test]
fn test_absorption_fmul_zero() {
    check_lit(
        "define double @f(double %x) {
b0:
  %x4 = fmul fast double %x, 4.0
  %r = fmul fast double %x4, 0.0
  ret double %r
}
",
        "define double @f(double %x) {
b0:
%r = fmul fast double %x, 0.000000e+00
ret double %r
}
",
        &[],
    );
}

/// `add-like-or.ll` @shl_add.
#[test]
fn test_add_like_or_shl_add() {
    check_lit(
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
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `add-like-or.ll` @test1. If we don't know that operands have no common bits
/// set, we can't convert the `or` into an `add`.
#[test]
fn test_add_like_or_test1() {
    check_lit(
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
%c_plus_one = add i32 %c, 1
ret i32 %c_plus_one
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `add-like-or.ll` @test4. Test that disjoint allow reassociation.
#[test]
fn test_add_like_or_test4() {
    check_lit(
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
%c_plus_one = add i32 %c, %b
ret i32 %c_plus_one
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `binop-identity.ll` @add_0.
#[test]
fn test_binop_identity_add_0() {
    check_lit(
        "define i32 @f(i32 %x) {
b0:
  %a1 = add i32 %x, -30
  %a2 = add i32 %a1, 30
  ret i32 %a2
}
",
        "define i32 @f(i32 %x) {
b0:
ret i32 %x
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `binop-identity.ll` @mul_1.
#[test]
fn test_binop_identity_mul_1() {
    check_lit(
        "define i32 @f(i32 %x) {
b0:
  %a1 = mul i32 %x, -1
  %a2 = mul i32 %a1, -1
  ret i32 %a2
}
",
        "define i32 @f(i32 %x) {
b0:
ret i32 %x
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `binop-identity.ll` @and_neg1.
#[test]
fn test_binop_identity_and_neg1() {
    check_lit(
        "define i8 @f(i8 %x) {
b0:
  %a1 = and i8 %x, 255
  %a2 = and i8 %a1, 255
  ret i8 %a2
}
",
        "define i8 @f(i8 %x) {
b0:
ret i8 %x
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `binop-identity.ll` @or_0.
#[test]
fn test_binop_identity_or_0() {
    check_lit(
        "define i8 @f(i8 %x) {
b0:
  %a1 = or i8 %x, 0
  %a2 = or i8 %a1, 0
  ret i8 %a2
}
",
        "define i8 @f(i8 %x) {
b0:
ret i8 %x
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `binop-identity.ll` @xor_0.
#[test]
fn test_binop_identity_xor_0() {
    check_lit(
        "define i8 @f(i8 %x) {
b0:
  %a1 = xor i8 %x, 42
  %a2 = xor i8 %a1, 42
  ret i8 %a2
}
",
        "define i8 @f(i8 %x) {
b0:
ret i8 %x
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `binop-identity.ll` @fadd_0.
#[test]
fn test_binop_identity_fadd_0() {
    check_lit(
        "define float @f(float %x) {
b0:
  %a1 = fadd fast float %x, -30.0
  %a2 = fadd fast float %a1, 30.0
  ret float %a2
}
",
        "define float @f(float %x) {
b0:
%a2 = fadd fast float %x, 0.000000e+00
ret float %a2
}
",
        &[],
    );
}

/// `binop-identity.ll` @fmul_1.
#[test]
fn test_binop_identity_fmul_1() {
    check_lit(
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
        &[],
    );
}

/// `commute.ll` @test1.
#[test]
fn test_commute_test1() {
    check_lit(
        "declare void @use(i32)

define void @f(i32 %x, i32 %y) {
b0:

  %n1 = mul i32 %x, %y
  %n2 = mul i32 %y, %x
  %n3 = sub i32 %n1, %n2
  call void @use(i32 %n1)
  call void @use(i32 %n3)
  ret void
}
",
        "define void @f(i32 %x, i32 %y) {
b0:
%n1 = mul i32 %y, %x
%n2 = mul i32 %y, %x
%n3 = sub i32 %n1, %n2
call void @use(i32 %n1)
call void @use(i32 %n3)
ret void
}
",
        &[],
    );
}

/// `crash2.ll` @undef1.
#[test]
fn test_crash2_undef1() {
    check_lit(
        "@g = global i32 0

define float @f() {
b0:
  %t0 = fadd fast float bitcast (i32 ptrtoint (ptr @g to i32) to float), bitcast (i32 ptrtoint (ptr @g to i32) to float)
  %t1 = fsub fast float bitcast (i32 ptrtoint (ptr @g to i32) to float), %t0
  %t2 = fadd fast float bitcast (i32 ptrtoint (ptr @g to i32) to float), %t1
  ret float %t2
}
",
        "define float @f() {
b0:
%factor1 = fmul fast float -1.000000e+00, bitcast (i32 ptrtoint (ptr @g to i32) to float)
%reass_add = fadd fast float %factor1, bitcast (i32 ptrtoint (ptr @g to i32) to float)
%reass_mul = fmul fast float %reass_add, 2.000000e+00
ret float %reass_mul
}
",
        &[],
    );
}

/// `fast-AgressiveSubMove.ll` @test1.
#[test]
fn test_fast_agressivesubmove_test1() {
    check_lit(
        "define float @f(float %A) {
b0:
  %X = fadd float %A, 1.000000e+00
  %Y = fadd float %A, 1.000000e+00
  %r = fsub float %X, %Y
  ret float %r
}
",
        "define float @f(float %A) {
b0:
%x = fadd float %A, 1.000000e+00
%y = fadd float %A, 1.000000e+00
%r = fsub float %x, %y
ret float %r
}
",
        &[],
    );
}

/// `fast-AgressiveSubMove.ll` @test2.
#[test]
fn test_fast_agressivesubmove_test2() {
    check_lit(
        "define float @f(float %A) {
b0:
  %X = fadd fast float 1.000000e+00, %A
  %Y = fadd fast float 1.000000e+00, %A
  %r = fsub fast float %X, %Y
  ret float %r
}
",
        "define float @f(float %A) {
b0:
ret float 0.000000e+00
}
",
        &[],
    );
}

/// `fast-AgressiveSubMove.ll` @test2_reassoc.
#[test]
fn test_fast_agressivesubmove_test2_reassoc() {
    check_lit(
        "define float @f(float %A) {
b0:
  %X = fadd reassoc float 1.000000e+00, %A
  %Y = fadd reassoc float 1.000000e+00, %A
  %r = fsub reassoc float %X, %Y
  ret float %r
}
",
        "define float @f(float %A) {
b0:
%x = fadd reassoc float %A, 1.000000e+00
%y = fadd reassoc float %A, 1.000000e+00
%r = fsub reassoc float %x, %y
ret float %r
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test2. TODO: check if we can eliminate zero add.
#[test]
fn test_fast_basictest_test2() {
    check_lit(
        "define float @f(float %reg109, float %reg1111) {
b0:
  %reg115 = fadd fast float %reg109, -3.000000e+01
  %reg116 = fadd fast float %reg115, %reg1111
  %reg117 = fadd fast float %reg116, 3.000000e+01
  ret float %reg117
}
",
        "define float @f(float %reg109, float %reg1111) {
b0:
%reg116 = fadd fast float %reg109, 0.000000e+00
%reg117 = fadd fast float %reg116, %reg1111
ret float %reg117
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test2_no_FMF.
#[test]
fn test_fast_basictest_test2_no_fmf() {
    check_lit(
        "define float @f(float %reg109, float %reg1111) {
b0:
  %reg115 = fadd float %reg109, -3.000000e+01
  %reg116 = fadd float %reg115, %reg1111
  %reg117 = fadd float %reg116, 3.000000e+01
  ret float %reg117
}
",
        "define float @f(float %reg109, float %reg1111) {
b0:
%reg115 = fadd float %reg109, -3.000000e+01
%reg116 = fadd float %reg115, %reg1111
%reg117 = fadd float %reg116, 3.000000e+01
ret float %reg117
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test2_reassoc.
#[test]
fn test_fast_basictest_test2_reassoc() {
    check_lit(
        "define float @f(float %reg109, float %reg1111) {
b0:
  %reg115 = fadd reassoc float %reg109, -3.000000e+01
  %reg116 = fadd reassoc float %reg115, %reg1111
  %reg117 = fadd reassoc float %reg116, 3.000000e+01
  ret float %reg117
}
",
        "define float @f(float %reg109, float %reg1111) {
b0:
%reg115 = fadd reassoc float %reg109, -3.000000e+01
%reg116 = fadd reassoc float %reg115, %reg1111
%reg117 = fadd reassoc float %reg116, 3.000000e+01
ret float %reg117
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test3.
#[test]
fn test_fast_basictest_test3() {
    check_lit(
        "@fe = external global float
@fa = external global float
@fb = external global float
@fc = external global float
@ff = external global float

define void @f() {
b0:
  %A = load float, ptr @fa
  %B = load float, ptr @fb
  %C = load float, ptr @fc
  %t1 = fadd fast float %A, %B
  %t2 = fadd fast float %t1, %C
  %t3 = fadd fast float %A, %C
  %t4 = fadd fast float %t3, %B
  ; e = (a+b)+c;
  store float %t2, ptr @fe
  ; f = (a+c)+b
  store float %t4, ptr @ff
  ret void
}
",
        "define void @f() {
b0:
%a = load float, ptr @fa, align 4
%b = load float, ptr @fb, align 4
%c = load float, ptr @fc, align 4
%t1 = fadd fast float %b, %a
%t2 = fadd fast float %t1, %c
%t3 = fadd fast float %b, %a
%t4 = fadd fast float %t3, %c
store float %t2, ptr @fe, align 4
store float %t4, ptr @ff, align 4
ret void
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test4.
#[test]
fn test_fast_basictest_test4() {
    check_lit(
        "@fe = external global float
@fa = external global float
@fb = external global float
@fc = external global float
@ff = external global float

define void @f() {
b0:
  %A = load float, ptr @fa
  %B = load float, ptr @fb
  %C = load float, ptr @fc
  %t1 = fadd fast float %A, %B
  %t2 = fadd fast float %C, %t1
  %t3 = fadd fast float %C, %A
  %t4 = fadd fast float %t3, %B
  ; e = c+(a+b)
  store float %t2, ptr @fe
  ; f = (c+a)+b
  store float %t4, ptr @ff
  ret void
}
",
        "define void @f() {
b0:
%a = load float, ptr @fa, align 4
%b = load float, ptr @fb, align 4
%c = load float, ptr @fc, align 4
%t1 = fadd fast float %b, %a
%t2 = fadd fast float %t1, %c
%t3 = fadd fast float %b, %a
%t4 = fadd fast float %t3, %c
store float %t2, ptr @fe, align 4
store float %t4, ptr @ff, align 4
ret void
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test5.
#[test]
fn test_fast_basictest_test5() {
    check_lit(
        "@fe = external global float
@fa = external global float
@fb = external global float
@fc = external global float
@ff = external global float

define void @f() {
b0:
  %A = load float, ptr @fa
  %B = load float, ptr @fb
  %C = load float, ptr @fc
  %t1 = fadd fast float %B, %A
  %t2 = fadd fast float %C, %t1
  %t3 = fadd fast float %C, %A
  %t4 = fadd fast float %t3, %B
  ; e = c+(b+a)
  store float %t2, ptr @fe
  ; f = (c+a)+b
  store float %t4, ptr @ff
  ret void
}
",
        "define void @f() {
b0:
%a = load float, ptr @fa, align 4
%b = load float, ptr @fb, align 4
%c = load float, ptr @fc, align 4
%t1 = fadd fast float %b, %a
%t2 = fadd fast float %t1, %c
%t3 = fadd fast float %b, %a
%t4 = fadd fast float %t3, %c
store float %t2, ptr @fe, align 4
store float %t4, ptr @ff, align 4
ret void
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test6.
#[test]
fn test_fast_basictest_test6() {
    check_lit(
        "define float @f(float %A, float %B, float %C) {
b0:
  %aa = fmul fast float %A, %A
  %aab = fmul fast float %aa, %B
  %ac = fmul fast float %A, %C
  %aac = fmul fast float %ac, %A
  %r = fadd fast float %aab, %aac
  ret float %r
}
",
        "define float @f(float %A, float %B, float %C) {
b0:
%reass_add1 = fadd fast float %c, %b
%reass_mul2 = fmul fast float %a, %a
%reass_mul = fmul fast float %reass_mul2, %reass_add1
ret float %reass_mul
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test6_reassoc.
#[test]
fn test_fast_basictest_test6_reassoc() {
    check_lit(
        "define float @f(float %A, float %B, float %C) {
b0:
  %aa = fmul reassoc float %A, %A
  %aab = fmul reassoc float %aa, %B
  %ac = fmul reassoc float %A, %C
  %aac = fmul reassoc float %ac, %A
  %r = fadd reassoc float %aab, %aac
  ret float %r
}
",
        "define float @f(float %A, float %B, float %C) {
b0:
%aa = fmul reassoc float %a, %a
%aab = fmul reassoc float %aa, %b
%ac = fmul reassoc float %a, %c
%aac = fmul reassoc float %a, %ac
%r = fadd reassoc float %aab, %aac
ret float %r
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test7. (-X)*Y + Z -> Z-X*Y
#[test]
fn test_fast_basictest_test7() {
    check_lit(
        "define float @f(float %X, float %Y, float %Z) {
b0:
  %A = fsub fast float 0.0, %X
  %B = fmul fast float %A, %Y
  %C = fadd fast float %B, %Z
  ret float %C
}
",
        "define float @f(float %X, float %Y, float %Z) {
b0:
%b = fmul fast float %y, %x
%tmp1 = fsub fast float %z, %b
ret float %tmp1
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test7_unary_fneg.
#[test]
fn test_fast_basictest_test7_unary_fneg() {
    check_lit(
        "define float @f(float %X, float %Y, float %Z) {
b0:
  %A = fneg fast float %X
  %B = fmul fast float %A, %Y
  %C = fadd fast float %B, %Z
  ret float %C
}
",
        "define float @f(float %X, float %Y, float %Z) {
b0:
%b = fmul fast float %y, %x
%tmp1 = fsub fast float %z, %b
ret float %tmp1
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test7_reassoc_nsz.
#[test]
fn test_fast_basictest_test7_reassoc_nsz() {
    check_lit(
        "define float @f(float %X, float %Y, float %Z) {
b0:
  %A = fsub reassoc nsz float 0.0, %X
  %B = fmul reassoc nsz float %A, %Y
  %C = fadd reassoc nsz float %B, %Z
  ret float %C
}
",
        "define float @f(float %X, float %Y, float %Z) {
b0:
%b = fmul reassoc nsz float %y, %x
%tmp1 = fsub reassoc nsz float %z, %b
ret float %tmp1
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test7_reassoc. Verify that fold is not done only with
/// 'reassoc' ('nsz' is required)
#[test]
fn test_fast_basictest_test7_reassoc() {
    check_lit(
        "define float @f(float %X, float %Y, float %Z) {
b0:
  %A = fsub reassoc float 0.0, %X
  %B = fmul reassoc float %A, %Y
  %C = fadd reassoc float %B, %Z
  ret float %C
}
",
        "define float @f(float %X, float %Y, float %Z) {
b0:
%a = fsub reassoc float 0.000000e+00, %x
%b = fmul reassoc float %a, %y
%c = fadd reassoc float %b, %z
ret float %c
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test7_int. Integer version of: (-X)*Y + Z -> Z-X*Y
/// TODO: check if we can change the mul of -1 and the add to a sub.
#[test]
fn test_fast_basictest_test7_int() {
    check_lit(
        "define i32 @f(i32 %X, i32 %Y, i32 %Z) {
b0:
  %A = sub i32 0, %X
  %B = mul i32 %A, %Y
  %C = add i32 %B, %Z
  ret i32 %C
}
",
        "define i32 @f(i32 %X, i32 %Y, i32 %Z) {
b0:
%a = mul i32 %y, %x
%b = mul i32 %a, -1
%c = add i32 %b, %z
ret i32 %c
}
",
        &[&[0, 0, 0], &[1, 1, 1], &[-1, -1, -1], &[2, 5, 8], &[-7, -12, -17], &[100, 111, 122]],
    );
}

/// `fast-basictest.ll` @test8.
#[test]
fn test_fast_basictest_test8() {
    check_lit(
        "define float @f(float %X) {
b0:
  %Y = fmul fast float %X, 4.700000e+01
  %Z = fadd fast float %Y, %Y
  ret float %Z
}
",
        "define float @f(float %X) {
b0:
%factor = fmul fast float %x, 9.400000e+01
ret float %factor
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test9. Side note: (x + x + x) and (3*x) each have only
/// a single rounding.  So transforming x+x+x to 3*x is always safe, even
/// without any FMF. To avoid that special-case, we have the addition of 'x'
/// four times, here.
#[test]
fn test_fast_basictest_test9() {
    check_lit(
        "define float @f(float %X) {
b0:
  %Y = fadd fast float %X ,%X
  %Z = fadd fast float %Y, %X
  %W = fadd fast float %Z, %X
  ret float %W
}
",
        "define float @f(float %X) {
b0:
%factor = fmul fast float %x, 4.000000e+00
ret float %factor
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test10.
#[test]
fn test_fast_basictest_test10() {
    check_lit(
        "define float @f(float %W) {
b0:
  %X = fmul fast float %W, 127.0
  %Y = fadd fast float %X ,%X
  %Z = fadd fast float %Y, %X
  ret float %Z
}
",
        "define float @f(float %W) {
b0:
%factor = fmul fast float %w, 3.810000e+02
ret float %factor
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test11.
#[test]
fn test_fast_basictest_test11() {
    check_lit(
        "define float @f(float %X) {
b0:
  %A = fsub fast float 1.000000e+00, %X
  %B = fsub fast float 2.000000e+00, %X
  %C = fsub fast float 3.000000e+00, %X
  %Y = fadd fast float %A ,%B
  %Z = fadd fast float %Y, %C
  ret float %Z
}
",
        "define float @f(float %X) {
b0:
%factor = fmul fast float %x, -3.000000e+00
%z = fadd fast float %factor, 6.000000e+00
ret float %z
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test12.
#[test]
fn test_fast_basictest_test12() {
    check_lit(
        "define float @f(float %X1, float %X2, float %X3) {
b0:
  %A = fsub fast float 0.000000e+00, %X1
  %B = fmul fast float %A, %X2   ; -X1*X2
  %C = fmul fast float %X1, %X3  ; X1*X3
  %D = fadd fast float %B, %C    ; -X1*X2 + X1*X3 -> X1*(X3-X2)
  ret float %D
}
",
        "define float @f(float %X1, float %X2, float %X3) {
b0:
%b = fmul fast float %x2, %x1
%c = fmul fast float %x3, %x1
%tmp1 = fsub fast float %c, %b
ret float %tmp1
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test12_unary_fneg.
#[test]
fn test_fast_basictest_test12_unary_fneg() {
    check_lit(
        "define float @f(float %X1, float %X2, float %X3) {
b0:
  %A = fneg fast float %X1
  %B = fmul fast float %A, %X2   ; -X1*X2
  %C = fmul fast float %X1, %X3  ; X1*X3
  %D = fadd fast float %B, %C    ; -X1*X2 + X1*X3 -> X1*(X3-X2)
  ret float %D
}
",
        "define float @f(float %X1, float %X2, float %X3) {
b0:
%b = fmul fast float %x2, %x1
%c = fmul fast float %x3, %x1
%tmp1 = fsub fast float %c, %b
ret float %tmp1
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test12_reassoc_nsz.
#[test]
fn test_fast_basictest_test12_reassoc_nsz() {
    check_lit(
        "define float @f(float %X1, float %X2, float %X3) {
b0:
  %A = fsub reassoc nsz float 0.000000e+00, %X1
  %B = fmul reassoc nsz float %A, %X2   ; -X1*X2
  %C = fmul reassoc nsz float %X1, %X3  ; X1*X3
  %D = fadd reassoc nsz float %B, %C    ; -X1*X2 + X1*X3 -> X1*(X3-X2)
  ret float %D
}
",
        "define float @f(float %X1, float %X2, float %X3) {
b0:
%b = fmul reassoc nsz float %x2, %x1
%c = fmul reassoc nsz float %x3, %x1
%tmp1 = fsub reassoc nsz float %c, %b
ret float %tmp1
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test12_reassoc. TODO: check if 'nsz' is technically
/// required. Currently the optimization is not done with only 'reassoc' without
/// 'nsz'.
#[test]
fn test_fast_basictest_test12_reassoc() {
    check_lit(
        "define float @f(float %X1, float %X2, float %X3) {
b0:
  %A = fsub reassoc float 0.000000e+00, %X1
  %B = fmul reassoc float %A, %X2   ; -X1*X2
  %C = fmul reassoc float %X1, %X3  ; X1*X3
  %D = fadd reassoc float %B, %C    ; -X1*X2 + X1*X3 -> X1*(X3-X2)
  ret float %D
}
",
        "define float @f(float %X1, float %X2, float %X3) {
b0:
%a = fsub reassoc float 0.000000e+00, %x1
%b = fmul reassoc float %a, %x2
%c = fmul reassoc float %x1, %x3
%d = fadd reassoc float %b, %c
ret float %d
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test13.
#[test]
fn test_fast_basictest_test13() {
    check_lit(
        "define float @f(float %X1, float %X2) {
b0:
  %B = fmul fast float %X1, 47.   ; X1*47
  %C = fmul fast float %X2, -47.  ; X2*-47
  %D = fadd fast float %B, %C     ; X1*47 + X2*-47 -> 47*(X1-X2)
  ret float %D
}
",
        "define float @f(float %X1, float %X2) {
b0:
%b = fmul fast float %x1, 4.700000e+01
%c = fmul fast float %x2, 4.700000e+01
%tmp1 = fsub fast float %b, %c
ret float %tmp1
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test13_reassoc_nsz.
#[test]
fn test_fast_basictest_test13_reassoc_nsz() {
    check_lit(
        "define float @f(float %X1, float %X2) {
b0:
  %B = fmul reassoc nsz float %X1, 47.   ; X1*47
  %C = fmul reassoc nsz float %X2, -47.  ; X2*-47
  %D = fadd reassoc nsz float %B, %C     ; X1*47 + X2*-47 -> 47*(X1-X2)
  ret float %D
}
",
        "define float @f(float %X1, float %X2) {
b0:
%b = fmul reassoc nsz float %x1, 4.700000e+01
%c = fmul reassoc nsz float %x2, 4.700000e+01
%tmp1 = fsub reassoc nsz float %b, %c
ret float %tmp1
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test13_reassoc.
#[test]
fn test_fast_basictest_test13_reassoc() {
    check_lit(
        "define float @f(float %X1, float %X2) {
b0:
  %B = fmul reassoc float %X1, 47.   ; X1*47
  %C = fmul reassoc float %X2, -47.  ; X2*-47
  %D = fadd reassoc float %B, %C     ; X1*47 + X2*-47 -> 47*(X1-X2)
  ret float %D
}
",
        "define float @f(float %X1, float %X2) {
b0:
%b = fmul reassoc float %x1, 4.700000e+01
%c = fmul reassoc float %x2, 4.700000e+01
%tmp1 = fsub reassoc float %b, %c
ret float %tmp1
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test14.
#[test]
fn test_fast_basictest_test14() {
    check_lit(
        "define float @f(float %arg) {
b0:
  %t1 = fmul fast float 1.200000e+01, %arg
  %t2 = fmul fast float %t1, 1.200000e+01
  ret float %t2
}
",
        "define float @f(float %arg) {
b0:
%t2 = fmul fast float %arg, 1.440000e+02
ret float %t2
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test14_reassoc. TODO: check if we can transform the
/// code with 'reassoc' only. The same IR is transformed to one fmul in
/// instcombine pass.
#[test]
fn test_fast_basictest_test14_reassoc() {
    check_lit(
        "define float @f(float %arg) {
b0:
  %t1 = fmul reassoc float 1.200000e+01, %arg
  %t2 = fmul reassoc float %t1, 1.200000e+01
  ret float %t2
}
",
        "define float @f(float %arg) {
b0:
%t1 = fmul reassoc float %arg, 1.200000e+01
%t2 = fmul reassoc float %t1, 1.200000e+01
ret float %t2
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test15. TODO: check if we can remove dead fsub.
#[test]
fn test_fast_basictest_test15() {
    check_lit(
        "define float @f(float %b, float %a) {
b0:
  %n1 = fadd fast float %a, 1234.0
  %n2 = fadd fast float %b, %n1
  %n3 = fsub fast float 0.0, %a
  %n4 = fadd fast float %n2, %n3
  ret float %n4
}
",
        "define float @f(float %b, float %a) {
b0:
%n1 = fsub fast float 0.000000e+00, %a
%n2 = fadd fast float %b, 1.234000e+03
ret float %n2
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test15_unary_fneg. TODO: check if we can remove dead
/// fneg.
#[test]
fn test_fast_basictest_test15_unary_fneg() {
    check_lit(
        "define float @f(float %b, float %a) {
b0:
  %n1 = fadd fast float %a, 1234.0
  %n2 = fadd fast float %b, %n1
  %n3 = fneg fast float %a
  %n4 = fadd fast float %n2, %n3
  ret float %n4
}
",
        "define float @f(float %b, float %a) {
b0:
%n1 = fneg fast float %a
%n2 = fadd fast float %b, 1.234000e+03
ret float %n2
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test15_reassoc_nsz. TODO: check if we can remove dead
/// fsub.
#[test]
fn test_fast_basictest_test15_reassoc_nsz() {
    check_lit(
        "define float @f(float %b, float %a) {
b0:
  %n1 = fadd reassoc nsz float %a, 1234.0
  %n2 = fadd reassoc nsz float %b, %n1
  %n3 = fsub reassoc nsz float 0.0, %a
  %n4 = fadd reassoc nsz float %n2, %n3
  ret float %n4
}
",
        "define float @f(float %b, float %a) {
b0:
%n1 = fsub reassoc nsz float 0.000000e+00, %a
%n2 = fadd reassoc nsz float %b, 1.234000e+03
ret float %n2
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test15_reassoc.
#[test]
fn test_fast_basictest_test15_reassoc() {
    check_lit(
        "define float @f(float %b, float %a) {
b0:
  %n1 = fadd reassoc float %a, 1234.0
  %n2 = fadd reassoc float %b, %n1
  %n3 = fsub reassoc float 0.0, %a
  %n4 = fadd reassoc float %n2, %n3
  ret float %n4
}
",
        "define float @f(float %b, float %a) {
b0:
%n1 = fadd reassoc float %a, 1.234000e+03
%n2 = fadd reassoc float %b, %n1
%n3 = fsub reassoc float 0.000000e+00, %a
%n4 = fadd reassoc float %n3, %n2
ret float %n4
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test16.
#[test]
fn test_fast_basictest_test16() {
    check_lit(
        "define float @f(float %a, float %b, float %z) {
b0:
  %c = fsub fast float 0.000000e+00, %z
  %d = fmul fast float %a, %b
  %e = fmul fast float %c, %d
  %f = fmul fast float %e, 1.234500e+04
  %g = fsub fast float 0.000000e+00, %f
  ret float %g
}
",
        "define float @f(float %a, float %b, float %z) {
b0:
%c = fmul fast float %a, 1.234500e+04
%e = fmul fast float %c, %b
%f = fmul fast float %e, %z
%tmp1 = fadd fast float %f, 0.000000e+00
ret float %tmp1
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test16_unary_fneg.
#[test]
fn test_fast_basictest_test16_unary_fneg() {
    check_lit(
        "define float @f(float %a, float %b, float %z) {
b0:
  %c = fneg fast float %z
  %d = fmul fast float %a, %b
  %e = fmul fast float %c, %d
  %f = fmul fast float %e, 1.234500e+04
  %g = fneg fast float %f
  ret float %g
}
",
        "define float @f(float %a, float %b, float %z) {
b0:
%e = fmul fast float %a, 1.234500e+04
%f = fmul fast float %e, %b
%g = fmul fast float %f, %z
ret float %g
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test16_reassoc.
#[test]
fn test_fast_basictest_test16_reassoc() {
    check_lit(
        "define float @f(float %a, float %b, float %z) {
b0:
  %c = fsub reassoc float 0.000000e+00, %z
  %d = fmul reassoc float %a, %b
  %e = fmul reassoc float %c, %d
  %f = fmul reassoc float %e, 1.234500e+04
  %g = fsub reassoc float 0.000000e+00, %f
  ret float %g
}
",
        "define float @f(float %a, float %b, float %z) {
b0:
%c = fsub reassoc float 0.000000e+00, %z
%d = fmul reassoc float %a, %b
%e = fmul reassoc float %d, %c
%f = fmul reassoc float %e, 1.234500e+04
%g = fsub reassoc float 0.000000e+00, %f
ret float %g
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test17. TODO: check if we can remove: - fadd fast x, 0
/// ... as 'fast' implies 'nsz'
#[test]
fn test_fast_basictest_test17() {
    check_lit(
        "define float @f(float %a, float %b, float %z) {
b0:
  %d = fmul fast float %z, 4.000000e+01
  %c = fsub fast float 0.000000e+00, %d
  %e = fmul fast float %a, %c
  %f = fsub fast float 0.000000e+00, %e
  ret float %f
}
",
        "define float @f(float %a, float %b, float %z) {
b0:
%c = fmul fast float %a, 4.000000e+01
%e = fmul fast float %c, %z
%tmp1 = fadd fast float %e, 0.000000e+00
ret float %tmp1
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test17_unary_fneg.
#[test]
fn test_fast_basictest_test17_unary_fneg() {
    check_lit(
        "define float @f(float %a, float %b, float %z) {
b0:
  %d = fmul fast float %z, 4.000000e+01
  %c = fneg fast float %d
  %e = fmul fast float %a, %c
  %f = fneg fast float %e
  ret float %f
}
",
        "define float @f(float %a, float %b, float %z) {
b0:
%e = fmul fast float %a, 4.000000e+01
%f = fmul fast float %e, %z
ret float %f
}
",
        &[],
    );
}

/// `fast-basictest.ll` @test18. With sub reassociation, constant folding can
/// eliminate the 12 and -12 constants. TODO: check if we can remove fadd fast
/// x, 0 as 'fast' implies 'nsz'
#[test]
fn test_fast_basictest_test18() {
    check_lit(
        "define float @f(float %A, float %B) {
b0:
  %X = fadd fast float -1.200000e+01, %A
  %Y = fsub fast float %X, %B
  %Z = fadd fast float %Y, 1.200000e+01
  ret float %Z
}
",
        "define float @f(float %A, float %B) {
b0:
%b_neg = fneg fast float %b
%y = fadd fast float %a, 0.000000e+00
%z = fadd fast float %y, %b_neg
ret float %z
}
",
        &[],
    );
}

/// `fast-fp-commute.ll` @test1.
#[test]
fn test_fast_fp_commute_test1() {
    check_lit(
        "declare void @use(float)

define void @f(float %x, float %y) {
b0:
  %n1 = fmul fast float %x, %y
  %n2 = fmul fast float %y, %x
  %n3 = fsub fast float %n1, %n2
  call void @use(float %n1)
  call void @use(float %n3)
  ret void
}
",
        "define void @f(float %x, float %y) {
b0:
%n1 = fmul fast float %y, %x
%n2 = fmul fast float %y, %x
%n3 = fsub fast float %n1, %n2
call void @use(float %n1)
call void @use(float %n3)
ret void
}
",
        &[],
    );
}

/// `fast-fp-commute.ll` @test2.
#[test]
fn test_fast_fp_commute_test2() {
    check_lit(
        "define float @f(float %x, float %y) {
b0:
  %n1 = fmul fast float %x, %y
  %n2 = fmul fast float %y, %x
  %n3 = fsub fast float %n1, %n2
  ret float %n3
}
",
        "define float @f(float %x, float %y) {
b0:
%n1 = fmul fast float %y, %x
%n2 = fmul fast float %y, %x
%n3 = fsub fast float %n1, %n2
ret float %n3
}
",
        &[],
    );
}

/// `fast-fp-commute.ll` @test3.
#[test]
fn test_fast_fp_commute_test3() {
    check_lit(
        "define float @f(float %x, float %y) {
b0:
  %n1 = fmul fast float %x, %y
  %n2 = fmul fast float %y, %x
  %n3 = fadd fast float %n1, %n2
  ret float %n3
}
",
        "define float @f(float %x, float %y) {
b0:
%factor = fmul fast float %y, %x
%reass_mul = fmul fast float %factor, 2.000000e+00
ret float %reass_mul
}
",
        &[],
    );
}

/// `fast-multistep.ll` @fmultistep1.
#[test]
fn test_fast_multistep_fmultistep1() {
    check_lit(
        "define float @f(float %a, float %b, float %c) {
b0:
  %t0 = fmul fast float %a, %b
  %t1 = fmul fast float %a, %t0 ; a*(a*b)
  %t2 = fmul fast float %a, %c
  %t3 = fmul fast float %a, %t2 ; a*(a*c)
  %t4 = fadd fast float %t1, %t3
  ret float %t4
}
",
        "define float @f(float %a, float %b, float %c) {
b0:
%reass_add1 = fadd fast float %c, %b
%reass_mul2 = fmul fast float %a, %a
%reass_mul = fmul fast float %reass_mul2, %reass_add1
ret float %reass_mul
}
",
        &[],
    );
}

/// `fast-multistep.ll` @fmultistep2.
#[test]
fn test_fast_multistep_fmultistep2() {
    check_lit(
        "define float @f(float %a, float %b, float %c, float %d) {
b0:
  %t0 = fmul fast float %a, %b
  %t1 = fmul fast float %a, %c
  %t2 = fadd fast float %t1, %d ; a*c+d
  %t3 = fadd fast float %t0, %t2 ; a*b+(a*c+d)
  ret float %t3
}
",
        "define float @f(float %a, float %b, float %c, float %d) {
b0:
%reass_add = fadd fast float %c, %b
%reass_mul = fmul fast float %reass_add, %a
%t3 = fadd fast float %reass_mul, %d
ret float %t3
}
",
        &[],
    );
}

/// `fma-pairs.ll` @fsub_fmul_2.
#[test]
fn test_fma_pairs_fsub_fmul_2() {
    check_lit(
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
  %x0 = load double, ptr %x
  %y0 = load double, ptr %y
  %m0 = fmul reassoc nsz contract double %x0, %y0
  %z0 = load double, ptr %z
  %s0 = fsub reassoc nsz contract double %m0, %z0
  %x1p = getelementptr inbounds double, ptr %x, i64 1
  %x1 = load double, ptr %x1p
  %y1p = getelementptr inbounds double, ptr %y, i64 1
  %y1 = load double, ptr %y1p
  %m1 = fmul reassoc nsz contract double %x1, %y1
  %z1p = getelementptr inbounds double, ptr %z, i64 1
  %z1 = load double, ptr %z1p
  %s1 = fsub reassoc nsz contract double %m1, %z1
  %r = fadd reassoc nsz contract double %s0, %s1
  ret double %r
}
",
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
%x0 = load double, ptr %x, align 8
%y0 = load double, ptr %y, align 8
%m0 = fmul reassoc nsz contract double %y0, %x0
%z0 = load double, ptr %z, align 8
%z0_neg = fneg reassoc nsz contract double %z0
%s0 = fadd reassoc nsz contract double %m0, %z0_neg
%x1p = getelementptr inbounds double, ptr %x, i64 1
%x1 = load double, ptr %x1p, align 8
%y1p = getelementptr inbounds double, ptr %y, i64 1
%y1 = load double, ptr %y1p, align 8
%m1 = fmul reassoc nsz contract double %y1, %x1
%z1p = getelementptr inbounds double, ptr %z, i64 1
%z1 = load double, ptr %z1p, align 8
%z1_neg = fneg reassoc nsz contract double %z1
%s1 = fadd reassoc nsz contract double %m1, %z1_neg
%r = fadd reassoc nsz contract double %s1, %s0
ret double %r
}
",
        &[],
    );
}

/// `fma-pairs.ll` @fadd_fmul_2.
#[test]
fn test_fma_pairs_fadd_fmul_2() {
    check_lit(
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
  %x0 = load double, ptr %x
  %y0 = load double, ptr %y
  %m0 = fmul reassoc nsz contract double %x0, %y0
  %z0 = load double, ptr %z
  %a0 = fadd reassoc nsz contract double %m0, %z0
  %x1p = getelementptr inbounds double, ptr %x, i64 1
  %x1 = load double, ptr %x1p
  %y1p = getelementptr inbounds double, ptr %y, i64 1
  %y1 = load double, ptr %y1p
  %m1 = fmul reassoc nsz contract double %x1, %y1
  %z1p = getelementptr inbounds double, ptr %z, i64 1
  %z1 = load double, ptr %z1p
  %a1 = fadd reassoc nsz contract double %m1, %z1
  %r = fadd reassoc nsz contract double %a0, %a1
  ret double %r
}
",
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
%x0 = load double, ptr %x, align 8
%y0 = load double, ptr %y, align 8
%m0 = fmul reassoc nsz contract double %y0, %x0
%z0 = load double, ptr %z, align 8
%a0 = fadd reassoc nsz contract double %m0, %z0
%x1p = getelementptr inbounds double, ptr %x, i64 1
%x1 = load double, ptr %x1p, align 8
%y1p = getelementptr inbounds double, ptr %y, i64 1
%y1 = load double, ptr %y1p, align 8
%m1 = fmul reassoc nsz contract double %y1, %x1
%z1p = getelementptr inbounds double, ptr %z, i64 1
%z1 = load double, ptr %z1p, align 8
%a1 = fadd reassoc nsz contract double %m1, %z1
%r = fadd reassoc nsz contract double %a1, %a0
ret double %r
}
",
        &[],
    );
}

/// `fma-pairs.ll` @fadd_fmul_2_right.
#[test]
fn test_fma_pairs_fadd_fmul_2_right() {
    check_lit(
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
  %x0 = load double, ptr %x
  %y0 = load double, ptr %y
  %m0 = fmul reassoc nsz contract double %x0, %y0
  %z0 = load double, ptr %z
  %a0 = fadd reassoc nsz contract double %z0, %m0
  %x1p = getelementptr inbounds double, ptr %x, i64 1
  %x1 = load double, ptr %x1p
  %y1p = getelementptr inbounds double, ptr %y, i64 1
  %y1 = load double, ptr %y1p
  %m1 = fmul reassoc nsz contract double %x1, %y1
  %z1p = getelementptr inbounds double, ptr %z, i64 1
  %z1 = load double, ptr %z1p
  %a1 = fadd reassoc nsz contract double %z1, %m1
  %r = fadd reassoc nsz contract double %a0, %a1
  ret double %r
}
",
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
%x0 = load double, ptr %x, align 8
%y0 = load double, ptr %y, align 8
%m0 = fmul reassoc nsz contract double %y0, %x0
%z0 = load double, ptr %z, align 8
%a0 = fadd reassoc nsz contract double %z0, %m0
%x1p = getelementptr inbounds double, ptr %x, i64 1
%x1 = load double, ptr %x1p, align 8
%y1p = getelementptr inbounds double, ptr %y, i64 1
%y1 = load double, ptr %y1p, align 8
%m1 = fmul reassoc nsz contract double %y1, %x1
%z1p = getelementptr inbounds double, ptr %z, i64 1
%z1 = load double, ptr %z1p, align 8
%a1 = fadd reassoc nsz contract double %z1, %m1
%r = fadd reassoc nsz contract double %a1, %a0
ret double %r
}
",
        &[],
    );
}

/// `fma-pairs.ll` @fsub_fmul_2_right.
#[test]
fn test_fma_pairs_fsub_fmul_2_right() {
    check_lit(
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
  %x0 = load double, ptr %x
  %y0 = load double, ptr %y
  %m0 = fmul reassoc nsz contract double %x0, %y0
  %z0 = load double, ptr %z
  %s0 = fsub reassoc nsz contract double %z0, %m0
  %x1p = getelementptr inbounds double, ptr %x, i64 1
  %x1 = load double, ptr %x1p
  %y1p = getelementptr inbounds double, ptr %y, i64 1
  %y1 = load double, ptr %y1p
  %m1 = fmul reassoc nsz contract double %x1, %y1
  %z1p = getelementptr inbounds double, ptr %z, i64 1
  %z1 = load double, ptr %z1p
  %s1 = fsub reassoc nsz contract double %z1, %m1
  %r = fadd reassoc nsz contract double %s0, %s1
  ret double %r
}
",
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
%x0 = load double, ptr %x, align 8
%y0 = load double, ptr %y, align 8
%z0 = load double, ptr %z, align 8
%m0_neg = fmul reassoc nsz contract double %y0, %x0
%x1p = getelementptr inbounds double, ptr %x, i64 1
%x1 = load double, ptr %x1p, align 8
%y1p = getelementptr inbounds double, ptr %y, i64 1
%y1 = load double, ptr %y1p, align 8
%z1p = getelementptr inbounds double, ptr %z, i64 1
%z1 = load double, ptr %z1p, align 8
%m1_neg = fmul reassoc nsz contract double %y1, %x1
%reass_add = fadd reassoc nsz contract double %m1_neg, %m0_neg
%reass_mul = fmul reassoc nsz contract double -1.000000e+00, %reass_add
%s0 = fadd reassoc nsz contract double %z0, %z1
%r = fadd reassoc nsz contract double %s0, %reass_mul
ret double %r
}
",
        &[],
    );
}

/// `fma-pairs.ll` @fsub_fmul_4.
#[test]
fn test_fma_pairs_fsub_fmul_4() {
    check_lit(
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
  %x0 = load double, ptr %x
  %y0 = load double, ptr %y
  %m0 = fmul reassoc nsz contract double %x0, %y0
  %z0 = load double, ptr %z
  %s0 = fsub reassoc nsz contract double %m0, %z0
  %x1p = getelementptr inbounds double, ptr %x, i64 1
  %x1 = load double, ptr %x1p
  %y1p = getelementptr inbounds double, ptr %y, i64 1
  %y1 = load double, ptr %y1p
  %m1 = fmul reassoc nsz contract double %x1, %y1
  %z1p = getelementptr inbounds double, ptr %z, i64 1
  %z1 = load double, ptr %z1p
  %s1 = fsub reassoc nsz contract double %m1, %z1
  %x2p = getelementptr inbounds double, ptr %x, i64 2
  %x2 = load double, ptr %x2p
  %y2p = getelementptr inbounds double, ptr %y, i64 2
  %y2 = load double, ptr %y2p
  %m2 = fmul reassoc nsz contract double %x2, %y2
  %z2p = getelementptr inbounds double, ptr %z, i64 2
  %z2 = load double, ptr %z2p
  %s2 = fsub reassoc nsz contract double %m2, %z2
  %x3p = getelementptr inbounds double, ptr %x, i64 3
  %x3 = load double, ptr %x3p
  %y3p = getelementptr inbounds double, ptr %y, i64 3
  %y3 = load double, ptr %y3p
  %m3 = fmul reassoc nsz contract double %x3, %y3
  %z3p = getelementptr inbounds double, ptr %z, i64 3
  %z3 = load double, ptr %z3p
  %s3 = fsub reassoc nsz contract double %m3, %z3
  %r0 = fadd reassoc nsz contract double %s0, %s1
  %r1 = fadd reassoc nsz contract double %r0, %s2
  %r2 = fadd reassoc nsz contract double %r1, %s3
  ret double %r2
}
",
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
%x0 = load double, ptr %x, align 8
%y0 = load double, ptr %y, align 8
%m0 = fmul reassoc nsz contract double %y0, %x0
%z0 = load double, ptr %z, align 8
%z0_neg = fneg reassoc nsz contract double %z0
%s0 = fadd reassoc nsz contract double %m0, %z0_neg
%x1p = getelementptr inbounds double, ptr %x, i64 1
%x1 = load double, ptr %x1p, align 8
%y1p = getelementptr inbounds double, ptr %y, i64 1
%y1 = load double, ptr %y1p, align 8
%m1 = fmul reassoc nsz contract double %y1, %x1
%z1p = getelementptr inbounds double, ptr %z, i64 1
%z1 = load double, ptr %z1p, align 8
%z1_neg = fneg reassoc nsz contract double %z1
%s1 = fadd reassoc nsz contract double %m1, %z1_neg
%x2p = getelementptr inbounds double, ptr %x, i64 2
%x2 = load double, ptr %x2p, align 8
%y2p = getelementptr inbounds double, ptr %y, i64 2
%y2 = load double, ptr %y2p, align 8
%m2 = fmul reassoc nsz contract double %y2, %x2
%z2p = getelementptr inbounds double, ptr %z, i64 2
%z2 = load double, ptr %z2p, align 8
%z2_neg = fneg reassoc nsz contract double %z2
%s2 = fadd reassoc nsz contract double %m2, %z2_neg
%x3p = getelementptr inbounds double, ptr %x, i64 3
%x3 = load double, ptr %x3p, align 8
%y3p = getelementptr inbounds double, ptr %y, i64 3
%y3 = load double, ptr %y3p, align 8
%m3 = fmul reassoc nsz contract double %y3, %x3
%z3p = getelementptr inbounds double, ptr %z, i64 3
%z3 = load double, ptr %z3p, align 8
%z3_neg = fneg reassoc nsz contract double %z3
%s3 = fadd reassoc nsz contract double %m3, %z3_neg
%r0 = fadd reassoc nsz contract double %s1, %s0
%r1 = fadd reassoc nsz contract double %r0, %s2
%r2 = fadd reassoc nsz contract double %r1, %s3
ret double %r2
}
",
        &[],
    );
}

/// `fma-pairs.ll` @factorize_first.
#[test]
fn test_fma_pairs_factorize_first() {
    check_lit(
        "define float @f(float %a, float %b, float %c, float %d) {
b0:
  %t0 = fmul reassoc nsz contract float %a, %b
  %t1 = fmul reassoc nsz contract float %a, %c
  %t2 = fadd reassoc nsz contract float %t1, %d
  %t3 = fadd reassoc nsz contract float %t0, %t2
  ret float %t3
}
",
        "define float @f(float %a, float %b, float %c, float %d) {
b0:
%reass_add = fadd reassoc nsz contract float %c, %b
%reass_mul = fmul reassoc nsz contract float %reass_add, %a
%t3 = fadd reassoc nsz contract float %reass_mul, %d
ret float %t3
}
",
        &[],
    );
}

/// `fma-pairs.ll` @fadd_fmul_2_contract_only_mul.
#[test]
fn test_fma_pairs_fadd_fmul_2_contract_only_mul() {
    check_lit(
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
  %x0 = load double, ptr %x
  %y0 = load double, ptr %y
  %m0 = fmul contract double %x0, %y0
  %z0 = load double, ptr %z
  %a0 = fadd reassoc nsz contract double %m0, %z0
  %x1p = getelementptr inbounds double, ptr %x, i64 1
  %x1 = load double, ptr %x1p
  %y1p = getelementptr inbounds double, ptr %y, i64 1
  %y1 = load double, ptr %y1p
  %m1 = fmul contract double %x1, %y1
  %z1p = getelementptr inbounds double, ptr %z, i64 1
  %z1 = load double, ptr %z1p
  %a1 = fadd reassoc nsz contract double %m1, %z1
  %r = fadd reassoc nsz contract double %a0, %a1
  ret double %r
}
",
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
%x0 = load double, ptr %x, align 8
%y0 = load double, ptr %y, align 8
%m0 = fmul contract double %x0, %y0
%z0 = load double, ptr %z, align 8
%a0 = fadd reassoc nsz contract double %m0, %z0
%x1p = getelementptr inbounds double, ptr %x, i64 1
%x1 = load double, ptr %x1p, align 8
%y1p = getelementptr inbounds double, ptr %y, i64 1
%y1 = load double, ptr %y1p, align 8
%m1 = fmul contract double %x1, %y1
%z1p = getelementptr inbounds double, ptr %z, i64 1
%z1 = load double, ptr %z1p, align 8
%a1 = fadd reassoc nsz contract double %m1, %z1
%r = fadd reassoc nsz contract double %a1, %a0
ret double %r
}
",
        &[],
    );
}

/// `fma-pairs.ll` @factorize_contract_only_mul.
#[test]
fn test_fma_pairs_factorize_contract_only_mul() {
    check_lit(
        "define float @f(float %a, float %b, float %c, float %d) {
b0:
  %t0 = fmul reassoc nsz contract float %a, %b
  %t1 = fmul contract float %a, %c
  %t2 = fadd reassoc nsz contract float %t1, %d
  %t3 = fadd reassoc nsz contract float %t0, %t2
  ret float %t3
}
",
        "define float @f(float %a, float %b, float %c, float %d) {
b0:
%t0 = fmul reassoc nsz contract float %b, %a
%t1 = fmul contract float %a, %c
%t2 = fadd reassoc nsz contract float %t1, %d
%t3 = fadd reassoc nsz contract float %t2, %t0
ret float %t3
}
",
        &[],
    );
}

/// `fma-pairs.ll` @no_contract.
#[test]
fn test_fma_pairs_no_contract() {
    check_lit(
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
  %x0 = load double, ptr %x
  %y0 = load double, ptr %y
  %m0 = fmul reassoc nsz double %x0, %y0
  %z0 = load double, ptr %z
  %s0 = fsub reassoc nsz double %m0, %z0
  %x1p = getelementptr inbounds double, ptr %x, i64 1
  %x1 = load double, ptr %x1p
  %y1p = getelementptr inbounds double, ptr %y, i64 1
  %y1 = load double, ptr %y1p
  %m1 = fmul reassoc nsz double %x1, %y1
  %z1p = getelementptr inbounds double, ptr %z, i64 1
  %z1 = load double, ptr %z1p
  %s1 = fsub reassoc nsz double %m1, %z1
  %r = fadd reassoc nsz double %s0, %s1
  ret double %r
}
",
        "define double @f(ptr %x, ptr %y, ptr %z) {
b0:
%x0 = load double, ptr %x, align 8
%y0 = load double, ptr %y, align 8
%m0 = fmul reassoc nsz double %y0, %x0
%z0 = load double, ptr %z, align 8
%z0_neg = fneg reassoc nsz double %z0
%x1p = getelementptr inbounds double, ptr %x, i64 1
%x1 = load double, ptr %x1p, align 8
%y1p = getelementptr inbounds double, ptr %y, i64 1
%y1 = load double, ptr %y1p, align 8
%m1 = fmul reassoc nsz double %y1, %x1
%z1p = getelementptr inbounds double, ptr %z, i64 1
%z1 = load double, ptr %z1p, align 8
%z1_neg = fneg reassoc nsz double %z1
%s1 = fadd reassoc nsz double %m0, %z0_neg
%s0 = fadd reassoc nsz double %s1, %z1_neg
%r = fadd reassoc nsz double %s0, %m1
ret double %r
}
",
        &[],
    );
}

/// `fma-pairs.ll` @multi_use_mul.
#[test]
fn test_fma_pairs_multi_use_mul() {
    check_lit(
        "define double @f(ptr %x, ptr %y, ptr %z, ptr %p) {
b0:
  %x0 = load double, ptr %x
  %y0 = load double, ptr %y
  %m0 = fmul reassoc nsz contract double %x0, %y0
  store double %m0, ptr %p
  %z0 = load double, ptr %z
  %s0 = fsub reassoc nsz contract double %m0, %z0
  %x1p = getelementptr inbounds double, ptr %x, i64 1
  %x1 = load double, ptr %x1p
  %y1p = getelementptr inbounds double, ptr %y, i64 1
  %y1 = load double, ptr %y1p
  %m1 = fmul reassoc nsz contract double %x1, %y1
  %z1p = getelementptr inbounds double, ptr %z, i64 1
  %z1 = load double, ptr %z1p
  %s1 = fsub reassoc nsz contract double %m1, %z1
  %r = fadd reassoc nsz contract double %s0, %s1
  ret double %r
}
",
        "define double @f(ptr %x, ptr %y, ptr %z, ptr %p) {
b0:
%x0 = load double, ptr %x, align 8
%y0 = load double, ptr %y, align 8
%m0 = fmul reassoc nsz contract double %y0, %x0
store double %m0, ptr %p, align 8
%z0 = load double, ptr %z, align 8
%z0_neg = fneg reassoc nsz contract double %z0
%s0 = fadd reassoc nsz contract double %z0_neg, %m0
%x1p = getelementptr inbounds double, ptr %x, i64 1
%x1 = load double, ptr %x1p, align 8
%y1p = getelementptr inbounds double, ptr %y, i64 1
%y1 = load double, ptr %y1p, align 8
%m1 = fmul reassoc nsz contract double %y1, %x1
%z1p = getelementptr inbounds double, ptr %z, i64 1
%z1 = load double, ptr %z1p, align 8
%z1_neg = fneg reassoc nsz contract double %z1
%s1 = fadd reassoc nsz contract double %m1, %z1_neg
%r = fadd reassoc nsz contract double %s0, %s1
ret double %r
}
",
        &[],
    );
}

/// `fma-pairs.ll` @const_addend.
#[test]
fn test_fma_pairs_const_addend() {
    check_lit(
        "define double @f(ptr %x, ptr %y) {
b0:
  %x0 = load double, ptr %x
  %y0 = load double, ptr %y
  %m0 = fmul reassoc nsz contract double %x0, %y0
  %a0 = fadd reassoc nsz contract double %m0, 1.0
  %r = fadd reassoc nsz contract double %a0, 2.0
  ret double %r
}
",
        "define double @f(ptr %x, ptr %y) {
b0:
%x0 = load double, ptr %x, align 8
%y0 = load double, ptr %y, align 8
%m0 = fmul reassoc nsz contract double %y0, %x0
%r = fadd reassoc nsz contract double %m0, 3.000000e+00
ret double %r
}
",
        &[],
    );
}

/// `fma-pairs.ll` @hidden_fadd_tree.
#[test]
fn test_fma_pairs_hidden_fadd_tree() {
    check_lit(
        "define double @f(double %acc, double %a, double %b, double %c, double %d, double %e, double %f) {
b0:
  %acc0 = fadd fast double %a, %b
  %acc1 = fadd fast double %c, %d
  %acc2 = fadd fast double %b, %c
  %acc3 = fadd fast double %d, %a
  %acc4 = fadd fast double %acc0, %acc1
  %acc5 = fadd fast double %acc2, %acc3
  %acc6 = fadd fast double %acc4, %acc5

  %m0 = fmul fast double %e, %f
  %acc7 = fadd fast double %m0, %acc6

  %result = fadd fast double %acc, %acc7
  ret double %result
}
",
        "define double @f(double %acc, double %a, double %b, double %c, double %d, double %e, double %f) {
b0:
%m0 = fmul fast double %f, %e
%reass_add = fadd fast double %b, %a
%reass_add4 = fadd fast double %reass_add, %c
%reass_add5 = fadd fast double %reass_add4, %d
%reass_mul = fmul fast double %reass_add5, 2.000000e+00
%acc7 = fadd fast double %m0, %acc
%result = fadd fast double %acc7, %reass_mul
ret double %result
}
",
        &[],
    );
}

/// `fma-pairs.ll` @nested_fma_subtraction.
#[test]
fn test_fma_pairs_nested_fma_subtraction() {
    check_lit(
        "define double @f(double %acc, double %a, double %b, double %c, double %d, double %e, double %f) {
b0:
  %m0 = fmul fast double %a, %b
  %m1 = fmul fast double %c, %d
  %sub = fsub fast double %m0, %m1
  %m2 = fmul fast double %e, %f
  %add = fadd fast double %m2, %sub
  %result = fsub fast double %acc, %add
  ret double %result
}
",
        "define double @f(double %acc, double %a, double %b, double %c, double %d, double %e, double %f) {
b0:
%m0 = fmul fast double %b, %a
%m1_neg_neg = fmul fast double %d, %c
%m2_neg1 = fmul fast double %f, %e
%reass_add = fadd fast double %m2_neg1, %m0
%m2_neg = fmul fast double -1.000000e+00, %reass_add
%add_neg = fadd fast double %acc, %m1_neg_neg
%result = fadd fast double %m2_neg, %add_neg
ret double %result
}
",
        &[],
    );
}

/// `fma-pairs.ll` @multi_use_fmul_addend.
#[test]
fn test_fma_pairs_multi_use_fmul_addend() {
    check_lit(
        "define float @f(float %a, float %c, float %d, float %e, float %f) {
b0:
  %ac = fmul fast float %a, %c
  %ad = fmul fast float %a, %d
  %ae = fmul fast float %a, %e
  %e.sum = fadd fast float %ad, %ac
  %t1 = fmul fast float %e.sum, %e
  %f.sum0 = fadd fast float %ae, %ad
  %f.sum = fadd fast float %f.sum0, %ac
  %t2 = fmul fast float %f.sum, %f
  %r = fadd fast float %t1, %t2
  ret float %r
}
",
        "define float @f(float %a, float %c, float %d, float %e, float %f) {
b0:
%ac = fmul fast float %c, %a
%ad = fmul fast float %d, %a
%ae = fmul fast float %e, %a
%e_sum = fadd fast float %ad, %ac
%t1 = fmul fast float %e_sum, %e
%f_sum0 = fadd fast float %ad, %ac
%f_sum = fadd fast float %f_sum0, %ae
%t2 = fmul fast float %f_sum, %f
%r = fadd fast float %t2, %t1
ret float %r
}
",
        &[],
    );
}

/// `inverses.ll` @test1. (A&B)&~A == 0
#[test]
fn test_inverses_test1() {
    check_lit_dce(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %t2 = and i32 %b, %a
  %t4 = xor i32 %a, -1
  %t5 = and i32 %t2, %t4
  ret i32 %t5
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
ret i32 0
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `inverses.ll` @test2. A&~A == 0
#[test]
fn test_inverses_test2() {
    check_lit_dce(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %t1 = and i32 %a, 1234
  %t2 = and i32 %b, %t1
  %t4 = xor i32 %a, -1
  %t5 = and i32 %t2, %t4
  ret i32 %t5
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
ret i32 0
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `inverses.ll` @test3. (b+(a+1234))+-a -> b+1234
#[test]
fn test_inverses_test3() {
    check_lit_dce(
        "define i32 @f(i32 %b, i32 %a) {
b0:
  %t1 = add i32 %a, 1234
  %t2 = add i32 %b, %t1
  %t4 = sub i32 0, %a
  %t5 = add i32 %t2, %t4
  ret i32 %t5
}
",
        "define i32 @f(i32 %b, i32 %a) {
b0:
%t5 = add i32 %b, 1234
ret i32 %t5
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `inverses.ll` @test4. (b+(a+1234))+~a -> b+1233
#[test]
fn test_inverses_test4() {
    check_lit_dce(
        "define i32 @f(i32 %b, i32 %a) {
b0:
  %t1 = add i32 %a, 1234
  %t2 = add i32 %b, %t1
  %t4 = xor i32 %a, -1
  %t5 = add i32 %t2, %t4
  ret i32 %t5
}
",
        "define i32 @f(i32 %b, i32 %a) {
b0:
%t5 = add i32 %b, 1233
ret i32 %t5
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `load-combine-like-or.ll` @p0_i8_i8_i16. Basic pattern where two contiguous
/// i8 loads form a wider i16 load
#[test]
fn test_load_combine_like_or_p0_i8_i8_i16() {
    check_lit(
        "define i16 @f(ptr %ptr) {
b0:
  %i = getelementptr inbounds i8, ptr %ptr, i64 1
  %i2 = load i8, ptr %i
  %i3 = zext i8 %i2 to i16
  %i4 = shl i16 %i3, 8
  %i5 = load i8, ptr %ptr
  %i6 = zext i8 %i5 to i16
  %i7 = or i16 %i4, %i6
  %i8 = add i16 %i7, 42
  ret i16 %i8
}
",
        "define i16 @f(ptr %ptr) {
b0:
%i = getelementptr inbounds i8, ptr %ptr, i64 1
%i2 = load i8, ptr %i, align 1
%i3 = zext i8 %i2 to i16
%i4 = shl i16 %i3, 8
%i5 = load i8, ptr %ptr, align 1
%i6 = zext i8 %i5 to i16
%i7 = or i16 %i4, %i6
%i8 = add i16 %i7, 42
ret i16 %i8
}
",
        &[],
    );
}

/// `load-combine-like-or.ll` @p1_i8_i8_i16_swapped. Basic pattern where two
/// contiguous i8 loads form a wider i16 load, with swapped endianness
#[test]
fn test_load_combine_like_or_p1_i8_i8_i16_swapped() {
    check_lit(
        "define i16 @f(ptr %ptr) {
b0:
  %i = load i8, ptr %ptr
  %i2 = zext i8 %i to i16
  %i3 = shl i16 %i2, 8
  %i4 = getelementptr inbounds i8, ptr %ptr, i64 1
  %i5 = load i8, ptr %i4
  %i6 = zext i8 %i5 to i16
  %i7 = or i16 %i3, %i6
  %i8 = add i16 %i7, 42
  ret i16 %i8
}
",
        "define i16 @f(ptr %ptr) {
b0:
%i = load i8, ptr %ptr, align 1
%i2 = zext i8 %i to i16
%i3 = shl i16 %i2, 8
%i4 = getelementptr inbounds i8, ptr %ptr, i64 1
%i5 = load i8, ptr %i4, align 1
%i6 = zext i8 %i5 to i16
%i7 = or i16 %i3, %i6
%i8 = add i16 %i7, 42
ret i16 %i8
}
",
        &[],
    );
}

/// `load-combine-like-or.ll` @p2. Loads are spaced out by a bit, but we don't
/// check for that.
#[test]
fn test_load_combine_like_or_p2() {
    check_lit(
        "define i16 @f(ptr %ptr) {
b0:
  %i = getelementptr inbounds i8, ptr %ptr, i64 1
  %i2 = load i8, ptr %i
  %i3 = zext i8 %i2 to i16
  %i4 = shl i16 %i3, 9 ; wrong shift amount
  %i5 = load i8, ptr %ptr
  %i6 = zext i8 %i5 to i16
  %i7 = or i16 %i4, %i6
  %i8 = add i16 %i7, 42
  ret i16 %i8
}
",
        "define i16 @f(ptr %ptr) {
b0:
%i = getelementptr inbounds i8, ptr %ptr, i64 1
%i2 = load i8, ptr %i, align 1
%i3 = zext i8 %i2 to i16
%i4 = shl i16 %i3, 9
%i5 = load i8, ptr %ptr, align 1
%i6 = zext i8 %i5 to i16
%i7 = or i16 %i4, %i6
%i8 = add i16 %i7, 42
ret i16 %i8
}
",
        &[],
    );
}

/// `load-combine-like-or.ll` @p3. Both bytes are the same, but we don't check
/// for that.
#[test]
fn test_load_combine_like_or_p3() {
    check_lit(
        "define i16 @f(ptr %ptr) {
b0:
  %i = load i8, ptr %ptr
  %i2 = zext i8 %i to i16
  %i3 = shl i16 %i2, 8
  %i4 = or i16 %i3, %i2
  %i5 = add i16 %i4, 42
  ret i16 %i5
}
",
        "define i16 @f(ptr %ptr) {
b0:
%i = load i8, ptr %ptr, align 1
%i2 = zext i8 %i to i16
%i3 = shl i16 %i2, 8
%i4 = or i16 %i3, %i2
%i5 = add i16 %i4, 42
ret i16 %i5
}
",
        &[],
    );
}

/// `load-combine-like-or.ll` @n4. Low bits are not a load
#[test]
fn test_load_combine_like_or_n4() {
    check_lit(
        "define i16 @f(ptr %ptr) {
b0:
  %i = load i8, ptr %ptr
  %i2 = zext i8 %i to i16
  %i3 = shl i16 %i2, 8
  %i4 = or i16 %i3, 42 ; Second operand is bad
  %i5 = add i16 %i4, 42
  ret i16 %i5
}
",
        "define i16 @f(ptr %ptr) {
b0:
%i = load i8, ptr %ptr, align 1
%i2 = zext i8 %i to i16
%i3 = shl i16 %i2, 8
%i5 = add i16 %i3, 84
ret i16 %i5
}
",
        &[],
    );
}

/// `load-combine-like-or.ll` @n5. Low bits are not a load
#[test]
fn test_load_combine_like_or_n5() {
    check_lit(
        "define i16 @f(ptr %ptr, i8 %lowbits) {
b0:
  %i = load i8, ptr %ptr
  %i2 = zext i8 %i to i16
  %i3 = shl i16 %i2, 8
  %i4 = zext i8 %lowbits to i16 ; base operand is bad
  %i5 = or i16 %i3, %i4
  %i6 = add i16 %i5, 42
  ret i16 %i6
}
",
        "define i16 @f(ptr %ptr, i8 %lowbits) {
b0:
%i = load i8, ptr %ptr, align 1
%i2 = zext i8 %i to i16
%i3 = shl i16 %i2, 8
%i4 = zext i8 %lowbits to i16
%i5 = add i16 %i4, 42
%i6 = add i16 %i5, %i3
ret i16 %i6
}
",
        &[],
    );
}

/// `load-combine-like-or.ll` @n6. High bits are not a load
#[test]
fn test_load_combine_like_or_n6() {
    check_lit(
        "define i16 @f(ptr %ptr, i8 %highbits) {
b0:
  %i = getelementptr inbounds i8, ptr %ptr, i64 1
  %i2 = load i8, ptr %i
  %i4 = shl i16 42, 8 ; base operand is bad
  %i5 = load i8, ptr %ptr
  %i6 = zext i8 %i5 to i16
  %i7 = or i16 %i4, %i6
  %i8 = add i16 %i7, 42
  ret i16 %i8
}
",
        "define i16 @f(ptr %ptr, i8 %highbits) {
b0:
%i = getelementptr inbounds i8, ptr %ptr, i64 1
%i4 = shl i16 42, 8
%i5 = load i8, ptr %ptr, align 1
%i6 = zext i8 %i5 to i16
%i7 = add i16 %i4, 42
%i8 = add i16 %i7, %i6
ret i16 %i8
}
",
        &[],
    );
}

/// `matching-binops.ll` @and_shl.
#[test]
fn test_matching_binops_and_shl() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = shl i8 %x, %shamt
  %sy = shl i8 %y, %shamt
  %a = and i8 %sx, %z
  %r = and i8 %sy, %a
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = shl i8 %x, %shamt
%sy = shl i8 %y, %shamt
%a = and i8 %sx, %z
%r = and i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @or_shl.
#[test]
fn test_matching_binops_or_shl() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = shl i8 %x, %shamt
  %sy = shl i8 %y, %shamt
  %a = or i8 %sx, %z
  %r = or i8 %a, %sy
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = shl i8 %x, %shamt
%sy = shl i8 %y, %shamt
%a = or i8 %sx, %z
%r = or i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @xor_shl.
#[test]
fn test_matching_binops_xor_shl() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = shl i8 %x, %shamt
  %sy = shl i8 %y, %shamt
  %a = xor i8 %z, %sx
  %r = xor i8 %a, %sy
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = shl i8 %x, %shamt
%sy = shl i8 %y, %shamt
%a = xor i8 %sx, %z
%r = xor i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @and_lshr.
#[test]
fn test_matching_binops_and_lshr() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = lshr i8 %x, %shamt
  %sy = lshr i8 %y, %shamt
  %a = and i8 %z, %sx
  %r = and i8 %sy, %a
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = lshr i8 %x, %shamt
%sy = lshr i8 %y, %shamt
%a = and i8 %sx, %z
%r = and i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @or_lshr.
#[test]
fn test_matching_binops_or_lshr() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = lshr i8 %x, %shamt
  %sy = lshr i8 %y, %shamt
  %a = or i8 %sx, %z
  %r = or i8 %sy, %a
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = lshr i8 %x, %shamt
%sy = lshr i8 %y, %shamt
%a = or i8 %sx, %z
%r = or i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @xor_lshr.
#[test]
fn test_matching_binops_xor_lshr() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = lshr i8 %x, %shamt
  %sy = lshr i8 %y, %shamt
  %a = xor i8 %sx, %z
  %r = xor i8 %a, %sy
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = lshr i8 %x, %shamt
%sy = lshr i8 %y, %shamt
%a = xor i8 %sx, %z
%r = xor i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @and_ashr.
#[test]
fn test_matching_binops_and_ashr() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = ashr i8 %x, %shamt
  %sy = ashr i8 %y, %shamt
  %a = and i8 %z, %sx
  %r = and i8 %a, %sy
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = ashr i8 %x, %shamt
%sy = ashr i8 %y, %shamt
%a = and i8 %sx, %z
%r = and i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @or_ashr.
#[test]
fn test_matching_binops_or_ashr() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = ashr i8 %x, %shamt
  %sy = ashr i8 %y, %shamt
  %a = or i8 %z, %sx
  %r = or i8 %sy, %a
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = ashr i8 %x, %shamt
%sy = ashr i8 %y, %shamt
%a = or i8 %sx, %z
%r = or i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @or_and_shl.
#[test]
fn test_matching_binops_or_and_shl() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = shl i8 %x, %shamt
  %sy = shl i8 %y, %shamt
  %a = or i8 %sx, %z
  %r = and i8 %sy, %a
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = shl i8 %x, %shamt
%sy = shl i8 %y, %shamt
%a = or i8 %sx, %z
%r = and i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @or_lshr_shl.
#[test]
fn test_matching_binops_or_lshr_shl() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = lshr i8 %x, %shamt
  %sy = shl i8 %y, %shamt
  %a = or i8 %sx, %z
  %r = or i8 %a, %sy
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = lshr i8 %x, %shamt
%sy = shl i8 %y, %shamt
%a = or i8 %sx, %z
%r = or i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @xor_lshr_multiuse.
#[test]
fn test_matching_binops_xor_lshr_multiuse() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = lshr i8 %x, %shamt
  %sy = lshr i8 %y, %shamt
  %a = xor i8 %sx, %z
  %r = xor i8 %a, %sy
  %r2 = sdiv i8 %a, %r
  ret i8 %r2
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = lshr i8 %x, %shamt
%sy = lshr i8 %y, %shamt
%a = xor i8 %sx, %z
%r = xor i8 %a, %sy
%r2 = sdiv i8 %a, %r
ret i8 %r2
}
",
        &[],
    );
}

/// `matching-binops.ll` @add_lshr.
#[test]
fn test_matching_binops_add_lshr() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
  %sx = lshr i8 %x, %shamt
  %a = add i8 %sx, %z
  %sy = lshr i8 %y, %shamt
  %r = add i8 %a, %sy
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %shamt) {
b0:
%sx = lshr i8 %x, %shamt
%a = add i8 %sx, %z
%sy = lshr i8 %y, %shamt
%r = add i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @mul_sub.
#[test]
fn test_matching_binops_mul_sub() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %m) {
b0:
  %sx = sub i8 %x, %m
  %sy = sub i8 %y, %m
  %a = mul nsw i8 %sx, %z
  %r = mul nuw i8 %a, %sy
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %m) {
b0:
%sx = sub i8 %x, %m
%sy = sub i8 %y, %m
%a = mul nsw i8 %sx, %z
%r = mul nuw i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @add_mul.
#[test]
fn test_matching_binops_add_mul() {
    check_lit(
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %m) {
b0:
  %sx = mul nuw i8 %x, 42
  %a = add nuw i8 %sx, %z
  %sy = mul nsw i8 %y, %m
  %r = add nsw i8 %sy, %a
  ret i8 %r
}
",
        "define i8 @f(i8 %x, i8 %y, i8 %z, i8 %m) {
b0:
%sx = mul nuw i8 %x, 42
%a = add nuw i8 %z, %sx
%sy = mul nsw i8 %m, %y
%r = add nsw i8 %a, %sy
ret i8 %r
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `matching-binops.ll` @fadd_fmul.
#[test]
fn test_matching_binops_fadd_fmul() {
    check_lit(
        "define float @f(float %x, float %y, float %z, float %m) {
b0:
  %sx = fmul float %x, %m
  %a = fadd fast float %sx, %z
  %sy = fmul float %y, %m
  %r = fadd fast float %sy, %a
  ret float %r
}
",
        "define float @f(float %x, float %y, float %z, float %m) {
b0:
%sx = fmul float %x, %m
%a = fadd fast float %sx, %z
%sy = fmul float %y, %m
%r = fadd fast float %a, %sy
ret float %r
}
",
        &[],
    );
}

/// `matching-binops.ll` @fmul_fdiv.
#[test]
fn test_matching_binops_fmul_fdiv() {
    check_lit(
        "define float @f(float %x, float %y, float %z, float %m) {
b0:
  %sx = fdiv float %x, %m
  %sy = fdiv float %y, 42.0
  %a = fmul fast float %z, %sx
  %r = fmul fast float %sy, %a
  ret float %r
}
",
        "define float @f(float %x, float %y, float %z, float %m) {
b0:
%sx = fdiv float %x, %m
%sy = fdiv float %y, 4.200000e+01
%a = fmul fast float %sy, %z
%r = fmul fast float %a, %sx
ret float %r
}
",
        &[],
    );
}

/// `min_int.ll` @minint.
#[test]
fn test_min_int_minint() {
    check_lit_dce(
        "define i32 @f(i32 %i) {
b0:
  %mul = mul i32 %i, -2147483648
  %add = add i32 %mul, 1
  ret i32 %add
}
",
        "define i32 @f(i32 %i) {
b0:
%mul = mul i32 %i, -2147483648
%add = add i32 %mul, 1
ret i32 %add
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `mixed-fast-nonfast-fp.ll` @foo.
#[test]
fn test_mixed_fast_nonfast_fp_foo() {
    check_lit(
        "define float @f(float %a, float %b, float %c) {
b0:
  %mul1 = fmul fast float %a, %c
  %mul2 = fmul fast float %a, %b
  %mul3 = fmul float %a, %b   ; STRICT
  %mul4 = fmul fast float %a, %c
  %add1 = fadd fast  float %mul1, %mul3
  %add2 = fadd fast float %mul4, %mul2
  %add3 = fadd fast float %add1, %add2
  ret float %add3
}
",
        "define float @f(float %a, float %b, float %c) {
b0:
%mul3 = fmul float %a, %b
%factor = fmul fast float %c, 2.000000e+00
%reass_add1 = fadd fast float %factor, %b
%reass_mul = fmul fast float %reass_add1, %a
%add3 = fadd fast float %reass_mul, %mul3
ret float %add3
}
",
        &[],
    );
}

/// `mixed-fast-nonfast-fp.ll` @foo_reassoc.
#[test]
fn test_mixed_fast_nonfast_fp_foo_reassoc() {
    check_lit(
        "define float @f(float %a, float %b, float %c) {
b0:
  %mul1 = fmul reassoc float %a, %c
  %mul2 = fmul fast float %a, %b
  %mul3 = fmul float %a, %b   ; STRICT
  %mul4 = fmul reassoc float %a, %c
  %add1 = fadd fast  float %mul1, %mul3
  %add2 = fadd reassoc float %mul4, %mul2
  %add3 = fadd fast float %add1, %add2
  ret float %add3
}
",
        "define float @f(float %a, float %b, float %c) {
b0:
%mul1 = fmul reassoc float %a, %c
%mul2 = fmul fast float %b, %a
%mul3 = fmul float %a, %b
%mul4 = fmul reassoc float %a, %c
%add1 = fadd fast float %mul1, %mul3
%add2 = fadd reassoc float %mul2, %mul4
%add3 = fadd fast float %add1, %add2
ret float %add3
}
",
        &[],
    );
}

/// `mulfactor.ll` @test1.
#[test]
fn test_mulfactor_test1() {
    check_lit(
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
%reass_add = add i32 %t6, %b
%reass_mul = mul i32 %reass_add, %b
%t11 = add i32 %reass_mul, %t2
ret i32 %t11
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `mulfactor.ll` @test2.
#[test]
fn test_mulfactor_test2() {
    check_lit(
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
%reass_mul = mul i32 %t, 42
%d = add i32 %reass_mul, 15
ret i32 %d
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `mulfactor.ll` @test3. (x^8)
#[test]
fn test_mulfactor_test3() {
    check_lit(
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
%tmp1 = mul i32 %x, %x
%tmp2 = mul i32 %tmp1, %tmp1
%tmp3 = mul i32 %tmp2, %tmp2
ret i32 %tmp3
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `mulfactor.ll` @test4. (x^7)
#[test]
fn test_mulfactor_test4() {
    check_lit(
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
%tmp1 = mul i32 %x, %x
%tmp2 = mul i32 %tmp1, %x
%tmp3 = mul i32 %tmp2, %x
%f = mul i32 %tmp3, %tmp2
ret i32 %f
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `mulfactor.ll` @test5. (x^4) * (y^2)
#[test]
fn test_mulfactor_test5() {
    check_lit(
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
%tmp1 = mul i32 %x, %x
%tmp2 = mul i32 %tmp1, %y
%tmp3 = mul i32 %tmp2, %tmp2
ret i32 %tmp3
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `mulfactor.ll` @test6. (x^5) * (y^3) * z
#[test]
fn test_mulfactor_test6() {
    check_lit(
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
%tmp1 = mul i32 %x, %x
%tmp2 = mul i32 %tmp1, %y
%f = mul i32 %y, %x
%g = mul i32 %f, %tmp2
%tmp3 = mul i32 %g, %tmp2
%h = mul i32 %tmp3, %z
ret i32 %h
}
",
        &[&[0, 0, 0], &[1, 1, 1], &[-1, -1, -1], &[2, 5, 8], &[-7, -12, -17], &[100, 111, 122]],
    );
}

/// `mulfactor.ll` @test7. (x^4) * (y^3) * (z^2)
#[test]
fn test_mulfactor_test7() {
    check_lit(
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
%tmp1 = mul i32 %x, %x
%tmp2 = mul i32 %tmp1, %y
%tmp3 = mul i32 %tmp2, %z
%tmp4 = mul i32 %tmp3, %y
%h = mul i32 %tmp4, %tmp3
ret i32 %h
}
",
        &[&[0, 0, 0], &[1, 1, 1], &[-1, -1, -1], &[2, 5, 8], &[-7, -12, -17], &[100, 111, 122]],
    );
}

/// `multistep.ll` @multistep1.
#[test]
fn test_multistep_multistep1() {
    check_lit(
        "define i64 @f(i64 %a, i64 %b, i64 %c) {
b0:
  %t0 = mul i64 %a, %b
  %t1 = mul i64 %a, %t0 ; a*(a*b)
  %t2 = mul i64 %a, %c
  %t3 = mul i64 %a, %t2 ; a*(a*c)
  %t4 = add i64 %t1, %t3
  ret i64 %t4
}
",
        "define i64 @f(i64 %a, i64 %b, i64 %c) {
b0:
%reass_add1 = add i64 %c, %b
%reass_mul2 = mul i64 %a, %a
%reass_mul = mul i64 %reass_mul2, %reass_add1
ret i64 %reass_mul
}
",
        &[&[0, 0, 0], &[1, 1, 1], &[-1, -1, -1], &[2, 5, 8], &[-7, -12, -17], &[100, 111, 122]],
    );
}

/// `multistep.ll` @multistep2.
#[test]
fn test_multistep_multistep2() {
    check_lit(
        "define i64 @f(i64 %a, i64 %b, i64 %c, i64 %d) {
b0:
  %t0 = mul i64 %a, %b
  %t1 = mul i64 %a, %c
  %t2 = add i64 %t1, %d ; a*c+d
  %t3 = add i64 %t0, %t2 ; a*b+(a*c+d)
  ret i64 %t3
}
",
        "define i64 @f(i64 %a, i64 %b, i64 %c, i64 %d) {
b0:
%reass_add = add i64 %c, %b
%reass_mul = mul i64 %reass_add, %a
%t3 = add i64 %reass_mul, %d
ret i64 %t3
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `negation.ll` @test1.
#[test]
fn test_negation_test1() {
    check_lit(
        "define i32 @f(i32 %a, i32 %b, i32 %z) {
b0:
  %c = sub i32 0, %z
  %d = mul i32 %a, %b
  %e = mul i32 %c, %d
  %f = mul i32 %e, 12345
  %g = sub i32 0, %f
  ret i32 %g
}
",
        "define i32 @f(i32 %a, i32 %b, i32 %z) {
b0:
%e = mul i32 %a, 12345
%f = mul i32 %e, %b
%g = mul i32 %f, %z
ret i32 %g
}
",
        &[&[0, 0, 0], &[1, 1, 1], &[-1, -1, -1], &[2, 5, 8], &[-7, -12, -17], &[100, 111, 122]],
    );
}

/// `negation.ll` @test2.
#[test]
fn test_negation_test2() {
    check_lit(
        "define i32 @f(i32 %a, i32 %b, i32 %z) {
b0:
  %d = mul i32 %z, 40
  %c = sub i32 0, %d
  %e = mul i32 %a, %c
  %f = sub i32 0, %e
  ret i32 %f
}
",
        "define i32 @f(i32 %a, i32 %b, i32 %z) {
b0:
%e = mul i32 %a, 40
%f = mul i32 %e, %z
ret i32 %f
}
",
        &[&[0, 0, 0], &[1, 1, 1], &[-1, -1, -1], &[2, 5, 8], &[-7, -12, -17], &[100, 111, 122]],
    );
}

/// `optional-flags.ll` @test0.
#[test]
fn test_optional_flags_test0() {
    check_lit(
        "define i64 @f(i64 %a, i64 %b, i64 %c) {
b0:
  %y = add nsw i64 %c, %b
  %z = add i64 %y, %a
  ret i64 %z
}
",
        "define i64 @f(i64 %a, i64 %b, i64 %c) {
b0:
%y = add i64 %b, %a
%z = add i64 %y, %c
ret i64 %z
}
",
        &[&[0, 0, 0], &[1, 1, 1], &[-1, -1, -1], &[2, 5, 8], &[-7, -12, -17], &[100, 111, 122]],
    );
}

/// `optional-flags.ll` @test1.
#[test]
fn test_optional_flags_test1() {
    check_lit(
        "define i64 @f(i64 %a, i64 %b, i64 %c) {
b0:
  %y = add i64 %c, %b
  %z = add nsw i64 %y, %a
  ret i64 %z
}
",
        "define i64 @f(i64 %a, i64 %b, i64 %c) {
b0:
%y = add i64 %b, %a
%z = add i64 %y, %c
ret i64 %z
}
",
        &[&[0, 0, 0], &[1, 1, 1], &[-1, -1, -1], &[2, 5, 8], &[-7, -12, -17], &[100, 111, 122]],
    );
}

/// `optional-flags.ll` @test2. PR9215
#[test]
fn test_optional_flags_test2() {
    check_lit(
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %s = add nsw i32 %x, %y
  ret i32 %s
}
",
        "define i32 @f(i32 %x, i32 %y) {
b0:
%s = add nsw i32 %y, %x
ret i32 %s
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `or-disjoint.ll` @or_disjoint_both.
#[test]
fn test_or_disjoint_or_disjoint_both() {
    check_lit(
        "define i16 @f(i16 %a, i16 %b) {
b0:
  %or.1 = or disjoint i16 %b, %a
  %or.2 = or disjoint i16 %or.1, 1
  ret i16 %or.2
}
",
        "define i16 @f(i16 %a, i16 %b) {
b0:
%or_1 = or disjoint i16 %a, 1
%or_2 = or disjoint i16 %or_1, %b
ret i16 %or_2
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `or-disjoint.ll` @or_disjoint_first.
#[test]
fn test_or_disjoint_or_disjoint_first() {
    check_lit(
        "define i16 @f(i16 %a, i16 %b) {
b0:
  %or.1 = or i16 %b, %a
  %or.2 = or disjoint i16 %or.1, 1
  ret i16 %or.2
}
",
        "define i16 @f(i16 %a, i16 %b) {
b0:
%or_1 = or i16 %a, 1
%or_2 = or i16 %or_1, %b
ret i16 %or_2
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `or-disjoint.ll` @or_disjoint_last.
#[test]
fn test_or_disjoint_or_disjoint_last() {
    check_lit(
        "define i16 @f(i16 %a, i16 %b) {
b0:
  %or.1 = or disjoint i16 %b, %a
  %or.2 = or i16 %or.1, 1
  ret i16 %or.2
}
",
        "define i16 @f(i16 %a, i16 %b) {
b0:
%or_1 = or i16 %a, 1
%or_2 = or i16 %or_1, %b
ret i16 %or_2
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `propagate-flags.ll` @func.
#[test]
fn test_propagate_flags_func() {
    check_lit(
        "define double @f(double %a, double %b) {
b0:
  %mul1 = fmul fast double %a, %a
  %mul2 = fmul fast double %b, %b
  %mul3 = fmul fast double %mul1, %mul2
  ret double %mul3
}
",
        "define double @f(double %a, double %b) {
b0:
%tmp1 = fmul fast double %b, %a
%tmp2 = fmul fast double %tmp1, %tmp1
ret double %tmp2
}
",
        &[],
    );
}

/// `reassoc-add-nsw.ll` @nsw_preserve_nuw_nsw.
#[test]
fn test_reassoc_add_nsw_nsw_preserve_nuw_nsw() {
    check_lit(
        "define i32 @f(ptr %ptr0, ptr %ptr1, ptr %ptr2) {
b0:
  %v0 = load i32, ptr %ptr0
  %v1 = load i32, ptr %ptr1
  %v2 = load i32, ptr %ptr2
  %add0 = add nuw nsw i32 %v1, %v2
  %add1 = add nuw nsw i32 %add0, %v0
  ret i32 %add1
}
",
        "define i32 @f(ptr %ptr0, ptr %ptr1, ptr %ptr2) {
b0:
%v0 = load i32, ptr %ptr0, align 4
%v1 = load i32, ptr %ptr1, align 4
%v2 = load i32, ptr %ptr2, align 4
%add0 = add nuw nsw i32 %v1, %v0
%add1 = add nuw nsw i32 %add0, %v2
ret i32 %add1
}
",
        &[],
    );
}

/// `reassoc-mul-nuw.ll` @nuw_preserve_negative. We cannot preserve nuw flags
/// for mul
#[test]
fn test_reassoc_mul_nuw_nuw_preserve_negative() {
    check_lit(
        "define i4 @f(i4 %a, i4 %b, i4 %c) {
b0:
  %v0 = mul nuw i4 %a, %c
  %v1 = mul nuw i4 %v0, %b
  ret i4 %v1
}
",
        "define i4 @f(i4 %a, i4 %b, i4 %c) {
b0:
%v0 = mul i4 %b, %a
%v1 = mul i4 %v0, %c
ret i4 %v1
}
",
        &[&[0, 0, 0], &[1, 1, 1], &[-1, -1, -1], &[2, 5, 8], &[-7, -12, -17], &[100, 111, 122]],
    );
}

/// `reassoc-mul-nuw.ll` @nuw_preserve_non_zero. TODO: we can add nuw flags if
/// we know all operands are non-zero.
#[test]
fn test_reassoc_mul_nuw_nuw_preserve_non_zero() {
    check_lit(
        "define i4 @f(i4 %a, i4 %b, i4 %c) {
b0:
  %a0 = add nuw i4 %a, 1
  %b0 = add nuw i4 %b, 1
  %c0 = add nuw i4 %c, 1
  %v0 = mul nuw i4 %a0, %c0
  %v1 = mul nuw i4 %v0, %b0
  ret i4 %v1
}
",
        "define i4 @f(i4 %a, i4 %b, i4 %c) {
b0:
%a0 = add nuw i4 %a, 1
%b0 = add nuw i4 %b, 1
%c0 = add nuw i4 %c, 1
%v0 = mul nuw i4 %b0, %a0
%v1 = mul nuw i4 %v0, %c0
ret i4 %v1
}
",
        &[&[0, 0, 0], &[1, 1, 1], &[-1, -1, -1], &[2, 5, 8], &[-7, -12, -17], &[100, 111, 122]],
    );
}

/// `reassoc-mul-nuw.ll` @re_order_mul_nuw.
#[test]
fn test_reassoc_mul_nuw_re_order_mul_nuw() {
    check_lit(
        "define i4 @f(i4 %xx0, i4 %xx1, i4 %xx2, i4 %xx3) {
b0:
  %x0 = add nuw i4 %xx0, 1
  %x1 = add nuw i4 %xx1, 1
  %x2 = add nuw i4 %xx2, 1
  %x3 = add nuw i4 %xx3, 1
  %mul_a = mul nuw i4 %x0, %x1
  %mul_b = mul nuw i4 %x2, %x3
  %mul_c = mul nuw i4 %mul_a, %mul_b
  ret i4 %mul_c
}
",
        "define i4 @f(i4 %xx0, i4 %xx1, i4 %xx2, i4 %xx3) {
b0:
%x0 = add nuw i4 %xx0, 1
%x1 = add nuw i4 %xx1, 1
%x2 = add nuw i4 %xx2, 1
%x3 = add nuw i4 %xx3, 1
%mul_b = mul nuw i4 %x1, %x0
%mul_a = mul nuw i4 %mul_b, %x2
%mul_c = mul nuw i4 %mul_a, %x3
ret i4 %mul_c
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `reassoc-mul-nuw.ll` @re_order_mul_nuw_fail_maybe_zero.
#[test]
fn test_reassoc_mul_nuw_re_order_mul_nuw_fail_maybe_zero() {
    check_lit(
        "define i4 @f(i4 %xx0, i4 %xx1, i4 %xx2, i4 %xx3) {
b0:
  %x0 = add nsw i4 %xx0, 1
  %x1 = add nuw i4 %xx1, 1
  %x2 = add nuw i4 %xx2, 1
  %x3 = add nuw i4 %xx3, 1
  %mul_a = mul nuw i4 %x0, %x1
  %mul_b = mul nuw i4 %x2, %x3
  %mul_c = mul nuw i4 %mul_a, %mul_b
  ret i4 %mul_c
}
",
        "define i4 @f(i4 %xx0, i4 %xx1, i4 %xx2, i4 %xx3) {
b0:
%x0 = add nsw i4 %xx0, 1
%x1 = add nuw i4 %xx1, 1
%x2 = add nuw i4 %xx2, 1
%x3 = add nuw i4 %xx3, 1
%mul_b = mul i4 %x1, %x0
%mul_a = mul i4 %mul_b, %x2
%mul_c = mul i4 %mul_a, %x3
ret i4 %mul_c
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `reassoc-mul-nuw.ll` @re_order_mul_nsw_nuw.
#[test]
fn test_reassoc_mul_nuw_re_order_mul_nsw_nuw() {
    check_lit(
        "define i4 @f(i4 %xx0, i4 %xx1, i4 %xx2, i4 %xx3) {
b0:
  %x0 = add nuw i4 %xx0, 1
  %x1 = add nuw i4 %xx1, 1
  %x2 = add nuw i4 %xx2, 1
  %x3 = add nuw i4 %xx3, 1
  %mul_a = mul nuw nsw i4 %x0, %x1
  %mul_b = mul nuw nsw i4 %x2, %x3
  %mul_c = mul nuw nsw i4 %mul_a, %mul_b
  ret i4 %mul_c
}
",
        "define i4 @f(i4 %xx0, i4 %xx1, i4 %xx2, i4 %xx3) {
b0:
%x0 = add nuw i4 %xx0, 1
%x1 = add nuw i4 %xx1, 1
%x2 = add nuw i4 %xx2, 1
%x3 = add nuw i4 %xx3, 1
%mul_b = mul nuw nsw i4 %x1, %x0
%mul_a = mul nuw nsw i4 %mul_b, %x2
%mul_c = mul nuw nsw i4 %mul_a, %x3
ret i4 %mul_c
}
",
        &[&[0, 0, 0, 0], &[1, 1, 1, 1], &[-1, -1, -1, -1], &[2, 5, 8, 11], &[-7, -12, -17, -22], &[100, 111, 122, 133]],
    );
}

/// `reassoc_bool.ll` @scalar.
#[test]
fn test_reassoc_bool_scalar() {
    check_lit(
        "define i1 @f(i1 %b0, i1 %b1, i1 %b2, i1 %b3, i1 %b4, i1 %b5, i1 %b6, i1 %b7) {
b0:
  %or01 = or i1 %b0, %b1
  %or23 = or i1 %b2, %b3
  %or45 = or i1 %b4, %b5
  %or67 = or i1 %b6, %b7
  %or0123 = or i1 %or01, %or23
  %or4567 = or i1 %or45, %or67
  %or01234567 = or i1 %or0123, %or4567
  ret i1 %or01234567
}
",
        "define i1 @f(i1 %b0, i1 %b1, i1 %b2, i1 %b3, i1 %b4, i1 %b5, i1 %b6, i1 %b7) {
b0:
%or01 = or i1 %b0, %b1
%or23 = or i1 %b2, %b3
%or45 = or i1 %b4, %b5
%or67 = or i1 %b6, %b7
%or0123 = or i1 %or01, %or23
%or4567 = or i1 %or45, %or67
%or01234567 = or i1 %or0123, %or4567
ret i1 %or01234567
}
",
        &[
            &[0, 0, 0, 0, 0, 0, 0, 0],
            &[1, 1, 1, 1, 1, 1, 1, 1],
            &[-1, -1, -1, -1, -1, -1, -1, -1],
            &[2, 5, 8, 11, 14, 17, 20, 23],
            &[-7, -12, -17, -22, -27, -32, -37, -42],
            &[100, 111, 122, 133, 144, 155, 166, 177],
        ],
    );
}

/// `reassoc_bool.ll` @scalar1.
#[test]
fn test_reassoc_bool_scalar1() {
    check_lit(
        "define i1 @f(i1 %a, i1 %b0, i1 %b1, i1 %b2, i1 %b3, i1 %b4, i1 %b5, i1 %b6, i1 %b7) {
b0:
  %or0 = or i1 %b0, %a
  %or1 = or i1 %b1, %a
  %or2 = or i1 %b2, %a
  %or3 = or i1 %b3, %a
  %or4 = or i1 %b4, %a
  %or5 = or i1 %b5, %a
  %or6 = or i1 %b6, %a
  %or7 = or i1 %b7, %a
  %xor0 = xor i1 %or0, %or1
  %xor1 = xor i1 %xor0, %or2
  %xor2 = xor i1 %xor1, %or3
  %xor3 = xor i1 %xor2, %or4
  %xor4 = xor i1 %xor3, %or5
  %xor5 = xor i1 %xor4, %or6
  %xor6 = xor i1 %xor5, %or7
  %or001 = or i1 %xor0, %xor1
  %or023 = or i1 %xor2, %xor3
  %or045 = or i1 %xor4, %xor5
  %or060 = or i1 %xor6, %xor0
  %or0123 = or i1 %or001, %or023
  %or4560 = or i1 %or045, %or060
  %or01234567 = or i1 %or0123, %or4560
  ret i1 %or01234567
}
",
        "define i1 @f(i1 %a, i1 %b0, i1 %b1, i1 %b2, i1 %b3, i1 %b4, i1 %b5, i1 %b6, i1 %b7) {
b0:
%or0 = or i1 %a, %b0
%or1 = or i1 %a, %b1
%or2 = or i1 %a, %b2
%or3 = or i1 %a, %b3
%or4 = or i1 %a, %b4
%or5 = or i1 %a, %b5
%or6 = or i1 %a, %b6
%or7 = or i1 %a, %b7
%xor0 = xor i1 %or0, %or1
%xor1 = xor i1 %xor0, %or2
%xor2 = xor i1 %xor1, %or3
%xor3 = xor i1 %xor2, %or4
%xor4 = xor i1 %xor3, %or5
%xor5 = xor i1 %xor4, %or6
%xor6 = xor i1 %xor5, %or7
%or001 = or i1 %xor0, %xor1
%or023 = or i1 %xor2, %xor3
%or045 = or i1 %xor4, %xor5
%or060 = or i1 %xor0, %xor6
%or0123 = or i1 %or001, %or023
%or4560 = or i1 %or045, %or060
%or01234567 = or i1 %or0123, %or4560
ret i1 %or01234567
}
",
        &[
            &[0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[1, 1, 1, 1, 1, 1, 1, 1, 1],
            &[-1, -1, -1, -1, -1, -1, -1, -1, -1],
            &[2, 5, 8, 11, 14, 17, 20, 23, 26],
            &[-7, -12, -17, -22, -27, -32, -37, -42, -47],
            &[100, 111, 122, 133, 144, 155, 166, 177, 188],
        ],
    );
}

/// `reassoc_bool.ll` @scalar2.
#[test]
fn test_reassoc_bool_scalar2() {
    check_lit(
        "define i1 @f(i1 %a, i1 %b0, i1 %b1, i1 %b2, i1 %b3, i1 %b4, i1 %b5, i1 %b6, i1 %b7) {
b0:
  %or0 = or i1 %b0, %a
  %or1 = or i1 %b1, %a
  %or2 = or i1 %b2, %a
  %or3 = or i1 %b3, %a
  %or4 = or i1 %b4, %a
  %or5 = or i1 %b5, %a
  %or6 = or i1 %b6, %a
  %or7 = or i1 %b7, %a
  %xor0 = xor i1 %or0, %or1
  %xor1 = xor i1 %or2, %or3
  %xor2 = xor i1 %or4, %or5
  %xor3 = xor i1 %or6, %or7
  %or01 = xor i1 %xor0, %xor1
  %or23 = xor i1 %xor2, %xor3
  %or0123 = xor i1 %or01, %or23
  ret i1 %or0123
}
",
        "define i1 @f(i1 %a, i1 %b0, i1 %b1, i1 %b2, i1 %b3, i1 %b4, i1 %b5, i1 %b6, i1 %b7) {
b0:
%or0 = or i1 %a, %b0
%or1 = or i1 %a, %b1
%or2 = or i1 %a, %b2
%or3 = or i1 %a, %b3
%or4 = or i1 %a, %b4
%or5 = or i1 %a, %b5
%or6 = or i1 %a, %b6
%or7 = or i1 %a, %b7
%xor0 = xor i1 %or0, %or1
%xor1 = xor i1 %or2, %or3
%xor2 = xor i1 %or4, %or5
%xor3 = xor i1 %or6, %or7
%or01 = xor i1 %xor0, %xor1
%or23 = xor i1 %xor2, %xor3
%or0123 = xor i1 %or01, %or23
ret i1 %or0123
}
",
        &[
            &[0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[1, 1, 1, 1, 1, 1, 1, 1, 1],
            &[-1, -1, -1, -1, -1, -1, -1, -1, -1],
            &[2, 5, 8, 11, 14, 17, 20, 23, 26],
            &[-7, -12, -17, -22, -27, -32, -37, -42, -47],
            &[100, 111, 122, 133, 144, 155, 166, 177, 188],
        ],
    );
}

/// `reassoc_bool.ll` @scalar3.
#[test]
fn test_reassoc_bool_scalar3() {
    check_lit(
        "define i1 @f(i1 %a, i1 %b0, i1 %b1, i1 %b2, i1 %b3, i1 %b4, i1 %b5, i1 %b6, i1 %b7) {
b0:
  %xor0 = xor i1 %b0, %a
  %xor1 = xor i1 %b1, %a
  %xor2 = xor i1 %b2, %a
  %xor3 = xor i1 %b3, %a
  %xor4 = xor i1 %b4, %a
  %xor5 = xor i1 %b5, %a
  %xor6 = xor i1 %b6, %a
  %xor7 = xor i1 %b7, %a
  %and0 = and i1 %xor0, %xor1
  %and1 = and i1 %xor2, %xor3
  %and2 = and i1 %xor4, %xor5
  %and3 = and i1 %xor6, %xor7
  %or01 = and i1 %and0, %and1
  %or23 = and i1 %and2, %and3
  %or0123 = and i1 %or01, %or23
  ret i1 %or0123
}
",
        "define i1 @f(i1 %a, i1 %b0, i1 %b1, i1 %b2, i1 %b3, i1 %b4, i1 %b5, i1 %b6, i1 %b7) {
b0:
%xor0 = xor i1 %a, %b0
%xor1 = xor i1 %a, %b1
%xor2 = xor i1 %a, %b2
%xor3 = xor i1 %a, %b3
%xor4 = xor i1 %a, %b4
%xor5 = xor i1 %a, %b5
%xor6 = xor i1 %a, %b6
%xor7 = xor i1 %a, %b7
%and0 = and i1 %xor0, %xor1
%and1 = and i1 %xor2, %xor3
%and2 = and i1 %xor4, %xor5
%and3 = and i1 %xor6, %xor7
%or01 = and i1 %and0, %and1
%or23 = and i1 %and2, %and3
%or0123 = and i1 %or01, %or23
ret i1 %or0123
}
",
        &[
            &[0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[1, 1, 1, 1, 1, 1, 1, 1, 1],
            &[-1, -1, -1, -1, -1, -1, -1, -1, -1],
            &[2, 5, 8, 11, 14, 17, 20, 23, 26],
            &[-7, -12, -17, -22, -27, -32, -37, -42, -47],
            &[100, 111, 122, 133, 144, 155, 166, 177, 188],
        ],
    );
}

/// `reassoc_bool.ll` @scalar4.
#[test]
fn test_reassoc_bool_scalar4() {
    check_lit(
        "define i1 @f(i1 %a, i1 %b0, i1 %b1, i1 %b2, i1 %b3, i1 %b4, i1 %b5, i1 %b6, i1 %b7) {
b0:
  %xor0 = xor i1 %b0, %a
  %xor1 = xor i1 %b1, %a
  %xor2 = xor i1 %b2, %a
  %xor3 = xor i1 %b3, %a
  %xor4 = xor i1 %b4, %a
  %xor5 = xor i1 %b5, %a
  %xor6 = xor i1 %b6, %a
  %xor7 = xor i1 %b7, %a
  %or0 = or i1 %xor0, %xor1
  %or1 = or i1 %xor2, %xor3
  %or2 = or i1 %xor4, %xor5
  %or3 = or i1 %xor6, %xor7
  %or4 = or i1 %b0, %b1
  %or5 = or i1 %b2, %b3
  %or6 = or i1 %b4, %b5
  %or7 = or i1 %b6, %b7
  %or01 = or i1 %or0, %or1
  %or23 = or i1 %or2, %or3
  %or45 = or i1 %or4, %or5
  %or67 = or i1 %or6, %or7
  %or0123 = or i1 %or01, %or23
  %or4567 = or i1 %or45, %or67
  %or01234567 = or i1 %or0123, %or4567
  ret i1 %or01234567
}
",
        "define i1 @f(i1 %a, i1 %b0, i1 %b1, i1 %b2, i1 %b3, i1 %b4, i1 %b5, i1 %b6, i1 %b7) {
b0:
%xor0 = xor i1 %a, %b0
%xor1 = xor i1 %a, %b1
%xor2 = xor i1 %a, %b2
%xor3 = xor i1 %a, %b3
%xor4 = xor i1 %a, %b4
%xor5 = xor i1 %a, %b5
%xor6 = xor i1 %a, %b6
%xor7 = xor i1 %a, %b7
%or0 = or i1 %xor0, %xor1
%or1 = or i1 %xor2, %xor3
%or2 = or i1 %xor4, %xor5
%or3 = or i1 %xor6, %xor7
%or4 = or i1 %b0, %b1
%or5 = or i1 %b2, %b3
%or6 = or i1 %b4, %b5
%or7 = or i1 %b6, %b7
%or01 = or i1 %or0, %or1
%or23 = or i1 %or2, %or3
%or45 = or i1 %or4, %or5
%or67 = or i1 %or6, %or7
%or0123 = or i1 %or01, %or23
%or4567 = or i1 %or45, %or67
%or01234567 = or i1 %or4567, %or0123
ret i1 %or01234567
}
",
        &[
            &[0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[1, 1, 1, 1, 1, 1, 1, 1, 1],
            &[-1, -1, -1, -1, -1, -1, -1, -1, -1],
            &[2, 5, 8, 11, 14, 17, 20, 23, 26],
            &[-7, -12, -17, -22, -27, -32, -37, -42, -47],
            &[100, 111, 122, 133, 144, 155, 166, 177, 188],
        ],
    );
}

/// `reassociate-distribute.ll` @test_basic. Basic case: a*8697 - (a+b)*6436 →
/// a*2261 + b*(-6436)
#[test]
fn test_reassociate_distribute_test_basic() {
    check_lit(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %mul  = mul nsw i32 %a, 8697
  %add  = add nsw i32 %a, %b
  %mul1 = mul nsw i32 %add, 6436
  %sub  = sub nsw i32 %mul, %mul1
  ret i32 %sub
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
%mul1_neg = mul i32 %b, -6436
%reass_mul = mul i32 %a, 2261
%sub = add i32 %reass_mul, %mul1_neg
ret i32 %sub
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `reassociate-distribute.ll` @test_with_sub. Sub instead of add inside: a*500
/// - (a-b)*300
#[test]
fn test_reassociate_distribute_test_with_sub() {
    check_lit(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %mul  = mul nsw i32 %a, 500
  %sub1 = sub nsw i32 %a, %b
  %mul1 = mul nsw i32 %sub1, 300
  %sub  = sub nsw i32 %mul, %mul1
  ret i32 %sub
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
%mul2_neg = mul i32 %b, 300
%reass_mul = mul i32 %a, 200
%sub = add i32 %reass_mul, %mul2_neg
ret i32 %sub
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `reassociate-distribute.ll` @test_b_matches. b operand matches: b*11 +
/// (a+b)*4
#[test]
fn test_reassociate_distribute_test_b_matches() {
    check_lit(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %mul  = mul nsw i32 %b, 11
  %add  = add nsw i32 %a, %b
  %mul1 = mul nsw i32 %add, 4
  %add2 = add nsw i32 %mul, %mul1
  ret i32 %add2
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
%mul2 = mul i32 %a, 4
%reass_mul = mul i32 %b, 15
%add2 = add i32 %reass_mul, %mul2
ret i32 %add2
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `reassociate-distribute.ll` @test_nsw_dropped. NSW/NUW must be dropped on
/// new instructions after distribution (x+y) *nsw 20 does NOT imply (x *nsw 20)
/// + (y *nsw 20)
#[test]
fn test_reassociate_distribute_test_nsw_dropped() {
    check_lit(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %mul1 = mul nsw i16 %x, 15
  %add  = add nsw i16 %x, %y
  %mul2 = mul nsw i16 %add, 25
  %add2 = add i16 %mul1, %mul2
  ret i16 %add2
}
",
        "define i16 @f(i16 %x, i16 %y) {
b0:
%mul1 = mul i16 %y, 25
%reass_mul = mul i16 %x, 40
%add2 = add i16 %mul1, %reass_mul
ret i16 %add2
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `reassociate-distribute.ll` @test_no_match. No sibling constant mul —
/// nothing to combine with
#[test]
fn test_reassociate_distribute_test_no_match() {
    check_lit(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %add = add nsw i32 %a, %b
  %mul = mul nsw i32 %add, 999
  ret i32 %mul
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
%add = add nsw i32 %b, %a
%mul = mul nsw i32 %add, 999
ret i32 %mul
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `reassociate-distribute.ll` @test_multi_use_add. AddSub has multiple uses
/// (second use is an xor) — oneUse required for distribution
#[test]
fn test_reassociate_distribute_test_multi_use_add() {
    check_lit(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %mul  = mul nsw i32 %a, 777
  %add  = add nsw i32 %a, %b
  %mul1 = mul nsw i32 %add, 444
  %sub  = sub nsw i32 %mul, %mul1
  %ret  = xor i32 %sub, %add
  ret i32 %ret
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
%mul = mul nsw i32 %a, 777
%add = add nsw i32 %b, %a
%mul1 = mul nsw i32 %add, 444
%sub = sub nsw i32 %mul, %mul1
%ret = xor i32 %sub, %add
ret i32 %ret
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `reassociate-distribute.ll` @test_no_const_on_other_mul. Sibling mul has no
/// constant operand — should not trigger
#[test]
fn test_reassociate_distribute_test_no_const_on_other_mul() {
    check_lit(
        "define i32 @f(i32 %a, i32 %b, i32 %c) {
b0:
  %mul  = mul nsw i32 %a, %c
  %add  = add nsw i32 %a, %b
  %mul1 = mul nsw i32 %add, 555
  %sub  = sub nsw i32 %mul, %mul1
  ret i32 %sub
}
",
        "define i32 @f(i32 %a, i32 %b, i32 %c) {
b0:
%mul = mul nsw i32 %c, %a
%add = add nsw i32 %b, %a
%mul1 = mul nsw i32 %add, 555
%sub = sub nsw i32 %mul, %mul1
ret i32 %sub
}
",
        &[&[0, 0, 0], &[1, 1, 1], &[-1, -1, -1], &[2, 5, 8], &[-7, -12, -17], &[100, 111, 122]],
    );
}

/// `repeats.ll` @nilpotent.
#[test]
fn test_repeats_nilpotent() {
    check_lit(
        "define i8 @f(i8 %x) {
b0:
  %tmp = xor i8 %x, %x
  ret i8 %tmp
}
",
        "define i8 @f(i8 %x) {
b0:
ret i8 0
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `repeats.ll` @idempotent.
#[test]
fn test_repeats_idempotent() {
    check_lit(
        "define i2 @f(i2 %x) {
b0:
  %tmp1 = and i2 %x, %x
  %tmp2 = and i2 %tmp1, %x
  %tmp3 = and i2 %tmp2, %x
  ret i2 %tmp3
}
",
        "define i2 @f(i2 %x) {
b0:
ret i2 %x
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `repeats.ll` @add.
#[test]
fn test_repeats_add() {
    check_lit(
        "define i2 @f(i2 %x) {
b0:
  %tmp1 = add i2 %x, %x
  %tmp2 = add i2 %tmp1, %x
  %tmp3 = add i2 %tmp2, %x
  ret i2 %tmp3
}
",
        "define i2 @f(i2 %x) {
b0:
ret i2 0
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `repeats.ll` @cst_add.
#[test]
fn test_repeats_cst_add() {
    check_lit(
        "define i2 @f() {
b0:
  %tmp1 = add i2 1, 1
  %tmp2 = add i2 %tmp1, 1
  ret i2 %tmp2
}
",
        "define i2 @f() {
b0:
ret i2 -1
}
",
        &[],
    );
}

/// `repeats.ll` @cst_mul.
#[test]
fn test_repeats_cst_mul() {
    check_lit(
        "define i8 @f() {
b0:
  %tmp1 = mul i8 3, 3
  %tmp2 = mul i8 %tmp1, 3
  %tmp3 = mul i8 %tmp2, 3
  %tmp4 = mul i8 %tmp3, 3
  ret i8 %tmp4
}
",
        "define i8 @f() {
b0:
ret i8 -13
}
",
        &[],
    );
}

/// `wrap-flags.ll` @shl_to_mul_nsw.
#[test]
fn test_wrap_flags_shl_to_mul_nsw() {
    check_lit_dce(
        "define i32 @f(i32 %i) {
entry:
  %mul = shl nsw i32 %i, 31
  %mul2 = add i32 %mul, 1
  ret i32 %mul2
}
",
        "define i32 @f(i32 %i) {
entry:
%mul = mul i32 %i, -2147483648
%mul2 = add i32 %mul, 1
ret i32 %mul2
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `wrap-flags.ll` @shl_to_mul_nsw_2.
#[test]
fn test_wrap_flags_shl_to_mul_nsw_2() {
    check_lit_dce(
        "define i32 @f(i32 %i) {
entry:
  %mul = shl nsw i32 %i, 30
  %mul2 = add i32 %mul, 1
  ret i32 %mul2
}
",
        "define i32 @f(i32 %i) {
entry:
%mul = mul nsw i32 %i, 1073741824
%mul2 = add i32 %mul, 1
ret i32 %mul2
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `wrap-flags.ll` @shl_to_mul_nuw.
#[test]
fn test_wrap_flags_shl_to_mul_nuw() {
    check_lit_dce(
        "define i32 @f(i32 %i) {
entry:
  %mul = shl nuw i32 %i, 2
  %mul2 = add i32 %mul, 1
  ret i32 %mul2
}
",
        "define i32 @f(i32 %i) {
entry:
%mul = mul nuw i32 %i, 4
%mul2 = add i32 %mul, 1
ret i32 %mul2
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `wrap-flags.ll` @shl_to_mul_nuw_nsw.
#[test]
fn test_wrap_flags_shl_to_mul_nuw_nsw() {
    check_lit_dce(
        "define i32 @f(i32 %i) {
entry:
  %mul = shl nuw nsw i32 %i, 2
  %mul2 = add i32 %mul, 1
  ret i32 %mul2
}
",
        "define i32 @f(i32 %i) {
entry:
%mul = mul nuw nsw i32 %i, 4
%mul2 = add i32 %mul, 1
ret i32 %mul2
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `wrap-flags.ll` @shl_to_mul_nuw_nsw_bitwidth_m1.
#[test]
fn test_wrap_flags_shl_to_mul_nuw_nsw_bitwidth_m1() {
    check_lit_dce(
        "define i32 @f(i32 %i) {
entry:
  %mul = shl nuw nsw i32 %i, 31
  %mul2 = add i32 %mul, 1
  ret i32 %mul2
}
",
        "define i32 @f(i32 %i) {
entry:
%mul = mul nuw nsw i32 %i, -2147483648
%mul2 = add i32 %mul, 1
ret i32 %mul2
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `wrap-flags.ll` @pr23926.
#[test]
fn test_wrap_flags_pr23926() {
    check_lit_dce(
        "define i2 @f(i2 %X1, i2 %X2) {
b0:
  %add = add nuw i2 %X1, 1
  %sub = sub nuw nsw i2 %X2, %add
  ret i2 %sub
}
",
        "define i2 @f(i2 %X1, i2 %X2) {
b0:
%x1_neg = sub i2 0, %x1
%add_neg = add i2 %x1_neg, -1
%sub = add i2 %add_neg, %x2
ret i2 %sub
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `xor_reassoc.ll` @xor1. (x | c1) ^ (x | c2) => (x & c3) ^ c3, where c3 =
/// c1^c2
#[test]
fn test_xor_reassoc_xor1() {
    check_lit(
        "define i32 @f(i32 %x) {
b0:
  %or = or i32 %x, 123
  %or1 = or i32 %x, 456
  %xor = xor i32 %or, %or1
  ret i32 %xor
}
",
        "define i32 @f(i32 %x) {
b0:
%and_ra = and i32 %x, 435
%xor = xor i32 %and_ra, 435
ret i32 %xor
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `xor_reassoc.ll` @xor2. Test rule : (x & c1) ^ (x & c2) = (x & (c1^c2)) Real
/// testing case : (x & 123) ^ y ^ (x & 345) => (x & 435) ^ y
#[test]
fn test_xor_reassoc_xor2() {
    check_lit(
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %and = and i32 %x, 123
  %xor = xor i32 %and, %y
  %and1 = and i32 %x, 456
  %xor2 = xor i32 %xor, %and1
  ret i32 %xor2
}
",
        "define i32 @f(i32 %x, i32 %y) {
b0:
%and_ra = and i32 %x, 435
%xor2 = xor i32 %and_ra, %y
ret i32 %xor2
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `xor_reassoc.ll` @xor3. Test rule: (x | c1) ^ (x & c2) = (x & c3) ^ c1,
/// where c3 = ~c1 ^ c2 c3 = ~c1 ^ c2
#[test]
fn test_xor_reassoc_xor3() {
    check_lit(
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %or = or i32 %x, 123
  %xor = xor i32 %or, %y
  %and = and i32 %x, 456
  %xor1 = xor i32 %xor, %and
  ret i32 %xor1
}
",
        "define i32 @f(i32 %x, i32 %y) {
b0:
%and_ra = and i32 %x, -436
%xor = xor i32 %y, 123
%xor1 = xor i32 %xor, %and_ra
ret i32 %xor1
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `xor_reassoc.ll` @xor4. Test rule: (x | c1) ^ c2 = (x & ~c1) ^ (c1 ^ c2)
#[test]
fn test_xor_reassoc_xor4() {
    check_lit(
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %and = and i32 %x, -124
  %xor = xor i32 %y, 435
  %xor1 = xor i32 %xor, %and
  ret i32 %xor1
}
",
        "define i32 @f(i32 %x, i32 %y) {
b0:
%and = and i32 %x, -124
%xor = xor i32 %y, 435
%xor1 = xor i32 %xor, %and
ret i32 %xor1
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `xor_reassoc.ll` @xor_special1. Special case1: (x | c1) ^ (x & ~c1) = c1
#[test]
fn test_xor_reassoc_xor_special1() {
    check_lit(
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %or = or i32 %x, 123
  %xor = xor i32 %or, %y
  %and = and i32 %x, -124
  %xor1 = xor i32 %xor, %and
  ret i32 %xor1
}
",
        "define i32 @f(i32 %x, i32 %y) {
b0:
%xor1 = xor i32 %y, 123
ret i32 %xor1
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `xor_reassoc.ll` @xor_special2. Special case1: (x | c1) ^ (x & c1) = x ^ c1
#[test]
fn test_xor_reassoc_xor_special2() {
    check_lit(
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %or = or i32 %x, 123
  %xor = xor i32 %or, %y
  %and = and i32 %x, 123
  %xor1 = xor i32 %xor, %and
  ret i32 %xor1
}
",
        "define i32 @f(i32 %x, i32 %y) {
b0:
%xor = xor i32 %x, 123
%xor1 = xor i32 %xor, %y
ret i32 %xor1
}
",
        &[&[0, 0], &[1, 1], &[-1, -1], &[2, 5], &[-7, -12], &[100, 111]],
    );
}

/// `xor_reassoc.ll` @xor_special3. (x | c1) ^ (x | c1) => 0
#[test]
fn test_xor_reassoc_xor_special3() {
    check_lit(
        "define i32 @f(i32 %x) {
b0:
  %or = or i32 %x, 123
  %or1 = or i32 %x, 123
  %xor = xor i32 %or, %or1
  ret i32 %xor
}
",
        "define i32 @f(i32 %x) {
b0:
ret i32 0
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `xor_reassoc.ll` @xor_special4. (x & c1) ^ (x & c1) => 0
#[test]
fn test_xor_reassoc_xor_special4() {
    check_lit(
        "define i32 @f(i32 %x) {
b0:
  %or = and i32 %x, 123
  %or1 = and i32 123, %x
  %xor = xor i32 %or, %or1
  ret i32 %xor
}
",
        "define i32 @f(i32 %x) {
b0:
ret i32 0
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `xor_reassoc.ll` @xor_ra_size1. (x | c1) ^ (x | c2) => (x & c3) ^ c3 is
/// enabled if one of operands has multiple uses
#[test]
fn test_xor_reassoc_xor_ra_size1() {
    check_lit(
        "define i32 @f(i32 %x) {
b0:
  %or = or i32 %x, 123
  %or1 = or i32 %x, 456
  %xor = xor i32 %or, %or1

  %add = add i32 %xor, %or
  ret i32 %add
}
",
        "define i32 @f(i32 %x) {
b0:
%or = or i32 %x, 123
%and_ra = and i32 %x, 435
%xor = xor i32 %and_ra, 435
%add = add i32 %xor, %or
ret i32 %add
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `xor_reassoc.ll` @xor_ra_size2. (x | c1) ^ (x | c2) => (x & c3) ^ c3 is
/// disenabled if bothf operands has multiple uses.
#[test]
fn test_xor_reassoc_xor_ra_size2() {
    check_lit(
        "define i32 @f(i32 %x) {
b0:
  %or = or i32 %x, 123
  %or1 = or i32 %x, 456
  %xor = xor i32 %or, %or1

  %add = add i32 %xor, %or
  %add2 = add i32 %add, %or1
  ret i32 %add2
}
",
        "define i32 @f(i32 %x) {
b0:
%or = or i32 %x, 123
%or1 = or i32 %x, 456
%xor = xor i32 %or, %or1
%add = add i32 %or1, %or
%add2 = add i32 %add, %xor
ret i32 %add2
}
",
        &[&[0], &[1], &[-1], &[2], &[-7], &[100]],
    );
}

/// `xor_reassoc.ll` @xor_bug2. The bug was that when the compiler optimize
/// "(x | c1)" ^ "(x & c2)", it may swap the two xor-subexpressions if they are
/// not in canoninical order; however, when optimizer swaps two sub-expressions,
/// if forgot to swap the cached value of c1 and c2 accordingly, hence cause the
/// problem.
#[test]
fn test_xor_reassoc_xor_bug2() {
    check_lit(
        "define i32 @f(i32 %p0, i32 %p1, i32 %p2, i32 %p3) {
b0:
  %n5 = mul i32 %p0, 123
  %n6 = add i32 %p2, 24
  %n7 = add i32 %p1, 8
  %n8 = and i32 %p1, 3456789
  %n9 = or i32 %n8,  4567890
  %n10 = and i32 %p1, 543210987
  %n11 = or i32 %p1, 891034567
  %n12 = and i32 %p2, 255
  %n13 = xor i32 %n9, %n10
  %n14 = xor i32 %n11, %n13
  %n15 = xor i32 %n5, %n14
  %n16 = and i32 %p3, 255
  %n17 = xor i32 %n16, 42
  %n18 = add i32 %n6, %n7
  %n19 = add i32 %n18, %n12
  %n20 = add i32 %n19, %n15
  ret i32 %n20
}
",
        "define i32 @f(i32 %p0, i32 %p1, i32 %p2, i32 %p3) {
b0:
%n5 = mul i32 %p0, 123
%n6 = and i32 %p1, 3456789
%n7 = or i32 %n6, 4567890
%n8 = and i32 %p2, 255
%and_ra = and i32 %p1, -360490541
%n9 = xor i32 %n5, 891034567
%n10 = xor i32 %n9, %and_ra
%n11 = xor i32 %n10, %n7
%n12 = and i32 %p3, 255
%n13 = add i32 %p1, 32
%n14 = add i32 %n13, %p2
%n15 = add i32 %n14, %n8
%n16 = add i32 %n15, %n11
ret i32 %n16
}
",
        &[],
    );
}
