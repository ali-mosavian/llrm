use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitSelectInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

/// `@f(params)` returning `ret` after `visitSelectInst` alone.
fn combined(
    body: &str,
    ret_ty: &str,
) -> String {
    let text = format!("define {ret_ty} @f(i1 %c, {ret_ty} %x, {ret_ty} %y) {{\nb0:\n{body}}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// x_isqrt's `if (x >= r + bit) x -= r + bit`: the select chose the result of a
/// `sub` the other arm is the first operand of; the pin selects the amount
/// subtracted (`select c, y, 0`) and subtracts after. The old form kept the
/// `sub` before the select, a value more live across the loop.
#[test]
fn a_select_of_a_sub_and_its_minuend_selects_the_amount() {
    let out = combined("  %d = sub i16 %x, %y\n  %r = select i1 %c, i16 %d, i16 %x\n  ret i16 %r\n", "i16");
    assert!(out.contains("select i1 %c, i16 %y, i16 0") && out.contains("sub i16 %x, %"), "{out}");
    assert_eq!(out.matches("sub").count(), 1, "{out}");
}

/// The operation in the false arm swaps the arms of the new select.
#[test]
fn an_operation_in_the_false_arm_selects_the_identity_first() {
    let out = combined("  %d = add i16 %x, %y\n  %r = select i1 %c, i16 %x, i16 %d\n  ret i16 %r\n", "i16");
    assert!(out.contains("select i1 %c, i16 0, i16 %y") && out.contains("add i16 %x, %"), "{out}");
}

/// A commutative operation folds through either operand.
#[test]
fn a_commutative_operation_folds_through_its_second_operand() {
    let out = combined("  %d = and i16 %y, %x\n  %r = select i1 %c, i16 %d, i16 %x\n  ret i16 %r\n", "i16");
    assert!(out.contains("select i1 %c, i16 %y, i16 -1") && out.contains("and i16 %x, %"), "{out}");
}

/// A shift folds on the amount, not on the value shifted.
#[test]
fn a_shift_folds_on_its_amount_only() {
    let by = combined("  %d = shl i16 %x, %y\n  %r = select i1 %c, i16 %d, i16 %x\n  ret i16 %r\n", "i16");
    assert!(by.contains("select i1 %c, i16 %y, i16 0") && by.contains("shl i16 %x, %"), "{by}");
    let of = combined("  %d = shl i16 %y, %x\n  %r = select i1 %c, i16 %d, i16 %x\n  ret i16 %r\n", "i16");
    assert!(of.contains("select i1 %c, i16 %d, i16 %x"), "{of}");
}

/// A second use of the operation keeps the select as it was.
#[test]
fn an_operation_with_another_use_is_left() {
    let out = combined(
        "  %d = sub i16 %x, %y\n  %r = select i1 %c, i16 %d, i16 %x\n  %s = add i16 %r, %d\n  ret i16 %s\n",
        "i16",
    );
    assert!(out.contains("select i1 %c, i16 %d, i16 %x"), "{out}");
}

/// A constant operand makes a select of two constants: only of 0, 1 and -1.
#[test]
fn a_select_between_constants_is_made_only_of_zero_one_and_minus_one() {
    let five = combined("  %d = sub i16 %x, 5\n  %r = select i1 %c, i16 %d, i16 %x\n  ret i16 %r\n", "i16");
    assert!(five.contains("select i1 %c, i16 %d, i16 %x"), "{five}");
    let one = combined("  %d = sub i16 %x, 1\n  %r = select i1 %c, i16 %d, i16 %x\n  ret i16 %r\n", "i16");
    // The new select of 1 and 0 is itself a `zext` of the condition.
    assert!(one.contains("zext i1 %c to i16") && one.contains("sub i16 %x, %"), "{one}");
}
