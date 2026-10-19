use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{CastOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_casts::visit_trunc;
use crate::testing::{parsed, printed};

/// `visitTrunc` alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Cast(CastOp::Trunc) => visit_trunc(c, inst),
        _ => Visit::None,
    }
}

/// `@f` after `visitTrunc` alone, under `layout`.
fn combined_under(
    layout: &str,
    text: &str,
) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let layout = DataLayout::parse(layout).expect("a datalayout");
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &layout, function, visit);
    printed(&module)
}

/// `define RET @f(PARAMS)` with `body` before and `after` after `visitTrunc`,
/// with i8, i16, i32 and i64 native (as `opt` is run here).
fn check(
    ret: &str,
    params: &str,
    body: &str,
    after: &str,
) {
    let text = format!("define {ret} @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("define {ret} @f({params}) {{\nb0:\n{after}}}\n");
    assert_eq!(combined_under("n8:16:32:64", &text), want);
}

/// `EvaluateInDifferentType`: `trunc (zext x) + 1` is computed in the narrow
/// type, and so is a select of extensions (`opt` agrees on both).
#[test]
fn a_truncated_tree_is_computed_in_the_narrow_type() {
    check(
        "i8",
        "i8 %x",
        "  %a = zext i8 %x to i32\n  %b = add i32 %a, 1\n  %r = trunc i32 %b to i8\n  ret i8 %r\n",
        "  %0 = add i8 %x, 1\n  ret i8 %0\n",
    );
    check(
        "i8",
        "i1 %c, i8 %x, i8 %y",
        "  %a = zext i8 %x to i32\n  %b = zext i8 %y to i32\n  %s = select i1 %c, i32 %a, i32 %b\n  %r = trunc i32 %s to i8\n  ret i8 %r\n",
        "  %0 = select i1 %c, i8 %x, i8 %y\n  ret i8 %0\n",
    );
}

/// A value read twice by the tree is built once: `(z * 3) + (z * 3)` in i8 has
/// one multiply (from the C++; `opt` folds on to `mul 6`).
#[test]
fn a_value_the_tree_reads_twice_is_evaluated_once() {
    check(
        "i8",
        "i16 %x",
        "  %z = zext i16 %x to i32\n  %a = mul i32 %z, 3\n  %b = add i32 %a, %a\n  %r = trunc i32 %b to i8\n  ret i8 %r\n",
        "  %0 = trunc i16 %x to i8\n  %1 = mul i8 %0, 3\n  %2 = add i8 %1, %1\n  ret i8 %2\n",
    );
}

/// `commonCastTransforms`: a trunc of a trunc is one trunc (`opt` agrees); a
/// trunc of a phi of numbers is a phi of numbers.
#[test]
fn a_trunc_of_a_trunc_is_one_trunc_and_a_trunc_of_a_phi_folds_into_it() {
    check(
        "i8",
        "i64 %x",
        "  %e = trunc i64 %x to i32\n  %r = trunc i32 %e to i8\n  ret i8 %r\n",
        "  %0 = trunc i64 %x to i8\n  ret i8 %0\n",
    );
    let text = "define i8 @f(i1 %c) {
b0:
  br i1 %c, label %a, label %b

a:
  br label %j

b:
  br label %j

j:
  %p = phi i32 [ 1, %a ], [ 258, %b ]
  %r = trunc i32 %p to i8
  ret i8 %r
}
";
    let want = "define i8 @f(i1 %c) {
b0:
  br i1 %c, label %a, label %b

a:
  br label %j

b:
  br label %j

j:
  %0 = phi i8 [ 1, %a ], [ 2, %b ]
  ret i8 %0
}
";
    assert_eq!(combined_under("n8:16:32:64", text), want);
}

/// `narrowBinOp`: a truncated binary operation with a constant or an
/// extension of the narrow type is done in the narrow type (`opt` agrees).
#[test]
fn a_truncated_binop_with_a_constant_or_an_extension_is_narrow() {
    check(
        "i8",
        "i32 %x",
        "  %s = add i32 %x, 7\n  %r = trunc i32 %s to i8\n  ret i8 %r\n",
        "  %0 = trunc i32 %x to i8\n  %1 = add i8 %0, 7\n  ret i8 %1\n",
    );
    check(
        "i8",
        "i32 %x",
        "  %s = sub i32 7, %x\n  %r = trunc i32 %s to i8\n  ret i8 %r\n",
        "  %0 = trunc i32 %x to i8\n  %1 = sub i8 7, %0\n  ret i8 %1\n",
    );
    check(
        "i8",
        "i8 %x, i32 %y",
        "  %a = zext i8 %x to i32\n  %s = add i32 %a, %y\n  %r = trunc i32 %s to i8\n  ret i8 %r\n",
        "  %0 = trunc i32 %y to i8\n  %1 = add i8 %x, %0\n  ret i8 %1\n",
    );
}

