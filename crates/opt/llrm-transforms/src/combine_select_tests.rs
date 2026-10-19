use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `FoldOpIntoSelect` of an instruction whose first
/// operand is a select.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if !matches!(
        c.function.instruction(inst).opcode,
        Opcode::Binary(_) | Opcode::ICmp(_)
    ) {
        return Visit::None;
    }
    let Some(Operand::Value(first)) = c.function.instruction(inst).operands.first().copied() else {
        return Visit::None;
    };
    match c.function.value(first).def {
        ValueDef::Instruction(select) if c.function.instruction(select).opcode == Opcode::Select => {
            c.fold_op_into_select(inst, select, false, false)
        }
        _ => Visit::None,
    }
}

/// `@f` after `FoldOpIntoSelect` alone, through the driver.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// Both arms are numbers: the add folds into each.
#[test]
fn an_add_of_a_select_of_numbers_is_a_select_of_sums() {
    assert_eq!(
        combined(
            "define i16 @f(i1 %c) {\nb0:\n  %s = select i1 %c, i16 3, i16 4\n  %r = add i16 %s, 5\n  ret i16 %r\n}\n"
        ),
        "define i16 @f(i1 %c) {\nb0:\n  %0 = select i1 %c, i16 8, i16 9\n  ret i16 %0\n}\n"
    );
}

/// One arm folds, the other is unknown: the add is copied for that arm.
#[test]
fn the_arm_that_does_not_fold_gets_a_copy_of_the_operation() {
    assert_eq!(
        combined(
            "define i16 @f(i1 %c, i16 %y) {\nb0:\n  %s = select i1 %c, i16 3, i16 %y\n  %r = add i16 %s, 5\n  ret i16 %r\n}\n"
        ),
        "define i16 @f(i1 %c, i16 %y) {\nb0:\n  %0 = add i16 %y, 5\n  %1 = select i1 %c, i16 8, i16 %0\n  ret i16 %1\n}\n"
    );
}

/// Neither arm folds: nothing changes.
#[test]
fn a_select_of_unknowns_is_left() {
    let text = "define i16 @f(i1 %c, i16 %x, i16 %y, i16 %z) {\nb0:\n  %s = select i1 %c, i16 %x, i16 %y\n  %r = add i16 %s, %z\n  ret i16 %r\n}\n";
    assert_eq!(combined(text), text);
}

/// A select with another user is shared: left.
#[test]
fn a_select_with_another_user_is_left() {
    let text = "define i16 @f(i1 %c) {\nb0:\n  %s = select i1 %c, i16 3, i16 4\n  %r = add i16 %s, 5\n  %t = mul i16 %r, %s\n  ret i16 %t\n}\n";
    assert_eq!(combined(text), text);
}

/// A select of `i1` is a logic operation for the select family, not here.
#[test]
fn a_bool_select_is_left() {
    let text = "define i1 @f(i1 %c, i1 %x) {\nb0:\n  %s = select i1 %c, i1 %x, i1 true\n  %r = and i1 %s, true\n  ret i1 %r\n}\n";
    assert_eq!(combined(text), text);
}

/// The condition `icmp eq %y, 7` says `%y` is 7 on the true arm: `7 - %y` is 0
/// there; the false arm gets a copy.
#[test]
fn an_arm_where_the_condition_says_the_operand_is_a_number_folds() {
    assert_eq!(
        combined(
            "define i16 @f(i16 %y, i16 %z) {\nb0:\n  %c = icmp eq i16 %y, 7\n  %s = select i1 %c, i16 7, i16 %z\n  %t = sub i16 %s, %y\n  ret i16 %t\n}\n"
        ),
        "define i16 @f(i16 %y, i16 %z) {\nb0:\n  %c = icmp eq i16 %y, 7\n  %0 = sub i16 %z, %y\n  %1 = select i1 %c, i16 0, i16 %0\n  ret i16 %1\n}\n"
    );
}

/// `zext` of the select's own condition is 1 on the true arm and 0 on the
/// false: `3 + 1` and `4 + 0`.
#[test]
fn a_zext_of_the_condition_is_one_on_the_true_arm_and_zero_on_the_false() {
    assert_eq!(
        combined(
            "define i16 @f(i1 %c) {\nb0:\n  %s = select i1 %c, i16 3, i16 4\n  %e = zext i1 %c to i16\n  %t = add i16 %s, %e\n  ret i16 %t\n}\n"
        ),
        "define i16 @f(i1 %c) {\nb0:\n  %0 = select i1 %c, i16 4, i16 4\n  ret i16 %0\n}\n"
    );
}
