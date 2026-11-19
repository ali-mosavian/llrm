use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_compares::visit_icmp;
use crate::testing::{parsed, printed};

fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if matches!(c.function.instruction(inst).opcode, Opcode::ICmp(_)) { visit_icmp(c, inst) } else { Visit::None }
}

/// `define i1 @f(PARAMS)` with `body` before and `after` after `visitICmpInst`
/// alone.
fn check(
    params: &str,
    body: &str,
    after: &str,
) {
    let text = format!("define i1 @f({params}) {{\nb0:\n{body}}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    let want = format!("define i1 @f({params}) {{\nb0:\n{after}}}\n");
    assert_eq!(printed(&module), want);
}

/// A byte tested for zero after `uitofp` and a bitcast (`oper_f_div_u8`'s
/// divisor test) kept the float round trip: `ucvtf; fmov; cbz`, 12 B over the
/// pin's `cbz` on the byte.
#[test]
fn zero_test_of_the_bits_of_a_uitofp_is_a_zero_test_of_the_integer() {
    check(
        "i8 %x",
        "  %f = uitofp i8 %x to float\n  %b = bitcast float %f to i32\n  %r = icmp eq i32 %b, 0\n  ret i1 %r\n",
        "  %0 = icmp eq i8 %x, 0\n  ret i1 %0\n",
    );
}

/// `ne` is kept as `ne`.
#[test]
fn nonzero_test_of_the_bits_of_a_uitofp_is_a_nonzero_test_of_the_integer() {
    check(
        "i8 %x",
        "  %f = uitofp i8 %x to float\n  %b = bitcast float %f to i32\n  %r = icmp ne i32 %b, 0\n  ret i1 %r\n",
        "  %0 = icmp ne i8 %x, 0\n  ret i1 %0\n",
    );
}

/// The sign of a converted integer is its sign bit, and zero is zero.
#[test]
fn zero_and_sign_tests_of_the_bits_of_a_sitofp_are_tests_of_the_integer() {
    for (pred, rhs) in [("eq", 0i64), ("ne", 0), ("slt", 0), ("sgt", 0)] {
        check(
            "i16 %x",
            &format!(
                "  %f = sitofp i16 %x to float\n  %b = bitcast float %f to i32\n  %r = icmp {pred} i32 %b, {rhs}\n  ret i1 %r\n"
            ),
            &format!("  %0 = icmp {pred} i16 %x, {rhs}\n  ret i1 %0\n"),
        );
    }
}

/// `slt 1` and `sgt -1` move to the integer too.
#[test]
fn one_and_minus_one_bounds_of_the_bits_of_a_sitofp_are_bounds_of_the_integer() {
    check(
        "i16 %x",
        "  %f = sitofp i16 %x to float\n  %b = bitcast float %f to i32\n  %r = icmp slt i32 %b, 1\n  ret i1 %r\n",
        "  %0 = icmp slt i16 %x, 1\n  ret i1 %0\n",
    );
    check(
        "i16 %x",
        "  %f = sitofp i16 %x to float\n  %b = bitcast float %f to i32\n  %r = icmp sgt i32 %b, -1\n  ret i1 %r\n",
        "  %0 = icmp sgt i16 %x, -1\n  ret i1 %0\n",
    );
}

/// An unsigned compare of the bits of a `uitofp` is not the compare of the
/// integer: nothing changes.
#[test]
fn a_relational_test_of_the_bits_of_a_uitofp_is_kept() {
    check(
        "i8 %x",
        "  %f = uitofp i8 %x to float\n  %b = bitcast float %f to i32\n  %r = icmp ult i32 %b, 5\n  ret i1 %r\n",
        "  %f = uitofp i8 %x to float\n  %b = bitcast float %f to i32\n  %r = icmp samesign ult i32 %b, 5\n  ret i1 %r\n",
    );
}
