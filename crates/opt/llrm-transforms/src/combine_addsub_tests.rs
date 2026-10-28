use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use super::visit_add;
use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visitAdd` alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Add) { visit_add(c, inst) } else { Visit::None }
}

/// `@f` after `visitAdd` alone.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `define RET @f(PARAMS)` with `body` before and `after` after `visitAdd`.
fn check(
    ret: &str,
    params: &str,
    body: &str,
    after: &str,
) {
    let text = format!("define {ret} @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("define {ret} @f({params}) {{\nb0:\n{after}}}\n");
    assert_eq!(combined(&text), want);
}

/// `foldBoxMultiply`: the four half-width multiplies are one (`opt`: `mul i16
/// %x, %y`).
#[test]
fn four_half_multiplies_are_one_multiply() {
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %xlo = and i16 %x, 255\n  %ylo = and i16 %y, 255\n  %xh = lshr i16 %x, 8\n  %yh = lshr i16 %y, 8\n  %m1 = mul i16 %yh, %x\n  %m2 = mul i16 %xh, %y\n  %cross = add i16 %m1, %m2\n  %sh = shl i16 %cross, 8\n  %lo = mul i16 %ylo, %xlo\n  %r = add i16 %sh, %lo\n  ret i16 %r\n",
        "  %0 = mul i16 %x, %y\n  ret i16 %0\n",
    );
}

/// `factorizeMathWithShlOps`: `(x << n) + (y << n)` is `(x + y) << n`, the
/// wrap flags only if every operation has them (`opt` agrees).
#[test]
fn two_shifts_by_one_amount_share_it() {
    check(
        "i16",
        "i16 %x, i16 %y, i16 %n",
        "  %a = shl i16 %x, %n\n  %b = shl i16 %y, %n\n  %r = add i16 %a, %b\n  ret i16 %r\n",
        "  %0 = add i16 %x, %y\n  %1 = shl i16 %0, %n\n  ret i16 %1\n",
    );
    check(
        "i16",
        "i16 %x, i16 %y, i16 %n",
        "  %a = shl nuw nsw i16 %x, %n\n  %b = shl nuw nsw i16 %y, %n\n  %r = add nuw nsw i16 %a, %b\n  ret i16 %r\n",
        "  %0 = add nuw nsw i16 %x, %y\n  %1 = shl nuw nsw i16 %0, %n\n  ret i16 %1\n",
    );
}

/// `foldAddWithConstant`: a constant minus x, plus a constant, is one constant
/// minus x (`opt`: `sub i16 15, %x`).
#[test]
fn a_constant_sub_plus_a_constant_is_one_sub() {
    check(
        "i16",
        "i16 %x",
        "  %a = sub i16 10, %x\n  %r = add i16 %a, 5\n  ret i16 %r\n",
        "  %0 = sub i16 15, %x\n  ret i16 %0\n",
    );
}

/// `add (sub X, Y), -1` is `add (not Y), X` (`opt`).
#[test]
fn a_sub_plus_minus_one_adds_the_not() {
    check(
        "i16",
        "i16 %x, i16 %y",
        "  %a = sub i16 %x, %y\n  %r = add i16 %a, -1\n  ret i16 %r\n",
        "  %0 = xor i16 %y, -1\n  %1 = add i16 %x, %0\n  ret i16 %1\n",
    );
}

/// `zext(bool) + C` and `sext(bool) + C` are selects (`opt`).
#[test]
fn a_bool_extended_plus_a_constant_is_a_select() {
    check(
        "i16",
        "i1 %b",
        "  %z = zext i1 %b to i16\n  %r = add i16 %z, 7\n  ret i16 %r\n",
        "  %0 = select i1 %b, i16 8, i16 7\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i1 %b",
        "  %z = sext i1 %b to i16\n  %r = add i16 %z, 7\n  ret i16 %r\n",
        "  %0 = select i1 %b, i16 6, i16 7\n  ret i16 %0\n",
    );
}

