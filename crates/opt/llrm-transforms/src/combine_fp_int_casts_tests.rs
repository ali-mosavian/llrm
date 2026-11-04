use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visitFAdd`, `visitFSub` and `visitFMul` alone, through the driver.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FMul) => crate::combine_fp::visit_fadd_or_fmul(c, inst),
        Opcode::Binary(BinaryOp::FSub) => crate::combine_fp::visit_fsub(c, inst),
        _ => Visit::None,
    }
}

fn combined(
    ret: &str,
    params: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("define {ret} @f({params}) {{\nb0:\n{body}}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// Pinned `opt -passes=instcombine`: `fadd (sitofp a), (sitofp b)` of two
/// sign-extended 16-bit values is one `add nsw` and one `sitofp`. Left as two
/// conversions, `(long double)a + (long double)b` cost two `fild` and an
/// `fadd` where the pin spends one `fild`.
#[test]
fn test_a_sum_of_two_exact_conversions_is_a_conversion_of_the_sum() {
    let out = combined(
        "x86_fp80",
        "i16 %p, i16 %q",
        "  %a = sext i16 %p to i32\n  %b = sext i16 %q to i32\n  %x = sitofp i32 %a to x86_fp80\n  %y = sitofp i32 %b to x86_fp80\n  %r = fadd x86_fp80 %x, %y\n  ret x86_fp80 %r\n",
    );
    assert!(out.contains("add nsw i32 %a, %b"), "{out}");
    assert!(out.matches("sitofp").count() == 1 && !out.contains("fadd"), "{out}");
}

/// A `uitofp nneg` beside a `sitofp` is read as signed (`mix`'s `(LD)i +
/// (LD)(i * 3)`, whose induction variables LSR then made two of).
#[test]
fn test_a_nonnegative_unsigned_conversion_joins_a_signed_one() {
    let out = combined(
        "x86_fp80",
        "i32 %a",
        "  %n = and i32 %a, 65535\n  %m = mul nuw nsw i32 %n, 3\n  %x = sitofp i32 %n to x86_fp80\n  %y = uitofp nneg i32 %m to x86_fp80\n  %r = fadd x86_fp80 %x, %y\n  ret x86_fp80 %r\n",
    );
    assert!(!out.contains("fadd") && out.matches("tofp").count() == 1, "{out}");
}

/// The sum may not be exact in a double: 32-bit operands that can use 32 bits
/// leave 33 bits of sum in 53 of mantissa, which is fine, but 64-bit ones do
/// not. No fold.
#[test]
fn test_a_sum_that_a_mantissa_cannot_hold_stays_floating_point() {
    let out = combined(
        "double",
        "i64 %a, i64 %b",
        "  %x = sitofp i64 %a to double\n  %y = sitofp i64 %b to double\n  %r = fadd double %x, %y\n  ret double %r\n",
    );
    assert!(out.contains("fadd double"), "{out}");
}

/// A product keeps the zero's sign only if no operand can be zero: `-0.0`
/// from `(-3) * 0` is `0.0` as an integer product.
#[test]
fn test_a_product_of_signed_conversions_that_may_be_zero_is_not_folded() {
    let out = combined(
        "double",
        "i16 %a, i16 %b",
        "  %x = sitofp i16 %a to double\n  %y = sitofp i16 %b to double\n  %r = fmul double %x, %y\n  ret double %r\n",
    );
    assert!(out.contains("fmul double"), "{out}");
}

/// A constant that is a whole number joins the integer operation.
#[test]
fn test_a_whole_number_constant_joins_the_integer_sum() {
    let out = combined(
        "double",
        "i8 %p",
        "  %a = sext i8 %p to i16\n  %x = sitofp i16 %a to double\n  %r = fadd double %x, 3.0\n  ret double %r\n",
    );
    assert!(out.contains("add nsw i16 %a, 3") && !out.contains("fadd"), "{out}");
}

/// Pinned `opt -passes=instcombine`: two conversions of integers of different
/// widths are not folded (`IntTy != IntOps[1]->getType()`, L1650). Folded, the
/// sum was `add i16 %a, %b` with an `i32` operand.
#[test]
fn test_conversions_of_integers_of_different_widths_stay_floating_point() {
    let out = combined(
        "x86_fp80",
        "i8 %p, i16 %q",
        "  %a = sext i8 %p to i16\n  %b = sext i16 %q to i32\n  %x = sitofp i16 %a to x86_fp80\n  %y = sitofp i32 %b to x86_fp80\n  %r = fadd x86_fp80 %x, %y\n  ret x86_fp80 %r\n",
    );
    assert!(out.contains("fadd x86_fp80"), "{out}");
}