/// `narrowBinOp`: `trunc (lshr (trunc A), C)` shifts A itself (`opt` agrees).
#[test]
fn a_truncated_shift_of_a_truncate_shifts_the_source() {
    check(
        "i16",
        "i64 %a",
        "  %t = trunc i64 %a to i32\n  %s = lshr i32 %t, 12\n  %r = trunc i32 %s to i16\n  ret i16 %r\n",
        "  %0 = lshr i64 %a, 12\n  %1 = trunc i64 %0 to i16\n  ret i16 %1\n",
    );
}

/// `trunc (lshr (sext A), C)` is `ashr A, C`, with a cast after when the types
/// differ (`opt` agrees on both).
#[test]
fn a_truncated_shift_of_a_sext_is_an_ashr() {
    check(
        "i16",
        "i16 %a",
        "  %e = sext i16 %a to i32\n  %s = lshr i32 %e, 3\n  %r = trunc i32 %s to i16\n  ret i16 %r\n",
        "  %0 = ashr i16 %a, 3\n  ret i16 %0\n",
    );
    check(
        "i16",
        "i8 %a",
        "  %e = sext i8 %a to i32\n  %s = lshr i32 %e, 3\n  %r = trunc i32 %s to i16\n  ret i16 %r\n",
        "  %0 = ashr i8 %a, 3\n  %1 = sext i8 %0 to i16\n  ret i16 %1\n",
    );
}

/// `trunc (shl X, C)` is `shl (trunc X), C` (`opt` agrees).
#[test]
fn a_truncated_shl_is_a_narrow_shl() {
    check(
        "i8",
        "i32 %x",
        "  %s = shl i32 %x, 3\n  %r = trunc i32 %s to i8\n  ret i8 %r\n",
        "  %0 = trunc i32 %x to i8\n  %1 = shl i8 %0, 3\n  ret i8 %1\n",
    );
}

/// A trunc to `i1` of a shift, a shifted constant, an xor or a nuw shift is a
/// compare (`opt` agrees, except `ashr -8` where it writes `ugt %x, 2`).
#[test]
fn a_trunc_to_bool_is_a_compare() {
    let case = |body: &str, after: &str| {
        check("i1", "i32 %x, i32 %y", &format!("{body}  ret i1 %r\n"), &format!("{after}  ret i1 %0\n"));
    };
    case("  %s = lshr i32 %x, 31\n  %r = trunc i32 %s to i1\n", "  %0 = icmp slt i32 %x, 0\n");
    case("  %s = shl i32 1, %x\n  %r = trunc i32 %s to i1\n", "  %0 = icmp eq i32 %x, 0\n");
    case("  %s = xor i32 %x, %y\n  %r = trunc nuw i32 %s to i1\n", "  %0 = icmp ne i32 %x, %y\n");
    case("  %s = lshr i32 8, %x\n  %r = trunc i32 %s to i1\n", "  %0 = icmp eq i32 %x, 3\n");
    case("  %s = lshr i32 15, %x\n  %r = trunc i32 %s to i1\n", "  %0 = icmp ult i32 %x, 4\n");
    case("  %s = ashr i32 -8, %x\n  %r = trunc i32 %s to i1\n", "  %0 = icmp uge i32 %x, 3\n");
    case("  %s = lshr exact i32 %x, 3\n  %r = trunc nuw i32 %s to i1\n", "  %0 = icmp ne i32 %x, 0\n");
    case("  %a = shl i32 4, %y\n  %s = lshr i32 %a, 5\n  %r = trunc i32 %s to i1\n", "  %0 = icmp eq i32 %y, 3\n");
    check(
        "i1",
        "i32 %x, i32 %y",
        "  %s = lshr i32 %x, 5\n  %r = trunc i32 %s to i1\n  ret i1 %r\n",
        "  %0 = and i32 %x, 32\n  %1 = icmp ne i32 %0, 0\n  ret i1 %1\n",
    );
}

/// A trunc learns `nuw` when the high bits are zero and `nsw` when they are
/// sign copies (`opt` agrees on `nuw`; it rewrites the `ashr` for `nsw`).
#[test]
fn a_trunc_learns_its_wrap_flags() {
    let same = "define i8 @f(i32 %x) {\nb0:\n  %s = lshr i32 %x, 24\n  %r = trunc nuw i32 %s to i8\n  ret i8 %r\n}\n";
    assert_eq!(combined_under("n8:16:32:64", &same.replace("trunc nuw", "trunc")), same);
    let signed = "define i8 @f(i32 %x) {\nb0:\n  %s = ashr i32 %x, 24\n  %r = trunc nsw i32 %s to i8\n  ret i8 %r\n}\n";
    assert_eq!(combined_under("n8:16:32:64", &signed.replace("trunc nsw", "trunc")), signed);
}