/// `~X + C` is `(C - 1) - X`, `nsw` kept when `C - 1` cannot overflow (`opt`:
/// `sub nsw i16 4, %x`).
#[test]
fn not_plus_a_constant_is_a_sub_keeping_nsw() {
    check(
        "i16",
        "i16 %x",
        "  %n = xor i16 %x, -1\n  %r = add nsw i16 %n, 5\n  ret i16 %r\n",
        "  %0 = sub nsw i16 4, %x\n  ret i16 %0\n",
    );
}

/// `(X s>> 15) + 1` is `zext (X > -1)` (`opt`).
#[test]
fn a_sign_splat_plus_one_is_the_not_negative_test() {
    check(
        "i16",
        "i16 %x",
        "  %s = ashr i16 %x, 15\n  %r = add i16 %s, 1\n  ret i16 %r\n",
        "  %0 = icmp sgt i16 %x, -1\n  %1 = zext i1 %0 to i16\n  ret i16 %1\n",
    );
}

/// `(X | C1) + C2` where the `or` is disjoint is `X + (C1 + C2)` (`opt`: `add
/// i16 %x, 11`); `(X | C2) + -C2` is `(X | C2) ^ C2`.
#[test]
fn a_disjoint_or_is_an_add_and_an_or_plus_its_negation_is_an_xor() {
    check(
        "i16",
        "i16 %x",
        "  %o = or disjoint i16 %x, 8\n  %r = add i16 %o, 3\n  ret i16 %r\n",
        "  %0 = add i16 %x, 11\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %o = or i16 %x, 4\n  %r = add i16 %o, -4\n  ret i16 %r\n",
        "  %o = or i16 %x, 4\n  %0 = xor i16 %o, 4\n  ret i16 %0\n",
    );
}

/// `X + signmask` is `X ^ signmask`, `or disjoint` where wrapping is not
/// allowed (`opt` agrees on both).
#[test]
fn adding_the_sign_mask_flips_or_sets_the_sign_bit() {
    check(
        "i16",
        "i16 %x",
        "  %r = add nsw i16 %x, -32768\n  ret i16 %r\n",
        "  %0 = or disjoint i16 %x, -32768\n  ret i16 %0\n",
    );
    check("i16", "i16 %x", "  %r = add i16 %x, -32768\n  ret i16 %r\n", "  %0 = xor i16 %x, -32768\n  ret i16 %0\n");
}

/// `add (zext (xor X, -128)), -128` is `sext X` (`opt`).
#[test]
fn a_xor_zext_add_is_a_sext() {
    check(
        "i16",
        "i8 %x",
        "  %xx = xor i8 %x, -128\n  %z = zext i8 %xx to i16\n  %r = add i16 %z, -128\n  ret i16 %r\n",
        "  %0 = sext i8 %x to i16\n  ret i16 %0\n",
    );
}

/// `(X ^ signmask) + C` is `X + (signmask ^ C)`; with the bits of X known the
/// `xor` is a `sub` (`opt` agrees: `add i16 %x, -32763`, and `sub 18, %m`
/// after its other folds).
#[test]
fn an_xor_with_a_sign_mask_or_known_bits_is_an_add_or_a_sub() {
    check(
        "i16",
        "i16 %x",
        "  %xx = xor i16 %x, -32768\n  %r = add i16 %xx, 5\n  ret i16 %r\n",
        "  %0 = add i16 %x, -32763\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %m = and i16 %x, 12\n  %xx = xor i16 %m, 15\n  %r = add i16 %xx, 3\n  ret i16 %r\n",
        "  %m = and i16 %x, 12\n  %0 = sub i16 18, %m\n  ret i16 %0\n",
    );
}

/// The sext-in-register idiom: a value with cleared high bits, `xor 128` and
/// `add -128`, is a shift pair (from the C++; `opt` goes on to drop it as a
/// no-op).
#[test]
fn the_sext_in_register_idiom_is_a_shift_pair() {
    check(
        "i16",
        "i16 %x",
        "  %m = and i16 %x, 127\n  %xx = xor i16 %m, 128\n  %r = add i16 %xx, -128\n  ret i16 %r\n",
        "  %m = and i16 %x, 127\n  %0 = shl i16 %m, 8\n  %1 = ashr i16 %0, 8\n  ret i16 %1\n",
    );
}

/// `sext(bool) + 1` is a select: the C++ reaches that fold before `add (sext
/// i1 X), 1` is `zext (not X)` (`opt` gets the `zext` from `visitSelect`), and
/// `(X s>> 15) + 1` before `add (ashr (shl X, 15), 15), 1` is `and (not X),
/// 1`, so that second fold is not reached for a shift of 15 either.
#[test]
fn a_sext_bool_plus_one_and_a_low_bit_flip_are_cheaper() {
    check(
        "i16",
        "i1 %b",
        "  %z = sext i1 %b to i16\n  %r = add i16 %z, 1\n  ret i16 %r\n",
        "  %0 = select i1 %b, i16 0, i16 1\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %s = shl i16 %x, 15\n  %a = ashr i16 %s, 15\n  %r = add i16 %a, 1\n  ret i16 %r\n",
        "  %s = shl i16 %x, 15\n  %0 = icmp sgt i16 %s, -1\n  %1 = zext i1 %0 to i16\n  ret i16 %1\n",
    );
}

/// `add (zext (add X, -3)), 3` is `zext X` when X is at least 3. Known here
/// only by the C++ condition: with nothing known about `%x` the add stays
/// (`opt` keeps it and sets `nuw nsw`).
#[test]
fn a_zext_of_a_narrow_add_is_kept_when_the_bound_is_unknown() {
    let text = "define i32 @f(i16 %x) {\nb0:\n  %a = add i16 %x, -3\n  %z = zext i16 %a to i32\n  %r = add i32 %z, 3\n  ret i32 %r\n}\n";
    assert_eq!(combined(text), text);
}

/// `combineAddSubWithShlAddSub`: `A + (-B << N)` is `A - (B << N)` (`opt`).
#[test]
fn a_shifted_negation_added_is_a_subtracted_shift() {
    check(
        "i16",
        "i16 %a, i16 %b, i16 %n",
        "  %nb = sub i16 0, %b\n  %s = shl i16 %nb, %n\n  %r = add i16 %a, %s\n  ret i16 %r\n",
        "  %0 = shl i16 %b, %n\n  %1 = sub i16 %a, %0\n  ret i16 %1\n",
    );
}

/// `foldAddLikeCommutative`: `(A - B) + (C - A)` is `C - B`; `((X s/ -4) <<
/// 2) + X` is `X s% 4`; `(A + 12) + (B & -13)` is `A + (B | 12)` (`opt`
/// agrees on all three).
#[test]
fn add_like_pairs_fold() {
    check(
        "i16",
        "i16 %a, i16 %b, i16 %c",
        "  %l = sub i16 %a, %b\n  %rr = sub i16 %c, %a\n  %r = add i16 %l, %rr\n  ret i16 %r\n",
        "  %0 = sub i16 %c, %b\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %x",
        "  %d = sdiv i16 %x, -4\n  %s = shl i16 %d, 2\n  %r = add i16 %s, %x\n  ret i16 %r\n",
        "  %0 = srem i16 %x, 4\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i16 %a, i16 %b",
        "  %l = add i16 %a, 12\n  %m = and i16 %b, -13\n  %r = add i16 %l, %m\n  ret i16 %r\n",
        "  %0 = or i16 %b, 12\n  %1 = add i16 %a, %0\n  ret i16 %1\n",
    );
}

/// An `i1` add is an `xor`; `x + x` is `x << 1` keeping the wrap flags (`opt`).
#[test]
fn a_bool_add_is_a_xor_and_a_doubling_is_a_shift() {
    check("i1", "i1 %a, i1 %b", "  %r = add i1 %a, %b\n  ret i1 %r\n", "  %0 = xor i1 %a, %b\n  ret i1 %0\n");
    check("i16", "i16 %x", "  %r = add nsw i16 %x, %x\n  ret i16 %r\n", "  %0 = shl nsw i16 %x, 1\n  ret i16 %0\n");
}

/// An add of a select against a constant is a select of sums: the select
/// family's `foldBinOpIntoSelectOrPhi` runs from `foldAddWithConstant`.
#[test]
fn an_add_of_a_select_of_numbers_is_a_select_of_sums() {
    let out = combined(
        "define i16 @f(i1 %c) {\nb0:\n  %s = select i1 %c, i16 3, i16 4\n  %r = add i16 %s, 5\n  ret i16 %r\n}\n",
    );
    assert!(out.contains("select i1 %c, i16 8, i16 9"), "{out}");
}

/// Two phis added, a constant pair for one predecessor: the add moves to the
/// other predecessor (`foldBinopWithPhiOperands`).
#[test]
fn an_add_of_two_phis_with_a_constant_pair_moves_to_the_other_predecessor() {
    let text = "define i16 @f(i1 %c, i16 %i, i16 %j) {
b0:
  br i1 %c, label %a, label %b

a:
  br label %m

b:
  br label %m

m:
  %p = phi i16 [ 3, %a ], [ %i, %b ]
  %q = phi i16 [ 4, %a ], [ %j, %b ]
  %r = add i16 %p, %q
  ret i16 %r
}
";
    let out = combined(text);
    assert!(out.contains("phi i16 [ %0, %b ], [ 7, %a ]") || out.contains("phi i16 [ 7, %a ], [ %0, %b ]"), "{out}");
}

/// `narrowMathIfNoOverflow`: `zext (and X, 15) + 3` cannot wrap in 8 bits, so
/// it is the narrow add, extended. Stayed a 32-bit add before.
#[test]
fn an_extended_value_plus_a_constant_that_cannot_wrap_is_added_narrow() {
    check(
        "i32",
        "i8 %x",
        "  %m = and i8 %x, 15\n  %a = zext i8 %m to i32\n  %r = add i32 %a, 3\n  ret i32 %r\n",
        "  %m = and i8 %x, 15\n  %0 = add nuw i8 %m, 3\n  %1 = zext i8 %0 to i32\n  ret i32 %1\n",
    );
}

/// `narrowMathIfNoOverflow`: two extensions of the same type whose sum fits.
#[test]
fn two_extensions_whose_sum_fits_are_added_narrow() {
    check(
        "i32",
        "i8 %x, i8 %y",
        "  %m = and i8 %x, 15\n  %a = zext i8 %m to i32\n  %n = and i8 %y, 7\n  %b = zext i8 %n to i32\n  %r = add i32 %a, %b\n  ret i32 %r\n",
        "  %m = and i8 %x, 15\n  %n = and i8 %y, 7\n  %0 = add nuw i8 %m, %n\n  %1 = zext i8 %0 to i32\n  ret i32 %1\n",
    );
}

/// `umax(X, C) + -C` is `usub.sat(X, C)` (`opt` agrees). Stayed a max and an
/// add before.
#[test]
fn a_umax_plus_its_negated_constant_is_a_saturating_subtract() {
    let text = "declare i32 @llvm.umax.i32(i32, i32)

define i32 @f(i32 %x) {
b0:
  %m = call i32 @llvm.umax.i32(i32 %x, i32 5)
  %r = add i32 %m, -5
  ret i32 %r
}
";
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    crate::combine::combine_with_declared(
        context,
        &globals,
        &metadata,
        &DataLayout::default(),
        function,
        visit,
        &mut declared,
    );
    declared.place(&mut module).unwrap();
    let out = printed(&module);
    assert!(out.contains("call i32 @llvm.usub.sat.i32(i32 %x, i32 5)"), "{out}");
}
